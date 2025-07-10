pub mod connection_manager;

use actix_web::{web, HttpRequest, HttpResponse, Result as ActixResult};
use actix_ws::{Message, MessageStream, Session};
use futures_util::StreamExt;
use tracing::{error, info};

use crate::types::{BaseUrl, MacaroonHex};

pub async fn websocket_handler(
    req: HttpRequest,
    stream: web::Payload,
    _base_url: web::Data<BaseUrl>,
    _macaroon: web::Data<MacaroonHex>,
) -> ActixResult<HttpResponse> {
    let (response, session, msg_stream) = actix_ws::handle(&req, stream)?;

    info!("WebSocket connection established");

    // Use actix_rt::spawn instead of tokio::spawn to handle the connection
    actix_rt::spawn(handle_websocket_connection(session, msg_stream));

    Ok(response)
}

async fn handle_websocket_connection(mut session: Session, mut msg_stream: MessageStream) {
    // TODO: Implement WebSocket connection to tapd
    // For now, this is a placeholder that handles basic WebSocket messages

    while let Some(msg) = msg_stream.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                info!("Received WebSocket message: {}", text);

                // Echo the message back for now
                if let Err(e) = session.text(format!("Echo: {text}")).await {
                    error!("Failed to send WebSocket message: {}", e);
                    break;
                }
            }
            Ok(Message::Close(_)) => {
                info!("WebSocket connection closed");
                break;
            }
            Ok(Message::Ping(bytes)) => {
                if let Err(e) = session.pong(&bytes).await {
                    error!("Failed to send pong: {}", e);
                    break;
                }
            }
            Ok(_) => {}
            Err(e) => {
                error!("WebSocket message error: {}", e);
                break;
            }
        }
    }
}
