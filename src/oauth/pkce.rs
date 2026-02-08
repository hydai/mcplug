use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};

pub struct PkceChallenge {
    pub code_verifier: String,
    pub code_challenge: String,
}

pub fn generate_pkce() -> PkceChallenge {
    let mut buf = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rng(), &mut buf);
    let code_verifier = URL_SAFE_NO_PAD.encode(buf);

    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();
    let code_challenge = URL_SAFE_NO_PAD.encode(hash);

    PkceChallenge {
        code_verifier,
        code_challenge,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_verifier_length() {
        let pkce = generate_pkce();
        // 32 bytes base64url-encoded without padding: ceil(32*4/3) = 43 chars
        assert_eq!(pkce.code_verifier.len(), 43);
    }

    #[test]
    fn pkce_challenge_is_sha256_of_verifier() {
        let pkce = generate_pkce();

        let mut hasher = Sha256::new();
        hasher.update(pkce.code_verifier.as_bytes());
        let expected = URL_SAFE_NO_PAD.encode(hasher.finalize());

        assert_eq!(pkce.code_challenge, expected);
    }

    #[test]
    fn pkce_generates_unique_values() {
        let a = generate_pkce();
        let b = generate_pkce();
        assert_ne!(a.code_verifier, b.code_verifier);
        assert_ne!(a.code_challenge, b.code_challenge);
    }

    #[test]
    fn pkce_verifier_uses_url_safe_chars() {
        let pkce = generate_pkce();
        // base64url charset: A-Z, a-z, 0-9, -, _ (no +, /, or =)
        for ch in pkce.code_verifier.chars() {
            assert!(
                ch.is_ascii_alphanumeric() || ch == '-' || ch == '_',
                "Invalid char in verifier: '{ch}'"
            );
        }
        for ch in pkce.code_challenge.chars() {
            assert!(
                ch.is_ascii_alphanumeric() || ch == '-' || ch == '_',
                "Invalid char in challenge: '{ch}'"
            );
        }
    }
}
