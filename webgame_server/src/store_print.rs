use std::marker::PhantomData;
use async_trait::async_trait;
use uuid::Uuid;

use crate::protocol::{ GameState, GameRecord };
use crate::store::GameStore;
use crate::game::{Game, UniverseGame};

pub struct PrintStore<GameStateType> {
    _phantom: PhantomData<GameStateType>
}

#[async_trait]
impl<GameStateType: GameState> GameStore for PrintStore<GameStateType> {
// impl<GameStateType> GameStore for PrintStore {
    type GameStateT = GameStateType;

    fn new( path: &str ) -> Self {
        println!("Creating dummy print store with path {}", path);
        PrintStore {
            _phantom: PhantomData
        }
    }

    async fn save(&self, game: &dyn UniverseGame<GameStateType> ) -> bool {
    // fn save<State: GameState>(&self, game: &dyn UniverseGame<GameStateType> ) -> bool {
        println!("Storing {:?}", game.get_info());
        true
    }

    async fn delete(&self, game_id: Uuid ) -> bool {
        println!("Deleting {}", game_id);
        true
    }
}
