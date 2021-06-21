use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::{Clock, UnixTimestamp},
    entrypoint::ProgramResult,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
};
use spl_associated_token_account::{create_associated_token_account, get_associated_token_address};
use spl_token::{
    instruction::transfer_checked,
    state::{Account, Mint},
};

use crate::{error::AuctionError, instruction::AuctionInstruction, state::Auction};

pub struct Processor {}
impl Processor {
    pub fn process(_program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
        let instruction = AuctionInstruction::unpack(input)?;
        match instruction {
            AuctionInstruction::InitializeAuction {
                token_amount,
                time_start,
                time_step,
                price_start,
                price_step,
            } => Self::process_initialize_auction(
                accounts,
                token_amount,
                time_start,
                time_step,
                price_start,
                price_step,
            ),
            AuctionInstruction::MakeBid { token_amount } => {
                Self::process_bid(accounts, token_amount)
            }
            AuctionInstruction::WithdrawTokens {} => Self::process_withdraw_tokens(accounts),
            AuctionInstruction::WithdrawSOL {} => Self::process_withdraw_sol(accounts),
        }
    }

    pub fn process_initialize_auction(
        accounts: &[AccountInfo],
        token_amount: u64,
        time_start: UnixTimestamp,
        time_step: UnixTimestamp,
        price_start: u64,
        price_step: u64,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();

        if time_start < Clock::get()?.unix_timestamp {
            return Err(AuctionError::InvalidInitializationTime.into());
        }
        if time_step < 0 {
            return Err(AuctionError::InvalidInitializationTime.into());
        }

        let auction_info = next_account_info(account_info_iter)?;
        let auction_authority_info = next_account_info(account_info_iter)?;
        let system_program_info = next_account_info(account_info_iter)?;
        let funder_info = next_account_info(account_info_iter)?;
        let rent_sysvar_info = next_account_info(account_info_iter)?;
        let atoken_program_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;
        let token_info = next_account_info(account_info_iter)?;
        let token_source_info = next_account_info(account_info_iter)?;
        let token_auction_info = next_account_info(account_info_iter)?;
        let token_auction_owner_info = next_account_info(account_info_iter)?;
        let token_authority_info = next_account_info(account_info_iter)?;

        let address = Pubkey::create_program_address(&[auction_info.key.as_ref()], &crate::id());
        if address.as_ref() != Ok(token_auction_owner_info.key) {
            return Err(AuctionError::InvalidAuctionTokenOwnerAddress.into());
        }

        let address = get_associated_token_address(token_auction_owner_info.key, token_info.key);
        if &address != token_auction_info.key {
            return Err(AuctionError::InvalidAuctionTokenAddress.into());
        }

        // Initialize auction
        let mut auction = Auction::unpack_unchecked(&auction_info.data.borrow())?;
        if auction.is_initialized {
            return Err(AuctionError::AlreadyInUse.into());
        }

        auction.is_initialized = true;
        auction.authority = *auction_authority_info.key;
        auction.token = *token_info.key;
        auction.time_start = time_start;
        auction.time_step = time_step;
        auction.price_start = price_start;
        auction.price_step = price_step;

        Auction::pack(auction, &mut auction_info.data.borrow_mut())?;

        // Create derived account for SOL
        invoke_signed(
            &system_instruction::create_account(
                funder_info.key,
                token_auction_owner_info.key,
                Rent::get()?.minimum_balance(0),
                0,
                system_program_info.key,
            ),
            &[
                system_program_info.clone(),
                funder_info.clone(),
                token_auction_owner_info.clone(),
            ],
            &[&[auction_info.key.as_ref()]],
        )?;

        // Create derived account for token
        invoke(
            &create_associated_token_account(
                funder_info.key,
                token_auction_owner_info.key,
                token_info.key,
            ),
            &[
                atoken_program_info.clone(),
                funder_info.clone(),
                token_auction_info.clone(),
                token_auction_owner_info.clone(),
                token_info.clone(),
                system_program_info.clone(),
                token_program_info.clone(),
                rent_sysvar_info.clone(),
            ],
        )?;

        // Move tokens
        let token = Mint::unpack(&token_info.data.borrow())?;
        invoke(
            &transfer_checked(
                token_program_info.key,
                token_source_info.key,
                token_info.key,
                token_auction_info.key,
                token_authority_info.key,
                &[],
                token_amount,
                token.decimals,
            )?,
            &[
                token_program_info.clone(),
                token_info.clone(),
                token_source_info.clone(),
                token_auction_info.clone(),
                token_authority_info.clone(),
            ],
        )?;

        Ok(())
    }

    pub fn process_bid(accounts: &[AccountInfo], token_amount: u64) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();

        let auction_info = next_account_info(account_info_iter)?;
        let system_program_info = next_account_info(account_info_iter)?;
        let funder_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;
        let token_info = next_account_info(account_info_iter)?;
        let token_auction_info = next_account_info(account_info_iter)?;
        let token_auction_owner_info = next_account_info(account_info_iter)?;
        let token_customer_info = next_account_info(account_info_iter)?;

        // Check that auction started
        let auction = Auction::unpack_unchecked(&auction_info.data.borrow())?;
        let (token, current_price) = Self::get_current_price(&auction, token_info)?;
        // Check that auction still live
        let current_price = current_price.ok_or(AuctionError::Finished)?;

        // Check available balance
        let token_auction = Account::unpack_unchecked(&token_auction_info.data.borrow())?;
        if token_auction.amount == 0 {
            return Err(AuctionError::EverythingSoldOut.into());
        }
        let token_amount = token_amount.min(token_auction.amount);

        // Transfer SOL
        invoke(
            &system_instruction::transfer(
                funder_info.key,
                token_auction_owner_info.key,
                token_amount * current_price,
            ),
            &[
                system_program_info.clone(),
                funder_info.clone(),
                token_auction_owner_info.clone(),
            ],
        )?;

        // Transfer Tokens
        invoke_signed(
            &transfer_checked(
                token_program_info.key,
                token_auction_info.key,
                token_info.key,
                token_customer_info.key,
                token_auction_owner_info.key,
                &[],
                token_amount,
                token.decimals,
            )?,
            &[
                token_program_info.clone(),
                token_info.clone(),
                token_auction_info.clone(),
                token_customer_info.clone(),
                token_auction_owner_info.clone(),
            ],
            &[&[auction_info.key.as_ref()]],
        )?;

        Ok(())
    }

    pub fn process_withdraw_tokens(accounts: &[AccountInfo]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();

        let auction_info = next_account_info(account_info_iter)?;
        let auction_authority_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;
        let token_info = next_account_info(account_info_iter)?;
        let token_auction_info = next_account_info(account_info_iter)?;
        let token_auction_owner_info = next_account_info(account_info_iter)?;
        let token_dest_info = next_account_info(account_info_iter)?;

        let auction = Auction::unpack_unchecked(&auction_info.data.borrow())?;
        Self::validate_owner(&auction.authority, auction_authority_info)?;

        // Check that auction finished
        let (token, current_price) = Self::get_current_price(&auction, token_info)?;
        if current_price.is_some() {
            return Err(AuctionError::NotFinished.into());
        }

        let token_auction = Account::unpack_unchecked(&token_auction_info.data.borrow())?;

        // Transfer Tokens
        invoke_signed(
            &transfer_checked(
                token_program_info.key,
                token_auction_info.key,
                token_info.key,
                token_dest_info.key,
                token_auction_owner_info.key,
                &[],
                token_auction.amount,
                token.decimals,
            )?,
            &[
                token_program_info.clone(),
                token_info.clone(),
                token_auction_info.clone(),
                token_dest_info.clone(),
                token_auction_owner_info.clone(),
            ],
            &[&[auction_info.key.as_ref()]],
        )?;

        Ok(())
    }

    pub fn process_withdraw_sol(accounts: &[AccountInfo]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();

        let auction_info = next_account_info(account_info_iter)?;
        let auction_authority_info = next_account_info(account_info_iter)?;
        let system_program_info = next_account_info(account_info_iter)?;
        let token_info = next_account_info(account_info_iter)?;
        let token_auction_owner_info = next_account_info(account_info_iter)?;
        let dest_info = next_account_info(account_info_iter)?;

        let auction = Auction::unpack_unchecked(&auction_info.data.borrow())?;
        Self::validate_owner(&auction.authority, auction_authority_info)?;

        // Check that auction finished
        let (_token, current_price) = Self::get_current_price(&auction, token_info)?;
        if current_price.is_some() {
            return Err(AuctionError::NotFinished.into());
        }

        invoke_signed(
            &system_instruction::transfer(
                token_auction_owner_info.key,
                dest_info.key,
                token_auction_owner_info.lamports(),
            ),
            &[
                system_program_info.clone(),
                token_auction_owner_info.clone(),
                dest_info.clone(),
            ],
            &[&[auction_info.key.as_ref()]],
        )?;

        Ok(())
    }

    fn validate_owner(expected_owner: &Pubkey, owner_account_info: &AccountInfo) -> ProgramResult {
        if expected_owner != owner_account_info.key {
            return Err(AuctionError::OwnerMismatch.into());
        }
        if !owner_account_info.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        Ok(())
    }

    fn get_current_price(
        auction: &Auction,
        token_info: &AccountInfo,
    ) -> Result<(Mint, Option<u64>), ProgramError> {
        let current_time = Clock::get()?.unix_timestamp;
        let token = Mint::unpack(&token_info.data.borrow())?;

        // Check that auction started
        if auction.time_start > current_time {
            return Err(AuctionError::NotStarted.into());
        }

        // Calculate current price and check that auction is not finished
        let steps = (current_time - auction.time_start).div_euclid(auction.time_step);
        let current_price = auction
            .price_start
            .checked_sub(auction.price_step * steps as u64)
            .filter(|v| *v != 0);

        Ok((token, current_price))
    }
}
