//! The usage meter, read with the plan login: readings after generations and
//! while the window has focus, stale readings kept. Nothing here affects
//! other requests, and a failed reading never ends the plan login.
//!
//! Readings follow Headroom's schedule: one right after signing in and one
//! after every completed generation, whatever the focus; one when focus comes
//! back, unless the last attempt is under 30 seconds old. While the window
//! has focus, a reading every minute while the meter is active (a generation
//! running, the last reading changed a window's numbers, or a window from
//! 75 % up to but not at its limit), every five minutes otherwise, and one
//! when a window's reset time passes. After a failed reading the next is five
//! minutes later, whatever else holds. A failed reading keeps the last
//! windows, marked stale with the reason.
//!
//! A refused reading is read once more with a refreshed token; refused again,
//! it is a failed reading.

mod tap;

use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, Weak};
use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use tokio::task::JoinHandle;
use uuid::Uuid;

use super::Features;
use crate::CoreDeps;
use crate::ports::{Clock, EventSink};
use crate::protocol::{
    CoreError, CoreEvent, UsageFailure, UsageLevel, UsageReading, UsageState, UsageWindow,
};
use crate::provider::{ProviderError, TokenSource};
use crate::store::Store;
use crate::usage::{Usage, UsageReader, UsageReaderDeps, Window, level_of};

/// How long after the last attempt a reading is due while the window has
/// focus and the meter is idle, or after a failed reading.
const EVERY: TimeDelta = TimeDelta::minutes(5);
/// How long after the last attempt a reading is due while the window has
/// focus and the meter is active.
const EVERY_ACTIVE: TimeDelta = TimeDelta::minutes(1);
/// How old the last attempt must be for focus coming back to read.
const FOCUS_BACK: TimeDelta = TimeDelta::seconds(30);
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
                }),
                tokens: OnceLock::new(),
                events: deps.events.clone(),
                clock: deps.clock.clone(),
                state: Mutex::default(),
                reading: tokio::sync::Mutex::default(),
            }),
        })
    }

    /// `deps` for the other features: their events go through a sink that
    /// forwards each one unchanged, then tells the meter when the session
    /// signed in or out, or a generation started or finished.
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

    /// The plan login the meter reads with, once the session is built.
    pub(crate) fn use_plan_login(&self, tokens: Arc<dyn TokenSource>) {
        let _ = self.meter.tokens.set(tokens);
    }

    /// The meter as it stands; with a plan login and no reading yet, a
    /// reading is taken first.
    pub(crate) async fn get(&self, _cx: &Features) -> Result<UsageState, CoreError> {
        self.meter.read_if(Due::Unread).await;
        Ok(self.meter.view())
    }

    /// Readings on schedule run only while the window has focus; focus coming
    /// back reads at once unless the last attempt is under 30 seconds old.
    pub(crate) async fn set_window_focused(
        &self,
        _cx: &Features,
        focused: bool,
    ) -> Result<(), CoreError> {
        let meter = &self.meter;
        let back = {
            let mut state = meter.state();
            let back = focused && !state.focused;
            state.focused = focused;
            if focused && state.schedule.is_none() {
                state.schedule = Some(tokio::spawn(schedule(Arc::downgrade(meter))));
            }
            back
        };
        if back {
            let meter = meter.clone();
            let at = meter.clock.now();
            tokio::spawn(async move { meter.read_if(Due::FocusBack(at)).await });
        }
        Ok(())
    }
}

impl Drop for Feature {
    fn drop(&mut self) {
        if let Some(schedule) = self.meter.state().schedule.take() {
            schedule.abort();
        }
    }
}

/// The meter shared by the requests, the schedule and the tap.
struct Meter {
    reader: UsageReader,
    /// The session's plan login.
    tokens: OnceLock<Arc<dyn TokenSource>>,
    events: Arc<dyn EventSink>,
    clock: Arc<dyn Clock>,
    state: Mutex<State>,
    /// Held for the whole of a reading: one at a time.
    reading: tokio::sync::Mutex<()>,
}

#[derive(Default)]
struct State {
    shown: Shown,
    /// Moves whenever the session signs in or out, so that what a reading
    /// started before finds is dropped.
    epoch: u64,
    /// When the last reading started.
    last_attempt: Option<DateTime<Utc>>,
    /// The last reading failed.
    last_failed: bool,
    /// The last reading changed a window's numbers.
    last_changed: bool,
    /// The generations running, by id.
    running: HashSet<Uuid>,
    focused: bool,
    schedule: Option<JoinHandle<()>>,
}

impl State {
    /// The windows of the reading shown, stale or not.
    fn windows(&self) -> &[UsageWindow] {
        match &self.shown {
            Shown::Read(reading) => &reading.windows,
            _ => &[],
        }
    }

    /// A generation is running, the last reading changed a window's numbers,
    /// or a window is from 75 % up to but not at its limit.
    fn active(&self) -> bool {
        !self.running.is_empty()
            || self.last_changed
            || self
                .windows()
                .iter()
                .any(|window| matches!(window.level, UsageLevel::High | UsageLevel::Critical))
    }

    /// Whether a reading is due on schedule at `now`, focus aside.
    fn scheduled(&self, now: DateTime<Utc>) -> bool {
        let Some(last) = self.last_attempt else {
            return true;
        };
        if self.last_failed {
            return now - last >= EVERY;
        }
        let every = if self.active() { EVERY_ACTIVE } else { EVERY };
        now - last >= every
            || self
                .windows()
                .iter()
                .filter_map(|window| window.resets_at)
                .any(|resets_at| last < resets_at && resets_at <= now)
    }
}

#[derive(Debug, Clone, Default)]
enum Shown {
    #[default]
    NoPlanLogin,
    AwaitingReading,
    Read(UsageReading),
    FirstReadFailed(UsageFailure),
}

impl Shown {
    fn view(&self) -> UsageState {
        match self {
            Self::NoPlanLogin => UsageState::NoPlanLogin,
            Self::AwaitingReading => UsageState::AwaitingReading,
            Self::Read(reading) => UsageState::Connected {
                reading: reading.clone(),
            },
            Self::FirstReadFailed(reason) => UsageState::FirstReadFailed { reason: *reason },
        }
    }
}

/// When a reading is taken.
#[derive(Debug, Clone, Copy)]
enum Due {
    /// Right after signing in, and after a completed generation.
    Now,
    /// No reading has succeeded yet: a login kept from an earlier launch, or
    /// a failed first reading.
    Unread,
    /// Focus came back at this moment, 30 seconds or more after the last
    /// attempt.
    FocusBack(DateTime<Utc>),
    /// The window has focus and a reading is due on schedule.
    OnSchedule,
}

impl Due {
    fn holds(self, state: &State, now: DateTime<Utc>) -> bool {
        match self {
            Self::Now => true,
            Self::Unread => !matches!(state.shown, Shown::Read(_)),
            Self::FocusBack(at) => {
                state.focused
                    && state
                        .last_attempt
                        .is_none_or(|last| at - last >= FOCUS_BACK)
            }
            Self::OnSchedule => state.focused && state.scheduled(now),
        }
    }
}

/// How one reading went.
enum Outcome {
    Read(Usage),
    Failed(UsageFailure),
    /// There is no plan login to read with.
    NoPlanLogin,
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
    fn show(&self, state: &mut State, shown: Shown) {
        state.shown = shown;
        self.events.emit(CoreEvent::UsageChanged {
            usage: state.shown.view(),
        });
    }

    /// The session signed in: an empty meter waits for its first reading,
    /// taken off the emitting path.
    fn signed_in(self: Arc<Self>) {
        {
            let mut state = self.state();
            if !matches!(state.shown, Shown::NoPlanLogin) {
                return;
            }
            state.epoch += 1;
            forget_attempts(&mut state);
            self.show(&mut state, Shown::AwaitingReading);
        }
        self.read_now();
    }

    /// The session signed out: the meter empties and a reading in flight is
    /// dropped.
    fn signed_out(&self) {
        let mut state = self.state();
        state.epoch += 1;
        forget_attempts(&mut state);
        if !matches!(state.shown, Shown::NoPlanLogin) {
            self.show(&mut state, Shown::NoPlanLogin);
        }
    }

    /// A generation started (`running`) or finished.
    fn generation(&self, generation: Uuid, running: bool) {
        let mut state = self.state();
        if running {
            state.running.insert(generation);
        } else {
            state.running.remove(&generation);
        }
    }

    /// Takes a reading off the emitting path.
    fn read_now(self: Arc<Self>) {
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move { self.read_if(Due::Now).await });
        }
    }

    /// Reads the usage with the plan login when `due` holds, then shows what
    /// it found.
    async fn read_if(&self, due: Due) {
        let Some(tokens) = self.tokens.get().cloned() else {
            return;
        };
        let _one = self.reading.lock().await;
        let epoch = {
            let mut state = self.state();
            let now = self.clock.now();
            if !due.holds(&state, now) {
                return;
            }
            state.last_attempt = Some(now);
            state.epoch
        };
        let outcome = self.attempt(tokens.as_ref()).await;

        let mut state = self.state();
        if state.epoch != epoch {
            return;
        }
        state.last_failed = matches!(outcome, Outcome::Failed(_));
        let shown = match outcome {
            Outcome::Read(usage) => {
                let windows: Vec<UsageWindow> = usage.windows.iter().map(window_view).collect();
                state.last_changed = numbers_changed(state.windows(), &windows)
                    && matches!(state.shown, Shown::Read(_));
                Shown::Read(UsageReading {
                    windows,
                    taken_at: self.clock.now(),
                    stale: None,
                })
            }
            Outcome::Failed(reason) => {
                state.last_changed = false;
                match &state.shown {
                    Shown::Read(last) => Shown::Read(UsageReading {
                        stale: Some(reason),
                        ..last.clone()
                    }),
                    _ => Shown::FirstReadFailed(reason),
                }
            }
            Outcome::NoPlanLogin => {
                state.last_changed = false;
                if matches!(state.shown, Shown::NoPlanLogin) {
                    return;
                }
                Shown::NoPlanLogin
            }
        };
        self.show(&mut state, shown);
    }

    /// One reading with the plan login's current token, which the login
    /// refreshes first when it is about to expire. A refused reading is read
    /// once more with a refreshed token; refused again, it is a failed one:
    /// the meter never ends the login.
    async fn attempt(&self, tokens: &dyn TokenSource) -> Outcome {
        let token = match tokens.access_token().await {
            Ok(token) => token,
            Err(error) => return without_token(error),
        };
        match self.reader.read(&token).await {
            Ok(usage) => return Outcome::Read(usage),
            Err(UsageFailure::Auth) => {}
            Err(reason) => return Outcome::Failed(reason),
        }
        let token = match tokens.refresh().await {
            Ok(token) => token,
            Err(error) => return without_token(error),
        };
        match self.reader.read(&token).await {
            Ok(usage) => Outcome::Read(usage),
            Err(reason) => Outcome::Failed(reason),
        }
    }
}

/// The session signed in or out: what the last attempt found no longer
/// counts.
fn forget_attempts(state: &mut State) {
    state.last_attempt = None;
    state.last_failed = false;
    state.last_changed = false;
}

/// The outcome of a reading the plan login gave no token for.
fn without_token(error: ProviderError) -> Outcome {
    match error {
        ProviderError::SignInRequired { .. } => Outcome::NoPlanLogin,
        ProviderError::Offline => Outcome::Failed(UsageFailure::Network),
        _ => Outcome::Failed(UsageFailure::Auth),
    }
}

/// Whether `new` names other windows than `old`, or any window used another
/// share.
fn numbers_changed(old: &[UsageWindow], new: &[UsageWindow]) -> bool {
    old.len() != new.len()
        || old
            .iter()
            .zip(new)
            .any(|(old, new)| old.name != new.name || old.used != new.used)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn window(name: &str, used: f64) -> UsageWindow {
        UsageWindow {
            name: name.to_owned(),
            length_seconds: 18_000,
            used,
            resets_at: None,
            level: UsageLevel::Normal,
        }
    }

    #[test]
    fn a_window_used_more_or_another_window_changes_the_numbers() {
        let old = [window("5h", 0.62), window("7d", 0.41)];
        assert!(!numbers_changed(&old, &old.clone()));
        assert!(numbers_changed(
            &old,
            &[window("5h", 0.63), window("7d", 0.41)]
        ));
        assert!(numbers_changed(&old, &[window("5h", 0.62)]));
        assert!(numbers_changed(
            &old,
            &[window("5h", 0.62), window("1d", 0.41)]
        ));
    }
}
