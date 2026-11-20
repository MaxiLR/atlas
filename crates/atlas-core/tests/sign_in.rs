//! Sign in with ChatGPT against a fake authorisation server on `127.0.0.1`
//! with a test signing key. The fake browser follows the authorisation page's
//! redirect to the core's callback listener.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use atlas_core::auth::{
    AuthError, HostId, Login, PlanAuth, PlanAuthDeps, RefreshError, Revocation, SignInMode,
};
use atlas_core::ports::{Browser, BrowserError, Clock, Endpoints, SecretRecord};
use atlas_core::testing::{FakeClock, MemorySecretStore};
use axum::extract::{Form, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jsonwebtoken::jwk::{Jwk, JwkSet, PublicKeyUse};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use url::Url;

const SCOPES: &str =
    "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";
const PLAN_SCOPE: &str = "chatgpt.tokens.use.direct";
const RESOURCE: &str = "https://api.openai.com/v1";
const SUBJECT: &str = "user-7Qe2";
const EMAIL: &str = "ada@example.com";
const KEY_ID: &str = "test-key-1";

const AUTHORIZE: &str = "/auth/api/accounts/authorize";
const TOKEN: &str = "/auth/api/accounts/oauth/token";
/// Deliberately not a conventional path: the core must take it from the
/// discovery document.
const REVOKE: &str = "/auth/oauth/v2/revoke-session";

const TERMINAL_CODES: &[&str] = &[
    "invalid_grant",
    "invalid_refresh_token",
    "token_expired",
    "refresh_token_expired",
    "refresh_token_invalidated",
    "refresh_token_reused",
];

/// The fake server's signing key. A test key, used nowhere else.
const SIGNING_KEY: &str = "-----BEGIN PRIVATE KEY-----
MIIEvwIBADANBgkqhkiG9w0BAQEFAASCBKkwggSlAgEAAoIBAQDHRdJQOn0qc91I
lhfJmh1ctcsVf4fr7Xa599xGDbr1SICaretP9GjcwFQsGeuHt3+ULuOyCDxLuLhE
ZsFK7I+QYHFrDGLJGO2JqTx2TAioQc2C0Wkqs4Vfr35U0H1T5Kr5Q5sZJNEJhCnK
5lU73CMPx1MMoyVDtYcyP9sAC+p/kenxR2kSAcSKgFLEUBvVbQWunDdws3Cb0X6e
Hr7cKPzo8h0WJ8xDgoWjQFlx3RJCgzQCLBGnLztfq92VYJwO9Ffu3I7FUWAFpbNr
F2xm59G8RlH/P7MDQ/iomF576X4FumuMn377wFFrSD5QCeIQyBBuSNo5QtE9Hfoq
3X6Fq50ZAgMBAAECggEAAi4cX+5T4jwERsAFoJoECjie8SxFA2rEqsIwcb/CtN5H
MeoHMge9gfTEtvrNnd1/EtL0gMUgS7/InE73fGdcJrs3YnFvPz37k077r1ujmJun
sO+zk9kE84gK/MiPJnTQ/gskTkEd3FnKz0J32kDNTWXj3baHONdqlD0omzaGKUEd
ed9+YjehFkaCcvq069XTeENMDyJEaJ7sywxgrqwfQfnZIpIIre81g/Eh/B38Ma3k
4cLPfIeqZ92m6wnKAWRN53Z9X/wfNCSlxzbW+7E4WqK/RbxKNxQQIHv1W0qzNqcx
0LhnZIgYAL0GATBBQAIQdn6hClCaMIxmRPmJXlej1QKBgQDj56mXNYj4FcgsZB9C
0oxkr1DdqjQ6XpJsgr7OQjeA/powUqMY3jvUZaA2mzwByKMz78FCetPEJd3nlX6j
sBXWBMePAx1UmSipQgKU+WVPf1aIoa603k18GnwMZhF5RITR8ydr185MPQtl62rz
PEFgvUJBz3yyqeVekSMSAnYoSwKBgQDf1pHyGSoj9y8kxeR52B9cDl+WUaCUieQ7
4/jZu5M/5aoBUqHjm5YnAtbKM9QCq9Rf4fgcLE2QYtgJCI5oD3621wSiCzhDByQp
/JZgsw+GN4t9f/+xmwt6kxQqXTt0Bq/mCYCae5q1+hmW+nyADYpyFJumADd6o29M
6timZZI5qwKBgQDf39/OM76axPrf8HJs99sFxNoRBXNMqz327fB1OMLBKllYgrTI
ZvYqzLG3MoKd1fPLvwb/QRzpH7F+HnJKZVzt4cAOkiarxdAVhEx4/wwwLYxZp0mx
TnabJIEaxOEBWPeILe1ZmKKEbarWsY1b5qLqXr/JWgLKlu5EGRp0RIrBRQKBgQCB
NNSOESSYGikmauzVwIFpnAplNL0iwO3z/EY7OOFosuONW+uC/N6RqjveAEjTOeif
aErVyGUFgWj+3eeF/dvBIyDTgPJ7DnY7nkgXjYTajtmAGhC4rqzTlm6/fY3CDdIf
UUDg65SEkteOpQyUxjwyWd721Y0wx4KJvc5NTrg3fQKBgQDAdNQs5omvBCTjgT1W
kcqbBXKyPukHFGzd2ok1hOoUURfuKObCsMafwU0eHogENbJqr1cR2ZpfQeKna1KG
rjKosyLx8bP1jedh35CouWEtyPphxf8hyI3yu9maUbcptYI750FMuDCLc8VZqxJv
vlmXEqvKDaJbDuudP4oRvJ1NXw==
-----END PRIVATE KEY-----
";

/// A key the server does not publish, for a badly signed ID token.
const FOREIGN_KEY: &str = "-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQDQNpKiT7em9psZ
ee2BduhEsgfN5Up3JqeuXw4jVh8ms46MsH/RwrZ0+l49B1MpDR1EYe9NAtBmUW/z
s0YkINTtPqCs11140PEdndljcsZBDy9dja4lWYhJbbEdnYkcjd0SBxJvXHp1CXK+
ug79x3ARVrdjAGofJYH0srK+1VvlyWi1GMuFgI8cewIIbEqavWoyY7fUQWMq4yZT
xEYtey8wF7Bvr4uK6YLMdJxzr5jWqGbI/49zYhOumpC1+osoCiVAnpFaECjA/aJ5
aHo9ZVycoqXiaX+HhcPveZoPPbFyWF2vnPQohpcgPO1kzNWl/RaLjoG4qY01nu03
39eAUV2/AgMBAAECggEAO/69mc9ml9GPN3QOn7NnkgPKg+WB43tb7b6uEDiMmVY7
yQZCjmqQsHxEE0m9CKMiv8WIcqRQ8Gqe25XYrChZslLNcYBNCrcKAwKs1R8JSU3a
1BYzC8gUoTPeSS5Ph87I2Jz5DuWS0455ZcOu9ik1fYmDJh/EdF0WaS/kmWBZLoip
E6Xzpri9wcpOZTKDE/2VG1G+FhzwJyUIzo8THjI1K8evEGXucMhREfhEvaKTVjwr
ot7vn04cLitIdwGFMO+hNg7GyzDwUL3idASJHySqtZ2vDM3QTpQIj76mNo8ofV0+
B2E+fCsFSWWlR59Oy0DVrLMPaMqE+K+7quyO+OpZgQKBgQD5fMg+sP1x6tGPm+gz
C1Ja5T9P5SEjkqAB2wFENsTOqOlyfO3K3bnb7aziYf51YCnkJp8BU95qvX8IEm5l
cNCwnX1OuK4v5WKEgTXmn7/4oemdT0zcu0doXK5vapnBgJ0s8eKr9t3Hb+HRnI/K
+gbqT6UP9J0WlqOot5pyZKrUUQKBgQDVpfjuQ8c3FGENkhUIqx5enxfOeh6zClZW
zHzs2ke19gGvHcxzbBPq1qDPaIRH1l9AOH5YdqG1P071VJ87bU20+vnTmn/pU5Pq
e8NYFIn7PQkdsGHQIBJyTAomgW3jFe45oC84xlN+j7n3po5m4AX+gDn33kSQLBFH
SOdJmHXdDwKBgCNHHL2Yo03faWCIKC5HlI6byfcPLD9udUet4yD1KaEbS19QAQPl
lGlf00lzl9qbDXFgcK1sYY/wWa0Xhhq7+BZxPo3KGX9WTTb1WlzRDAEpS/Jdiz3P
EYrVSCkjBrjQ6ei4D1hHoRaZYHIWVFLcRewN9QYrjabrCBtMUv+H8ovxAoGAJwZp
IQSKaBo13Ce55dpz8q62x4VxBB04RiSWESGSBb/I8PbNeKveO4A24Yvi1chnn7By
C7Cnuxa0UoXBjDZcAlnxtmA7SyB0HVm8BePCjHzKpHyqcTzagbw4Am6T5u4DlM0Y
sxMu/yg7vOvUiSAUmDdN32PfPEKAaXf8n2nU5asCgYEA08B+TFpY9WStM07UPPso
zdjgdgul1/NgUrUhZdbff+TpKnufZf5jxdyCB29EcQsS75+INqrgk7bVY3b1SjmE
xG1/RbDkhSC/cVg2dakxVGiew8FbJ8MwU5yZGisk5hVa+A7E9w5qGSbGWRRnKeqp
ySDab/lolZzFcmrZQUfA4DQ=
-----END PRIVATE KEY-----
";

// ---------------------------------------------------------------------------
// The fake authorisation server
// ---------------------------------------------------------------------------

/// One way to spoil the ID token the token endpoint returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flaw {
    WrongNonce,
    WrongAudience,
    Expired,
    BadSignature,
}

/// How the server behaves; a test changes it between steps.
#[derive(Debug, Clone)]
struct Knobs {
    /// Whether the user grants `chatgpt.tokens.use.direct`.
    grant_plan_scope: bool,
    /// The user declines on the consent page.
    decline: bool,
    /// The callback carries a state other than the one sent.
    forge_state: bool,
    /// A new registration's callback leaves out the issued client id.
    omit_issued_client_id: bool,
    /// A reauthorisation's callback names this client id.
    reauth_client_id: Option<String>,
    /// The next code exchanges answer 500.
    failing_exchanges: u32,
    id_token_flaw: Option<Flaw>,
    /// Replies to the next refreshes, before rotating normally again.
    refresh_replies: VecDeque<(u16, Value)>,
}

impl Default for Knobs {
    fn default() -> Self {
        Self {
            grant_plan_scope: true,
            decline: false,
            forge_state: false,
            omit_issued_client_id: false,
            reauth_client_id: None,
            failing_exchanges: 0,
            id_token_flaw: None,
            refresh_replies: VecDeque::new(),
        }
    }
}

/// A request the server received, with what the secret store held at that
/// moment.
#[derive(Debug, Clone)]
struct Seen {
    path: String,
    /// The query for a GET, the form for a POST.
    params: HashMap<String, String>,
    client_id_in_store: Option<String>,
    login_in_store: bool,
}

#[derive(Debug, Clone)]
struct Grant {
    client_id: String,
    nonce: String,
    challenge: String,
    redirect_uri: String,
    scope: String,
}

#[derive(Default)]
struct AuthState {
    knobs: Knobs,
    seen: Vec<Seen>,
    registered: Vec<String>,
    codes: HashMap<String, Grant>,
    /// Refresh tokens still valid, with their client id and scope.
    live_refresh: HashMap<String, (String, String)>,
    issued_id_tokens: Vec<String>,
    counter: u32,
}

struct AuthServerInner {
    base: Url,
    secrets: Arc<MemorySecretStore>,
    clock: Arc<FakeClock>,
    state: Mutex<AuthState>,
}

impl AuthServerInner {
    fn issuer(&self) -> String {
        format!("{}auth", self.base)
    }

    fn state(&self) -> std::sync::MutexGuard<'_, AuthState> {
        self.state.lock().expect("server lock poisoned")
    }

    fn record(&self, path: &str, params: &HashMap<String, String>) {
        let seen = Seen {
            path: path.to_owned(),
            params: params.clone(),
            client_id_in_store: self.secrets.get(SecretRecord::IssuedClientId),
            login_in_store: self.secrets.get(SecretRecord::PlanLogin).is_some(),
        };
        self.state().seen.push(seen);
    }

    fn id_token(&self, grant: &Grant, flaw: Option<Flaw>) -> String {
        let now = self.clock.now().timestamp();
        let (iat, exp) = if flaw == Some(Flaw::Expired) {
            (now - 7200, now - 3600)
        } else {
            (now, now + 3600)
        };
        let claims = json!({
            "iss": self.issuer(),
            "sub": SUBJECT,
            "email": EMAIL,
            "aud": if flaw == Some(Flaw::WrongAudience) { "oaiapp_someone_else" } else { grant.client_id.as_str() },
            "nonce": if flaw == Some(Flaw::WrongNonce) { "not-the-nonce" } else { grant.nonce.as_str() },
            "iat": iat,
            "exp": exp,
        });
        let pem = if flaw == Some(Flaw::BadSignature) {
            FOREIGN_KEY
        } else {
            SIGNING_KEY
        };
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(KEY_ID.to_owned());
        let key = EncodingKey::from_rsa_pem(pem.as_bytes()).expect("a valid test key");
        jsonwebtoken::encode(&header, &claims, &key).expect("a signed ID token")
    }
}

fn oauth_error(status: u16, body: Value) -> Response {
    (
        StatusCode::from_u16(status).expect("a valid status"),
        Json(body),
    )
        .into_response()
}

async fn discovery(State(server): State<Arc<AuthServerInner>>) -> Json<Value> {
    let base = server.base.as_str().trim_end_matches('/');
    Json(json!({
        "issuer": server.issuer(),
        "authorization_endpoint": format!("{base}{AUTHORIZE}"),
        "token_endpoint": format!("{base}{TOKEN}"),
        "jwks_uri": format!("{base}/auth/.well-known/jwks.json"),
        "revocation_endpoint": format!("{base}{REVOKE}"),
    }))
}

async fn jwks() -> Json<Value> {
    let key = EncodingKey::from_rsa_pem(SIGNING_KEY.as_bytes()).expect("a valid test key");
    let mut jwk = Jwk::from_encoding_key(&key, Algorithm::RS256).expect("a public JWK");
    jwk.common.key_id = Some(KEY_ID.to_owned());
    jwk.common.public_key_use = Some(PublicKeyUse::Signature);
    Json(serde_json::to_value(JwkSet { keys: vec![jwk] }).expect("a JWK set"))
}

async fn authorize(
    State(server): State<Arc<AuthServerInner>>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    server.record(AUTHORIZE, &params);
    let param = |name: &str| params.get(name).cloned().unwrap_or_default();
    let redirect_uri = param("redirect_uri");
    let mut state = server.state();
    let knobs = state.knobs.clone();
    let mut callback = Url::parse(&redirect_uri).expect("a redirect URI");
    let returned_state = if knobs.forge_state {
        "forged-state".to_owned()
    } else {
        param("state")
    };
    if knobs.decline {
        callback
            .query_pairs_mut()
            .append_pair("error", "access_denied")
            .append_pair("state", &returned_state);
        return Redirect::to(callback.as_str()).into_response();
    }
    let requested = param("client_id");
    let (client_id, callback_client_id) = if requested == "dynamic_agent_client" {
        state.counter += 1;
        let issued = format!("oaiapp_atlas_{}", state.counter);
        state.registered.push(issued.clone());
        let named = (!knobs.omit_issued_client_id).then(|| issued.clone());
        (issued, named)
    } else if state.registered.contains(&requested) {
        (requested, knobs.reauth_client_id.clone())
    } else {
        callback
            .query_pairs_mut()
            .append_pair("error", "unauthorized_client")
            .append_pair("state", &returned_state);
        return Redirect::to(callback.as_str()).into_response();
    };
    let scope = param("scope")
        .split(' ')
        .filter(|scope| knobs.grant_plan_scope || *scope != PLAN_SCOPE)
        .collect::<Vec<_>>()
        .join(" ");
    state.counter += 1;
    let code = format!("code-{}", state.counter);
    state.codes.insert(
        code.clone(),
        Grant {
            client_id,
            nonce: param("nonce"),
            challenge: param("code_challenge"),
            redirect_uri,
            scope: scope.clone(),
        },
    );
    {
        let mut pairs = callback.query_pairs_mut();
        pairs
            .append_pair("code", &code)
            .append_pair("scope", &scope)
            .append_pair("state", &returned_state);
        if let Some(client_id) = callback_client_id {
            pairs.append_pair("client_id", &client_id);
        }
    }
    Redirect::to(callback.as_str()).into_response()
}

async fn token(
    State(server): State<Arc<AuthServerInner>>,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    server.record(TOKEN, &form);
    let field = |name: &str| form.get(name).cloned().unwrap_or_default();
    match field("grant_type").as_str() {
        "authorization_code" => {
            let mut state = server.state();
            if state.knobs.failing_exchanges > 0 {
                state.knobs.failing_exchanges -= 1;
                return oauth_error(500, json!({ "error": "server_error" }));
            }
            let Some(grant) = state.codes.remove(&field("code")) else {
                return oauth_error(400, json!({ "error": "invalid_grant" }));
            };
            let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(field("code_verifier")));
            if grant.client_id != field("client_id")
                || grant.challenge != challenge
                || grant.redirect_uri != field("redirect_uri")
                || field("resource") != RESOURCE
            {
                return oauth_error(400, json!({ "error": "invalid_grant" }));
            }
            let id_token = server.id_token(&grant, state.knobs.id_token_flaw);
            state.issued_id_tokens.push(id_token.clone());
            state.counter += 1;
            let access = format!("access-{}", state.counter);
            let refresh = format!("refresh-{}", state.counter);
            state
                .live_refresh
                .insert(refresh.clone(), (grant.client_id, grant.scope.clone()));
            Json(json!({
                "access_token": access,
                "refresh_token": refresh,
                "id_token": id_token,
                "token_type": "Bearer",
                "expires_in": 3600,
                "scope": grant.scope,
            }))
            .into_response()
        }
        "refresh_token" => {
            // Long enough for a concurrent caller to arrive meanwhile.
            tokio::time::sleep(Duration::from_millis(100)).await;
            let mut state = server.state();
            if let Some((status, body)) = state.knobs.refresh_replies.pop_front() {
                return oauth_error(status, body);
            }
            let Some((client_id, scope)) = state.live_refresh.remove(&field("refresh_token"))
            else {
                return oauth_error(400, json!({ "error": "invalid_grant" }));
            };
            if client_id != field("client_id") || field("resource") != RESOURCE {
                return oauth_error(400, json!({ "error": "invalid_grant" }));
            }
            state.counter += 1;
            let access = format!("access-{}", state.counter);
            let refresh = format!("refresh-{}", state.counter);
            state
                .live_refresh
                .insert(refresh.clone(), (client_id, scope.clone()));
            Json(json!({
                "access_token": access,
                "refresh_token": refresh,
                "token_type": "Bearer",
                "expires_in": 3600,
                "scope": scope,
            }))
            .into_response()
        }
        _ => oauth_error(400, json!({ "error": "unsupported_grant_type" })),
    }
}

async fn revoke(
    State(server): State<Arc<AuthServerInner>>,
    Form(form): Form<HashMap<String, String>>,
) -> StatusCode {
    server.record(REVOKE, &form);
    server.state().live_refresh.remove(&form["token"]);
    StatusCode::OK
}

struct FakeAuthServer {
    inner: Arc<AuthServerInner>,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl FakeAuthServer {
    async fn start(secrets: Arc<MemorySecretStore>, clock: Arc<FakeClock>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind a free port");
        let address = listener.local_addr().expect("the bound address");
        let inner = Arc::new(AuthServerInner {
            base: Url::parse(&format!("http://{address}/")).expect("a valid URL"),
            secrets,
            clock,
            state: Mutex::new(AuthState::default()),
        });
        let app = Router::new()
            .route("/auth/.well-known/openid-configuration", get(discovery))
            .route("/auth/.well-known/jwks.json", get(jwks))
            .route(AUTHORIZE, get(authorize))
            .route(TOKEN, post(token))
            .route(REVOKE, post(revoke))
            .with_state(inner.clone());
        let (shutdown, stopped) = oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .expect("the fake authorisation server runs");
        });
        Self {
            inner,
            shutdown: Some(shutdown),
            task: Some(task),
        }
    }

    fn endpoints(&self) -> Endpoints {
        Endpoints::local(&self.inner.base)
    }

    /// Stops the server and waits until every connection is closed.
    async fn stop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            task.await.expect("the server stops");
        }
    }

    fn knobs(&self, change: impl FnOnce(&mut Knobs)) {
        change(&mut self.inner.state().knobs);
    }

    fn seen(&self, path: &str) -> Vec<Seen> {
        self.inner
            .state()
            .seen
            .iter()
            .filter(|seen| seen.path == path)
            .cloned()
            .collect()
    }

    fn token_requests(&self, grant_type: &str) -> Vec<Seen> {
        self.seen(TOKEN)
            .into_iter()
            .filter(|seen| seen.params.get("grant_type").map(String::as_str) == Some(grant_type))
            .collect()
    }

    fn issued_id_tokens(&self) -> Vec<String> {
        self.inner.state().issued_id_tokens.clone()
    }

    fn registrations(&self) -> usize {
        self.inner.state().registered.len()
    }
}

// ---------------------------------------------------------------------------
// The fake browser
// ---------------------------------------------------------------------------

/// Records each URL and visits it, following redirects, as the user's
/// browser would after the user approves.
#[derive(Default)]
struct RedirectingBrowser {
    opened: Mutex<Vec<Url>>,
}

impl RedirectingBrowser {
    fn opened(&self) -> Vec<Url> {
        self.opened.lock().expect("browser lock poisoned").clone()
    }
}

impl Browser for RedirectingBrowser {
    fn open(&self, url: &Url) -> Result<(), BrowserError> {
        self.opened
            .lock()
            .expect("browser lock poisoned")
            .push(url.clone());
        let url = url.clone();
        tokio::spawn(async move {
            let _ = reqwest::Client::new().get(url).send().await;
        });
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// The harness
// ---------------------------------------------------------------------------

struct Harness {
    server: FakeAuthServer,
    secrets: Arc<MemorySecretStore>,
    clock: Arc<FakeClock>,
    browser: Arc<RedirectingBrowser>,
    host_id: HostId,
    host_id_path: PathBuf,
    auth: PlanAuth,
    _dir: TempDir,
}

impl Harness {
    async fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temp dir");
        let host_id_path = dir.path().join("host-id");
        let host_id = HostId::load_or_create(&host_id_path).expect("a host id");
        let secrets = Arc::new(MemorySecretStore::default());
        let clock = Arc::new(FakeClock::default());
        let browser = Arc::new(RedirectingBrowser::default());
        let server = FakeAuthServer::start(secrets.clone(), clock.clone()).await;
        let auth = PlanAuth::new(PlanAuthDeps {
            endpoints: server.endpoints(),
            secrets: secrets.clone(),
            browser: browser.clone(),
            clock: clock.clone(),
            host_id: host_id.clone(),
        });
        Self {
            server,
            secrets,
            clock,
            browser,
            host_id,
            host_id_path,
            auth,
            _dir: dir,
        }
    }

    /// A harness after one successful first sign-in.
    async fn signed_in() -> Self {
        let harness = Self::new().await;
        harness
            .sign_in(SignInMode::SignIn)
            .await
            .expect("the first sign-in succeeds");
        harness
    }

    async fn sign_in(&self, mode: SignInMode) -> Result<Login, AuthError> {
        let pending = self.auth.start(mode).await?;
        tokio::time::timeout(Duration::from_secs(10), pending.finish())
            .await
            .expect("the sign-in ends")
    }

    /// The query of the last authorisation page the browser opened.
    fn last_authorize(&self) -> HashMap<String, String> {
        let url = self.browser.opened().pop().expect("the browser was opened");
        url.query_pairs().into_owned().collect()
    }

    fn client_id(&self) -> Option<String> {
        self.secrets.get(SecretRecord::IssuedClientId)
    }

    fn has_tokens(&self) -> bool {
        self.secrets.get(SecretRecord::PlanLogin).is_some()
    }
}

fn code_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

// ---------------------------------------------------------------------------
// Host id
// ---------------------------------------------------------------------------

#[test]
fn the_host_id_is_a_urn_uuid_generated_once_and_reused() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let path = dir.path().join("host-id");
    let first = HostId::load_or_create(&path).expect("a host id");
    let again = HostId::load_or_create(&path).expect("the same host id");

    assert_eq!(first, again);
    let uuid = first
        .0
        .strip_prefix("urn:uuid:")
        .expect("a urn:uuid: host id");
    uuid::Uuid::parse_str(uuid).expect("a UUID after the prefix");

    let other_dir = tempfile::tempdir().expect("a temp dir");
    let other = HostId::load_or_create(&other_dir.path().join("host-id")).expect("a host id");
    assert_ne!(first, other, "another installation gets its own host id");
}

// ---------------------------------------------------------------------------
// First sign-in
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_first_sign_in_sends_the_registration_parameters_and_saves_the_client_id_before_the_exchange()
 {
    let harness = Harness::new().await;

    let login = harness
        .sign_in(SignInMode::SignIn)
        .await
        .expect("signed in");

    let opened = harness.browser.opened();
    assert_eq!(opened.len(), 1);
    let page = &opened[0];
    assert_eq!(page.path(), AUTHORIZE);
    assert_eq!(page.host_str(), Some("127.0.0.1"));
    let query = harness.last_authorize();
    assert_eq!(query["client_id"], "dynamic_agent_client");
    assert_eq!(query["agent_name_hint"], "Atlas");
    assert_eq!(query["ext_agent_host_id"], harness.host_id.0);
    assert_eq!(query["response_type"], "code");
    assert_eq!(query["scope"], SCOPES);
    assert_eq!(query["resource"], RESOURCE);
    assert_eq!(query["code_challenge_method"], "S256");
    assert!(query["state"].len() >= 16, "a random state");
    assert!(query["nonce"].len() >= 16, "a random nonce");
    assert!(!query.contains_key("id_token_hint"));
    assert!(!query.contains_key("prompt"));
    let redirect = Url::parse(&query["redirect_uri"]).expect("a redirect URI");
    assert_eq!(redirect.scheme(), "http");
    assert_eq!(redirect.host_str(), Some("127.0.0.1"));
    assert_eq!(redirect.path(), "/auth/callback");
    assert!(redirect.port().is_some());

    let exchanges = harness.server.token_requests("authorization_code");
    assert_eq!(exchanges.len(), 1);
    let exchange = &exchanges[0];
    assert_eq!(exchange.params["client_id"], "oaiapp_atlas_1");
    assert_eq!(exchange.params["redirect_uri"], query["redirect_uri"]);
    assert_eq!(exchange.params["resource"], RESOURCE);
    assert!(exchange.params.contains_key("code"));
    assert_eq!(
        code_challenge(&exchange.params["code_verifier"]),
        query["code_challenge"],
        "the verifier matches the S256 challenge"
    );
    assert!(
        exchange
            .client_id_in_store
            .as_deref()
            .is_some_and(|stored| stored.contains("oaiapp_atlas_1")),
        "the issued client id is saved before the code is exchanged, found {:?}",
        exchange.client_id_in_store
    );

    assert_eq!(login.identity.subject, SUBJECT);
    assert_eq!(login.identity.email, EMAIL);
    assert!(login.plan_usage);
    assert!(login.scopes.iter().any(|scope| scope == PLAN_SCOPE));
    assert_eq!(harness.auth.login().expect("the login reads"), Some(login));
    assert!(harness.has_tokens());
    assert_eq!(
        harness
            .auth
            .access_token()
            .expect("the token reads")
            .map(|token| token.secret().to_owned()),
        Some("access-3".to_owned())
    );
}

#[tokio::test]
async fn a_failed_exchange_keeps_the_issued_client_id_and_the_next_attempt_uses_it() {
    let harness = Harness::new().await;
    harness.server.knobs(|knobs| knobs.failing_exchanges = 1);

    let failed = harness.sign_in(SignInMode::SignIn).await;
    assert!(
        matches!(failed, Err(AuthError::TokenEndpoint(_))),
        "{failed:?}"
    );
    assert!(
        harness
            .client_id()
            .is_some_and(|stored| stored.contains("oaiapp_atlas_1"))
    );
    assert!(!harness.has_tokens());

    let login = harness
        .sign_in(SignInMode::SignIn)
        .await
        .expect("the second attempt signs in");
    assert!(login.plan_usage);
    let query = harness.last_authorize();
    assert_eq!(query["client_id"], "oaiapp_atlas_1");
    assert!(!query.contains_key("agent_name_hint"));
    assert!(!query.contains_key("id_token_hint"), "no ID token retained");
    assert_eq!(query["ext_agent_host_id"], harness.host_id.0);
    assert_eq!(harness.server.registrations(), 1);
    let exchanges = harness.server.token_requests("authorization_code");
    assert_eq!(exchanges.len(), 2);
    assert_eq!(exchanges[1].params["client_id"], "oaiapp_atlas_1");
    assert!(harness.has_tokens());
}

#[tokio::test]
async fn a_new_registration_without_an_issued_client_id_is_incomplete() {
    let harness = Harness::new().await;
    harness
        .server
        .knobs(|knobs| knobs.omit_issued_client_id = true);

    let result = harness.sign_in(SignInMode::SignIn).await;

    assert_eq!(result, Err(AuthError::RegistrationIncomplete));
    assert!(harness.server.seen(TOKEN).is_empty());
    assert!(!harness.has_tokens());
}

#[tokio::test]
async fn a_different_client_id_on_reauthorisation_is_rejected() {
    let harness = Harness::signed_in().await;
    harness
        .server
        .knobs(|knobs| knobs.reauth_client_id = Some("oaiapp_intruder".to_owned()));

    let result = harness.sign_in(SignInMode::SignIn).await;

    assert_eq!(result, Err(AuthError::ClientMismatch));
    assert_eq!(harness.server.token_requests("authorization_code").len(), 1);
    assert!(harness.client_id().is_some_and(
        |stored| stored.contains("oaiapp_atlas_1") && !stored.contains("oaiapp_intruder")
    ));
}

// ---------------------------------------------------------------------------
// Declined permission and the plan scope
// ---------------------------------------------------------------------------

#[tokio::test]
async fn declined_permission_ends_the_attempt_without_an_exchange() {
    let harness = Harness::new().await;
    harness.server.knobs(|knobs| knobs.decline = true);

    let result = harness.sign_in(SignInMode::SignIn).await;

    assert_eq!(result, Err(AuthError::Declined));
    assert!(harness.server.seen(TOKEN).is_empty());
    assert!(!harness.has_tokens());
    assert_eq!(harness.auth.login().expect("the login reads"), None);
}

#[tokio::test]
async fn a_sign_in_without_the_plan_scope_is_kept_with_plan_usage_off_and_enabling_turns_it_on() {
    let harness = Harness::new().await;
    harness.server.knobs(|knobs| knobs.grant_plan_scope = false);

    let login = harness
        .sign_in(SignInMode::SignIn)
        .await
        .expect("signed in without the plan scope");
    assert!(!login.plan_usage);
    assert!(!login.scopes.iter().any(|scope| scope == PLAN_SCOPE));
    assert_eq!(login.identity.email, EMAIL);
    assert!(harness.has_tokens(), "the sign-in is kept");
    assert_eq!(
        harness
            .auth
            .login()
            .expect("the login reads")
            .map(|login| login.plan_usage),
        Some(false)
    );

    harness.server.knobs(|knobs| knobs.grant_plan_scope = true);
    let enabled = harness
        .sign_in(SignInMode::EnablePlanUsage)
        .await
        .expect("plan usage enabled");

    let query = harness.last_authorize();
    assert_eq!(query["client_id"], "oaiapp_atlas_1");
    assert!(!query.contains_key("agent_name_hint"));
    assert_eq!(query["scope"], SCOPES);
    assert_eq!(query["prompt"], "consent");
    assert_eq!(query["resource"], RESOURCE);
    assert_eq!(query["ext_agent_host_id"], harness.host_id.0);
    assert_eq!(
        query.get("id_token_hint"),
        harness.server.issued_id_tokens().first(),
        "the retained ID token identifies the account"
    );
    assert!(enabled.plan_usage);
    assert_eq!(
        harness
            .auth
            .login()
            .expect("the login reads")
            .map(|login| login.plan_usage),
        Some(true)
    );
    assert_eq!(harness.server.registrations(), 1);
}

#[tokio::test]
async fn enabling_plan_usage_needs_a_registration() {
    let harness = Harness::new().await;

    let result = harness.auth.start(SignInMode::EnablePlanUsage).await;

    assert!(matches!(result, Err(AuthError::NotRegistered)));
    assert!(harness.browser.opened().is_empty());
}

// ---------------------------------------------------------------------------
// Rejected callbacks and ID tokens
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_wrong_state_is_rejected_without_an_exchange() {
    let harness = Harness::new().await;
    harness.server.knobs(|knobs| knobs.forge_state = true);

    let result = harness.sign_in(SignInMode::SignIn).await;

    assert_eq!(result, Err(AuthError::StateMismatch));
    assert!(harness.server.seen(TOKEN).is_empty());
    assert!(!harness.has_tokens());
}

async fn assert_id_token_rejected(flaw: Flaw) {
    let harness = Harness::new().await;
    harness
        .server
        .knobs(|knobs| knobs.id_token_flaw = Some(flaw));

    let result = harness.sign_in(SignInMode::SignIn).await;

    assert!(
        matches!(result, Err(AuthError::IdTokenRejected(_))),
        "{flaw:?}: {result:?}"
    );
    assert_eq!(harness.server.token_requests("authorization_code").len(), 1);
    assert!(!harness.has_tokens(), "{flaw:?} stored tokens");
    assert_eq!(harness.auth.login().expect("the login reads"), None);
    assert_eq!(harness.auth.access_token().expect("the token reads"), None);
}

#[tokio::test]
async fn an_id_token_with_the_wrong_nonce_is_rejected() {
    assert_id_token_rejected(Flaw::WrongNonce).await;
}

#[tokio::test]
async fn an_id_token_for_another_audience_is_rejected() {
    assert_id_token_rejected(Flaw::WrongAudience).await;
}

#[tokio::test]
async fn an_expired_id_token_is_rejected() {
    assert_id_token_rejected(Flaw::Expired).await;
}

#[tokio::test]
async fn a_badly_signed_id_token_is_rejected() {
    assert_id_token_rejected(Flaw::BadSignature).await;
}

// ---------------------------------------------------------------------------
// Refresh
// ---------------------------------------------------------------------------

#[tokio::test]
async fn refresh_rotates_the_tokens_together() {
    let harness = Harness::signed_in().await;
    harness.clock.advance(Duration::from_secs(3000));

    let token = harness.auth.refresh().await.expect("refreshed");

    assert_eq!(token.secret(), "access-4");
    let refreshes = harness.server.token_requests("refresh_token");
    assert_eq!(refreshes.len(), 1);
    let form = &refreshes[0].params;
    assert_eq!(form["client_id"], "oaiapp_atlas_1");
    assert_eq!(form["refresh_token"], "refresh-3");
    assert_eq!(form["resource"], RESOURCE);
    assert!(!form.contains_key("scope"), "the grant is retained");
    assert_eq!(
        harness
            .auth
            .access_token()
            .expect("the token reads")
            .map(|token| token.secret().to_owned()),
        Some("access-4".to_owned())
    );
    let login = harness
        .auth
        .login()
        .expect("the login reads")
        .expect("still signed in");
    assert_eq!(
        login.expires_at,
        harness.clock.now() + chrono::Duration::seconds(3600)
    );

    let again = harness.auth.refresh().await.expect("refreshed again");
    assert_eq!(again.secret(), "access-5");
    let refreshes = harness.server.token_requests("refresh_token");
    assert_eq!(
        refreshes[1].params["refresh_token"], "refresh-4",
        "the rotated refresh token replaced the old one"
    );
}

#[tokio::test]
async fn two_concurrent_refreshes_send_one_request() {
    let harness = Harness::signed_in().await;

    let (first, second) = tokio::join!(harness.auth.refresh(), harness.auth.refresh());

    let first = first.expect("the first caller gets a token");
    let second = second.expect("the second caller gets a token");
    assert_eq!(first, second);
    assert_eq!(first.secret(), "access-4");
    assert_eq!(harness.server.token_requests("refresh_token").len(), 1);
}

#[tokio::test]
async fn each_terminal_refresh_error_ends_the_login_and_keeps_the_client_id() {
    for (index, code) in TERMINAL_CODES.iter().enumerate() {
        let harness = Harness::signed_in().await;
        // Both shapes an error code arrives in.
        let body = if index % 2 == 0 {
            json!({ "error": code, "error_description": "the refresh token is unusable" })
        } else {
            json!({ "error": { "code": code, "message": "the refresh token is unusable" } })
        };
        harness
            .server
            .knobs(|knobs| knobs.refresh_replies.push_back((400, body)));

        let result = harness.auth.refresh().await;

        assert_eq!(
            result,
            Err(RefreshError::LoginEnded {
                code: (*code).to_owned()
            }),
            "{code}"
        );
        assert!(!harness.has_tokens(), "{code} left the tokens");
        assert_eq!(harness.auth.login().expect("the login reads"), None);
        assert!(
            harness
                .client_id()
                .is_some_and(|stored| stored.contains("oaiapp_atlas_1")),
            "{code} lost the client id"
        );
    }
}

#[tokio::test]
async fn a_server_error_on_refresh_keeps_the_tokens() {
    let harness = Harness::signed_in().await;
    harness.server.knobs(|knobs| {
        knobs
            .refresh_replies
            .push_back((500, json!({ "error": "server_error" })))
    });

    let result = harness.auth.refresh().await;

    assert!(
        matches!(result, Err(RefreshError::Unavailable(_))),
        "{result:?}"
    );
    assert!(harness.has_tokens());
    assert!(harness.auth.login().expect("the login reads").is_some());
    let token = harness
        .auth
        .refresh()
        .await
        .expect("the next refresh succeeds");
    assert_eq!(token.secret(), "access-4");
    let refreshes = harness.server.token_requests("refresh_token");
    assert_eq!(refreshes[1].params["refresh_token"], "refresh-3");
}

#[tokio::test]
async fn a_network_error_on_refresh_keeps_the_tokens() {
    let mut harness = Harness::signed_in().await;
    harness.server.stop().await;

    let result = harness.auth.refresh().await;

    assert!(
        matches!(result, Err(RefreshError::Unavailable(_))),
        "{result:?}"
    );
    assert!(harness.has_tokens());
    assert!(harness.auth.login().expect("the login reads").is_some());
}

// ---------------------------------------------------------------------------
// Sign-out
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sign_out_revokes_then_clears_and_keeps_the_client_id_and_host_id() {
    let harness = Harness::signed_in().await;

    let signed_out = harness.auth.sign_out().await.expect("signed out");

    assert_eq!(signed_out.revocation, Revocation::Confirmed);
    let revocations = harness.server.seen(REVOKE);
    assert_eq!(revocations.len(), 1);
    let revocation = &revocations[0];
    assert_eq!(revocation.params["token"], "refresh-3");
    assert_eq!(revocation.params["token_type_hint"], "refresh_token");
    assert_eq!(revocation.params["client_id"], "oaiapp_atlas_1");
    assert!(
        revocation.login_in_store,
        "the tokens are cleared after revocation"
    );
    assert!(!harness.has_tokens());
    assert_eq!(harness.auth.login().expect("the login reads"), None);
    assert!(
        harness
            .client_id()
            .is_some_and(|stored| stored.contains("oaiapp_atlas_1"))
    );
    assert_eq!(
        HostId::load_or_create(&harness.host_id_path).expect("the host id"),
        harness.host_id
    );

    harness
        .sign_in(SignInMode::SignIn)
        .await
        .expect("signed in again");
    let query = harness.last_authorize();
    assert_eq!(query["client_id"], "oaiapp_atlas_1");
    assert!(!query.contains_key("agent_name_hint"));
    assert!(
        !query.contains_key("id_token_hint"),
        "no hint after signing out"
    );
    assert_eq!(query["ext_agent_host_id"], harness.host_id.0);
    assert_eq!(harness.server.registrations(), 1);
}

#[tokio::test]
async fn sign_out_with_the_server_down_clears_locally_and_reports_the_revocation_unconfirmed() {
    let mut harness = Harness::signed_in().await;
    harness.server.stop().await;

    let signed_out = harness.auth.sign_out().await.expect("signed out locally");

    assert!(
        matches!(signed_out.revocation, Revocation::Unconfirmed { .. }),
        "{signed_out:?}"
    );
    assert!(!harness.has_tokens());
    assert_eq!(harness.auth.login().expect("the login reads"), None);
    assert!(
        harness
            .client_id()
            .is_some_and(|stored| stored.contains("oaiapp_atlas_1"))
    );
}

// ---------------------------------------------------------------------------
// The redirect URI
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_redirect_uri_is_never_localhost_and_only_its_port_varies() {
    let harness = Harness::new().await;
    let first = harness
        .auth
        .start(SignInMode::SignIn)
        .await
        .expect("started");
    let second = harness
        .auth
        .start(SignInMode::SignIn)
        .await
        .expect("started");

    let mut uris = Vec::new();
    for (pending, opened) in [first, second].iter().zip(harness.browser.opened()) {
        let query: HashMap<String, String> = opened.query_pairs().into_owned().collect();
        let sent = Url::parse(&query["redirect_uri"]).expect("a redirect URI");
        assert_eq!(&sent, pending.redirect_uri());
        assert!(!sent.as_str().contains("localhost"));
        uris.push(sent);
    }
    assert_ne!(
        uris[0].port(),
        uris[1].port(),
        "two live listeners use two ports"
    );
    for uri in &mut uris {
        uri.set_port(None).expect("a port can be cleared");
    }
    assert_eq!(uris[0], uris[1]);
    assert_eq!(uris[0].as_str(), "http://127.0.0.1/auth/callback");
}
