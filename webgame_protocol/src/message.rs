use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::game::{GameInfo, GameExtendedInfo, GameRecord, GameState};
use crate::player::PlayerInfo;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command<GamePlayCommand, SetPlayerRoleCommand, GameStateSnapshot, DebugOperation, VariantCommand> {
    Ping,
    Authenticate(AuthenticateCommand),
    SendText(SendTextCommand),
    NewGame(VariantCommand),
    JoinGame(JoinGameCommand),
    LeaveGame,
    MarkReady,
    InviteBot,
    Continue,

    GamePlay(GamePlayCommand),
    SetPlayerRole(SetPlayerRoleCommand),

    DebugUi(DebugUiCommand<GameStateSnapshot>), // Used to send a custom state to a client, allows to quickly view the UI at a given state of the game without having to play all the hands leading to this state.
    DebugGame(DebugGameCommand<DebugOperation>), // Send an operation to the game
    ShowUuid, // get uuid of connected client : for use with debugUi
    ShowServerStatus, // get server infos : active games, players connected...
    ShowServerGames, // get stored games
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolErrorKind {
    /// Client tried to authenticate twice
    AlreadyAuthenticated,
    /// Tried to do something while unauthenticated
    NotAuthenticated,
    /// Client sent in some garbage
    InvalidCommand,
    /// Cannot be done at this time
    BadState,
    /// Something wasn't found
    NotFound,
    /// Invalid input.
    BadInput,
    /// This should never happen.
    InternalError,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolError {
    kind: ProtocolErrorKind,
    message: String,
}

impl ProtocolError {
    pub fn new<S: Into<String>>(kind: ProtocolErrorKind, s: S) -> ProtocolError {
        ProtocolError {
            kind,
            message: s.into(),
        }
    }

    pub fn kind(&self) -> ProtocolErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuthenticateCommand {
    pub nickname: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DebugGameCommand<DebugOperation> {
    pub game_id: Uuid,
    pub operation: DebugOperation,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DebugUiCommand<GameStateSnapshot> {
    pub player_id: Uuid,
    pub snapshot: GameStateSnapshot,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SendTextCommand {
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JoinGameCommand {
    pub join_code: String,
}

// TODO : read https://serde.rs/lifetimes.html

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message<
    GamePlayerStateT,
    GameStateSnapshotT: Send,
    DebugOperationT: Send,
    PlayEventT: Send> {
    Connected,
    Pong,
    ServerStatus(ServerStatus),
    // ServerStoredGames(ServerStoredGames<GamePlayerStateT>),
    Chat(ChatMessage),
    PlayerConnected(GamePlayerStateT),
    PlayerDisconnected(PlayerDisconnectedMessage),
    PregameStarted,
    GameJoined(GameInfo),
    GameLeft,
    Authenticated(PlayerInfo),
    Error(ProtocolError),
    PlayEvent(PlayEventT),
    GameStateSnapshot(GameStateSnapshotT),
    DebugOperation(DebugOperationT)
}

// #[derive(Serialize, Deserialize, Debug, Clone)]
// pub struct ServerStoredGames<StateT: GameState> {
//     pub games: Vec<GameRecord<StateT>>,
// }

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServerStatus {
    pub players: Vec<Uuid>,
    pub games: Vec<GameExtendedInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMessage {
    pub player_id: Uuid,
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayerDisconnectedMessage {
    pub player_id: Uuid,
}
