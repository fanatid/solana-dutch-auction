use solana_program::{
    clock::UnixTimestamp,
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
    system_program, sysvar,
};
use std::convert::TryInto;
use std::mem::size_of;

use crate::error::AuctionError;

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub enum AuctionInstruction {
    // Initialize auction by set auction parameters and transfer tokens for sell.
    // Accounts:
    //  0. `[writeable]` Auction account to initialize.
    //  1. `[]` Auction authority key.
    //  2. `[]` System account
    //  3. `[writeable]` Funding account.
    //  4. `[]` Sysvar Rent account.
    //  5. `[]` `spl-associated-token-account` program account.
    //  6. `[]` Token account.
    //  7. `[]` Token mint account.
    //  8. `[writeable]` Token source account.
    //  9. `[writeable]` Auction associated token account.
    // 10. `[writeable]` Owner of auction associated token account.
    // 11. `[writeable,signer]` Token source account's owner/delegate.
    InitializeAuction {
        token_amount: u64,
        time_start: UnixTimestamp,
        time_step: UnixTimestamp,
        price_start: u64,
        price_step: u64,
    },
    // Attempt to buy Token with SOL.
    // Accounts:
    //  0. `[]` Auction account.
    //  1. `[]` System account.
    //  2. `[writeable]` Funding account.
    //  3. `[]` Token account.
    //  4. `[]` Token mint account.
    //  5. `[writeable]` Auction associated token account.
    //  6. `[writeable]` Owner of auction associated token account.
    //  7. `[writeable]` Customer token account.
    MakeBid {
        token_amount: u64,
    },
    // Withdraw SOL from auction.
    // Accounts:
    //  0. `[]` Auction account.
    //  1. `[signer]` Auction authority key.
    //  2. `[]` System account.
    //  3. `[]` Token mint account.
    //  4. `[]` Owner of auction associated token account.
    //  5. `[writeable]` Destination account.
    WithdrawSOL,
    // Withdraw Tokens from auction when finished.
    // Accounts:
    //  0. `[]` Auction account.
    //  1. `[signer]` Auction authority key.
    //  2. `[]` Token account.
    //  3. `[]` Token mint account.
    //  4. `[writeable]` Auction associated token account.
    //  5. `[]` Owner of auction associated token account.
    //  6. `[writeable]` Destination token account.
    WithdrawTokens,
}

// Would be nice to use `deku` crate for pack/unpack, but it's not available for `bpf` target.
impl AuctionInstruction {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        use AuctionError::InvalidInstruction;

        let (&tag, rest) = input.split_first().ok_or(InvalidInstruction)?;
        let (this, rest) = match tag {
            0 => {
                let (token_amount, rest) = unpack_u64(rest)?;
                let (time_start, rest) = unpack_unix_timestamp(rest)?;
                let (time_step, rest) = unpack_unix_timestamp(rest)?;
                let (price_start, rest) = unpack_u64(rest)?;
                let (price_step, rest) = unpack_u64(rest)?;

                Ok((
                    Self::InitializeAuction {
                        token_amount,
                        time_start,
                        time_step,
                        price_start,
                        price_step,
                    },
                    rest,
                ))
            }
            1 => {
                let (token_amount, rest) = unpack_u64(rest)?;
                Ok((Self::MakeBid { token_amount }, rest))
            }
            2 => Ok((Self::WithdrawSOL, rest)),
            3 => Ok((Self::WithdrawTokens, rest)),
            _ => Err(InvalidInstruction),
        }?;

        if !rest.is_empty() {
            return Err(InvalidInstruction.into());
        }

        Ok(this)
    }

    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match self {
            Self::InitializeAuction {
                token_amount,
                time_start,
                time_step,
                price_start,
                price_step,
            } => {
                buf.push(0);
                buf.extend_from_slice(&token_amount.to_le_bytes());
                buf.extend_from_slice(&time_start.to_le_bytes());
                buf.extend_from_slice(&time_step.to_le_bytes());
                buf.extend_from_slice(&price_start.to_le_bytes());
                buf.extend_from_slice(&price_step.to_le_bytes());
            }
            Self::MakeBid { token_amount } => {
                buf.push(1);
                buf.extend_from_slice(&token_amount.to_le_bytes());
            }
            Self::WithdrawSOL => buf.push(2),
            Self::WithdrawTokens => buf.push(3),
        };
        buf
    }
}

fn unpack_unix_timestamp(input: &[u8]) -> Result<(UnixTimestamp, &[u8]), AuctionError> {
    let (value, rest) = input.split_at(8);
    Ok((
        value
            .try_into()
            .ok()
            .map(UnixTimestamp::from_le_bytes)
            .ok_or(AuctionError::InvalidInstruction)?,
        rest,
    ))
}

fn unpack_u64(input: &[u8]) -> Result<(u64, &[u8]), AuctionError> {
    let (value, rest) = input.split_at(8);
    Ok((
        value
            .try_into()
            .ok()
            .map(u64::from_le_bytes)
            .ok_or(AuctionError::InvalidInstruction)?,
        rest,
    ))
}

pub fn initialize_auction(
    auction_pubkey: &Pubkey,
    auction_authority_pubkey: &Pubkey,
    funding_pubkey: &Pubkey,
    token_pubkey: &Pubkey,
    token_source_pubkey: &Pubkey,
    token_auction_pubkey: &Pubkey,
    token_auction_owner_info: &Pubkey,
    token_authority_pubkey: &Pubkey,
    token_amount: u64,
    time_start: UnixTimestamp,
    time_step: UnixTimestamp,
    price_start: u64,
    price_step: u64,
) -> Result<Instruction, ProgramError> {
    Ok(Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(*auction_pubkey, false),
            AccountMeta::new_readonly(*auction_authority_pubkey, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new(*funding_pubkey, false),
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new_readonly(spl_associated_token_account::id(), false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(*token_pubkey, false),
            AccountMeta::new(*token_source_pubkey, false),
            AccountMeta::new(*token_auction_pubkey, false),
            AccountMeta::new(*token_auction_owner_info, false),
            AccountMeta::new(*token_authority_pubkey, true),
        ],
        data: AuctionInstruction::InitializeAuction {
            token_amount,
            time_start,
            time_step,
            price_start,
            price_step,
        }
        .pack(),
    })
}

pub fn make_bid(
    auction_pubkey: &Pubkey,
    funding_pubkey: &Pubkey,
    token_pubkey: &Pubkey,
    token_auction_pubkey: &Pubkey,
    token_auction_owner_info: &Pubkey,
    token_customer_pubkey: &Pubkey,
    token_amount: u64,
) -> Result<Instruction, ProgramError> {
    Ok(Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(*auction_pubkey, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new(*funding_pubkey, false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(*token_pubkey, false),
            AccountMeta::new(*token_auction_pubkey, false),
            AccountMeta::new(*token_auction_owner_info, false),
            AccountMeta::new(*token_customer_pubkey, false),
        ],
        data: AuctionInstruction::MakeBid { token_amount }.pack(),
    })
}

pub fn withdraw_sol(
    auction_pubkey: &Pubkey,
    auction_authority_pubkey: &Pubkey,
    token_pubkey: &Pubkey,
    token_auction_owner_info: &Pubkey,
    dest_pubkey: &Pubkey,
) -> Result<Instruction, ProgramError> {
    Ok(Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(*auction_pubkey, false),
            AccountMeta::new_readonly(*auction_authority_pubkey, true),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(*token_pubkey, false),
            AccountMeta::new(*token_auction_owner_info, false),
            AccountMeta::new(*dest_pubkey, false),
        ],
        data: AuctionInstruction::WithdrawSOL.pack(),
    })
}

pub fn withdraw_tokens(
    auction_pubkey: &Pubkey,
    auction_authority_pubkey: &Pubkey,
    token_pubkey: &Pubkey,
    token_auction_pubkey: &Pubkey,
    token_auction_owner_info: &Pubkey,
    token_dest_pubkey: &Pubkey,
) -> Result<Instruction, ProgramError> {
    Ok(Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(*auction_pubkey, false),
            AccountMeta::new_readonly(*auction_authority_pubkey, true),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(*token_pubkey, false),
            AccountMeta::new(*token_auction_pubkey, false),
            AccountMeta::new_readonly(*token_auction_owner_info, false),
            AccountMeta::new(*token_dest_pubkey, false),
        ],
        data: AuctionInstruction::WithdrawTokens.pack(),
    })
}
