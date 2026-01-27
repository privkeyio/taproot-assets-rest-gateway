use crate::crypto::{
    derive_public_key_from_receiver_id, verify_schnorr_signature, verify_signature,
};
use crate::database::{ReceiverInfo, SharedDatabase};
use crate::error::AppError;
use base64::Engine;
use bitcoin::bech32;
use chrono::Utc;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};

use super::mailbox::ReceiveRequest;

const CHALLENGE_EXPIRY_SECS: u64 = 300;
const TIMESTAMP_TOLERANCE_SECS: i64 = 30;
const MAX_ACTIVE_CHALLENGES: usize = 10_000;

#[derive(Debug, Clone)]
pub(crate) struct ChallengeData {
    pub challenge_id: String,
    pub timestamp: i64,
    pub nonce: String,
    pub issued_at: Instant,
}

lazy_static::lazy_static! {
    static ref ACTIVE_CHALLENGES: Mutex<HashMap<String, ChallengeData>> = Mutex::new(HashMap::new());
}

pub(crate) async fn generate_challenge() -> Result<serde_json::Value, AppError> {
    let challenge_id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().timestamp();
    let nonce = base64::engine::general_purpose::STANDARD.encode(uuid::Uuid::new_v4().as_bytes());

    let challenge_data = ChallengeData {
        challenge_id: challenge_id.clone(),
        timestamp,
        nonce: nonce.clone(),
        issued_at: Instant::now(),
    };

    {
        let mut challenges = ACTIVE_CHALLENGES.lock().unwrap();

        challenges.retain(|_, data| data.issued_at.elapsed().as_secs() < CHALLENGE_EXPIRY_SECS);

        if challenges.len() >= MAX_ACTIVE_CHALLENGES {
            return Err(AppError::ValidationError(
                "Too many pending challenges. Please try again later.".to_string(),
            ));
        }

        challenges.insert(challenge_id.clone(), challenge_data);
    }

    Ok(serde_json::json!({
        "challenge_id": challenge_id,
        "timestamp": timestamp,
        "nonce": nonce,
        "message": format!("Sign this challenge: {}-{}-{}", challenge_id, timestamp, nonce)
    }))
}

pub(crate) async fn validate_authentication(
    init: &serde_json::Value,
    auth_sig: &serde_json::Value,
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    database: Option<&SharedDatabase>,
) -> Result<bool, AppError> {
    let receiver_id = init
        .get("receiver_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::InvalidInput("Missing receiver_id in init data".to_string()))?;

    let signature = auth_sig
        .get("signature")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::InvalidInput("Missing signature in auth_sig".to_string()))?;

    let challenge_id = auth_sig
        .get("challenge_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::InvalidInput("Missing challenge_id in auth_sig".to_string()))?;

    let signed_timestamp = auth_sig
        .get("timestamp")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| AppError::InvalidInput("Missing timestamp in auth_sig".to_string()))?;

    if signature.is_empty() || signature.len() < 32 {
        warn!("Invalid signature format: too short");
        return Ok(false);
    }

    if receiver_id.is_empty() {
        warn!("Invalid receiver_id: empty");
        return Ok(false);
    }

    if !signature.chars().all(|c| c.is_ascii_hexdigit())
        && base64::engine::general_purpose::STANDARD
            .decode(signature)
            .is_err()
    {
        warn!("Invalid signature encoding: not hex or base64");
        return Ok(false);
    }

    let challenge_data = {
        let mut challenges = ACTIVE_CHALLENGES.lock().unwrap();
        let data = challenges
            .get(challenge_id)
            .ok_or_else(|| {
                warn!("Challenge not found: {}", challenge_id);
                AppError::InvalidInput("Invalid or expired challenge".to_string())
            })?
            .clone();

        if data.issued_at.elapsed().as_secs() > CHALLENGE_EXPIRY_SECS {
            warn!("Challenge expired: {}", challenge_id);
            challenges.remove(challenge_id);
            return Ok(false);
        }

        data
    };

    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::InvalidInput("System time error".to_string()))?
        .as_secs() as i64;

    let time_diff = (current_time - signed_timestamp).abs();
    if time_diff > TIMESTAMP_TOLERANCE_SECS {
        warn!(
            "Timestamp validation failed: time difference {} seconds exceeds tolerance",
            time_diff
        );
        return Ok(false);
    }

    let challenge_time_diff = (challenge_data.timestamp - signed_timestamp).abs();
    if challenge_time_diff > TIMESTAMP_TOLERANCE_SECS {
        warn!(
            "Challenge timestamp mismatch: difference {} seconds",
            challenge_time_diff
        );
        return Ok(false);
    }

    let expected_message = format!(
        "Sign this challenge: {}-{}-{}",
        challenge_data.challenge_id, challenge_data.timestamp, challenge_data.nonce
    );

    if !verify_signature_with_receiver(&expected_message, signature, receiver_id, database).await? {
        warn!("Cryptographic signature verification failed");
        return Ok(false);
    }

    if !validate_macaroon_permissions(client, base_url, macaroon_hex, receiver_id).await? {
        warn!("Macaroon permission validation failed");
        return Ok(false);
    }

    if !validate_receiver_id(receiver_id, client, base_url, macaroon_hex, database).await? {
        warn!("Receiver ID validation failed: {}", receiver_id);
        return Ok(false);
    }

    {
        let mut challenges = ACTIVE_CHALLENGES.lock().unwrap();
        challenges.remove(challenge_id);
    }

    if let Some(db) = database {
        let public_key = if let Some(pk) = auth_sig.get("public_key").and_then(|v| v.as_str()) {
            pk.to_string()
        } else if let Some(pk) = derive_public_key_from_receiver_id(receiver_id)? {
            pk
        } else {
            format!("unknown_{receiver_id}")
        };

        let receiver_info = ReceiverInfo {
            receiver_id: receiver_id.to_string(),
            public_key,
            address: init
                .get("address")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            created_at: Utc::now().timestamp(),
            last_seen: Utc::now().timestamp(),
            is_active: true,
            metadata: Some(serde_json::json!({
                "auth_method": "mailbox",
                "last_challenge_id": challenge_id,
            })),
        };

        if let Err(e) = db.store_receiver_info(&receiver_info).await {
            warn!("Failed to store receiver info in database: {}", e);
        }
    }

    info!(
        "Authentication successfully validated for receiver_id: {}",
        receiver_id
    );
    Ok(true)
}

async fn verify_signature_with_receiver(
    message: &str,
    signature: &str,
    receiver_id: &str,
    database: Option<&SharedDatabase>,
) -> Result<bool, AppError> {
    if let Some(public_key) = derive_public_key_from_receiver_id(receiver_id)? {
        if public_key.len() == 64 {
            return verify_schnorr_signature(message, signature, &public_key);
        } else {
            return verify_signature(message, signature, &public_key);
        }
    }

    if let Some(db) = database {
        if let Some(receiver_info) = db.get_receiver_info(receiver_id).await? {
            if receiver_info.public_key.len() == 64 {
                return verify_schnorr_signature(message, signature, &receiver_info.public_key);
            } else {
                return verify_signature(message, signature, &receiver_info.public_key);
            }
        }
    }

    warn!("Unable to find public key for receiver_id: {}", receiver_id);
    Ok(false)
}

async fn validate_macaroon_permissions(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    receiver_id: &str,
) -> Result<bool, AppError> {
    let info_url = format!("{base_url}/v1/taproot-assets/mailbox/info");
    let info_response = client
        .get(&info_url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| {
            error!("Failed to validate macaroon with backend: {}", e);
            AppError::RequestError(e)
        })?;

    if !info_response.status().is_success() {
        warn!(
            "Macaroon validation failed with status: {}",
            info_response.status()
        );
        return Ok(false);
    }

    let info_json = info_response
        .json::<serde_json::Value>()
        .await
        .map_err(AppError::RequestError)?;

    if let Some(mailbox_enabled) = info_json.get("mailbox_enabled").and_then(|v| v.as_bool()) {
        if !mailbox_enabled {
            warn!("Mailbox feature is not enabled on the backend");
            return Ok(false);
        }
    }

    let test_receive = ReceiveRequest {
        init: serde_json::json!({
            "receiver_id": receiver_id,
            "test": true
        }),
        auth_sig: serde_json::json!({
            "test": true
        }),
    };

    let receive_url = format!("{base_url}/v1/taproot-assets/mailbox/receive");
    let receive_response = client
        .post(&receive_url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&test_receive)
        .timeout(Duration::from_secs(2))
        .send()
        .await;

    match receive_response {
        Ok(resp) => {
            if resp.status() == reqwest::StatusCode::FORBIDDEN {
                warn!("Macaroon lacks mailbox receive permissions");
                return Ok(false);
            }
        }
        Err(e) if e.is_timeout() => {
            debug!("Permission check timed out, assuming permissions are valid");
        }
        Err(e) => {
            warn!("Failed to check mailbox permissions: {}", e);
        }
    }

    info!(
        "Macaroon permissions validated for receiver_id: {}",
        receiver_id
    );
    Ok(true)
}

fn is_valid_bech32_chars(s: &str) -> bool {
    const BECH32_CHARSET: &[u8] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
    s.chars()
        .all(|c| c.is_ascii_lowercase() && BECH32_CHARSET.contains(&(c as u8)))
}

fn validate_taproot_address_format(address: &str) -> Result<bool, AppError> {
    if !address.starts_with("taprt1") {
        return Ok(false);
    }

    let data_part = &address[6..];

    if !is_valid_bech32_chars(data_part) {
        return Ok(false);
    }

    match bech32::decode(address) {
        Ok((hrp, data)) => Ok(hrp.as_str() == "taprt1" && !data.is_empty()),
        Err(_) => Ok(false),
    }
}

async fn validate_receiver_id(
    receiver_id: &str,
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    database: Option<&SharedDatabase>,
) -> Result<bool, AppError> {
    if receiver_id.len() < 8 {
        warn!("Receiver ID too short: {}", receiver_id);
        return Ok(false);
    }

    if !is_valid_bech32_chars(receiver_id)
        && !receiver_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        warn!("Receiver ID contains invalid characters: {}", receiver_id);
        return Ok(false);
    }

    if (derive_public_key_from_receiver_id(receiver_id)?).is_some() {
        info!("Receiver ID is a valid public key: {}", receiver_id);
        return Ok(true);
    }

    if let Some(db) = database {
        if let Some(receiver_info) = db.get_receiver_info(receiver_id).await? {
            if receiver_info.is_active {
                info!(
                    "Receiver ID found in database and is active: {}",
                    receiver_id
                );
                return Ok(true);
            } else {
                warn!("Receiver ID found but is inactive: {}", receiver_id);
                return Ok(false);
            }
        }
    }

    let decode_url = format!("{base_url}/v1/taproot-assets/addrs/decode");
    let test_address = format!("taprt1{receiver_id}");

    match validate_taproot_address_format(&test_address) {
        Ok(true) => {
            let response = client
                .post(&decode_url)
                .header("Grpc-Metadata-macaroon", macaroon_hex)
                .json(&serde_json::json!({"addr": test_address}))
                .timeout(Duration::from_secs(2))
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    info!("Receiver ID validated via tapd backend: {}", receiver_id);
                    Ok(true)
                }
                _ => {
                    warn!(
                        "Receiver ID has valid format but failed backend validation: {}",
                        receiver_id
                    );
                    Ok(false)
                }
            }
        }
        Ok(false) => {
            warn!(
                "Receiver ID does not form a valid Taproot address: {}",
                receiver_id
            );
            Ok(false)
        }
        Err(e) => {
            warn!("Error validating Taproot address format: {}", e);
            Ok(false)
        }
    }
}
