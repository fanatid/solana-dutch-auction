use num_derive::FromPrimitive;
use solana_program::{decode_error::DecodeError, program_error::ProgramError};
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq, FromPrimitive)]
pub enum AuctionError {
    // 0
    #[error("Already in use")]
    AlreadyInUse,
    #[error("Invalid instruction")]
    InvalidInstruction,
    #[error("Invalid UnixTimestamp")]
    InvalidInitializationTime,
    #[error("Invalid derived auction Token owner address")]
    InvalidAuctionTokenOwnerAddress,
    #[error("Invalid associated auction Token address")]
    InvalidAuctionTokenAddress,
    // 5
    #[error("Auction not started yet")]
    NotStarted,
    #[error("Auction finished")]
    Finished,
    #[error("Everything sold out")]
    EverythingSoldOut,
    #[error("Owner does not match")]
    OwnerMismatch,
    #[error("Auction not finished yet")]
    NotFinished,
}

impl From<AuctionError> for ProgramError {
    fn from(e: AuctionError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
impl<T> DecodeError<T> for AuctionError {
    fn type_of() -> &'static str {
        "AuctionError"
    }
}
