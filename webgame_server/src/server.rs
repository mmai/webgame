use std::convert::Infallible;
use std::sync::Arc;

use std::pin::Pin;

use std::net::SocketAddr;
use std::fmt::Debug;

use serde::{Serialize, de::DeserializeOwned};
use futures::{FutureExt, StreamExt};
use hyper::{service::make_service_fn, Server};
use tokio::sync::mpsc;
use uuid::Uuid;
use warp::{ws, Filter};

//For keep alive ping pong
// use std::time::Duration;
use crate::protocol::{
    AuthenticateCommand, ChatMessage, ServerStatus, Command, JoinGameCommand, Message, ProtocolError,
    ProtocolErrorKind, SendTextCommand, Variant,
    DebugUiCommand, DebugGameCommand,
    GameState,
};
use crate::universe::Universe;

// see https://users.rust-lang.org/t/how-to-store-async-function-pointer/38343/2
pub type GamePlayHandler<GamePlayCommand, GameStateType, PlayEventT> = fn( Arc<Universe<GameStateType, PlayEventT>>, Uuid, GamePlayCommand ) 
    -> Pin<Box<dyn std::future::Future<Output = Result<(), ProtocolError>>
        + Send // required by non-single-threaded executors
    >>;
pub type SetPlayerRoleHandler<SetPlayerRoleCommand, GameStateType, PlayEventT> = fn( Arc<Universe<GameStateType, PlayEventT>>, Uuid, SetPlayerRoleCommand ) 
    -> Pin<Box<dyn std::future::Future<Output = Result<(), ProtocolError>>
        + Send // required by non-single-threaded executors
    >>;

async fn on_websocket_connect<
    GamePlayCommand: Debug+DeserializeOwned,
    SetPlayerRoleCommand: Debug+DeserializeOwned,
    GameStateType: GameState+Default, PlayEventT:Send+Serialize>(
    universe: Arc<Universe<GameStateType, PlayEventT>>,
    guid_uuid: String,
    ws: ws::WebSocket,
    on_gameplay: GamePlayHandler<GamePlayCommand, GameStateType, PlayEventT>,
    on_setplayerrole: SetPlayerRoleHandler<SetPlayerRoleCommand, GameStateType, PlayEventT>,
    ) 
 where GameStateType::VariantParameters:Debug+DeserializeOwned+Send+Serialize
{ 
    let (user_ws_tx, mut user_ws_rx) = ws.split();
    let (tx, rx) = mpsc::unbounded_channel();

    tokio::task::spawn(rx.forward(user_ws_tx).map(|result| {
        if let Err(e) = result {
            log::error!("websocket send error: {}", e);
        }
    }));

    //Debug
    // let games = universe.show_games().await;
    // log::info!("games before searching {:?}", games);

    // log::info!("uid infos: {}", guid_uuid);
    let uid_elems: Vec<&str> = guid_uuid.split("_").collect();
    let guid = uid_elems[0];
    let uuid = if uid_elems.len() > 1 {
        uid_elems[1]
    } else {
        "none"
    };
    let (user, gameuid) = universe.add_user(tx, guid.into(), uuid.into()).await;
    log::info!("user {:?} connected", user.id);
    if universe.user_is_authenticated(user.id).await {
        universe
            .send(user.id, &Message::Authenticated(user.clone().into()))
            .await;
    }
    if let Some(game_id) = gameuid {
        if let Some(game) = universe.get_game(game_id).await {
            universe
                .send(user.id, &Message::GameJoined(game.game_info()))
                .await;
            game.broadcast_current_state().await;
        }
    }

    //keep alive : send a ping every 50 seconds
    // let when = Duration::from_millis(50000);
    // let interval = tokio::time::interval(when);
    // let task = interval.for_each(move |_| {
    //     let _  = tx.send(Ok(ws::Message::ping(Vec::new())));
    //     Ok(())
    // })
    // .map_err(|e| panic!("delay errored; err={:?}", e));
    // ... or ?
    // let task = loop {
    //     interval.next().await;
    //     tx.send(Ok(ws::Message::ping(Vec::new())));
    //
    // }
    //end keep alive


    while let Some(result) = user_ws_rx.next().await {
        match result {
            Ok(msg) => {
                log::debug!("Got message from websocket: {:?}", &msg);
                if let Err(err) = on_user_message(universe.clone(), user.id, msg, on_gameplay, on_setplayerrole).await {
                    universe.send(user.id, &Message::Error(err)).await;
                }
            }
            Err(e) => {
                log::error!("websocket error(uid={}): {}", user.id, e);
                break;
            }
        }
    }

    on_user_disconnected(universe, user.id).await;
}

async fn on_user_disconnected<GameStateType:GameState+Default, PlayEventT:Send+Serialize>(universe: Arc<Universe<GameStateType, PlayEventT>>, user_id: Uuid) {
    // If all users have disconnected, we remove the game itself
    if let Some(game) = universe.get_user_game(user_id).await {
        // At this point we check if there is only this disconnecting user left
        if game.connected_players().await.len() < 2 {
            universe.remove_game(game.id()).await;
            log::info!("last user disconnecting, closing game");
        }
    }
    universe.remove_user(user_id).await;
    log::info!("user {:#?} disconnected", user_id);
}

async fn on_user_message<
    GamePlayCommand: DeserializeOwned + std::fmt::Debug,
    SetPlayerRoleCommand: DeserializeOwned + std::fmt::Debug, 
    GameStateType:GameState+Default,
    PlayEventT:Send+Serialize,
    >
       (
    universe: Arc<Universe<GameStateType, PlayEventT>>,
    user_id: Uuid,
    msg: ws::Message,
    on_gameplay: GamePlayHandler<GamePlayCommand, GameStateType, PlayEventT>,
    on_setplayerrole: SetPlayerRoleHandler<SetPlayerRoleCommand, GameStateType, PlayEventT>,
) -> Result<(), ProtocolError> 
       where GameStateType::VariantParameters: DeserializeOwned + std::fmt::Debug
{
    if msg.is_ping() {
        // XXX A warp ping. where does it come from ? Whatever, we manage it like our custom pings
        log::error!("received a warp ping: {:?}", msg);
        return on_ping(universe, user_id).await;
    }

    let req_json = match msg.to_str() {
        Ok(text) => text,
        Err(()) => {
            return Err(ProtocolError::new(
                ProtocolErrorKind::InvalidCommand,
                "not a valid text frame",
            ))
        }
    };

    let cmd: Command<GamePlayCommand, SetPlayerRoleCommand, GameStateType::Snapshot, GameStateType::Operation, Variant<GameStateType::VariantParameters>> = match serde_json::from_str(&req_json) {
        Ok(req) => req,
        Err(err) => {
            log::debug!("error parsing json {}", err);
            return Err(ProtocolError::new(
                ProtocolErrorKind::InvalidCommand,
                err.to_string(),
            ));
        }
    };

    log::debug!("command: {:?}", &cmd);

    if !universe.user_is_authenticated(user_id).await {
        match cmd {
            Command::Authenticate(data) => on_player_authenticate(universe, user_id, data).await,

            //For debug purposes only
            Command::ShowServerStatus => on_server_status(universe, user_id).await,
            Command::ShowUuid => on_show_uuid(universe, user_id).await,
            Command::DebugUi(data) => on_debug_ui(universe, data).await,
            Command::DebugGame(data) => on_debug_game(universe, data).await,

            _ => Err(ProtocolError::new(
                ProtocolErrorKind::NotAuthenticated,
                "cannot perform this command unauthenticated",
            )),
        }
    } else {
        match cmd {
            Command::Ping => on_ping(universe, user_id).await,

            Command::NewGame(variant) => on_new_game(universe, user_id, variant).await,
            Command::JoinGame(cmd) => on_join_game(universe, user_id, cmd).await,
            Command::MarkReady => on_player_mark_ready(universe, user_id).await,
            Command::LeaveGame => on_leave_game(universe, user_id).await,

            Command::Continue => on_player_continue(universe, user_id).await,
            Command::SendText(cmd) => on_user_send_text(universe, user_id, cmd).await,

            Command::SetPlayerRole(cmd) => on_setplayerrole(universe, user_id, cmd).await,
            Command::GamePlay(cmd) => on_gameplay(universe, user_id, cmd).await,
            //For debug purposes only
            Command::ShowUuid => on_show_uuid(universe, user_id).await,
            Command::DebugUi(data) => on_debug_ui(universe, data).await,
            Command::DebugGame(data) => on_debug_game(universe, data).await,
            Command::ShowServerStatus => on_server_status(universe, user_id).await,

            // this should not happen here.
            Command::Authenticate(..) => Err(ProtocolError::new(
                ProtocolErrorKind::AlreadyAuthenticated,
                "cannot authenticate twice",
            )),
        }
    }
}


async fn on_new_game<'de, GameStateType, PlayEventT>(universe: Arc<Universe<GameStateType, PlayEventT>>, user_id: Uuid, variant: Variant<GameStateType::VariantParameters>) -> Result<(), ProtocolError> 
where 
    GameStateType:      GameState+Default,
    PlayEventT:         Send+Serialize
{
    universe.remove_user_from_game(user_id).await;
    let game = universe.new_game(variant).await;
    game.add_player(user_id).await;
    universe
        .send(user_id, &Message::GameJoined(game.game_info()))
        .await;
    game.broadcast_current_state().await;
    Ok(())
}

// async fn on_new_game<'de, GameStateType:GameState+Default, GamePlayerStateT:PlayerState, GameStateSnapshotT:GameStateSnapshot, DebugOperationT:DebugOperation, PlayEventT:Send+Serialize, VariantParameters>(universe: Arc<Universe<GameStateType, PlayEventT>>, user_id: Uuid, variant: Variant<VariantParameters>) -> Result<(), ProtocolError> {
//     universe.remove_user_from_game(user_id).await;
//     let game = universe.new_game(variant).await;
//     game.add_player(user_id).await;
//     universe
//         .send(user_id, &Message::GameJoined(game.game_info()))
//         .await;
//     game.broadcast_current_state().await;
//     Ok(())
// }

async fn on_join_game<'de, GameStateType:GameState+Default, PlayEventT:Send+Serialize>(
    universe: Arc<Universe<GameStateType, PlayEventT>>,
    user_id: Uuid,
    cmd: JoinGameCommand,
) -> Result<(), ProtocolError> {
    let game = universe.join_game(user_id, cmd.join_code).await?;
    universe
        .send(user_id, &Message::GameJoined(game.game_info()))
        .await;
    game.broadcast_current_state().await;
    Ok(())
}

async fn on_leave_game<'de, GameStateType:GameState+Default, PlayEventT:Send+Serialize>(universe: Arc<Universe<GameStateType, PlayEventT>>, user_id: Uuid) -> Result<(), ProtocolError> {
    log::info!(
        "player {:?} leaving game",
        user_id
    );
    universe.remove_user_from_game(user_id).await;
    universe.send(user_id, &Message::GameLeft).await;
    Ok(())
}

async fn on_ping<'de, GameStateType:GameState+Default, PlayEventT:Send+Serialize>(
    universe: Arc<Universe<GameStateType, PlayEventT>>,
    user_id: Uuid,
) -> Result<(), ProtocolError> {
    universe
        .send(user_id, &Message::Pong)
        .await;
    Ok(())
}

async fn on_show_uuid<'de, GameStateType:GameState+Default, PlayEventT:Send+Serialize>(
    universe: Arc<Universe<GameStateType, PlayEventT>>,
    user_id: Uuid,
) -> Result<(), ProtocolError> {
    let pid = universe.show_users(user_id).await[0];
    universe
        .send(user_id, &Message::Chat(ChatMessage { player_id:pid, text:String::new() }))
        .await;
    Ok(())
}

async fn on_server_status<'de, GameStateType:GameState+Default, PlayEventT:Send+Serialize>(
    universe: Arc<Universe<GameStateType, PlayEventT>>,
    user_id: Uuid,
) -> Result<(), ProtocolError> {
    let players = universe.show_users(user_id).await;
    let games = universe.show_games().await;
    universe
        .send(user_id, &Message::ServerStatus(ServerStatus { players, games }))
        .await;
    Ok(())
}

async fn on_debug_ui<'de, GameStateType:GameState+Default, PlayEventT:Send+Serialize>(
    universe: Arc<Universe<GameStateType, PlayEventT>>,
    cmd: DebugUiCommand<GameStateType::Snapshot>,
) -> Result<(), ProtocolError> {
    universe
        .send(cmd.player_id, &Message::GameStateSnapshot(cmd.snapshot))
        .await;
    Ok(())
}

async fn on_debug_game<'de, GameStateType:GameState+Default, PlayEventT:Send+Serialize>(
    universe: Arc<Universe<GameStateType, PlayEventT>>,
    cmd: DebugGameCommand<GameStateType::Operation>,
) -> Result<(), ProtocolError> {

    if let Some(game) = universe.get_game(cmd.game_id).await {
        universe
            .debug_game(cmd.game_id, cmd.operation)
            .await?;
        game.broadcast_current_state().await;
    }

    Ok(())
}

async fn on_player_authenticate<'de, GameStateType:GameState+Default, PlayEventT:Send+Serialize>(
    universe: Arc<Universe<GameStateType, PlayEventT>>,
    user_id: Uuid,
    cmd: AuthenticateCommand,
) -> Result<(), ProtocolError> {
    let nickname = cmd.nickname.trim().to_owned();
    if nickname.is_empty() || nickname.len() > 16 {
        return Err(ProtocolError::new(
            ProtocolErrorKind::BadInput,
            "nickname must be between 1 and 16 characters",
        ));
    }

    let player_info = universe.authenticate_user(user_id, nickname).await?;
    log::info!(
        "player {:?} authenticated as {:?}",
        user_id,
        &player_info.nickname
    );

    universe
        .send(user_id, &Message::Authenticated(player_info.clone().into()))
        .await;

    Ok(())
}

//XXX obsolete ?
pub async fn on_player_continue<'de, GameStateType:GameState+Default, PlayEventT:Send+Serialize>(
    universe: Arc<Universe<GameStateType, PlayEventT>>,
    user_id: Uuid,
) -> Result<(), ProtocolError> {
    if let Some(game) = universe.get_user_game(user_id).await {
        game.mark_player_ready(user_id).await;
        game.broadcast_current_state().await;
    }
    Ok(())
}

pub async fn on_player_mark_ready<'de, GameStateType:GameState+Default, PlayEventT:Send+Serialize>(
    universe: Arc<Universe<GameStateType, PlayEventT>>,
    user_id: Uuid,
) -> Result<(), ProtocolError> {
    if let Some(game) = universe.get_user_game(user_id).await {
        if game.is_joinable().await {
            let mut needs_update = game.mark_player_ready(user_id).await;
            game.broadcast_current_state().await;
            // This loop is used to broacast several states during init phase
            while needs_update {
                needs_update = game.update_init_state().await;
                game.broadcast_current_state().await;
            }
        }
    }
    Ok(())
}

pub async fn on_user_send_text<'de, GameStateType:GameState+Default, PlayEventT:Send+Serialize>(
    universe: Arc<Universe<GameStateType, PlayEventT>>,
    user_id: Uuid,
    cmd: SendTextCommand,
) -> Result<(), ProtocolError> {
    if let Some(game) = universe.get_user_game(user_id).await {
        game.broadcast(&Message::Chat(ChatMessage {
            player_id: user_id,
            text: cmd.text,
        }))
        .await;
        Ok(())
    } else {
        Err(ProtocolError::new(
            ProtocolErrorKind::BadState,
            "not in a game",
        ))
    }
}

pub async fn serve<GamePlayCommand: Send+Debug+DeserializeOwned+'static, SetPlayerRoleCommand: Send+Debug+DeserializeOwned+'static,
GameStateType:GameState+'static, PlayEventT:Serialize+Send+Sync+'static> (
    public_dir: String,
    db_uri: &str,
    socket: SocketAddr,
    on_gameplay: GamePlayHandler<GamePlayCommand, GameStateType, PlayEventT>,
    on_setplayerrole: SetPlayerRoleHandler<SetPlayerRoleCommand, GameStateType, PlayEventT>
) 
where GameStateType::VariantParameters:Serialize+Debug+DeserializeOwned+Send+Sync+'static
{
    let universe = Arc::new(Universe::new(db_uri));
    let make_svc = make_service_fn(move |_| {
        let universe = universe.clone();
        let pdir = public_dir.clone();

        let routes = warp::path("ws") // Websockets on /ws entry point
            .and(warp::ws())
            .and(warp::path::param()) // enable params on websocket : ws/monparam
            .and(warp::any().map(move || universe.clone()))
            .and(warp::any().map(move || on_gameplay))
            .and(warp::any().map(move || on_setplayerrole))
            .map(|ws: warp::ws::Ws,
                guid_uuid,
                universe: Arc<Universe<GameStateType, PlayEventT>>,
                on_gameplay: GamePlayHandler<GamePlayCommand, GameStateType, PlayEventT>,
                on_setplayerrole: SetPlayerRoleHandler<SetPlayerRoleCommand, GameStateType, PlayEventT>
                | {
                // when the connection is upgraded to a websocket
                ws.on_upgrade(move |ws| on_websocket_connect(universe, guid_uuid, ws, on_gameplay, on_setplayerrole))
            })
        // .or(warp::fs::dir("public/")); // Static files
        .or(warp::fs::dir(pdir)); // Static files
        let svc = warp::service(routes);
        async move { Ok::<_, Infallible>(svc) }
    });

    let mut listenfd = listenfd::ListenFd::from_env();
    let server = if let Some(l) = listenfd.take_tcp_listener(0).unwrap() {
        Server::from_tcp(l).unwrap()
    } else {
        Server::bind(&socket)
    };
    server.serve(make_svc).await.unwrap();
}
