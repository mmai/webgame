use std::marker::PhantomData;

use crate::protocol::GameState;
use crate::store::{ GameStore, GameRecord };
use crate::game::{Game, UniverseGame};

pub struct PrintStore<GameStateType> {
    _phantom: PhantomData<GameStateType>
}

impl<GameStateType: GameState> GameStore for PrintStore<GameStateType> {
// impl<GameStateType> GameStore for PrintStore {
    type GameStateT = GameStateType;

    fn new( path: &str ) -> Self {
        println!("Creating dummy print store with path {}", path);
        PrintStore {
            _phantom: PhantomData
        }
    }

    fn save(&self, game: &dyn UniverseGame<GameStateType> ) -> bool {
    // fn save<State: GameState>(&self, game: &dyn UniverseGame<GameStateType> ) -> bool {
        println!("Storing {:?}", game.get_info());
        true
    }
}
