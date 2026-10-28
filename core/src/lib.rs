//! Landfall: the rules engine of a colonial strategy game.
//!
//! The engine has no interface of its own and reads no files. A game is
//! started from `Rules` (see `data/rules.json`) and driven through the
//! methods on `game::Game`; the web front end talks to it through the
//! `landfall_web` crate.

pub mod actions;
pub mod ai;
pub mod assembly;
pub mod colony;
pub mod diplomacy;
pub mod events;
pub mod game;
pub mod mapgen;
pub mod natives;
pub mod naval;
pub mod routes;
pub mod rules;
pub mod sound;
pub mod taxes;
pub mod types;
pub mod world;
