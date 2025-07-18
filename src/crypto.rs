use crate::error::AppError;
use base64::Engine;
use bitcoin::hashes::{sha256, Hash};
use secp256k1::{ecdsa::Signature, Message, PublicKey, Secp256k1};
use sha2::{Digest, Sha256};
use std::str::FromStr;
use tracing::{debug, error, info};

/// Verifies a signature against a message and public key
pub fn verify_signature(
    message: &str,
    signature_str: &str,
    public_key_str: &str,
) -> Result<bool, AppError> {
    let secp = Secp256k1::new();

    // Parse the public key
    let public_key = PublicKey::from_str(public_key_str).map_err(|e| {
        error!("Failed to parse public key: {}", e);
        AppError::InvalidInput(format!("Invalid public key format: {}", e))
    })?;

    // Parse the signature
    let signature =
        if signature_str.len() == 128 && signature_str.chars().all(|c| c.is_ascii_hexdigit()) {
            // Hex encoded signature
            let sig_bytes = hex::decode(signature_str).map_err(|e| {
                error!("Failed to decode hex signature: {}", e);
                AppError::InvalidInput(format!("Invalid hex signature: {}", e))
            })?;
            Signature::from_compact(&sig_bytes).map_err(|e| {
                error!("Failed to parse signature from bytes: {}", e);
                AppError::InvalidInput(format!("Invalid signature format: {}", e))
            })?
        } else {
            // Try base64 encoded signature
            let sig_bytes = base64::engine::general_purpose::STANDARD
                .decode(signature_str)
                .map_err(|e| {
                    error!("Failed to decode base64 signature: {}", e);
                    AppError::InvalidInput(format!("Invalid base64 signature: {}", e))
                })?;
            Signature::from_compact(&sig_bytes).map_err(|e| {
                error!("Failed to parse signature from bytes: {}", e);
                AppError::InvalidInput(format!("Invalid signature format: {}", e))
            })?
        };

    // Hash the message
    let mut hasher = Sha256::new();
    hasher.update(message.as_bytes());
    let hash = hasher.finalize();

    // Create a secp256k1 message from the hash
    let msg = Message::from_digest_slice(&hash).map_err(|e| {
        error!("Failed to create message from hash: {}", e);
        AppError::InvalidInput(format!("Failed to create message: {}", e))
    })?;

    // Verify the signature
    match secp.verify_ecdsa(&msg, &signature, &public_key) {
        Ok(()) => {
            info!("Signature verification successful");
            Ok(true)
        }
        Err(e) => {
            debug!("Signature verification failed: {}", e);
            Ok(false)
        }
    }
}

/// Verifies a Schnorr signature (for Taproot compatibility)
pub fn verify_schnorr_signature(
    message: &str,
    signature_str: &str,
    public_key_str: &str,
) -> Result<bool, AppError> {
    let secp = Secp256k1::new();

    // Parse the x-only public key (32 bytes)
    let xonly_pubkey = secp256k1::XOnlyPublicKey::from_str(public_key_str).map_err(|e| {
        error!("Failed to parse x-only public key: {}", e);
        AppError::InvalidInput(format!("Invalid x-only public key format: {}", e))
    })?;

    // Parse the Schnorr signature (64 bytes)
    let signature =
        if signature_str.len() == 128 && signature_str.chars().all(|c| c.is_ascii_hexdigit()) {
            let sig_bytes = hex::decode(signature_str).map_err(|e| {
                error!("Failed to decode hex Schnorr signature: {}", e);
                AppError::InvalidInput(format!("Invalid hex signature: {}", e))
            })?;
            secp256k1::schnorr::Signature::from_slice(&sig_bytes).map_err(|e| {
                error!("Failed to parse Schnorr signature: {}", e);
                AppError::InvalidInput(format!("Invalid Schnorr signature format: {}", e))
            })?
        } else {
            let sig_bytes = base64::engine::general_purpose::STANDARD
                .decode(signature_str)
                .map_err(|e| {
                    error!("Failed to decode base64 Schnorr signature: {}", e);
                    AppError::InvalidInput(format!("Invalid base64 signature: {}", e))
                })?;
            secp256k1::schnorr::Signature::from_slice(&sig_bytes).map_err(|e| {
                error!("Failed to parse Schnorr signature: {}", e);
                AppError::InvalidInput(format!("Invalid Schnorr signature format: {}", e))
            })?
        };

    // Hash the message with SHA256
    let hash = sha256::Hash::hash(message.as_bytes());
    let msg = Message::from_digest(hash.to_byte_array());

    // Verify the Schnorr signature
    match secp.verify_schnorr(&signature, &msg, &xonly_pubkey) {
        Ok(()) => {
            info!("Schnorr signature verification successful");
            Ok(true)
        }
        Err(e) => {
            debug!("Schnorr signature verification failed: {}", e);
            Ok(false)
        }
    }
}

/// Derives a public key from a receiver ID (if receiver ID is a public key)
pub fn derive_public_key_from_receiver_id(receiver_id: &str) -> Result<Option<String>, AppError> {
    // Check if receiver_id is already a public key (33 or 65 bytes hex encoded)
    if (receiver_id.len() == 66 || receiver_id.len() == 130)
        && receiver_id.chars().all(|c| c.is_ascii_hexdigit())
    {
        // Validate it's a valid public key
        match PublicKey::from_str(receiver_id) {
            Ok(_) => return Ok(Some(receiver_id.to_string())),
            Err(_) => {}
        }
    }

    // Check if it's an x-only public key (32 bytes hex encoded) for Taproot
    if receiver_id.len() == 64 && receiver_id.chars().all(|c| c.is_ascii_hexdigit()) {
        match secp256k1::XOnlyPublicKey::from_str(receiver_id) {
            Ok(_) => return Ok(Some(receiver_id.to_string())),
            Err(_) => {}
        }
    }

    // If receiver_id is not a direct public key, it might be an identifier
    // that needs to be looked up in the database
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_signature_invalid_pubkey() {
        let result = verify_signature("test message", "abcdef1234567890", "invalid_pubkey");
        assert!(result.is_err());
    }

    #[test]
    fn test_derive_public_key_from_valid_hex() {
        // Valid compressed public key (33 bytes = 66 hex chars)
        let compressed_pubkey = format!("02{}", "a".repeat(64));
        let result = derive_public_key_from_receiver_id(&compressed_pubkey).unwrap();
        assert_eq!(result, Some(compressed_pubkey));

        // Valid x-only public key (32 bytes = 64 hex chars)
        let xonly_pubkey = "a".repeat(64);
        let result = derive_public_key_from_receiver_id(&xonly_pubkey).unwrap();
        assert_eq!(result, Some(xonly_pubkey));
    }

    #[test]
    fn test_derive_public_key_from_non_hex() {
        let result = derive_public_key_from_receiver_id("user_123_abc").unwrap();
        assert_eq!(result, None);
    }
}
