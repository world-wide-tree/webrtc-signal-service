//! Пишем сигнальный сервер для того чтобы устроить обмен между
//! двумя устройствами SDP, тем временем настроив их рукапожатие
//! TODO: пишем вебсокетный сервер
//! маршруты:
//! * /answer - сторона которая будет стримить
//! * /offer - сторона которая будет наблюдать
//! 


use std::{collections::HashMap, sync::Arc};

use axum::{extract::{ws::WebSocket, ConnectInfo, Path, State, WebSocketUpgrade}, http::StatusCode, response::{IntoResponse, Response}, routing::{get, post}, serve, Extension, Json, Router};
use futures::{SinkExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio::{net::{unix::SocketAddr, TcpListener}, sync::{mpsc::{channel, Receiver, Sender}, Mutex, OnceCell}};
use webrtc::{api::media_engine::MediaEngine, ice_transport::ice_candidate::{RTCIceCandidate, RTCIceCandidateInit}, peer_connection::{configuration::RTCConfiguration, sdp::session_description::RTCSessionDescription}};

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceIceDto{
    device_id: String,
    ice: RTCIceCandidate
}
#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceOfferDto{
    device_id: String,
    offer: RTCSessionDescription
}
#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceAnswerDto{
    device_id: String,
    answer: RTCSessionDescription
}
#[derive(Debug, Serialize, Deserialize)]
pub enum Cmd{
    Offer(DeviceOfferDto),
    Answer(DeviceAnswerDto),
    Candidate(DeviceIceDto),
    NotSupported
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Msg{
    user: String,
    cmd: Cmd,
}

struct AppState{
    pub connections: HashMap<String, WebSocket>,
}
impl AppState {
    pub fn new() -> Self{
        Self{
            connections: HashMap::new()
        }
    }
}
#[tokio::main]
async fn main() {
    // let state = Arc::new(Mutex::new(AppState::default()));
    let state = Arc::new(Mutex::new(AppState::new()));
    let listener = TcpListener::bind("0.0.0.0:3030").await.unwrap();
    let route = Router::new()
        .route("/signaling/:device_id", get(ws_user_connection_handler))
        .route("/offer/:device_id", post(post_offer))
        .route("/answer/:device_id", post(post_answer))
        .route("/candidate/:connected_deivce_id", post(post_candidate))
        .with_state(state)
    ;

    serve(listener,route).await.unwrap();
}
async fn post_candidate(
    Path(connected_device_id): Path<String>,
    State(state): State<Arc<Mutex<AppState>>>,
    Json(ice): Json<DeviceIceDto>,
) -> impl IntoResponse{
    let mut state = state.lock().await;
    if let Some(ws) = state.connections.get_mut(&connected_device_id){
        let msg = Cmd::Candidate(ice);
        if let Err(e) = ws.send(axum::extract::ws::Message::Text(serde_json::to_string(&msg).unwrap())).await{
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        } else {
            (axum::http::StatusCode::OK, "Candidate sended!".to_string())
        }
    } else {
        (axum::http::StatusCode::NOT_FOUND, "DeviceNotFound".to_string())
    }
}
async fn post_offer(
    Path(device_id): Path<String>,
    State(state): State<Arc<Mutex<AppState>>>,
    Json(sdp): Json<DeviceOfferDto>,
) -> impl IntoResponse{
    let mut state = state.lock().await;
    if let Some(ws) = state.connections.get_mut(&device_id){
        let msg = Cmd::Offer(sdp);
        if let Err(e) = ws.send(axum::extract::ws::Message::Text(serde_json::to_string(&msg).unwrap())).await{
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        } else {
            (axum::http::StatusCode::OK, "Offer sended!".to_string())
        }
    } else {
        (axum::http::StatusCode::NOT_FOUND, "DeviceNotFound".to_string())
    }
}
async fn post_answer(
    Path(device_id): Path<String>,
    State(state): State<Arc<Mutex<AppState>>>,
    Json(sdp): Json<DeviceAnswerDto>,
) -> impl IntoResponse{
    let mut state = state.lock().await;
    if let Some(ws) = state.connections.get_mut(&device_id){
        let msg = Cmd::Answer(sdp);
        if let Err(e) = ws.send(axum::extract::ws::Message::Text(serde_json::to_string(&msg).unwrap())).await{
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        } else {
            (axum::http::StatusCode::OK, "Answer sended!".to_string())
        }
    } else {
        (axum::http::StatusCode::NOT_FOUND, "DeviceNotFound".to_string())
    }
}
async fn ws_user_connection_handler(
    Path(device_id): Path<String>,
    State(state): State<Arc<Mutex<AppState>>>,
    ws: WebSocketUpgrade
) -> Response{
    ws.on_upgrade(|ws| handle_ws(ws, device_id, state))
}
async fn handle_ws(ws: WebSocket, device_id: String, state: Arc<Mutex<AppState>>){
    state.lock().await.connections.insert(device_id, ws);    
}
// pub async fn handle_user_ws(mut socket: WebSocket, mut rx: Receiver<Msg>, state: Arc<Mutex<HashMap<String, Sender<Msg>>>>){
//     let (mut sink,mut stream) = socket.split();
//     loop {
//         if let Ok(msg) = stream.try_next().await{
//             if let Some(msg) = msg{
//                 let cmd = axum2reqwest_msg_convert(msg).json::<Msg>();
//                 if let Ok(msg) = cmd{
//                     {
//                         let mut state = state.lock().await;
//                         if let Some(tx) = state.get(&msg.user){
//                             let user = msg.user.clone();
//                             if let Err(e) = tx.send(msg).await{
//                                 eprintln!("{e}");
//                                 state.remove(&user);
//                             }
//                         }
//                     }
//                 } else {
//                     let msg = Cmd::NotSupported;
//                     let _ = sink.send(axum::extract::ws::Message::Text(serde_json::to_string(&msg).unwrap())).await;
//                     continue;
//                 }
//             } else {
//                 break;
//             }
//         }
//         if let Ok(cmd) = rx.try_recv(){
//             if let Err(e) = sink.send(axum::extract::ws::Message::Text(serde_json::to_string(&cmd).unwrap())).await{
//                 eprintln!("{e}");
//             }
//         }
//         tokio::time::sleep(std::time::Duration::from_millis(100)).await;
//     }    
// }

// fn axum2reqwest_msg_convert(amsg: axum::extract::ws::Message) -> reqwest_websocket::Message{
//     match amsg{
//         axum::extract::ws::Message::Binary(b) => {
//             reqwest_websocket::Message::Binary(b)
//         },
//         axum::extract::ws::Message::Text(t) => {
//             reqwest_websocket::Message::Text(t)
//         },
//         axum::extract::ws::Message::Ping(ping) => {
//             reqwest_websocket::Message::Ping(ping)
//         },
//         axum::extract::ws::Message::Pong(pong) => {
//             reqwest_websocket::Message::Pong(pong)
//         },
//         _ => {
//             reqwest_websocket::Message::Close { code: reqwest_websocket::CloseCode::Normal, reason: "".to_string() }
//         }
//     }
// }