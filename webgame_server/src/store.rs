use chrono::{Utc, DateTime};
use serde::{Serialize, Deserialize};
use async_trait::async_trait;

use std::convert::From;

use crate::protocol::{GameState, GameRecord};
use crate::game::{Game, UniverseGame};

#[async_trait]
pub trait GameStore {
    type GameStateT: GameState;
    // type ItemIterator: Iterator<Item=GameRecord<Self::GameStateT>>;

    fn new( path: &str ) -> Self;
    async fn save(&self, game: &dyn UniverseGame<Self::GameStateT> ) -> bool;
    // fn iter(&self) -> Self::ItemIterator;
}

