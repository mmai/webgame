use chrono::{Utc, DateTime};

use crate::protocol::GameState;
use crate::game::{Game, UniverseGame};

pub struct GameRecord<State: GameState> {
    date_started: DateTime<Utc>,
    date_updated: DateTime<Utc>,
    state: State,
}

pub trait GameStore {
    type GameStateT: GameState;

    fn new( path: &str ) -> Self;
    fn save(&self, game: &dyn UniverseGame<Self::GameStateT> ) -> bool;
}
