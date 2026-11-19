//! The plan login: sign in, plan usage on, off or not allowed, sign out, the
//! `TokenSource` the provider draws from, and the `require_plan` guard.
//!
//! The tokens stay in the secret store, behind [`PlanAuth`]; what the session
//! keeps itself is the sign-in waiting for the browser, why the last login
//! ended, and whether the provider reported the plan not allowed for the
//! login.

use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use tokio::task::JoinHandle;

use super::Features;
use crate::CoreDeps;
use crate::auth::{
    AccessToken, AuthError, HostId, Login, PlanAuth, PlanAuthDeps, RefreshError, SignInMode,
};
use crate::ports::{Clock, EventSink};
use crate::protocol::{Account, CoreError, CoreEvent, PlanUsage, Session, SignedOutReason};
use crate::provider::{ProviderError, TokenSource};
use crate::store::Store;

/// How long before its expiry an access token is refreshed.
const REFRESH_MARGIN: chrono::TimeDelta = chrono::TimeDelta::minutes(5);

pub(crate) struct Feature {
    login: Arc<PlanLogin>,
    /// Whether the provider spends plan usage. The fake does not, so the
    /// guard lets everything through.
    plan_route: bool,
    /// The sign-in waiting for the browser. Held while one starts or ends.
    attempt: tokio::sync::Mutex<Option<JoinHandle<()>>>,
}

impl Feature {
    pub(crate) fn new(deps: &CoreDeps, store: &Arc<Store>) -> Result<Self, CoreError> {
        let host_id = HostId(format!("urn:uuid:{}", store.host_id()?));
        let login = Arc::new(PlanLogin {
            auth: PlanAuth::new(PlanAuthDeps {
                endpoints: deps.endpoints.clone(),
                secrets: deps.secrets.clone(),
                browser: deps.browser.clone(),
                clock: deps.clock.clone(),
                host_id,
            }),
            clock: deps.clock.clone(),
            events: deps.events.clone(),
            state: Mutex::default(),
        });
        let plan_route = deps.provider.use_plan_login(login.clone());
        Ok(Self {
            login,
            plan_route,
            attempt: tokio::sync::Mutex::default(),
        })
    }

    /// Refuses a request that spends plan usage unless plan usage is on.
    pub(crate) async fn require_plan(&self, _cx: &Features) -> Result<(), CoreError> {
        if !self.plan_route {
            return Ok(());
        }
        match self.login.plan_usage()? {
            Some(PlanUsage::On) => Ok(()),
            Some(PlanUsage::Off) => Err(CoreError::PlanUsageOff),
            Some(PlanUsage::NotAllowed) => Err(CoreError::PlanNotAllowed),
            None => Err(CoreError::SignInRequired),
        }
    }

    pub(crate) async fn get(&self, _cx: &Features) -> Result<Session, CoreError> {
        self.login.session()
    }

    /// Opens the browser on the authorisation page; the session is signing in
    /// until the browser comes back. Signed in with plan usage on or off, it
    /// answers the session as it is.
    pub(crate) async fn sign_in(&self, _cx: &Features) -> Result<Session, CoreError> {
        let mut attempt = self.attempt.lock().await;
        match self.login.session()? {
            Session::SignedOut { .. }
            | Session::SignedIn {
                plan_usage: PlanUsage::NotAllowed,
                ..
            } => self.start(&mut attempt, SignInMode::SignIn).await,
            session => Ok(session),
        }
    }

    pub(crate) async fn cancel_sign_in(&self, _cx: &Features) -> Result<Session, CoreError> {
        let mut attempt = self.attempt.lock().await;
        if let Some(pending) = attempt.take() {
            pending.abort();
        }
        let cancelled = {
            let mut state = self.login.state();
            let cancelled = state.signing_in.take().is_some();
            if cancelled {
                state.reason = None;
            }
            cancelled
        };
        if cancelled {
            self.login.changed()
        } else {
            self.login.session()
        }
    }

    /// Asks for the plan permission again, with consent, for a login that
    /// was granted none.
    pub(crate) async fn enable_plan_usage(&self, _cx: &Features) -> Result<Session, CoreError> {
        let mut attempt = self.attempt.lock().await;
        match self.login.session()? {
            Session::SignedIn {
                plan_usage: PlanUsage::Off,
                ..
            } => self.start(&mut attempt, SignInMode::EnablePlanUsage).await,
            Session::SignedIn {
                plan_usage: PlanUsage::NotAllowed,
                ..
            } => Err(CoreError::PlanNotAllowed),
            Session::SignedOut { .. } => Err(CoreError::SignInRequired),
            session => Ok(session),
        }
    }

    /// Ends the plan login and the usage meter's login together; a failure of
    /// one does not keep the other signed in.
    pub(crate) async fn sign_out(&self, cx: &Features) -> Result<Session, CoreError> {
        let mut attempt = self.attempt.lock().await;
        if let Some(pending) = attempt.take() {
            pending.abort();
        }
        self.login.state().signing_in = None;
        let plan = self.login.auth.sign_out().await;
        let usage = cx.usage.sign_out(cx).await;
        {
            let mut state = self.login.state();
            state.reason = Some(SignedOutReason::SignedOut);
            state.not_allowed = None;
        }
        let session = self.login.changed()?;
        plan.map_err(auth_error)?;
        usage?;
        Ok(session)
    }

    /// Opens the browser and waits for its return in the background.
    async fn start(
        &self,
        attempt: &mut Option<JoinHandle<()>>,
        mode: SignInMode,
    ) -> Result<Session, CoreError> {
        let pending = self.login.auth.start(mode).await.map_err(auth_error)?;
        let number = {
            let mut state = self.login.state();
            state.attempts += 1;
            state.signing_in = Some(state.attempts);
            state.attempts
        };
        let session = self.login.changed()?;
        let login = self.login.clone();
        *attempt = Some(tokio::spawn(async move {
            let result = pending.finish().await;
            login.finish(number, mode, result);
        }));
        Ok(session)
    }
}

impl Drop for Feature {
    fn drop(&mut self) {
        if let Some(pending) = self.attempt.get_mut().take() {
            pending.abort();
        }
    }
}

/// The plan login as the session and the provider share it.
struct PlanLogin {
    auth: PlanAuth,
    clock: Arc<dyn Clock>,
    events: Arc<dyn EventSink>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// The number of the sign-in waiting for the browser.
    signing_in: Option<u64>,
    /// Sign-ins started so far; a late return of an older one is ignored.
    attempts: u64,
    /// Why the last login ended or failed, shown while signed out.
    reason: Option<SignedOutReason>,
    /// The subject of the login the provider reported not allowed to use the
    /// plan. A new explicit sign-in clears it.
    not_allowed: Option<String>,
}

impl PlanLogin {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().expect("session lock poisoned")
    }

    /// The saved login's plan usage, whether or not a sign-in is waiting.
    fn plan_usage(&self) -> Result<Option<PlanUsage>, CoreError> {
        let login = self.auth.login().map_err(auth_error)?;
        Ok(login.map(|login| plan_usage(&login, &self.state())))
    }

    fn session(&self) -> Result<Session, CoreError> {
        let login = self.auth.login().map_err(auth_error)?;
        let state = self.state();
        if state.signing_in.is_some() {
            return Ok(Session::SigningIn);
        }
        Ok(match login {
            Some(login) => Session::SignedIn {
                plan_usage: plan_usage(&login, &state),
                account: Account {
                    email: login.identity.email,
                },
            },
            None => Session::SignedOut {
                reason: state.reason.clone(),
            },
        })
    }

    /// Emits the session as it is now.
    fn changed(&self) -> Result<Session, CoreError> {
        let session = self.session()?;
        self.events.emit(CoreEvent::SessionChanged {
            session: session.clone(),
        });
        Ok(session)
    }

    /// The browser came back for sign-in `number`.
    fn finish(&self, number: u64, mode: SignInMode, result: Result<Login, AuthError>) {
        {
            let mut state = self.state();
            if state.signing_in != Some(number) {
                return;
            }
            state.signing_in = None;
            match (mode, result) {
                (SignInMode::SignIn, Ok(_)) => {
                    state.reason = None;
                    state.not_allowed = None;
                }
                (SignInMode::SignIn, Err(AuthError::Declined)) => {
                    state.reason = Some(SignedOutReason::Declined);
                }
                (SignInMode::SignIn, Err(error)) => {
                    state.reason = Some(SignedOutReason::Failed {
                        error: auth_error(error),
                    });
                }
                // The saved login stays as it was.
                (SignInMode::EnablePlanUsage, _) => {}
            }
        }
        let _ = self.changed();
    }

    /// The login ended: its tokens are gone and the courses are untouched.
    fn end_login(&self) {
        {
            let mut state = self.state();
            state.reason = Some(SignedOutReason::LoginEnded);
            state.not_allowed = None;
        }
        let _ = self.changed();
    }
}

#[async_trait]
impl TokenSource for PlanLogin {
    async fn access_token(&self) -> Result<AccessToken, ProviderError> {
        let login = self
            .auth
            .login()
            .map_err(|error| failed(error.to_string()))?
            .ok_or_else(sign_in_required)?;
        if login.expires_at - REFRESH_MARGIN > self.clock.now()
            && let Some(token) = self
                .auth
                .access_token()
                .map_err(|error| failed(error.to_string()))?
        {
            return Ok(token);
        }
        self.refresh().await
    }

    async fn refresh(&self) -> Result<AccessToken, ProviderError> {
        match self.auth.refresh().await {
            Ok(token) => Ok(token),
            Err(RefreshError::LoginEnded { .. }) => {
                self.end_login();
                Err(sign_in_required())
            }
            Err(RefreshError::SignedOut) => Err(sign_in_required()),
            Err(error) => Err(failed(error.to_string())),
        }
    }

    async fn rejected(&self) {
        // The tokens are cleared even when the revocation is not confirmed.
        let _ = self.auth.sign_out().await;
        self.end_login();
    }

    async fn plan_not_allowed(&self) {
        let Ok(Some(login)) = self.auth.login() else {
            return;
        };
        self.state().not_allowed = Some(login.identity.subject);
        let _ = self.changed();
    }
}

fn plan_usage(login: &Login, state: &State) -> PlanUsage {
    if state.not_allowed.as_deref() == Some(login.identity.subject.as_str()) {
        PlanUsage::NotAllowed
    } else if login.plan_usage {
        PlanUsage::On
    } else {
        PlanUsage::Off
    }
}

fn auth_error(error: AuthError) -> CoreError {
    match error {
        AuthError::Network(_) => CoreError::Offline,
        AuthError::Secret(error) => CoreError::Store {
            message: error.to_string(),
        },
        error => CoreError::Provider {
            message: error.to_string(),
        },
    }
}

fn sign_in_required() -> ProviderError {
    ProviderError::SignInRequired
}

fn failed(message: String) -> ProviderError {
    ProviderError::Failed(message)
}
