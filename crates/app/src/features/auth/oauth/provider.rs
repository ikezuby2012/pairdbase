use crate::features::auth::domains::{OAuthProfile, OAuthProvider};
use crate::features::auth::auth_error::AuthError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct PkceChallenge {
    pub code_verifier: String,  // stored server-side
    pub code_challenge: String, // sent to provider
}

#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String, // e.g. https://app.pairdbase.io/api/v1/auth/callback/google
}

#[async_trait]
pub trait OAuthProviderClient: Send + Sync {
    fn provider(&self) -> OAuthProvider;

    /// Build the URL to redirect the user to (with PKCE)
    fn authorization_url(&self, state: &str, challenge: &PkceChallenge) -> String;

    /// Exchange the code for a profile (handles token exchange + userinfo fetch)
    async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<OAuthProfile, AuthError>;
}

pub fn generate_pkce() -> PkceChallenge {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use rand::RngExt;
    use sha2::{Digest, Sha256};

    let mut verifier_bytes = [0u8; 64];
    rand::rng().fill(&mut verifier_bytes);

    let code_verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);

    let hash = Sha256::digest(code_verifier.as_bytes());
    let code_challenge = URL_SAFE_NO_PAD.encode(hash);

    PkceChallenge {
        code_verifier,
        code_challenge,
    }
}

/// Generate a cryptographically random state string
pub fn generate_state() -> String {
    use rand::RngExt;

    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes);

    hex::encode(bytes)
}
