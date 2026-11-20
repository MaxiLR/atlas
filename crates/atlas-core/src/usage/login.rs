//! The usage login: OpenAI's device code under Codex's client id, and its
//! refresh. The method and the constants follow Headroom's Codex provider.

use std::time::Duration;

use chrono::TimeDelta;
use serde_json::{Map, Value, json};

use super::json::{after, claims, number, plan_words, text};
use super::{DeviceCode, UsageAccount, UsageLogin, UsageReader, object_of};
use crate::auth::{AccessToken, RefreshToken};
use crate::protocol::UsageFailure;

const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const USERCODE: &[&str] = &["api", "accounts", "deviceauth", "usercode"];
const POLL: &[&str] = &["api", "accounts", "deviceauth", "token"];
const TOKEN: &[&str] = &["oauth", "token"];
const DEVICE_PAGE: &[&str] = &["codex", "device"];
const REDIRECT: &[&str] = &["deviceauth", "callback"];

/// How long the candidate has to approve the code.
const LOGIN_TIME: TimeDelta = TimeDelta::minutes(15);
const DEFAULT_INTERVAL_SECONDS: f64 = 5.0;

const AUTH_CLAIM: &str = "https://api.openai.com/auth";
const PROFILE_CLAIM: &str = "https://api.openai.com/profile";

/// The device code approved on OpenAI's page.
struct Approval {
    authorization_code: String,
    code_verifier: String,
}

impl UsageReader {
    /// Asks for a device code, hands it to `present` with the page and the
    /// deadline, opens the page and polls until the candidate approves it,
    /// `cancelled` answers true, or the 15 minutes run out; then exchanges
    /// the approval for tokens.
    pub async fn login(
        &self,
        present: impl FnOnce(DeviceCode) + Send,
        cancelled: impl Fn() -> bool + Send + Sync,
    ) -> Result<UsageLogin, UsageFailure> {
        let usercode = self.auth_url(USERCODE);
        let device = object_of(
            self.post_json(usercode, &json!({ "client_id": CLIENT_ID }))
                .await?,
        )
        .await?;
        let (Some(device_auth_id), Some(user_code)) = (
            text(device.get("device_auth_id")),
            text(device.get("user_code")),
        ) else {
            return Err(UsageFailure::Unexpected);
        };
        let interval = number(device.get("interval"));
        let approval = self
            .approval(device_auth_id, user_code, interval, present, cancelled)
            .await?;

        let redirect = self.auth_url(REDIRECT);
        let response = self
            .post_form(
                self.auth_url(TOKEN),
                &[
                    ("grant_type", "authorization_code"),
                    ("client_id", CLIENT_ID),
                    ("code", &approval.authorization_code),
                    ("code_verifier", &approval.code_verifier),
                    ("redirect_uri", redirect.as_str()),
                ],
            )
            .await?;
        if response.status().is_client_error() {
            return Err(UsageFailure::Rejected);
        }
        let tokens = object_of(response).await?;
        let (Some(access_token), Some(refresh_token), Some(expires_in)) = (
            text(tokens.get("access_token")),
            text(tokens.get("refresh_token")),
            number(tokens.get("expires_in")),
        ) else {
            return Err(UsageFailure::Unexpected);
        };
        let account = account_of(&tokens).ok_or(UsageFailure::Unexpected)?;
        let signed_in_at = self.clock.now();
        Ok(UsageLogin {
            access_token: AccessToken::new(access_token.to_owned()),
            refresh_token: RefreshToken::new(refresh_token.to_owned()),
            expires_at: after(signed_in_at, expires_in).ok_or(UsageFailure::Unexpected)?,
            signed_in_at,
            account,
        })
    }

    /// Waits for the candidate to approve the device code on OpenAI's page,
    /// polling as often as OpenAI asks.
    async fn approval(
        &self,
        device_auth_id: &str,
        user_code: &str,
        interval: Option<f64>,
        present: impl FnOnce(DeviceCode),
        cancelled: impl Fn() -> bool,
    ) -> Result<Approval, UsageFailure> {
        let seconds = interval
            .filter(|seconds| *seconds > 0.0)
            .unwrap_or(DEFAULT_INTERVAL_SECONDS);
        let wait = Duration::try_from_secs_f64(seconds).unwrap_or(Duration::MAX);
        let deadline = self.clock.now() + LOGIN_TIME;
        let page = self.auth_url(DEVICE_PAGE);
        present(DeviceCode {
            user_code: user_code.to_owned(),
            verification_url: page.clone(),
            expires_at: deadline,
        });
        // The page is presented with the code, so a browser that does not
        // open leaves the candidate a way to approve it.
        let _ = self.browser.open(&page);
        let body = json!({ "device_auth_id": device_auth_id, "user_code": user_code });
        loop {
            self.clock.sleep(wait).await;
            if cancelled() {
                return Err(UsageFailure::Cancelled);
            }
            if self.clock.now() >= deadline {
                return Err(UsageFailure::Timeout);
            }
            let response = self.post_json(self.auth_url(POLL), &body).await?;
            if matches!(response.status().as_u16(), 403 | 404) {
                continue;
            }
            let approved = object_of(response).await?;
            let (Some(authorization_code), Some(code_verifier)) = (
                text(approved.get("authorization_code")),
                text(approved.get("code_verifier")),
            ) else {
                return Err(UsageFailure::Unexpected);
            };
            return Ok(Approval {
                authorization_code: authorization_code.to_owned(),
                code_verifier: code_verifier.to_owned(),
            });
        }
    }

    /// Exchanges the refresh token for a new access token. A rotated refresh
    /// token replaces the old one; a refused one ends the login (`Auth`).
    pub async fn refresh(&self, login: &UsageLogin) -> Result<UsageLogin, UsageFailure> {
        let response = self
            .post_form(
                self.auth_url(TOKEN),
                &[
                    ("grant_type", "refresh_token"),
                    ("refresh_token", login.refresh_token.secret()),
                    ("client_id", CLIENT_ID),
                ],
            )
            .await?;
        if matches!(response.status().as_u16(), 400 | 401 | 403) {
            return Err(UsageFailure::Auth);
        }
        let tokens = object_of(response).await?;
        let (Some(access_token), Some(expires_in)) = (
            text(tokens.get("access_token")),
            number(tokens.get("expires_in")),
        ) else {
            return Err(UsageFailure::Unexpected);
        };
        let refresh_token = match text(tokens.get("refresh_token")) {
            Some(rotated) => RefreshToken::new(rotated.to_owned()),
            None => login.refresh_token.clone(),
        };
        Ok(UsageLogin {
            access_token: AccessToken::new(access_token.to_owned()),
            refresh_token,
            expires_at: after(self.clock.now(), expires_in).ok_or(UsageFailure::Unexpected)?,
            ..login.clone()
        })
    }
}

/// Who signed in, from the access token's claims, else the ID token's.
fn account_of(tokens: &Map<String, Value>) -> Option<UsageAccount> {
    ["access_token", "id_token"].into_iter().find_map(|token| {
        let claims = claims(tokens.get(token))?;
        let auth = claims.get(AUTH_CLAIM);
        let account_id = text(auth.and_then(|auth| auth.get("chatgpt_account_id")))?;
        let email = text(
            claims
                .get(PROFILE_CLAIM)
                .and_then(|profile| profile.get("email")),
        );
        Some(UsageAccount {
            account_id: account_id.to_owned(),
            email: email.map(str::to_owned),
            plan: plan_words(auth.and_then(|auth| auth.get("chatgpt_plan_type"))),
        })
    })
}
