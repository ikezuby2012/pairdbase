use crate::features::auth::auth_error::AuthError;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // user_id
    pub org_id: String,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
}

pub struct TokenService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    access_ttl: Duration,
    refresh_ttl: Duration,
}

impl TokenService {
    pub fn new(secret: &str, access_ttl_secs: u64, refresh_ttl_secs: u64) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            access_ttl: Duration::seconds(access_ttl_secs as i64),
            refresh_ttl: Duration::seconds(refresh_ttl_secs as i64),
        }
    }

    pub fn issue_access_token(
        &self,
        user_id: Uuid,
        org_id: Uuid,
        role: &str,
    ) -> Result<String, AuthError> {
        let now = Utc::now();
        let claims = Claims {
            sub: user_id.to_string(),
            org_id: org_id.to_string(),
            role: role.to_string(),
            iat: now.timestamp(),
            exp: (now + self.access_ttl).timestamp(),
        };
        encode(&Header::new(Algorithm::HS256), &claims, &self.encoding_key)
            .map_err(|e| AuthError::Internal(e.to_string()))
    }

    pub fn issue_refresh_token(&self) -> (String, String, chrono::DateTime<Utc>) {
        use rand::RngExt;

        let mut bytes = [0u8; 64];
        rand::rng().fill(&mut bytes);

        let token = hex::encode(bytes);
        let hash = Self::hash_token(&token);
        let exp = Utc::now() + self.refresh_ttl;

        (token, hash, exp)
    }

    pub fn verify_access_token(&self, token: &str) -> Result<Claims, AuthError> {
        decode::<Claims>(
            token,
            &self.decoding_key,
            &Validation::new(Algorithm::HS256),
        )
        .map(|d| d.claims)
        .map_err(|_| AuthError::InvalidToken)
    }

    pub fn hash_token(token: &str) -> String {
        hex::encode(Sha256::digest(token.as_bytes()))
    }

    pub fn access_ttl_secs(&self) -> u64 {
        self.access_ttl.num_seconds() as u64
    }
}
