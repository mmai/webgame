use std::marker::PhantomData;
use async_trait::async_trait;
use uuid::Uuid;

use sled_extensions::bincode::Tree;
use sled_extensions::DbExt; // for open_bincode_tree
use sled_extensions::Error; // for open_bincode_tree

use crate::protocol::{ GameState, GameRecord };
use crate::store::GameStore;
use crate::game::{Game, UniverseGame};


pub struct SledStore<GameStateType: GameState+Clone> {
    _phantom: PhantomData<GameStateType>,
    games: Tree<GameRecord<GameStateType>>,
}

impl<GameStateType: GameState+Clone> SledStore<GameStateType> {
    pub fn data(&self) -> &sled_extensions::structured::Tree<GameRecord<GameStateType>, sled_extensions::bincode::BincodeEncoding> {
        &self.games
    }
}

// impl<GameStateType: GameState> IntoIterator for SledStore<GameStateType> {
//     type Item = GameRecord<GameStateType>;
//     // type Item = sled::Result<(sled::IVec, sled::IVec)>;
//     type IntoIter = sled_extensions::structured::Iter<Self::Item, sled_extensions::bincode::BincodeEncoding>;
//
//     fn into_iter(self) -> Self::IntoIter {
//         self.games.into_iter()
//     }
// }

#[async_trait]
impl<GameStateType: GameState+Clone> GameStore for SledStore<GameStateType> {
    type GameStateT = GameStateType;
    // type ItemIterator = sled::Iter;
    //
    // fn iter(&self) -> Self::ItemIterator {
    //     self.games.iter()
    // }

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
        // println!("Storing {:?}", info);

        let game_state = game.get_state(); // Arc<Mutex<GameState>>
        let game_state = game_state.lock().await; // MutexGuard<GameState>
        let mystate = (*game_state).clone();
        let do_steps = || -> Result<(), Error> {
            // self.games.insert(info.game_id.as_bytes(), mystate.into())?;
            self.games.insert(info.game_id.as_bytes(), GameRecord::create(mystate, info.clone()))?;
            Ok(())
        };
        if let Err(_err) = do_steps() { false } else { true }
    }

    async fn delete(&self, game_id: Uuid) -> bool {
        let res = self.games.remove(game_id.as_bytes());
        if let Err(_err) = res { false } else { true }
    }

}
