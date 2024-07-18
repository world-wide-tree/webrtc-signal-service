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
pub enum Cmd{
    Offer(RTCSessionDescription),
    Answer(RTCSessionDescription),
    Candidate(RTCIceCandidate),
    NotSupported
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Msg{
    user: String,
    cmd: Cmd,
}


#[tokio::main]
async fn main() {
    // let state = Arc::new(Mutex::new(AppState::default()));
    let state: Arc<Mutex<HashMap<String, Sender<Msg>>>> = Arc::new(Mutex::new(HashMap::new()));
    let listener = TcpListener::bind("0.0.0.0:3030").await.unwrap();
    let route = Router::new()
        .route("/signaling/:peer_id", get(ws_user_connection_handler))
        // .route("/streamer", get("ws_connection for device"))
        // .route("/candidate", method_router)
        // .route("/offer/:camera_id", post(offer_handler))
        // .route("/offer/:camera_id", get(get_offer))
        // .route("/answer/:camera_id", post(answer_handler))
        // .route("/signal/:camera_id", get(get_answer))
        // .route("/candidate/:camera_id", post(candidate_handler))
        // .route("/candidate/:camera_id", get(get_candidate))
        // .route("/candidate", get(get_all_candidate))
        // .layer(Extension(state))
        .with_state(state)
    ;

    serve(listener,route).await.unwrap();
}

pub async fn ws_user_connection_handler(
    Path(peer_id): Path<String>,
    State(state): State<Arc<Mutex<HashMap<String, Sender<Msg>>>>>,
    ws: WebSocketUpgrade
) -> Response{
    let (tx, rx) = channel(10);
    {
        state.lock().await.insert(peer_id, tx);
    }
    ws.on_upgrade(|ws|handle_user_ws(ws, rx, state))
}

pub async fn handle_user_ws(mut socket: WebSocket, mut rx: Receiver<Msg>, state: Arc<Mutex<HashMap<String, Sender<Msg>>>>){
    let (mut sink,mut stream) = socket.split();
    loop {
        if let Ok(msg) = stream.try_next().await{
            if let Some(msg) = msg{
                let cmd = axum2reqwest_msg_convert(msg).json::<Msg>();
                if let Ok(msg) = cmd{
                    {
                        let mut state = state.lock().await;
                        if let Some(tx) = state.get(&msg.user){
                            let user = msg.user.clone();
                            if let Err(e) = tx.send(msg).await{
                                eprintln!("{e}");
                                state.remove(&user);
                            }
                        }
                    }
                } else {
                    let msg = Cmd::NotSupported;
                    let _ = sink.send(axum::extract::ws::Message::Text(serde_json::to_string(&msg).unwrap())).await;
                    continue;
                }
            } else {
                break;
            }
        }
        if let Ok(cmd) = rx.try_recv(){
            if let Err(e) = sink.send(axum::extract::ws::Message::Text(serde_json::to_string(&cmd).unwrap())).await{
                eprintln!("{e}");
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }    
}

fn axum2reqwest_msg_convert(amsg: axum::extract::ws::Message) -> reqwest_websocket::Message{
    match amsg{
        axum::extract::ws::Message::Binary(b) => {
            reqwest_websocket::Message::Binary(b)
        },
        axum::extract::ws::Message::Text(t) => {
            reqwest_websocket::Message::Text(t)
        },
        axum::extract::ws::Message::Ping(ping) => {
            reqwest_websocket::Message::Ping(ping)
        },
        axum::extract::ws::Message::Pong(pong) => {
            reqwest_websocket::Message::Pong(pong)
        },
        _ => {
            reqwest_websocket::Message::Close { code: reqwest_websocket::CloseCode::Normal, reason: "".to_string() }
        }
    }
}