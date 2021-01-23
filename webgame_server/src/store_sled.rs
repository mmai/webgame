use std::marker::PhantomData;
use async_trait::async_trait;
use uuid::Uuid;

use sled_extensions::bincode::Tree;
use sled_extensions::DbExt; // for open_bincode_tree
use sled_extensions::Error; // for open_bincode_tree

use crate::protocol::GameState;
use crate::store::{ GameStore, GameRecord };
use crate::game::{Game, UniverseGame};


pub struct SledStore<GameStateType: GameState+Clone> {
    _phantom: PhantomData<GameStateType>,
    games: Tree<GameRecord<GameStateType>>,
}


#[async_trait]
impl<GameStateType: GameState+Clone> GameStore for SledStore<GameStateType> {
    type GameStateT = GameStateType;

    fn new( path: &str ) -> Self {
        let games = sled_extensions::Config::default()
            .path(path).open()
            .expect("Failed to open sled db")
            .open_bincode_tree("games")
            .expect("Failed to open games tree");
        SledStore {
            _phantom: PhantomData,
            games
        }
    }


    async fn save(&self, game: &dyn UniverseGame<GameStateType> ) -> bool {
    // async fn save(&self, game: &dyn UniverseGame<GameStateType> ) -> Result<(), Error> {
        let info = game.get_info();
        println!("Storing {:?}", info);

        let game_state = game.get_state(); // Arc<Mutex<GameState>>
        let game_state = game_state.lock().await; // MutexGuard<GameState>
        let mystate = (*game_state).clone();
        let do_steps = || -> Result<(), Error> {
            self.games.insert(info.game_id.as_bytes(), mystate.into())?;
            Ok(())
        };
        if let Err(_err) = do_steps() { false } else { true }
    }
}
