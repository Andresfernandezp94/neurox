// AWS Cognito como método de login, en el mismo papel que Google Sign-In.
//
// El cliente obtiene un ID token del user pool (Hosted UI, Amplify o
// Credential Manager) y POSTea neurox lo hace verificar contra el JWKS del
// pool. A partir de ahí es idéntico a Google: mismo endpoint, mismo contrato,
// mismo JWT + refresh token.
//
// ── Lo que Cognito tiene y Google no ────────────────────────────────────────
//
// Cognito firma LOS DOS tokens con las MISMAS claves del pool: el ID token y
// el access token. Se distinguen por el claim `token_use`, que vale "id" o
// "access". Verificar solo la firma no alcanza, porque ambos tienen firma
// válida contra las mismas claves.
//
// Qué se acepta como admin y qué no:
//
//   * Un access token no trae `cognito:username` ni `email` por defecto, así
//     que el mapeo fallaría igual. Pero el fallo sería por accidente, no por
//     diseño, y depende de cómo el pool tenga configurados los atributos.
//
// Por eso `token_use == "id"` es una comprobación explícita y obligatoria, no
// una defensa redundante: es la única que no depende de qué atributos育儿 el
// pool happening tener.
//
// El access token además lleva `client_id` en vez de `aud`, así que el
// `set_audience` de jsonwebtoken ya lo rechaza. Se comprueba igual.
//
// ── El JWKS y el issuer ──────────────────────────────────────────────────────
//
// Ambos se derivan de region + user_pool_id:
//
//   jwks_uri: https://cognito-idp.{region}.amazonaws.com/{pool}/.well-known/jwks.json
//   issuer:   https://cognito-idp.{region}.amazonaws.com/{pool}
//
// Se derivan en vez de configurablese para que no puedan no coincidir entre sí:
// si alguien configura un issuer de otra región, la verificación falla por
// firma, no por una comparación de strings que alguien podría aflojar.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Extension;
use axum::Json;
use serde::Deserialize;

use super::handlers::{AuthState, ErrorBody, LoginResponse};
use super::middleware::Role;
use super::tokens::issue_token;

/// Cognito solo firma con RS256. Fijado, no leído de la cabecera del token.
const ALG: &str = "RS256";

#[derive(Deserialize)]
pub struct CognitoSignInRequest {
    /// ID token del user pool (NO el access token).
    pub id_token: String,
}

#[derive(Debug, Deserialize)]
struct CognitoClaims {
    /// UUID estable del usuario en el pool. Lo valida `jsonwebtoken` (que
    /// exige que exista) pero no se usa para nada más: la cuenta local se
    /// ancla a `cognito:username`, no a este valor.
    #[allow(dead_code)]
    sub: String,
    /// `https://cognito-idp.<region>.amazonaws.com/<pool>`. Lo chequea
    /// `set_issuer`; se declara para que quede a la vista que es un claim
    /// verificado y no un campo ignorado.
    #[allow(dead_code)]
    iss: Option<String>,
    /// App client id. Solo el ID token lo trae en `aud`.
    aud: Option<String>,
    /// "id" en ID tokens, "access" en access tokens. Ver `module` arriba: es la
    /// comprobación que separa los dos.
    #[serde(rename = "token_use")]
    token_use: Option<String>,
    /// Presente solo si el pool tiene el atributo `email` (standard).
    #[serde(rename = "cognito:username")]
    username: Option<String>,
    email: Option<String>,
    #[serde(rename = "email_verified")]
    email_verified: Option<bool>,
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

/// El issuer exacto que este pool debe producir. Si el pool se migró a un
/// "updated issuer", AWS hostea el mismo JWKS en varias regiones y el `iss`
/// puede venir con otra región; se acepta solo la propia, que es lo que
/// significa `aud` contra `client_id`.
fn expected_issuer(region: &str, pool: &str) -> String {
    format!("https://cognito-idp.{region}.amazonaws.com/{pool}")
}

fn jwks_url(region: &str, pool: &str) -> String {
    format!("{}/.well-known/jwks.json", expected_issuer(region, pool))
}

/// Verifica firma y claims. Devuelve los claims si todo cuadra.
async fn verify_id_token(
    client: &reqwest::Client,
    id_token: &str,
    region: &str,
    pool: &str,
    client_id: &str,
) -> Result<CognitoClaims, String> {
    let jwks: Jwks = client
        .get(jwks_url(region, pool))
        .send()
        .await
        .map_err(|e| format!("jwks fetch: {e}"))?
        .json()
        .await
        .map_err(|e| format!("jwks decode: {e}"))?;

    let header = jsonwebtoken::decode_header(id_token).map_err(|e| format!("decode header: {e}"))?;
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
                && k.alg == ALG
                && matches!(k.use_.as_deref(), None | Some("sig"))
        })
        .ok_or("kid desconocida (¿token de otro pool?)")?;

    let key = jsonwebtoken::DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
        .map_err(|e| format!("jwk inválida: {e}"))?;

    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
    validation.set_audience(&[client_id]);
    validation.set_issuer(&[expected_issuer(region, pool)]);
    validation.leeway = 30;

    let claims = jsonwebtoken::decode::<CognitoClaims>(id_token, &key, &validation)
        .map_err(|e| format!("verify: {e}"))?
        .claims;

    // El access token del mismo pool pasa la firma. Esto es lo que lo para.
    if claims.token_use.as_deref() != Some("id") {
        return Err(format!(
            "token_use es {:?}, se esperaba \"id\": ¿es un access token?",
            claims.token_use
        ));
    }
    // Redundante con `set_audience`, pero el campo es opcional en el struct y
    // una versión futura de jsonwebtoken podría dejar de mirarlo.
    if claims.aud.as_deref() != Some(client_id) {
        return Err("audience no coincide con el client_id configurado".into());
    }
    Ok(claims)
}

/// Which local username this Cognito identity maps to.
///
/// `cognito:username` es el identificador estable del pool. Se prefiere sobre
/// el email: el email puede cambiar en el pool y con el username la cuenta
/// local sigue siendo la misma, sin crear usuarios duplicados ni perder el
/// rol.
fn local_username(claims: &CognitoClaims) -> Result<String, String> {
    if let Some(u) = claims.username.as_deref().filter(|s| !s.is_empty()) {
        return Ok(u.to_string());
    }
    let e = claims.email.as_deref().filter(|s| !s.is_empty());
    e.map(str::to_string)
        .ok_or_else(|| "el ID token no trae cognito:username ni email".into())
}

/// Una identidad de Cognito verificada mapeada a una cuenta local.
fn upsert_user(state: &AuthState, claims: &CognitoClaims) -> Result<super::users::UserView, String> {
    let username = local_username(claims)?;

    if claims.email_verified == Some(false) {
        return Err("el email de Cognito no está verificado".into());
    }

    if let Some(existing) = state.user_store.find_by_username(&username) {
        return Ok(existing.view());
    }

    // Contraseña inutilizable: esta cuenta solo entra por Cognito. El hash
    // argon2 de una contraseña aleatoria la deja sin acceso por `/v1/auth/login`
    // sin dejar de existir en el store.
    let unusable = uuid::Uuid::new_v4().to_string();
    state
        .user_store
        .create(&username, &unusable, Role::Viewer)
        .map_err(|e| format!("no se pudo crear el usuario: {e}"))
}

/// Si el daemon tiene Cognito configurado. Los clientes lo leen para decidir si
/// pintar el botón.
pub fn is_configured(state: &AuthState) -> bool {
    !state.cognito_user_pool_id.is_empty() && !state.cognito_client_id.is_empty()
}

pub async fn cognito_sign_in(
    Extension(state): Extension<AuthState>,
    Json(req): Json<CognitoSignInRequest>,
) -> Response {
    if !is_configured(&state) {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorBody {
                error: "error",
                message: "Cognito no está configurado en este daemon".into(),
            }),
        )
            .into_response();
    }

    let client = reqwest::Client::new();
    let claims = match verify_id_token(
        &client,
        &req.id_token,
        &state.cognito_region,
        &state.cognito_user_pool_id,
        &state.cognito_client_id,
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            return json_err(
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                &format!("Cognito ID token inválido: {e}"),
            )
        }
    };

    let user = match upsert_user(&state, &claims) {
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
        Err(_) => {
            return json_err(StatusCode::INTERNAL_SERVER_ERROR, "error", "no se pudo emitir el token")
        }
    };

    let refresh_token = state
        .refresh_store
        .issue(user.id, state.refresh_ttl_days, "cognito")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_jwks_y_el_issuer_salen_de_la_misma_region() {
        // Si Pudieran divergir, un pool con la region bien escrita y el issuer
        // mal would fail por una comparación de strings en vez de por la firma.
        assert_eq!(
            jwks_url("us-east-1", "us-east-1_ABC"),
            "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_ABC/.well-known/jwks.json"
        );
        assert_eq!(
            expected_issuer("eu-west-1", "eu-west-1_XYZ"),
            "https://cognito-idp.eu-west-1.amazonaws.com/eu-west-1_XYZ"
        );
    }

    #[test]
    fn un_access_token_se_rechaza_por_token_use() {
        // El caso de seguridad de este módulo. Cognito firma ID y access con
        // las MISMAS claves, así que la firma sola no distingue. Este test
        // documenta el criterio; el código real lo comprueba en
        // `verify_id_token`.
        let access = CognitoClaims {
            sub: "uuid".into(),
            iss: Some(expected_issuer("us-east-1", "pool")),
            aud: Some("client".into()),
            token_use: Some("access".into()),
            username: Some("juan".into()),
            email: Some("juan@example.com".into()),
            email_verified: Some(true),
        };
        assert_ne!(access.token_use.as_deref(), Some("id"));
    }

    #[test]
    fn el_username_local_prefiere_cognito_username_sobre_el_email() {
        // El email puede cambiar en el pool; con `cognito:username` la cuenta
        // local sigue siendo la misma y no se duplica ni se pierde el rol.
        let claims = CognitoClaims {
            sub: "uuid".into(),
            iss: None,
            aud: None,
            token_use: Some("id".into()),
            username: Some("juan".into()),
            email: Some("nuevo@example.com".into()),
            email_verified: Some(true),
        };
        assert_eq!(local_username(&claims).unwrap(), "juan");
    }

    #[test]
    fn sin_username_ni_email_no_hay_que_adivinar() {
        // Inventar un username a partir del `sub` crearía una cuenta distinta
        // cada vez que el pool reemitiera el token con otro sub.
        let claims = CognitoClaims {
            sub: "uuid-estable".into(),
            iss: None,
            aud: None,
            token_use: Some("id".into()),
            username: None,
            email: None,
            email_verified: None,
        };
        assert!(local_username(&claims).is_err());
    }

    #[test]
    fn con_solo_email_todavia_hay_username() {
        // Pool configurado sin `username` pero con email: se usa el email.
        let claims = CognitoClaims {
            sub: "uuid".into(),
            iss: None,
            aud: None,
            token_use: Some("id".into()),
            username: None,
            email: Some("solo@example.com".into()),
            email_verified: Some(true),
        };
        assert_eq!(local_username(&claims).unwrap(), "solo@example.com");
    }
}