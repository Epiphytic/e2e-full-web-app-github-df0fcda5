use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use rsa::pkcs8::DecodePublicKey;
use rsa::traits::PublicKeyParts;
use rsa::RsaPublicKey;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

pub fn validate_token(
    token: &str,
    public_key_pem: &[u8],
) -> Result<Claims, jsonwebtoken::errors::Error> {
    let decoding_key = DecodingKey::from_rsa_pem(public_key_pem)?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = true;
    let token_data = decode::<Claims>(token, &decoding_key, &validation)?;
    Ok(token_data.claims)
}

pub async fn jwks_endpoint(State(public_key_pem): State<Vec<u8>>) -> impl IntoResponse {
    let pem_str = std::str::from_utf8(&public_key_pem).unwrap();
    let public_key = RsaPublicKey::from_public_key_pem(pem_str).unwrap();

    let n = URL_SAFE_NO_PAD.encode(public_key.n().to_bytes_be());
    let e = URL_SAFE_NO_PAD.encode(public_key.e().to_bytes_be());

    Json(serde_json::json!({
        "keys": [{
            "kty": "RSA",
            "alg": "RS256",
            "use": "sig",
            "kid": "sqlite-editor-key-1",
            "n": n,
            "e": e,
        }]
    }))
}

pub async fn auth_middleware(
    State(public_key_pem): State<Vec<u8>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let token = token.or_else(|| {
        req.headers()
            .get("cookie")
            .and_then(|v| v.to_str().ok())
            .and_then(|cookies| cookies.split(';').find_map(|c| c.trim().strip_prefix("token=")))
    });

    match token {
        Some(token) => match validate_token(token, &public_key_pem) {
            Ok(claims) => {
                req.extensions_mut().insert(claims);
                Ok(next.run(req).await)
            }
            Err(_) => Err(StatusCode::UNAUTHORIZED),
        },
        None => Err(StatusCode::UNAUTHORIZED),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn now_secs() -> usize {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize
    }

    #[test]
    fn test_decode_valid_token() {
        let private_key =
            std::fs::read("certs/private.pem").expect("Run certs/generate-keys.sh first");
        let public_key = std::fs::read("certs/public.pem").unwrap();

        let claims = Claims {
            sub: "testuser".to_string(),
            exp: now_secs() + 3600,
        };

        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256),
            &claims,
            &jsonwebtoken::EncodingKey::from_rsa_pem(&private_key).unwrap(),
        )
        .unwrap();

        let decoded = validate_token(&token, &public_key).unwrap();
        assert_eq!(decoded.sub, "testuser");
    }

    #[test]
    fn test_reject_expired_token() {
        let private_key = std::fs::read("certs/private.pem").unwrap();
        let public_key = std::fs::read("certs/public.pem").unwrap();

        let claims = Claims {
            sub: "testuser".to_string(),
            exp: 1000, // expired long ago
        };

        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256),
            &claims,
            &jsonwebtoken::EncodingKey::from_rsa_pem(&private_key).unwrap(),
        )
        .unwrap();

        assert!(validate_token(&token, &public_key).is_err());
    }
}
