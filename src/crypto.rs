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
        AppError::InvalidInput(format!("Invalid public key format: {e}"))
    })?;

    // Parse the signature
    let signature =
        if signature_str.len() == 128 && signature_str.chars().all(|c| c.is_ascii_hexdigit()) {
            // Hex encoded signature
            let sig_bytes = hex::decode(signature_str).map_err(|e| {
                error!("Failed to decode hex signature: {}", e);
                AppError::InvalidInput(format!("Invalid hex signature: {e}"))
            })?;
            Signature::from_compact(&sig_bytes).map_err(|e| {
                error!("Failed to parse signature from bytes: {}", e);
                AppError::InvalidInput(format!("Invalid signature format: {e}"))
            })?
        } else {
            // Try base64 encoded signature
            let sig_bytes = base64::engine::general_purpose::STANDARD
                .decode(signature_str)
                .map_err(|e| {
                    error!("Failed to decode base64 signature: {}", e);
                    AppError::InvalidInput(format!("Invalid base64 signature: {e}"))
                })?;
            Signature::from_compact(&sig_bytes).map_err(|e| {
                error!("Failed to parse signature from bytes: {}", e);
                AppError::InvalidInput(format!("Invalid signature format: {e}"))
            })?
        };

    // Hash the message
    let mut hasher = Sha256::new();
    hasher.update(message.as_bytes());
    let hash = hasher.finalize();

    // Create a secp256k1 message from the hash
    let msg = Message::from_digest_slice(&hash).map_err(|e| {
        error!("Failed to create message from hash: {}", e);
        AppError::InvalidInput(format!("Failed to create message: {e}"))
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
        AppError::InvalidInput(format!("Invalid x-only public key format: {e}"))
    })?;

    // Parse the Schnorr signature (64 bytes)
    let signature =
        if signature_str.len() == 128 && signature_str.chars().all(|c| c.is_ascii_hexdigit()) {
            let sig_bytes = hex::decode(signature_str).map_err(|e| {
                error!("Failed to decode hex Schnorr signature: {}", e);
                AppError::InvalidInput(format!("Invalid hex signature: {e}"))
            })?;
            secp256k1::schnorr::Signature::from_slice(&sig_bytes).map_err(|e| {
                error!("Failed to parse Schnorr signature: {}", e);
                AppError::InvalidInput(format!("Invalid Schnorr signature format: {e}"))
            })?
        } else {
            let sig_bytes = base64::engine::general_purpose::STANDARD
                .decode(signature_str)
                .map_err(|e| {
                    error!("Failed to decode base64 Schnorr signature: {}", e);
                    AppError::InvalidInput(format!("Invalid base64 signature: {e}"))
                })?;
            secp256k1::schnorr::Signature::from_slice(&sig_bytes).map_err(|e| {
                error!("Failed to parse Schnorr signature: {}", e);
                AppError::InvalidInput(format!("Invalid Schnorr signature format: {e}"))
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
        if PublicKey::from_str(receiver_id).is_ok() {
            return Ok(Some(receiver_id.to_string()));
        }
    }

    // Check if it's an x-only public key (32 bytes hex encoded) for Taproot
    if receiver_id.len() == 64
        && receiver_id.chars().all(|c| c.is_ascii_hexdigit())
        && secp256k1::XOnlyPublicKey::from_str(receiver_id).is_ok()
    {
        return Ok(Some(receiver_id.to_string()));
    }

    // If receiver_id is not a direct public key, it might be an identifier
    // that needs to be looked up in the database
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1::{Secp256k1, SecretKey};

    // Helper function to create test keypairs
    fn create_test_keypair(seed: u8) -> (SecretKey, PublicKey) {
        let secp = Secp256k1::new();
        let mut key_bytes = [seed; 32];
        key_bytes[31] = seed; // Ensure it's not zero
        let secret_key = SecretKey::from_slice(&key_bytes).unwrap();
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        (secret_key, public_key)
    }

    fn create_test_schnorr_keypair(seed: u8) -> (secp256k1::Keypair, secp256k1::XOnlyPublicKey) {
        let secp = Secp256k1::new();
        let mut key_bytes = [seed; 32];
        key_bytes[31] = seed.wrapping_add(1); // Ensure it's not zero
        let keypair = secp256k1::Keypair::from_seckey_slice(&secp, &key_bytes).unwrap();
        let (xonly_pubkey, _parity) = keypair.x_only_public_key();
        (keypair, xonly_pubkey)
    }

    #[test]
    fn test_verify_signature_invalid_pubkey() {
        let result = verify_signature("test message", "abcdef1234567890", "invalid_pubkey");
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_ecdsa_signature_valid() {
        let secp = Secp256k1::new();
        let (secret_key, public_key) = create_test_keypair(0x01);

        let message = "Hello, Taproot Assets!";

        // Create hash of the message
        let mut hasher = Sha256::new();
        hasher.update(message.as_bytes());
        let hash = hasher.finalize();

        // Create signature
        let msg = Message::from_digest_slice(&hash).unwrap();
        let signature = secp.sign_ecdsa(&msg, &secret_key);

        // Convert to hex strings for the function
        let sig_hex = hex::encode(signature.serialize_compact());
        let pubkey_hex = public_key.to_string();

        // Verify the signature
        let result = verify_signature(message, &sig_hex, &pubkey_hex).unwrap();
        assert!(result, "Valid signature should verify successfully");
    }

    #[test]
    fn test_verify_ecdsa_signature_base64() {
        let secp = Secp256k1::new();
        let (secret_key, public_key) = create_test_keypair(0x02);

        let message = "Test message for base64";

        // Create signature
        let mut hasher = Sha256::new();
        hasher.update(message.as_bytes());
        let hash = hasher.finalize();
        let msg = Message::from_digest_slice(&hash).unwrap();
        let signature = secp.sign_ecdsa(&msg, &secret_key);

        // Convert to base64
        let sig_base64 =
            base64::engine::general_purpose::STANDARD.encode(signature.serialize_compact());
        let pubkey_hex = public_key.to_string();

        // Verify the signature
        let result = verify_signature(message, &sig_base64, &pubkey_hex).unwrap();
        assert!(result, "Valid base64 signature should verify successfully");
    }

    #[test]
    fn test_verify_ecdsa_signature_wrong_message() {
        let secp = Secp256k1::new();
        let (secret_key, public_key) = create_test_keypair(0x03);

        let original_message = "Original message";
        let tampered_message = "Tampered message";

        // Sign the original message
        let mut hasher = Sha256::new();
        hasher.update(original_message.as_bytes());
        let hash = hasher.finalize();
        let msg = Message::from_digest_slice(&hash).unwrap();
        let signature = secp.sign_ecdsa(&msg, &secret_key);

        let sig_hex = hex::encode(signature.serialize_compact());
        let pubkey_hex = public_key.to_string();

        // Try to verify with different message
        let result = verify_signature(tampered_message, &sig_hex, &pubkey_hex).unwrap();
        assert!(!result, "Signature should fail for wrong message");
    }

    #[test]
    fn test_verify_ecdsa_signature_wrong_pubkey() {
        let secp = Secp256k1::new();
        let (secret_key1, _public_key1) = create_test_keypair(0x04);
        let (_secret_key2, public_key2) = create_test_keypair(0x05);

        let message = "Test message";

        // Sign with first key
        let mut hasher = Sha256::new();
        hasher.update(message.as_bytes());
        let hash = hasher.finalize();
        let msg = Message::from_digest_slice(&hash).unwrap();
        let signature = secp.sign_ecdsa(&msg, &secret_key1);

        let sig_hex = hex::encode(signature.serialize_compact());
        let wrong_pubkey_hex = public_key2.to_string();

        // Try to verify with wrong public key
        let result = verify_signature(message, &sig_hex, &wrong_pubkey_hex).unwrap();
        assert!(!result, "Signature should fail for wrong public key");
    }

    #[test]
    fn test_verify_schnorr_signature_valid() {
        let secp = Secp256k1::signing_only();
        let (keypair, xonly_pubkey) = create_test_schnorr_keypair(0x06);

        let message = "Schnorr signature test";

        // Create signature
        let hash = sha256::Hash::hash(message.as_bytes());
        let msg = Message::from_digest(hash.to_byte_array());
        let signature = secp.sign_schnorr_no_aux_rand(&msg, &keypair);

        // Convert to hex strings
        let sig_hex = hex::encode(signature.as_ref());
        let pubkey_hex = xonly_pubkey.to_string();

        // Verify the signature
        let result = verify_schnorr_signature(message, &sig_hex, &pubkey_hex).unwrap();
        assert!(result, "Valid Schnorr signature should verify successfully");
    }

    #[test]
    fn test_verify_schnorr_signature_base64() {
        let secp = Secp256k1::signing_only();
        let (keypair, xonly_pubkey) = create_test_schnorr_keypair(0x07);

        let message = "Schnorr base64 test";

        // Create signature
        let hash = sha256::Hash::hash(message.as_bytes());
        let msg = Message::from_digest(hash.to_byte_array());
        let signature = secp.sign_schnorr_no_aux_rand(&msg, &keypair);

        // Convert to base64
        let sig_base64 = base64::engine::general_purpose::STANDARD.encode(signature.as_ref());
        let pubkey_hex = xonly_pubkey.to_string();

        // Verify the signature
        let result = verify_schnorr_signature(message, &sig_base64, &pubkey_hex).unwrap();
        assert!(
            result,
            "Valid Schnorr base64 signature should verify successfully"
        );
    }

    #[test]
    fn test_verify_schnorr_signature_wrong_message() {
        let secp = Secp256k1::signing_only();
        let (keypair, xonly_pubkey) = create_test_schnorr_keypair(0x08);

        let original_message = "Original Schnorr message";
        let tampered_message = "Tampered Schnorr message";

        // Sign the original
        let hash = sha256::Hash::hash(original_message.as_bytes());
        let msg = Message::from_digest(hash.to_byte_array());
        let signature = secp.sign_schnorr_no_aux_rand(&msg, &keypair);

        let sig_hex = hex::encode(signature.as_ref());
        let pubkey_hex = xonly_pubkey.to_string();

        // Try to verify with different message
        let result = verify_schnorr_signature(tampered_message, &sig_hex, &pubkey_hex).unwrap();
        assert!(!result, "Schnorr signature should fail for wrong message");
    }

    #[test]
    fn test_verify_schnorr_signature_wrong_pubkey() {
        let secp = Secp256k1::signing_only();
        let (keypair1, _xonly_pubkey1) = create_test_schnorr_keypair(0x09);
        let (_keypair2, xonly_pubkey2) = create_test_schnorr_keypair(0x0A);

        let message = "Test Schnorr message";

        // Sign with first key
        let hash = sha256::Hash::hash(message.as_bytes());
        let msg = Message::from_digest(hash.to_byte_array());
        let signature = secp.sign_schnorr_no_aux_rand(&msg, &keypair1);

        let sig_hex = hex::encode(signature.as_ref());
        let wrong_pubkey_hex = xonly_pubkey2.to_string();

        // Try to verify with wrong public key
        let result = verify_schnorr_signature(message, &sig_hex, &wrong_pubkey_hex).unwrap();
        assert!(
            !result,
            "Schnorr signature should fail for wrong public key"
        );
    }

    #[test]
    fn test_verify_signature_invalid_hex() {
        let pubkey = "02a1633cafcc01ebfb6d78e39f657a51cafbfd3c8e4c8d0f6d6a9daada9b8f8c87";

        // Invalid hex in signature
        let result = verify_signature("test", "not_hex_gg", pubkey);
        assert!(result.is_err(), "Should fail with invalid hex signature");

        // Invalid length signature
        let result = verify_signature("test", "abcd", pubkey);
        assert!(result.is_err(), "Should fail with invalid length signature");
    }

    #[test]
    fn test_verify_schnorr_signature_invalid_format() {
        let xonly_pubkey = "a1633cafcc01ebfb6d78e39f657a51cafbfd3c8e4c8d0f6d6a9daada9b8f8c87";

        // Invalid hex in signature (Schnorr signatures are 64 bytes = 128 hex chars)
        let result = verify_schnorr_signature("test", "not_hex", xonly_pubkey);
        assert!(
            result.is_err(),
            "Should fail with invalid hex Schnorr signature"
        );

        // Wrong length signature
        let result = verify_schnorr_signature("test", "abcd", xonly_pubkey);
        assert!(
            result.is_err(),
            "Should fail with wrong length Schnorr signature"
        );
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

    #[test]
    fn test_derive_public_key_edge_cases() {
        // Empty string
        assert_eq!(derive_public_key_from_receiver_id("").unwrap(), None);

        // Wrong length hex
        assert_eq!(derive_public_key_from_receiver_id("abcd").unwrap(), None);

        // Mixed case (should fail as we check for hex)
        assert_eq!(derive_public_key_from_receiver_id("AbCd").unwrap(), None);

        // 65 chars (invalid length)
        let invalid = "a".repeat(65);
        assert_eq!(derive_public_key_from_receiver_id(&invalid).unwrap(), None);
    }

    #[test]
    fn test_verify_signature_captures_result() {
        // Test that the function properly returns Ok(true) for valid signatures
        let secp = Secp256k1::new();
        let (secret_key, public_key) = create_test_keypair(0x0B);

        let message = "Test result capture";

        // Create valid signature
        let mut hasher = Sha256::new();
        hasher.update(message.as_bytes());
        let hash = hasher.finalize();
        let msg = Message::from_digest_slice(&hash).unwrap();
        let signature = secp.sign_ecdsa(&msg, &secret_key);

        let sig_hex = hex::encode(signature.serialize_compact());
        let pubkey_hex = public_key.to_string();

        // Capture the result in a variable
        let verification_result = verify_signature(message, &sig_hex, &pubkey_hex).unwrap();

        // Assert that it returns true
        assert_eq!(
            verification_result, true,
            "Should return Ok(true) for valid signature"
        );

        // Test invalid signature returns Ok(false)
        let wrong_message = "Wrong message";
        let verification_result = verify_signature(wrong_message, &sig_hex, &pubkey_hex).unwrap();
        assert_eq!(
            verification_result, false,
            "Should return Ok(false) for invalid signature"
        );
    }
}
