use axum::{routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, sync::broadcast};
use webrtc::{ice_transport::ice_candidate::RTCIceCandidateInit, peer_connection::sdp::session_description::RTCSessionDescription};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct SignalMessage{
    sdp: Option<RTCSessionDescription>,
    candidate: Option<RTCIceCandidateInit>
}

#[tokio::main]
async fn main() {
    let (tx, _rx) = broadcast::channel::<SignalMessage>(10);

    let app = Router::new()
        .route("/signal", post(signal_handler))
        .layer(axum::extract::Extension(tx));

    let addr = TcpListener::bind("0.0.0.0:3030").await.unwrap();//SocketAddr::from(([127, 0, 0, 1], 3030));
    println!("Signal server running on 0.0.0.0:3030");
    axum::serve(addr, app)
        .await
        .unwrap()
    ;
}


async fn signal_handler(
    axum::extract::Extension(tx): axum::extract::Extension<broadcast::Sender<SignalMessage>>,
    Json(message): Json<SignalMessage>,
) -> Json<SignalMessage> {
    let mut rx = tx.subscribe();

    if message.sdp.is_some() || message.candidate.is_some() {
        tx.send(message.clone()).unwrap();
    }

    let response = rx.recv().await.unwrap();
    Json(response)
}