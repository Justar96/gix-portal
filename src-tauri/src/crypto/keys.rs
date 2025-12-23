use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum KeyError {
    #[error("Failed to generate keypair")]
    GenerationFailed,
    #[error("Invalid key bytes")]
    InvalidKeyBytes,
    #[error("Signature verification failed")]
    VerificationFailed,
}

/// Wrapper around Ed25519 signing key for node identity
pub struct Identity {
    signing_key: SigningKey,
}

impl Identity {
    /// Generate a new random identity
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self { signing_key }
    }

    /// Restore identity from secret key bytes
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, KeyError> {
        let signing_key = SigningKey::from_bytes(bytes);
        Ok(Self { signing_key })
    }

    /// Get the secret key bytes for storage
    pub fn to_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// Get the public NodeId
    pub fn node_id(&self) -> NodeId {
        NodeId(self.signing_key.verifying_key().to_bytes())
    }

    /// Sign a message
    #[allow(dead_code)]
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }

    /// Get the verifying key for signature verification
    #[allow(dead_code)]
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }
}

/// Public node identifier (32 bytes Ed25519 public key)
#[derive(Clone, Copy, Hash, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub struct NodeId(pub [u8; 32]);

impl NodeId {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Display as truncated hex for UI (first 8 + last 8 chars)
    pub fn short_string(&self) -> String {
        let hex_str = self.to_hex();
        format!("{}...{}", &hex_str[..8], &hex_str[56..])
    }

    /// Full hex representation
    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    /// Parse from hex string
    #[allow(dead_code)]
    pub fn from_hex(s: &str) -> Result<Self, KeyError> {
        let bytes = hex::decode(s).map_err(|_| KeyError::InvalidKeyBytes)?;
        if bytes.len() != 32 {
            return Err(KeyError::InvalidKeyBytes);
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_generation() {
        let identity = Identity::generate();
        let node_id = identity.node_id();
        assert_eq!(node_id.as_bytes().len(), 32);
    }

    #[test]
    fn test_identity_serialization() {
        let identity = Identity::generate();
        let bytes = identity.to_bytes();
        let restored = Identity::from_bytes(&bytes).unwrap();
        assert_eq!(identity.node_id().as_bytes(), restored.node_id().as_bytes());
    }

    #[test]
    fn test_node_id_hex() {
        let identity = Identity::generate();
        let node_id = identity.node_id();
        let hex_str = node_id.to_hex();
        let restored = NodeId::from_hex(&hex_str).unwrap();
        assert_eq!(node_id.as_bytes(), restored.as_bytes());
    }
}
