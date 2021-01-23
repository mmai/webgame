use chrono::{Utc, DateTime};
use serde::{Serialize, Deserialize};
use async_trait::async_trait;

use std::convert::From;

use crate::protocol::GameState;
use crate::game::{Game, UniverseGame};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GameRecord<State: GameState> {
    date_started: DateTime<Utc>,
    date_updated: DateTime<Utc>,
    #[serde(deserialize_with = "State::deserialize")] //cf. https://github.com/serde-rs/serde/issues/1296
    state: State,
}

impl<State: GameState> From<State> for GameRecord<State> {
    fn from(state: State) -> Self {
        let date_started = Utc::now();//TODO don't update for existing
        let date_updated = Utc::now();
        GameRecord { 
            date_started,
            date_updated,
            state
        }
    }
}

#[async_trait]
pub trait GameStore {
    type GameStateT: GameState;

    fn new( path: &str ) -> Self;
    async fn save(&self, game: &dyn UniverseGame<Self::GameStateT> ) -> bool;
}
