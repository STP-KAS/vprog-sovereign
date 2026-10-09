//! One sovereign app: the rollup shape, with the rules picked by the program.
//!
//! Deposits, actions, a state digest, a settlement, and an exit claim. Two apps do not
//! call each other. Nothing here is a RISC0 proof, a Kaspa transaction, or a covenant.

mod merkle;
mod village;

pub use merkle::{verify, Proof, ACCOUNTS, DEPTH};
pub use village::{Card, Exit, Refuse, Token, VaultView, Village, ORGANISER, STALL};
