use std::sync::{Arc, Weak};
use futures::executor::block_on;
use tokio::sync::Mutex;
use serde::Serialize;

use uuid::Uuid;
use std::fmt;

use webgame_protocol::PlayerState;
use crate::protocol::{
    GameInfo, GameExtendedInfo, GameState, GameManager, GameEventsListener, // game
    Message, PlayerDisconnectedMessage, // message
    PlayerInfo, // player
    Variant,
};
use crate::universe::Universe;

//trait utilisé dans le store
pub trait UniverseGame<GameStateType> {
    // fn get_state(&self) -> &GameStateType;
    fn get_state(&self) -> &Arc<Mutex<GameStateType>>;
    fn get_info(&self) -> GameInfo;
}

impl<GameStateType: GameState, PlayEventType: Send+Serialize> UniverseGame<GameStateType> for Game<GameStateType, PlayEventType> {
    // fn get_state(&self) -> &GameStateType {
    fn get_state(&self) -> &Arc<Mutex<GameStateType>> {
        self.state_handle()
    }

    fn get_info(&self) -> GameInfo {
        let info = self.game_info().clone();
        info
    }
}
// fin trait utilisé dans le store

pub struct Game<GameStateType: GameState, PlayEventType> {
    pub id: Uuid,
    join_code: String,
    universe: Weak<Universe<GameStateType, PlayEventType>>,
    game_state: Arc<Mutex<GameStateType>>,
}

impl
    <'gs, GameStateType: GameState, PlayEventType> 
fmt::Debug for Game<GameStateType, PlayEventType> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Game")
         .field("id", &self.id)
         .field("join_code", &self.join_code)
         .finish()
    }
}

impl<'gs, GameStateType: Default+GameState,
    PlayEventType: Send+Serialize> 
    Game<GameStateType, PlayEventType> {

    pub fn new(join_code: String, universe: Arc<Universe<GameStateType, PlayEventType>>, variant: Variant<GameStateType::VariantParameters>) -> Game<GameStateType, PlayEventType> {
        let mut game_state = GameStateType::default();
        game_state.set_variant(variant);
        Game {
            id: Uuid::new_v4(),
            join_code,
            universe: Arc::downgrade(&universe),
            game_state: Arc::new(Mutex::new(game_state)),
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn state_handle(&self) -> &Arc<Mutex<GameStateType>> {
        &self.game_state
    }

    pub async fn manage_operation(&self, operation: GameStateType::Operation) {
        self.game_state.lock().await.manage_operation(operation);
    }

    pub fn join_code(&self) -> &str {
        &self.join_code
    }

    //Used for server diagnostics
    pub async fn game_extended_info(&self) -> GameExtendedInfo {
        let game_state = self.game_state.lock().await;
        let players: Vec<_> = game_state.get_players().keys().cloned().collect();
        GameExtendedInfo {
            game: self.game_info(),
            players
        }
    }

    pub fn game_info(&self) -> GameInfo {
        GameInfo {
            game_id: self.id,
            join_code: self.join_code.to_string(),
        }
    }

    pub async fn is_joinable(&self) -> bool {
        self.game_state.lock().await.is_joinable()
    }

    pub fn universe(&self) -> Arc<Universe<GameStateType, PlayEventType>> {
        self.universe.upgrade().unwrap()
    }

    pub async fn add_player(&self, user_id: Uuid) {
        let universe = self.universe();
        if !universe
            .set_user_game_id(user_id, Some(self.id()))
            .await
        {
            return;
        }

        // TODO: `set_user_game_id` also looks up.
        let user = match universe.get_user(user_id).await {
            Some(user) => user,
            None => return,
        };

        let mut game_state = self.game_state.lock().await;
        let pos = game_state.add_player(user.into());
        let player = game_state.player_by_pos(pos).unwrap().clone();
        drop(game_state);
        self.broadcast(&Message::PlayerConnected(player)).await;
    }

    pub async fn connected_players(&self) -> Vec<Uuid>  {
        let mut connected_ids: Vec<Uuid> = vec![];
        let game_state = self.game_state.lock().await;
        for player in game_state.get_players() {
            let uuid = *player.0;
            if self.universe().user_is_authenticated(uuid).await {
                connected_ids.push(uuid);
            }
        }
        connected_ids
    }

    pub async fn remove_user(&self, user_id: Uuid) {
        self.universe().set_user_game_id(user_id, None).await;

        let mut game_state = self.game_state.lock().await;

        if game_state.remove_player(user_id) {
            drop(game_state);
            self.broadcast(&Message::PlayerDisconnected(PlayerDisconnectedMessage {
                player_id: user_id,
            }))
            .await;
        }

        if self.is_empty().await {
            self.universe().remove_game(self.id()).await;
        }
    }

    pub async fn set_player_not_ready(&self, player_id: Uuid) {
        let mut game_state = self.game_state.lock().await;
        game_state.set_player_not_ready(player_id);
    }

    pub async fn mark_player_ready(&self, player_id: Uuid) -> bool {
        let mut game_state = self.game_state.lock().await;
        game_state.set_player_ready(player_id)
    }

    pub async fn update_init_state(&self) -> bool {
        let mut game_state = self.game_state.lock().await;
        game_state.update_init_state()
    }

    pub async fn broadcast(&self, message: &Message<GameStateType::GamePlayerState, GameStateType::Snapshot, GameStateType::Operation, PlayEventType>) {
        let universe = self.universe();
        let game_state = self.game_state.lock().await;
        for player_id in game_state.get_players().keys().copied() {
            universe.send(player_id, message).await;
        }
        universe.store_state(&self).await;
    }

    pub async fn send(&self, player_id: Uuid, message: &Message<GameStateType::GamePlayerState, GameStateType::Snapshot, GameStateType::Operation, PlayEventType>) {
        self.universe().send(player_id, message).await;
    }

    pub async fn broadcast_current_state(&self) {
        let game_state = self.game_state.lock().await;
        // self.broadcast_state(game_state).await;
        let universe = self.universe();
        for player_id in game_state.get_players().keys().copied() {
            let snapshot = game_state.make_snapshot(player_id);
            universe
                .send(
                    player_id,
                    &Message::GameStateSnapshot(snapshot),
                )
                .await;
        }
    }

    pub async fn broadcast_state(&self, game_state: &GameStateType) {
        let universe = self.universe();
        for player_id in game_state.get_players().keys().copied() {
            let snapshot = game_state.make_snapshot(player_id);
            universe
                .send(
                    player_id,
                    &Message::GameStateSnapshot(snapshot),
                )
                .await;
        }
    }

    pub async fn get_player(&self, player_id: &Uuid) -> Option<PlayerInfo> {
        let mut player: Option<PlayerInfo> = None;

        if let Some(state) = self.game_state.lock().await.get_players().get(player_id) {
            player = Some(state.clone().player());
        }
        player
    }

    pub async fn is_empty(&self) -> bool {
        self.game_state.lock().await.get_players().is_empty()
    }

}
