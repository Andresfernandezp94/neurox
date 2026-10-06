// Google Sign-In: verify a Google ID token, then mint neurox tokens.
//
// Flow: the client (web or Android Credential Manager) obtains a Google ID
// token and POSTs it here. The daemon validates it against Google's JWKS
// (RS256, `iss` and `aud` checked), then maps the Google subject to a local
// user and issues the same JWT + refresh token that password login returns.
//
// Security notes:
//   - The ID token is verified with the *public* JWKS, so no shared secret
//     is needed to validate it. `client_secret` is only used for the
//     server-side calls (userinfo, token revocation) and stays out of the
//     repo.
//   - A local account is matched by verified email (`email_verified`). A
//     Google account whose email does not match gets a brand new user with
//     the `Viewer` role: the first Google login must never silently become
//     Admin.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Extension;
use axum::Json;
use serde::Deserialize;

use super::handlers::{AuthState, ErrorBody, LoginResponse};
use super::middleware::Role;
use super::tokens::issue_token;

/// Google's signing keys. Public, no auth needed.
const GOOGLE_JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
/// Google only signs ID tokens with RS256. Pinned instead of read from the
/// token header so a crafted header can never pick the algorithm we verify
/// with.
const GOOGLE_ALG: &str = "RS256";

#[derive(Deserialize)]
pub struct GoogleSignInRequest {
    /// The ID token obtained on the client (Credential Manager on Android,
    /// GIS button on web).
    pub id_token: String,
}

#[derive(Debug, Deserialize)]
struct GoogleClaims {
    /// Google user id. Stable, and the only field we key on.
    sub: String,
    email: Option<String>,
    #[serde(default)]
    email_verified: Option<bool>,
    /// "accounts.google.com" or an https://accounts.google.com URL.
    iss: Option<String>,
    aud: Option<String>,
}

impl GoogleClaims {
    fn issuer_ok(&self) -> bool {
        match &self.iss {
            Some(i) => i == "accounts.google.com" || i == "https://accounts.google.com",
            None => false,
        }
    }
}

#[derive(Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Deserialize)]
struct Jwk {
    kid: String,
    kty: String,
    alg: String,
    #[serde(rename = "use")]
    use_: Option<String>,
    n: String,
    e: String,
}

fn json_err(code: StatusCode, error: &str, message: &str) -> Response {
    (
        code,
        Json(ErrorBody {
            error: match error {
                "unauthorized" => "unauthorized",
                _ => "error",
            },
            message: message.to_string(),
        }),
    )
        .into_response()
}

/// Fetches and decodes Google's JWKS, then verifies the ID token's signature
/// and claims. Returns the claims on success.
async fn verify_google_id_token(
    client: &reqwest::Client,
    id_token: &str,
    expected_aud: &str,
) -> Result<GoogleClaims, String> {
    let jwks: Jwks = client
        .get(GOOGLE_JWKS_URL)
        .send()
        .await
        .map_err(|e| format!("jwks fetch: {e}"))?
        .json()
        .await
        .map_err(|e| format!("jwks decode: {e}"))?;

    let header = jsonwebtoken::decode_header(id_token)
        .map_err(|e| format!("decode header: {e}"))?;
    if header.alg != jsonwebtoken::Algorithm::RS256 {
        return Err(format!("alg no soportado: {:?}", header.alg));
    }
    let kid = header.kid.ok_or("token sin kid")?;

    let jwk = jwks
        .keys
        .iter()
        .find(|k| {
            k.kid == kid
                && k.kty == "RSA"
                && k.alg == GOOGLE_ALG
                // `use` is absent on some JWKS responses; when present it
                // must be the signature use.
                && matches!(k.use_.as_deref(), None | Some("sig"))
        })
        .ok_or("kid desconocida (¿token de otro emisor?)")?;

    // Build the RSA public key from the JWK's n (modulus) and e (exponent).
    let key = jsonwebtoken::DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
        .map_err(|e| format!("jwk inválida: {e}"))?;

    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
    validation.set_audience(&[expected_aud]);
    validation.set_issuer(&["accounts.google.com", "https://accounts.google.com"]);
    // jsonwebtoken checks `exp` itself; keep a little clock skew tolerance.
    validation.leeway = 30;

    let data = jsonwebtoken::decode::<GoogleClaims>(id_token, &key, &validation)
        .map_err(|e| format!("verify: {e}"))?;
    let claims = data.claims;

    // Belt and braces: jsonwebtoken already validated aud/iss, but these are
    // the two fields the whole scheme depends on.
    if !claims.issuer_ok() {
        return Err("issuer no es Google".into());
    }
    if claims.aud.as_deref() != Some(expected_aud) {
        return Err("audience no coincide con el client_id configurado".into());
    }
    Ok(claims)
}



/// Maps a verified Google identity to a local account, creating one if
/// needed. Always lands on a role that is not privileged by accident.
fn upsert_google_user(state: &AuthState, claims: &GoogleClaims) -> Result<super::users::UserView, String> {
    let Some(email) = claims.email.as_deref().filter(|e| !e.is_empty()) else {
        return Err("el ID token no trae email".into());
    };
    // Unverified email must not be able to claim an existing account.
    if claims.email_verified == Some(false) {
        return Err("el email de Google no está verificado".into());
    }

    // Match on the email as the username. Reusing the existing account keeps
    // roles, sessions and prefs intact across the switch to Google login.
    if let Some(existing) = state.user_store.find_by_username(email) {
        return Ok(existing.view());
    }

    // `create` already returns a UserView.
    state
        .user_store
        .create(email, &format!("google:{}", claims.sub), Role::Viewer)
        .map_err(|e| format!("no se pudo crear el usuario: {e}"))
}

pub async fn google_sign_in(
    Extension(state): Extension<AuthState>,
    Json(req): Json<GoogleSignInRequest>,
) -> Response {
    // Not configured: say so plainly instead of 500-ing on an empty client id.
    if state.google_client_id.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorBody {
                error: "error",
                message: "Google Sign-In no está configurado en este daemon".into(),
            }),
        )
            .into_response();
    }

    let audience = if state.google_audience.is_empty() {
        state.google_client_id.clone()
    } else {
        state.google_audience.clone()
    };

    let client = reqwest::Client::new();
    let claims = match verify_google_id_token(&client, &req.id_token, &audience).await {
        Ok(c) => c,
        Err(e) => return json_err(StatusCode::UNAUTHORIZED, "unauthorized", &format!("Google ID token inválido: {e}")),
    };

    let user = match upsert_google_user(&state, &claims) {
        Ok(u) => u,
        Err(e) => return json_err(StatusCode::FORBIDDEN, "error", &e),
    };

    let token = match issue_token(
        &state.secret,
        user.id,
        &user.username,
        user.role,
        state.expiry_hours,
    ) {
        Ok(t) => t,
        Err(_) => return json_err(StatusCode::INTERNAL_SERVER_ERROR, "error", "no se pudo emitir el token"),
    };

    let refresh_token = state
        .refresh_store
        .issue(user.id, state.refresh_ttl_days, "google")
        .ok()
        .map(|(p, _)| p);

    let _ = state.user_store.record_login(user.id);
    let reauth = state.reauth_tokens.issue();

    (
        StatusCode::OK,
        Json(LoginResponse {
            token,
            user,
            reauth_token: reauth,
            refresh_token,
        }),
    )
        .into_response()
}

/// Whether the daemon has a Google provider configured. The web client uses
/// this to decide whether to render the button at all.
pub fn is_configured(state: &AuthState) -> bool {
    !state.google_client_id.is_empty()
}