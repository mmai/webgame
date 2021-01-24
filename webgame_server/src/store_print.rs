use std::marker::PhantomData;
use async_trait::async_trait;

use crate::protocol::GameState;
use crate::store::{ GameStore, GameRecord };
use crate::game::{Game, UniverseGame};

pub struct PrintStore<GameStateType> {
    _phantom: PhantomData<GameStateType>
}

#[async_trait]
impl<GameStateType: GameState> GameStore<GameStateType> for PrintStore<GameStateType> {

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
}
