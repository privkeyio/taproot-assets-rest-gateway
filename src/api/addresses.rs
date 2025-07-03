use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use actix_web::{web, HttpResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, instrument, warn};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Addr {
    pub encoded: Option<String>,
    pub asset_id: Option<String>,
    pub asset_type: Option<String>,
    pub amount: Option<String>,
    pub group_key: Option<String>,
    pub script_key: Option<String>,
    pub internal_key: Option<String>,
    pub tapscript_sibling: Option<String>,
    pub taproot_output_key: Option<String>,
    pub proof_courier_addr: Option<String>,
    pub asset_version: Option<String>,
    pub address_version: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewAddrRequest {
    pub asset_id: String,
    pub amt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tapscript_sibling: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proof_courier_addr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_version: Option<String>,
}

impl NewAddrRequest {
    /// Validates the address creation request
    pub fn validate(&self) -> Result<(), AppError> {
        // Basic validation for required fields
        if self.asset_id.trim().is_empty() {
            return Err(AppError::ValidationError(
                "asset_id cannot be empty".to_string(),
            ));
        }

        if self.amt.trim().is_empty() {
            return Err(AppError::ValidationError("amt cannot be empty".to_string()));
        }

        // Validate amount is a positive integer
        match self.amt.parse::<i64>() {
            Ok(amount) if amount <= 0 => {
                return Err(AppError::ValidationError(
                    "amt must be greater than zero".to_string(),
                ));
            }
            Err(_) => {
                return Err(AppError::ValidationError(
                    "amt must be a valid integer".to_string(),
                ));
            }
            _ => {} // Valid amount
        }

        // Check optional fields aren't empty if provided
        let optional_fields = [
            ("script_key", &self.script_key),
            ("internal_key", &self.internal_key),
        ];

        for (field_name, field_value) in &optional_fields {
            if let Some(value) = field_value {
                if value.trim().is_empty() {
                    return Err(AppError::ValidationError(format!(
                        "{field_name} cannot be an empty string"
                    )));
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DecodeAddrRequest {
    pub addr: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReceiveEventsRequest {
    pub filter_addr: Option<String>,
    pub filter_status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddressQueryParams {
    pub created_after: Option<String>,
    pub created_before: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[instrument(skip(client, macaroon_hex))]
pub async fn list_addresses(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    params: Option<&AddressQueryParams>,
) -> Result<Vec<Addr>, AppError> {
    debug!("Fetching taproot asset addresses");

    let mut url = format!("{base_url}/v1/taproot-assets/addrs");

    // Build query parameters if provided
    if let Some(query_params) = params {
        let mut query_parts = Vec::new();

        if let Some(ref after) = query_params.created_after {
            query_parts.push(format!("created_after={after}"));
        }
        if let Some(ref before) = query_params.created_before {
            query_parts.push(format!("created_before={before}"));
        }
        if let Some(limit) = query_params.limit {
            query_parts.push(format!("limit={limit}"));
        }
        if let Some(offset) = query_params.offset {
            query_parts.push(format!("offset={offset}"));
        }

        if !query_parts.is_empty() {
            url.push('?'); // Changed from push_str("?")
            url.push_str(&query_parts.join("&"));
        }
    }

    let response = client
        .get(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .send()
        .await
        .map_err(AppError::RequestError)?;

    let json = response
        .json::<HashMap<String, Vec<Addr>>>()
        .await
        .map_err(AppError::RequestError)?;

    let mut addresses = json.get("addrs").cloned().unwrap_or_default();

    // Apply client-side pagination if backend doesn't support it
    if let Some(params) = params {
        if let Some(offset) = params.offset {
            addresses = addresses.into_iter().skip(offset as usize).collect();
        }

        if let Some(limit) = params.limit {
            addresses = addresses.into_iter().take(limit as usize).collect();
        }
    }

    debug!("Retrieved {} addresses", addresses.len());
    Ok(addresses)
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn create_address(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: NewAddrRequest,
) -> Result<Addr, AppError> {
    // Validate before sending to backend
    request.validate()?;

    debug!("Creating new address for asset: {}", request.asset_id);

    let url = format!("{base_url}/v1/taproot-assets/addrs");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;

    let addr = response
        .json::<Addr>()
        .await
        .map_err(AppError::RequestError)?;

    if let Some(ref encoded) = addr.encoded {
        debug!("Created address: {}", encoded);
    }

    Ok(addr)
}

#[instrument(skip(client, macaroon_hex))]
pub async fn decode_address(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: DecodeAddrRequest,
) -> Result<Addr, AppError> {
    if request.addr.trim().is_empty() {
        return Err(AppError::ValidationError(
            "address cannot be empty".to_string(),
        ));
    }

    let url = format!("{base_url}/v1/taproot-assets/addrs/decode");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;

    response
        .json::<Addr>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex))]
pub async fn receive_events(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: ReceiveEventsRequest,
) -> Result<serde_json::Value, AppError> {
    debug!("Subscribing to receive events");

    let url = format!("{base_url}/v1/taproot-assets/addrs/receives");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;

    if !response.status().is_success() {
        warn!(
            "Receive events request failed with status: {}",
            response.status()
        );
    }

    response
        .json::<serde_json::Value>()
        .await
        .map_err(AppError::RequestError)
}

// Handler functions for actix-web routes
async fn list(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    query: web::Query<AddressQueryParams>,
) -> HttpResponse {
    match list_addresses(client.as_ref(), &base_url.0, &macaroon_hex.0, Some(&query)).await {
        Ok(addrs) => HttpResponse::Ok().json(serde_json::json!({ "addrs": addrs })),
        Err(e) => {
            let status = e.status_code();
            HttpResponse::build(status).json(serde_json::json!({
                "error": e.to_string(),
                "type": format!("{:?}", e)
            }))
        }
    }
}

async fn create(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<NewAddrRequest>,
) -> HttpResponse {
    handle_result(
        create_address(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn decode(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<DecodeAddrRequest>,
) -> HttpResponse {
    handle_result(
        decode_address(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn receive(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<ReceiveEventsRequest>,
) -> HttpResponse {
    handle_result(
        receive_events(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

/// Common handler for converting Results to HTTP responses
fn handle_result<T: serde::Serialize>(result: Result<T, AppError>) -> HttpResponse {
    match result {
        Ok(value) => HttpResponse::Ok().json(value),
        Err(e) => {
            let status = e.status_code();
            HttpResponse::build(status).json(serde_json::json!({
                "error": e.to_string(),
                "type": format!("{:?}", e)
            }))
        }
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/addrs")
            .route(web::get().to(list))
            .route(web::post().to(create)),
    )
    .service(web::resource("/addrs/decode").route(web::post().to(decode)))
    .service(web::resource("/addrs/receives").route(web::post().to(receive)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_asset_id() {
        let request = NewAddrRequest {
            asset_id: "".to_string(),
            amt: "100".to_string(),
            script_key: None,
            internal_key: None,
            tapscript_sibling: None,
            proof_courier_addr: None,
            asset_version: None,
            address_version: None,
        };

        let result = request.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("asset_id cannot be empty"));
    }

    #[test]
    fn test_validate_zero_amount() {
        let request = NewAddrRequest {
            asset_id: "test_asset".to_string(),
            amt: "0".to_string(),
            script_key: None,
            internal_key: None,
            tapscript_sibling: None,
            proof_courier_addr: None,
            asset_version: None,
            address_version: None,
        };

        let result = request.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("amt must be greater than zero"));
    }

    #[test]
    fn test_validate_negative_amount() {
        let request = NewAddrRequest {
            asset_id: "test_asset".to_string(),
            amt: "-100".to_string(),
            script_key: None,
            internal_key: None,
            tapscript_sibling: None,
            proof_courier_addr: None,
            asset_version: None,
            address_version: None,
        };

        assert!(request.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_amount_format() {
        let request = NewAddrRequest {
            asset_id: "test_asset".to_string(),
            amt: "not_a_number".to_string(),
            script_key: None,
            internal_key: None,
            tapscript_sibling: None,
            proof_courier_addr: None,
            asset_version: None,
            address_version: None,
        };

        let result = request.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("valid integer"));
    }

    #[test]
    fn test_validate_empty_optional_fields() {
        let request = NewAddrRequest {
            asset_id: "test_asset".to_string(),
            amt: "100".to_string(),
            script_key: Some("".to_string()),
            internal_key: Some("   ".to_string()), // whitespace only
            tapscript_sibling: None,
            proof_courier_addr: None,
            asset_version: None,
            address_version: None,
        };

        assert!(request.validate().is_err());
    }

    #[test]
    fn test_validate_success() {
        let request = NewAddrRequest {
            asset_id: "valid_asset_id".to_string(),
            amt: "1000".to_string(),
            script_key: Some("valid_script_key".to_string()),
            internal_key: None,
            tapscript_sibling: None,
            proof_courier_addr: None,
            asset_version: None,
            address_version: None,
        };

        assert!(request.validate().is_ok());
    }
}
