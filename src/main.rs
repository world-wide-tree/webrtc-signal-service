//! Пишем сигнальный сервер для того чтобы устроить обмен между
//! двумя устройствами SDP, тем временем настроив их рукапожатие
//! TODO: пишем вебсокетный сервер
//! маршруты:
//! * /answer - сторона которая будет стримить
//! * /offer - сторона которая будет наблюдать
//! 


use std::{collections::HashMap, sync::Arc};

use axum::{extract::{ws::WebSocket, ConnectInfo, Path, WebSocketUpgrade}, http::StatusCode, response::IntoResponse, routing::{get, post}, serve, Json, Router};
use serde::{Deserialize, Serialize};
use tokio::{net::{unix::SocketAddr, TcpListener}, sync::Mutex};
use webrtc::{api::media_engine::MediaEngine, ice_transport::ice_candidate::RTCIceCandidateInit, peer_connection::{configuration::RTCConfiguration, sdp::session_description::RTCSessionDescription}};

#[derive(Clone)]
struct AppState {
    offers: HashMap<String, RTCSessionDescription>, // Cameras
    answers: HashMap<String, RTCSessionDescription> // Clients
}



#[derive(Serialize, Deserialize, Clone)]
struct SignalMessage {

    sdp: RTCSessionDescription,
    candidate: RTCIceCandidateInit,
}

#[derive(Default)]
struct State {
    sdp: Option<SignalMessage>,
    ice_candidates: Vec<RTCIceCandidateInit>,
}
type AppState1 = Arc<Mutex<HashMap<String, State>>>;
#[tokio::main]
async fn main() {
    let db: HashMap<String, State> = HashMap::new(); // camera_id <-> SDP 
    let state = Arc::new(Mutex::new(db));
    let listener = TcpListener::bind("0.0.0.0:3030").await.unwrap();
    let route = Router::new()
        .route("/offer/:camera_id", post(offer_handler))
        .route("/answer/:camera_id", post(answer_handler))
        .route("/signal/:camera_id", get(get_signal))
        .layer(axum::extract::Extension(state))
    ;

    serve(listener,route).await.unwrap();
}
async fn answer_handler(
    Path(camera_id): Path<String>,
    axum::extract::Extension(state): axum::extract::Extension<Arc<Mutex<AppState>>>,
    Json(sdp): Json<RTCSessionDescription>
)->impl IntoResponse{
    let mut state = state.lock().await;
    if let Some(s) = state.offers.get(&camera_id){
        let s = s.clone();
        let _ = state.answers.insert(camera_id, sdp);
        Json(s).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
async fn get_signal(
    Path(camera_id): Path<String>,
    axum::extract::Extension(state): axum::extract::Extension<Arc<Mutex<AppState>>>,
) -> impl IntoResponse{
    let state = state.lock().await;
    if let Some(sdp) = state.answers.get(&camera_id){
        Json(sdp.clone()).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
async fn offer_handler(
    Path(camera_id): Path<String>,
    axum::extract::Extension(state): axum::extract::Extension<Arc<Mutex<AppState>>>,
    Json(sdp): Json<RTCSessionDescription>
){
    let mut state = state.lock().await;
    state.offers.insert(camera_id, sdp);

}
// async fn ws_handler(
//     ws: WebSocketUpgrade,
//     user_agent: Option<TypedHeader<headers::UserAgent>>,
//     ConnectInfo(addr): ConnectInfo<SocketAddr>,
// ) -> impl IntoResponse {
//     let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
//         user_agent.to_string()
//     } else {
//         String::from("Unknown browser")
//     };
//     println!("`{user_agent}` at {addr} connected.");
//     // finalize the upgrade process by returning upgrade callback.
//     // we can customize the callback by sending additional info such as address.
//     ws.on_upgrade(move |socket| handle_socket(socket, addr))
// }

// async fn handle_socket(mut socket: WebSocket, who: SocketAddr) {

// }



