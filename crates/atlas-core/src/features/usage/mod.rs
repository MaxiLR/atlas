//! The usage meter: its own sign-in, readings after generations and while the
//! window has focus, stale readings kept. Nothing here affects other requests.
//!
//! Readings follow Headroom's store: one right after connecting, one after
//! every completed generation, one when focus comes back or while it stays,
//! five minutes after the last attempt. A failed reading keeps the last
//! windows, marked stale with the reason; a refused refresh ends the login.

mod record;
mod tap;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::Features;
use crate::CoreDeps;
use crate::ports::{Clock, EventSink, SecretError, SecretRecord, SecretStore};
use crate::protocol::{CoreError, CoreEvent, UsageFailure, UsageReading, UsageState, UsageWindow};
use crate::store::Store;
use crate::usage::{DeviceCode, Usage, UsageLogin, UsageReader, UsageReaderDeps, Window, level_of};

/// How long after the last attempt a reading is due while the window has
/// focus.
const EVERY: TimeDelta = TimeDelta::minutes(5);
/// An access token expiring within this is refreshed before a reading.
const REFRESH_BEFORE: TimeDelta = TimeDelta::minutes(5);
/// How often, in real time, the schedule looks at the clock while the window
/// has focus. The clock is a port, so the schedule asks it the time rather
/// than sleeping on it; a jump of the clock is seen within one tick.
const TICK: Duration = Duration::from_secs(1);

pub(crate) struct Feature {
    meter: Arc<Meter>,
}

impl Feature {
    pub(crate) fn new(deps: &CoreDeps, _store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self {
            meter: Arc::new(Meter {
                reader: UsageReader::new(UsageReaderDeps {
                    endpoints: deps.endpoints.clone(),
                    clock: deps.clock.clone(),
                    browser: deps.browser.clone(),
                }),
                secrets: deps.secrets.clone(),
                events: deps.events.clone(),
                clock: deps.clock.clone(),
                state: Mutex::default(),
                reading: tokio::sync::Mutex::default(),
            }),
        })
    }

    /// `deps` for the other features: their events go through a sink that
    /// forwards each one unchanged, then asks the meter for a reading when a
    /// generation finished completed.
    pub(crate) fn tap(&self, deps: &CoreDeps) -> CoreDeps {
        CoreDeps {
            database_path: deps.database_path.clone(),
            provider: deps.provider.clone(),
            events: Arc::new(tap::Tap {
                events: deps.events.clone(),
                meter: Arc::downgrade(&self.meter),
            }),
            clock: deps.clock.clone(),
            secrets: deps.secrets.clone(),
            browser: deps.browser.clone(),
            pdf: deps.pdf.clone(),
            endpoints: deps.endpoints.clone(),
        }
    }

    /// The meter as it stands; a login kept from an earlier launch is read
    /// first.
    pub(crate) async fn get(&self, _cx: &Features) -> Result<UsageState, CoreError> {
        self.meter.current().await
    }

    /// Starts the usage login and answers once its code is shown; the login
    /// goes on in the background and ends with a reading. A meter already
    /// connected or connecting answers as it is.
    pub(crate) async fn connect(&self, _cx: &Features) -> Result<UsageState, CoreError> {
        let meter = &self.meter;
        let current = meter.current().await?;
        if matches!(
            current,
            UsageState::Connected { .. } | UsageState::Connecting { .. }
        ) {
            return Ok(current);
        }
        let (presented, shown) = oneshot::channel();
        {
            let mut state = meter.state();
            if state.connecting.is_none() {
                state.attempts += 1;
                let attempt = state.attempts;
                let cancelled = Arc::new(AtomicBool::new(false));
                let task =
                    tokio::spawn(meter.clone().sign_in(attempt, cancelled.clone(), presented));
                state.connecting = Some(Connecting {
                    attempt,
                    cancelled,
                    task,
                    before: state.shown.clone(),
                });
            }
        }
        // The code was shown, or the login ended before it could be.
        let _ = shown.await;
        Ok(meter.view())
    }

    /// Stops the login's polling and shows the meter as it was before.
    pub(crate) async fn cancel_connect(&self, _cx: &Features) -> Result<UsageState, CoreError> {
        let meter = &self.meter;
        let connecting = meter.state().connecting.take();
        let Some(connecting) = connecting else {
            return meter.current().await;
        };
        connecting.cancelled.store(true, Ordering::SeqCst);
        connecting.task.abort();
        // Once it returns, the login polls no more.
        let _ = connecting.task.await;
        let mut state = meter.state();
        Ok(meter.show(&mut state, connecting.before))
    }

    /// Readings every five minutes run only while the window has focus; focus
    /// coming back reads at once when the last attempt is that old.
    pub(crate) async fn set_window_focused(
        &self,
        _cx: &Features,
        focused: bool,
    ) -> Result<(), CoreError> {
        let meter = &self.meter;
        {
            let mut state = meter.state();
            state.focused = focused;
            if focused && state.schedule.is_none() {
                state.schedule = Some(tokio::spawn(schedule(Arc::downgrade(meter))));
            }
        }
        if focused {
            let meter = meter.clone();
            tokio::spawn(async move { meter.read_if(Due::OnSchedule).await });
        }
        Ok(())
    }

    /// Ends the usage login for the session's sign-out: its record is
    /// cleared and the meter is not connected.
    pub(crate) async fn sign_out(&self, _cx: &Features) -> Result<(), CoreError> {
        let meter = &self.meter;
        let connecting = {
            let mut state = meter.state();
            state.epoch += 1;
            state.login = None;
            state.loaded = true;
            state.last_attempt = None;
            state.connecting.take()
        };
        if let Some(connecting) = connecting {
            connecting.task.abort();
        }
        let deleted = meter.secrets.delete(SecretRecord::UsageLogin);
        let mut state = meter.state();
        meter.show(&mut state, Shown::NotConnected);
        deleted.map_err(store_error)
    }
}

impl Drop for Feature {
    fn drop(&mut self) {
        let mut state = self.meter.state();
        if let Some(schedule) = state.schedule.take() {
            schedule.abort();
        }
        if let Some(connecting) = state.connecting.take() {
            connecting.task.abort();
        }
    }
}

/// The meter shared by the requests, the schedule, the login and the tap.
struct Meter {
    reader: UsageReader,
    secrets: Arc<dyn SecretStore>,
    events: Arc<dyn EventSink>,
    clock: Arc<dyn Clock>,
    state: Mutex<State>,
    /// Held for the whole of a reading: one at a time.
    reading: tokio::sync::Mutex<()>,
}

#[derive(Default)]
struct State {
    /// Whether the login was looked for in the secret store.
    loaded: bool,
    login: Option<UsageLogin>,
    shown: Shown,
    /// Moves whenever the login is replaced or ended, so that what a reading
    /// started before finds is dropped.
    epoch: u64,
    /// When the last reading started.
    last_attempt: Option<DateTime<Utc>>,
    focused: bool,
    /// Logins started so far.
    attempts: u64,
    connecting: Option<Connecting>,
    schedule: Option<JoinHandle<()>>,
}

/// A login waiting for its code to be approved.
struct Connecting {
    attempt: u64,
    cancelled: Arc<AtomicBool>,
    task: JoinHandle<()>,
    /// Shown again if the login does not finish.
    before: Shown,
}

#[derive(Debug, Clone, Default)]
enum Shown {
    #[default]
    NotConnected,
    Connecting(DeviceCode),
    /// A login kept from an earlier launch, not read yet: the meter stays
    /// empty until it is.
    Unread,
    Read(UsageReading),
    FirstReadFailed(UsageFailure),
    SignedOut,
}

impl Shown {
    fn view(&self) -> UsageState {
        match self {
            Self::NotConnected | Self::Unread => UsageState::NotConnected,
            Self::Connecting(code) => UsageState::Connecting {
                user_code: code.user_code.clone(),
                verification_url: code.verification_url.to_string(),
                expires_at: code.expires_at,
            },
            Self::Read(reading) => UsageState::Connected {
                reading: reading.clone(),
            },
            Self::FirstReadFailed(reason) => UsageState::FirstReadFailed { reason: *reason },
            Self::SignedOut => UsageState::SignedOut,
        }
    }
}

/// When a reading is taken.
#[derive(Debug, Clone, Copy)]
enum Due {
    /// Right after connecting, and after a completed generation.
    Now,
    /// A login kept from an earlier launch has not been read yet.
    Unread,
    /// The window has focus and the last attempt is five minutes old.
    OnSchedule,
}

impl Due {
    fn holds(self, state: &State, now: DateTime<Utc>) -> bool {
        match self {
            Self::Now => true,
            Self::Unread => matches!(state.shown, Shown::Unread),
            Self::OnSchedule => {
                state.focused && state.last_attempt.is_none_or(|last| now - last >= EVERY)
            }
        }
    }
}

/// How one reading went.
enum Outcome {
    Read(Usage),
    Failed(UsageFailure),
    /// The refresh token was refused: the login is over.
    Ended,
}

impl Meter {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().expect("usage meter lock poisoned")
    }

    fn view(&self) -> UsageState {
        self.state().shown.view()
    }

    /// Shows `shown` and tells the interface; the lock keeps the events in
    /// the order of the changes.
    fn show(&self, state: &mut State, shown: Shown) -> UsageState {
        state.shown = shown;
        let usage = state.shown.view();
        self.events.emit(CoreEvent::UsageChanged {
            usage: usage.clone(),
        });
        usage
    }

    /// Looks for a login kept in the secret store, once.
    fn load(&self) -> Result<(), CoreError> {
        let mut state = self.state();
        if state.loaded {
            return Ok(());
        }
        let record = self
            .secrets
            .read(SecretRecord::UsageLogin)
            .map_err(store_error)?;
        state.loaded = true;
        if let Some(login) = record.as_deref().and_then(record::decode) {
            state.login = Some(login);
            state.shown = Shown::Unread;
        }
        Ok(())
    }

    async fn current(&self) -> Result<UsageState, CoreError> {
        self.load()?;
        self.read_if(Due::Unread).await;
        Ok(self.view())
    }

    /// Takes a reading off the emitting path.
    fn generation_completed(self: Arc<Self>) {
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move { self.read_if(Due::Now).await });
        }
    }

    /// Reads the usage when there is a login, no login is waiting for its
    /// code and `due` holds, then shows what it found.
    async fn read_if(&self, due: Due) {
        // A secret store that cannot be read has no login to read with.
        if self.load().is_err() {
            return;
        }
        let _one = self.reading.lock().await;
        let (login, epoch) = {
            let mut state = self.state();
            let now = self.clock.now();
            let Some(login) = state.login.clone() else {
                return;
            };
            if state.connecting.is_some() || !due.holds(&state, now) {
                return;
            }
            state.last_attempt = Some(now);
            (login, state.epoch)
        };
        let outcome = self.attempt(login, epoch).await;

        let mut state = self.state();
        if state.epoch != epoch {
            return;
        }
        let shown = match outcome {
            Outcome::Read(usage) => Shown::Read(UsageReading {
                windows: usage.windows.iter().map(window_view).collect(),
                taken_at: self.clock.now(),
                stale: None,
            }),
            Outcome::Failed(reason) => match &state.shown {
                Shown::Read(last) => Shown::Read(UsageReading {
                    stale: Some(reason),
                    ..last.clone()
                }),
                _ => Shown::FirstReadFailed(reason),
            },
            Outcome::Ended => {
                state.login = None;
                state.epoch += 1;
                // A record left behind holds a refused token; the next
                // launch finds it and ends the same way.
                let _ = self.secrets.delete(SecretRecord::UsageLogin);
                Shown::SignedOut
            }
        };
        self.show(&mut state, shown);
    }

    /// One reading, refreshing the token first when it expires soon and
    /// once more on a 401.
    async fn attempt(&self, login: UsageLogin, epoch: u64) -> Outcome {
        let login = if login.expires_at - self.clock.now() <= REFRESH_BEFORE {
            match self.renew(&login, epoch).await {
                Ok(renewed) => renewed,
                Err(outcome) => return outcome,
            }
        } else {
            login
        };
        match self.reader.read(&login).await {
            Ok(usage) => Outcome::Read(usage),
            Err(UsageFailure::Auth) => match self.renew(&login, epoch).await {
                Ok(renewed) => match self.reader.read(&renewed).await {
                    Ok(usage) => Outcome::Read(usage),
                    Err(reason) => Outcome::Failed(reason),
                },
                Err(outcome) => outcome,
            },
            Err(reason) => Outcome::Failed(reason),
        }
    }

    /// Refreshes the login and keeps the new tokens, unless the login was
    /// replaced or ended meanwhile.
    async fn renew(&self, login: &UsageLogin, epoch: u64) -> Result<UsageLogin, Outcome> {
        match self.reader.refresh(login).await {
            Ok(renewed) => {
                let mut state = self.state();
                if state.epoch == epoch {
                    // A store that refuses the write still has the tokens in
                    // memory for this launch.
                    let _ = self
                        .secrets
                        .write(SecretRecord::UsageLogin, &record::encode(&renewed));
                    state.login = Some(renewed.clone());
                }
                Ok(renewed)
            }
            Err(UsageFailure::Auth) => Err(Outcome::Ended),
            Err(reason) => Err(Outcome::Failed(reason)),
        }
    }

    /// Runs login `attempt`: shows its code, then, once approved, keeps the
    /// login and reads; a failed login shows the meter as it was before.
    async fn sign_in(
        self: Arc<Self>,
        attempt: u64,
        cancelled: Arc<AtomicBool>,
        presented: oneshot::Sender<()>,
    ) {
        let current = |state: &State| {
            state
                .connecting
                .as_ref()
                .is_some_and(|connecting| connecting.attempt == attempt)
        };
        let result = self
            .reader
            .login(
                |code| {
                    let mut state = self.state();
                    if current(&state) {
                        self.show(&mut state, Shown::Connecting(code));
                    }
                    drop(state);
                    let _ = presented.send(());
                },
                || cancelled.load(Ordering::SeqCst),
            )
            .await;

        let signed_in = {
            let mut state = self.state();
            if !current(&state) {
                return;
            }
            let Some(connecting) = state.connecting.take() else {
                return;
            };
            match result {
                Ok(login) => {
                    // A store that refuses the write still has the login in
                    // memory for this launch.
                    let _ = self
                        .secrets
                        .write(SecretRecord::UsageLogin, &record::encode(&login));
                    state.login = Some(login);
                    state.epoch += 1;
                    state.last_attempt = None;
                    true
                }
                Err(_) => {
                    self.show(&mut state, connecting.before);
                    false
                }
            }
        };
        if signed_in {
            self.read_if(Due::Now).await;
        }
    }
}

/// While the window has focus, reads whenever a reading is due; ends once
/// focus is lost.
async fn schedule(meter: Weak<Meter>) {
    loop {
        tokio::time::sleep(TICK).await;
        let Some(meter) = meter.upgrade() else {
            return;
        };
        {
            let mut state = meter.state();
            if !state.focused {
                state.schedule = None;
                return;
            }
        }
        meter.read_if(Due::OnSchedule).await;
    }
}

fn window_view(window: &Window) -> UsageWindow {
    UsageWindow {
        name: window.id.clone(),
        length_seconds: u32::try_from(window.length_seconds).unwrap_or(u32::MAX),
        used: window.used,
        resets_at: window.resets_at,
        level: level_of(window),
    }
}

fn store_error(error: SecretError) -> CoreError {
    CoreError::Store {
        message: error.to_string(),
    }
}
