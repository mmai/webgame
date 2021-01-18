use crate::protocol::GameState;
use store::{ GameStore, GameRecord };


pub struct SledStore {}

impl GameStore for SledStore {
    fn new( path: &str ) -> Self {
        let db = sled_extensions::Config::default()
            .path(path)
            .open()
            .expect("Failed to open sled db");
    }
}

struct Database {
    games: Tree<GameRecord>,
}

