//! Пишем сигнальный сервер для того чтобы устроить обмен между
//! двумя устройствами SDP, тем временем настроив их рукапожатие
//! TODO: пишем вебсокетный сервер
//! маршруты:
//! * /answer - сторона которая будет стримить
//! * /offer - сторона которая будет наблюдать
//! 


use std::{collections::{HashMap, HashSet}, sync::Arc};

use axum::{extract::{ws::WebSocket, ConnectInfo, Path, State, WebSocketUpgrade}, http::StatusCode, response::{IntoResponse, Response}, routing::{get, post}, serve, Extension, Json, Router};
use futures::{SinkExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio::{net::{unix::SocketAddr, TcpListener}, sync::{mpsc::{channel, Receiver, Sender}, Mutex, OnceCell}};
use tower_http::cors::{Any, CorsLayer};
use webrtc::{api::media_engine::MediaEngine, ice_transport::ice_candidate::{RTCIceCandidate, RTCIceCandidateInit}, peer_connection::{configuration::RTCConfiguration, sdp::session_description::RTCSessionDescription}};

#[derive(Debug, Serialize, Deserialize)]
pub struct UserIceDto{
    user_id: String,
    ice: String
}
#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceIceDto{
    device_id: String,
    camera_id: String,
    ice: String
}
#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceOfferDto{
    user_id: String,
    camera_id: String,
    offer: String
}
#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceAnswerDto{
    device_id: String,
    camera_id: String,
    answer: String
}
#[derive(Debug, Serialize, Deserialize)]
pub enum Cmd{
    OfferToDevice(DeviceOfferDto),
    AnswerToUser(DeviceAnswerDto),
    CandidateFromDevice(DeviceIceDto),
    CandidateFromUser(UserIceDto),
    NotSupported
}

struct Device{
    pub connection: WebSocket,
    pub cameras: HashSet<String>
}

struct AppState{
    pub devices: HashMap<String, Device>,
    pub users: HashMap<String, WebSocket>
}
impl AppState {
    pub fn new() -> Self{
        Self{
            devices: HashMap::new(),
            users: HashMap::new(),
        }
    }
}

#[tokio::main]
async fn main() {
    let cors = CorsLayer::new()
        .allow_origin(Any) // Allow requests from any origin
        .allow_methods(Any) // Allow any HTTP methods (GET, POST, etc.)
        .allow_headers(Any); // Allow any header
    // let state = Arc::new(Mutex::new(AppState::default()));
    let state = Arc::new(Mutex::new(AppState::new()));
    let listener = TcpListener::bind("0.0.0.0:3030").await.unwrap();
    let route = Router::new()
        // Device EndPoints
        .route("/device/signaling/:device_id", get(ws_device_connection_handler))
        .route("/device/add_camera/:device_id/:camera_id", post(add_camera_to_device_handler))
        .route("/device/answer/:user_id", post(post_answer_to_user_handler))
        .route("/device/candidate/:user_id", post(post_device_candidate_handler))
        // UserEndpoints
        .route("/user/signaling/:user_id", get(ws_user_connection_handler))
        .route("/user/offer/:device_id/:camera_id", post(post_offer_device_handler))
        .route("/user/candidate/:device_id/:camera_id", post(post_user_candidate_handler))
        //.route("/add_camera/:device_id", post(""))
        //.route("/offer/:device_id/:camera_id", post(post_offer))
        //.route("/answer/:device_id/:user_id", post(post_answer))
        //.route("/candidate/:camera_user_id", post(post_candidate))
        .with_state(state)
        .layer(cors)
    ;

    serve(listener,route).await.unwrap();
}
async fn post_user_candidate_handler(
    Path((device_id, camera_id)): Path<(String, String)>,
    State(state): State<Arc<Mutex<AppState>>>,
    Json(ice): Json<UserIceDto>
) -> impl IntoResponse{
    let mut state = state.lock().await;
    if let Some(device) = state.devices.get_mut(&device_id){
        let msg = Cmd::CandidateFromUser(ice);
        if device.cameras.get(&camera_id).is_none(){
            (axum::http::StatusCode::NOT_FOUND, "Camera not found on device!")
        } else {
            if let Err(e) = device.connection.send(axum::extract::ws::Message::Text(serde_json::to_string(&msg).unwrap())).await{
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Connot send offer to device!")
            } else {
                (axum::http::StatusCode::OK, "Offer sended soccess!")
            }
        }
    } else {
        (axum::http::StatusCode::NOT_FOUND, "Device not found!")
    }
}
async fn post_offer_device_handler(
    Path((device_id, camera_id)): Path<(String, String)>,
    State(state): State<Arc<Mutex<AppState>>>,
    Json(offer): Json<DeviceOfferDto>,
) -> impl IntoResponse{
    let mut state = state.lock().await;
    if let Some(device) = state.devices.get_mut(&device_id){
        let msg = Cmd::OfferToDevice(offer);
        println!("Device cameras: {:#?}", device.cameras);
        if device.cameras.get(&camera_id).is_none(){
            (axum::http::StatusCode::NOT_FOUND, "Camera not found on device!")
        } else {
            if let Err(e) = device.connection.send(axum::extract::ws::Message::Text(serde_json::to_string(&msg).unwrap())).await{
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Connot send offer to device!")
            } else {
                (axum::http::StatusCode::OK, "Offer sended soccess!")
            }
        }
    } else {
        (axum::http::StatusCode::NOT_FOUND, "Device not found!")
    }
}
async fn post_device_candidate_handler(
    Path(user_id): Path<String>,
    State(state): State<Arc<Mutex<AppState>>>,
    Json(ice): Json<DeviceIceDto>,
) -> impl IntoResponse{
    let mut state = state.lock().await;
    if let Some(ws) = state.users.get_mut(&user_id){
        let msg = Cmd::CandidateFromDevice(ice);
        if let Err(e) = ws.send(axum::extract::ws::Message::Text(serde_json::to_string(&msg).unwrap())).await{
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        } else {
            (axum::http::StatusCode::OK, "Candidate sended!".to_string())
        }
    } else {
        (axum::http::StatusCode::NOT_FOUND, "DeviceNotFound".to_string())
    }
}
async fn post_answer_to_user_handler(
    Path(user_id): Path<String>,
    State(state): State<Arc<Mutex<AppState>>>,
    Json(sdp): Json<DeviceAnswerDto>,
) -> impl IntoResponse{
    if let Some(user) = state.lock().await.users.get_mut(&user_id){
        let msg = Cmd::AnswerToUser(sdp);
        if let Err(e) = user.send(axum::extract::ws::Message::Text(serde_json::to_string(&msg).unwrap())).await{
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Cannot send sdp to user!")
        } else {
            (axum::http::StatusCode::OK, "Send sdp to user soccess!")
        }
    } else {
        (axum::http::StatusCode::NOT_FOUND, "User Not found!")
    }
}
async fn add_camera_to_device_handler(
    Path((device_id,camera_id)): Path<(String, String)>,
    State(state): State<Arc<Mutex<AppState>>>,
) -> impl IntoResponse{
    if let Some(device) = state.lock().await.devices.get_mut(&device_id){
        println!("Device cameras: {:#?}", device.cameras);
        if !device.cameras.insert(camera_id){
            println!("Device cameras: {:#?}", device.cameras);
            (axum::http::StatusCode::BAD_REQUEST, "Camera already exist")
        } else {
            println!("Device cameras: {:#?}", device.cameras);
            (axum::http::StatusCode::OK, "Camera added soccesfuly!")
        }
    } else {
        (axum::http::StatusCode::NOT_FOUND, "Device not found!")
    }
}
async fn ws_device_connection_handler(
    Path(device_id): Path<String>,
    State(state): State<Arc<Mutex<AppState>>>,
    ws: WebSocketUpgrade
) -> Response{
    ws.on_upgrade(|ws| handle_device_ws(ws, device_id, state))
}
async fn handle_device_ws(ws: WebSocket, device_id: String, state: Arc<Mutex<AppState>>){
    state.lock().await.devices.insert(device_id, Device { connection: ws, cameras: HashSet::new() });    
}
async fn ws_user_connection_handler(
    Path(user_id): Path<String>,
    State(state): State<Arc<Mutex<AppState>>>,
    ws: WebSocketUpgrade
) -> Response{
    ws.on_upgrade(|ws| handle_user_ws(ws, user_id, state))
}
async fn handle_user_ws(ws: WebSocket, user_id: String, state: Arc<Mutex<AppState>>){
    state.lock().await.users.insert(user_id, ws);    
}
// async fn post_candidate(
//     Path(connected_device_id): Path<String>,
//     State(state): State<Arc<Mutex<AppState>>>,
//     Json(ice): Json<DeviceIceDto>,
// ) -> impl IntoResponse{
//     let mut state = state.lock().await;
//     if let Some(ws) = state.connections.get_mut(&connected_device_id){
//         let msg = Cmd::Candidate(ice);
//         if let Err(e) = ws.send(axum::extract::ws::Message::Text(serde_json::to_string(&msg).unwrap())).await{
//             (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
//         } else {
//             (axum::http::StatusCode::OK, "Candidate sended!".to_string())
//         }
//     } else {
//         (axum::http::StatusCode::NOT_FOUND, "DeviceNotFound".to_string())
//     }
// }
// async fn post_offer(
//     Path(device_id): Path<String>,
//     State(state): State<Arc<Mutex<AppState>>>,
//     Json(sdp): Json<DeviceOfferDto>,
// ) -> impl IntoResponse{
//     let mut state = state.lock().await;
//     if let Some(ws) = state.connections.get_mut(&device_id){
//         let msg = Cmd::Offer(sdp);
//         if let Err(e) = ws.send(axum::extract::ws::Message::Text(serde_json::to_string(&msg).unwrap())).await{
//             (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
//         } else {
//             (axum::http::StatusCode::OK, "Offer sended!".to_string())
//         }
//     } else {
//         (axum::http::StatusCode::NOT_FOUND, "DeviceNotFound".to_string())
//     }
// }
// async fn post_answer(
//     Path(device_id): Path<String>,
//     State(state): State<Arc<Mutex<AppState>>>,
//     Json(sdp): Json<DeviceAnswerDto>,
// ) -> impl IntoResponse{
//     let mut state = state.lock().await;
//     if let Some(device) = state.connections.get_mut(&device_id){
//         let msg = Cmd::Answer(sdp);
//         if let Err(e) = device.connection.send(axum::extract::ws::Message::Text(serde_json::to_string(&msg).unwrap())).await{
//             (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
//         } else {
//             (axum::http::StatusCode::OK, "Answer sended!".to_string())
//         }
//     } else {
//         (axum::http::StatusCode::NOT_FOUND, "DeviceNotFound".to_string())
//     }
// }
// async fn ws_user_connection_handler(
//     Path(device_id): Path<String>,
//     State(state): State<Arc<Mutex<AppState>>>,
//     ws: WebSocketUpgrade
// ) -> Response{
//     ws.on_upgrade(|ws| handle_ws(ws, device_id, state))
// }
// async fn handle_ws(ws: WebSocket, device_id: String, state: Arc<Mutex<AppState>>){
//     state.lock().await.connections.insert(device_id, Device { connection: ws, cameras: HashSet::new() });    
// }
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