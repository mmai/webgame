use chrono::{Utc, DateTime};
use std::collections::BTreeMap;
use std::fmt::Debug;
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::player::{PlayerInfo, PlayerState};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GameInfo {
    pub game_id: Uuid,
    pub join_code: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Variant<VariantParameters> {
    pub parameters: VariantParameters,
}

//Used for server diagnostics
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GameExtendedInfo {
    pub game: GameInfo,
    pub players: Vec<Uuid>
}

//Used for storing
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GameRecord<State: GameState> {
    pub date_updated: DateTime<Utc>,
    pub info: GameInfo,
    #[serde(deserialize_with = "State::deserialize")] //cf. https://github.com/serde-rs/serde/issues/1296
    pub state: State,
}

impl<State: GameState> GameRecord<State> {
    pub fn create(state: State, info: GameInfo) -> Self {
        GameRecord { 
            date_updated: Utc::now(),
            info,
            state
        }
    }
}
// impl<State: GameState> From<State> for GameRecord<State> {
//     fn from(state: State) -> Self {
//         GameRecord { 
//             date_updated: Utc::now(),
//             state
//         }
//     }
// }


//XXX is static lifetime a problem ?
pub trait GameState: Sync+Default+Send+Serialize+DeserializeOwned+Clone+'static { 
    type PlayerPos: Send;
    type PlayerRole;
    type GamePlayerState: PlayerState;
    type Snapshot: GameStateSnapshot;
    type Operation: DebugOperation;
    type VariantParameters;

    fn is_joinable(&self) -> bool;
    fn get_players(&self) -> &BTreeMap<Uuid, Self::GamePlayerState>;
    fn add_player(&mut self, player_info: PlayerInfo) -> Self::PlayerPos; 
    fn remove_player(&mut self, player_id: Uuid) -> bool;
    fn set_player_role(&mut self, player_id: Uuid, role: Self::PlayerRole);
    fn get_player_role(&self, player_id: Uuid) -> Option<Self::PlayerRole>;
    fn player_by_pos(&self, position: Self::PlayerPos) -> Option<&Self::GamePlayerState>;
    fn make_snapshot(&self, player_id: Uuid) -> Self::Snapshot;
    fn set_player_ready(&mut self, player_id: Uuid) -> bool;
    fn update_init_state(&mut self) -> bool;
    fn set_player_not_ready(&mut self, player_id: Uuid);

    fn set_variant(&mut self, variant: Variant<Self::VariantParameters>);
    fn manage_operation(&mut self, operation: Self::Operation);

}

pub trait GameManager<'a, Listener: GameEventsListener<Self::Event>> {
    // type State: GameState;
    type Event; 
    // fn register_listener(&mut self, listener: Arc<RefCell<T>>);
    // fn register_listener(&mut self, listener: std::rc::Weak<dyn GameEventsListener<Self::Event>>);
    fn register_listener(&mut self, listener: &'a mut Listener);
    fn unregister_listener(&mut self, listener: &'a Listener);
    fn emit(&mut self, evt: Self::Event);
}

pub trait GameEventsListener<Event> {
    fn notify(&mut self, event: &Event);
}

pub trait GameStateSnapshot: Debug+Serialize+DeserializeOwned+Send+Sync { }
pub trait DebugOperation: Debug+Serialize+DeserializeOwned+Send+Sync { }
