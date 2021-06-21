#[cfg(not(feature = "no-entrypoint"))]
pub mod entrypoint;

pub mod error;
pub mod instruction;
pub mod processor;
pub mod state;

solana_program::declare_id!("DutchAuction1111111111111111111111111111111");
