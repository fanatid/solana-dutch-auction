use solana_program_test::{processor, tokio, ProgramTest, ProgramTestContext};
use solana_sdk::{
    account::{from_account, Account, ReadableAccount},
    clock::{Clock, UnixTimestamp},
    decode_error::DecodeError,
    instruction::{Instruction, InstructionError},
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
    rent::Rent,
    signature::Signer,
    signer::{keypair::Keypair, signers::Signers},
    system_instruction, sysvar,
    transaction::{Transaction, TransactionError},
    transport::{Result as RpcResult, TransportError},
};
use spl_associated_token_account::{create_associated_token_account, get_associated_token_address};
use spl_token::{
    instruction::{initialize_mint, mint_to_checked, set_authority, AuthorityType},
    state::{Account as TokenAccount, Mint},
};

use dutch_auction::{
    error::AuctionError, instruction as auction_instruction, processor::Processor, state::Auction,
};

const TOKEN_AMOUNT: u64 = 100;
const TOKEN_DECIMALS: u8 = 2;
const TIME_STEP: UnixTimestamp = 60;
const PRICE_START: u64 = 10 * u64::pow(10, 9);
const PRICE_STEP: u64 = 1 * u64::pow(10, 9);

#[tokio::test]
async fn dutch_auction() {
    let token_kp = Keypair::new();
    let (auction_kp, auction_token_owner_pk) = loop {
        let kp = Keypair::new();
        let derived = Pubkey::create_program_address(&[kp.pubkey().as_ref()], &dutch_auction::id());
        if let Ok(pk) = derived {
            break (kp, pk);
        }
    };
    let auction_pk = auction_kp.pubkey();
    let auction_token_pk =
        get_associated_token_address(&auction_token_owner_pk, &token_kp.pubkey());
    let auction_owner_kp = Keypair::new();
    let auction_owner_token_pk =
        get_associated_token_address(&auction_owner_kp.pubkey(), &token_kp.pubkey());
    let customer_kp = Keypair::new();
    let customer_token_pk = get_associated_token_address(&customer_kp.pubkey(), &token_kp.pubkey());

    let program_test = ProgramTest::new(
        "dutch_auction",
        dutch_auction::id(),
        processor!(Processor::process),
    );
    let mut ctx = program_test.start_with_context().await;
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).expect("invalid payer");

    let rent = ctx.banks_client.get_rent().await.expect("get_rent failed");

    // Create token, mint to auction owner and convert to NFT
    create_nft(
        &mut ctx,
        &payer,
        &rent,
        &token_kp,
        &auction_owner_kp.pubkey(),
        &auction_owner_token_pk,
    )
    .await;

    // Create auction, delegate tokens from auction owner to contract, initialize
    auction_initialize(
        &mut ctx,
        &payer,
        &rent,
        &token_kp,
        &auction_kp,
        &auction_token_owner_pk,
        &auction_token_pk,
        &auction_owner_kp,
        &auction_owner_token_pk,
    )
    .await;

    // Place diffrent bids in time
    trade(
        &mut ctx,
        &payer,
        &token_kp,
        &auction_pk,
        &auction_token_owner_pk,
        &auction_token_pk,
        &auction_owner_kp,
        &auction_owner_token_pk,
        &customer_kp,
        &customer_token_pk,
    )
    .await;

    verify(
        &mut ctx,
        &rent,
        &auction_owner_kp.pubkey(),
        &auction_token_owner_pk,
        &customer_token_pk,
    )
    .await;

    // Print data from storage.
    print_account::<Mint>(&mut ctx, "Token", token_kp.pubkey()).await;
    print_account::<Auction>(&mut ctx, "Auction", auction_kp.pubkey()).await;
    // print_account::<EmptyData>(&mut ctx, "Auction token owner", auction_token_owner_pk).await;
    print_account::<TokenAccount>(&mut ctx, "Auction token", auction_token_pk).await;
    print_account::<EmptyData>(&mut ctx, "Auction owner", auction_owner_kp.pubkey()).await;
    print_account::<TokenAccount>(&mut ctx, "Auction owner token", auction_owner_token_pk).await;
    print_account::<TokenAccount>(&mut ctx, "Customer token", customer_token_pk).await;
}

async fn send_tx<T: Signers>(
    ctx: &mut ProgramTestContext,
    instructions: &[Instruction],
    signing_keypairs: &T,
) -> RpcResult<()> {
    let recent_blockhash = ctx.banks_client.get_recent_blockhash().await;
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&ctx.payer.pubkey()),
        signing_keypairs,
        recent_blockhash.expect("get_recent_blockhash failed"),
    );
    ctx.banks_client.process_transaction(transaction).await
}

async fn create_nft(
    ctx: &mut ProgramTestContext,
    payer: &Keypair,
    rent: &Rent,
    token_kp: &Keypair,
    auction_owner_pk: &Pubkey,
    auction_owner_token_pk: &Pubkey,
) {
    let token_authority = Keypair::new();

    // In real application we should place `initialize_mint` instruction in same
    // transaction (from Solana docs).
    send_tx(
        ctx,
        &[system_instruction::create_account(
            &ctx.payer.pubkey(),
            &token_kp.pubkey(),
            rent.minimum_balance(Mint::LEN),
            Mint::LEN as u64,
            &spl_token::id(),
        )],
        &[payer, token_kp],
    )
    .await
    .expect("failed to create token account");

    send_tx(
        ctx,
        &[initialize_mint(
            &spl_token::id(),
            &token_kp.pubkey(),
            &token_authority.pubkey(),
            None,
            TOKEN_DECIMALS,
        )
        .expect("failed to create InitializeMint instruction")],
        &[payer],
    )
    .await
    .expect("failed to initiaze token account");

    send_tx(
        ctx,
        &[create_associated_token_account(
            &payer.pubkey(),
            auction_owner_pk,
            &token_kp.pubkey(),
        )],
        &[payer],
    )
    .await
    .expect("failed to create auction owner account for tokens");

    send_tx(
        ctx,
        &[mint_to_checked(
            &spl_token::id(),
            &token_kp.pubkey(),
            &auction_owner_token_pk,
            &token_authority.pubkey(),
            &[],
            TOKEN_AMOUNT,
            TOKEN_DECIMALS,
        )
        .expect("failed to create MintToChecked instruction")],
        &[payer, &token_authority],
    )
    .await
    .expect("failed to mint tokens into associated auction owner account");

    send_tx(
        ctx,
        &[set_authority(
            &spl_token::id(),
            &token_kp.pubkey(),
            None,
            AuthorityType::MintTokens,
            &token_authority.pubkey(),
            &[],
        )
        .expect("failed to create SetAuthority instruction")],
        &[payer, &token_authority],
    )
    .await
    .expect("failed to convert to NFT");
}

async fn auction_initialize(
    ctx: &mut ProgramTestContext,
    payer: &Keypair,
    rent: &Rent,
    token_kp: &Keypair,
    auction_kp: &Keypair,
    auction_token_owner_pk: &Pubkey,
    auction_token_pk: &Pubkey,
    auction_owner_kp: &Keypair,
    auction_owner_token_pk: &Pubkey,
) {
    // In real application we should place `initialize_account` in same transaction.
    send_tx(
        ctx,
        &[system_instruction::create_account(
            &payer.pubkey(),
            &auction_kp.pubkey(),
            rent.minimum_balance(Auction::LEN),
            Auction::LEN as u64,
            &dutch_auction::id(),
        )],
        &[payer, auction_kp],
    )
    .await
    .expect("failed to create auction account");

    let time_start = get_unix_timestamp(ctx).await;
    send_tx(
        ctx,
        &[auction_instruction::initialize_auction(
            &auction_kp.pubkey(),
            &auction_owner_kp.pubkey(),
            &payer.pubkey(),
            &token_kp.pubkey(),
            auction_owner_token_pk,
            auction_token_pk,
            auction_token_owner_pk,
            &auction_owner_kp.pubkey(),
            TOKEN_AMOUNT,
            time_start + TIME_STEP,
            TIME_STEP,
            PRICE_START,
            PRICE_STEP,
        )
        .expect("failed to create InitializeAuction instruction")],
        &[payer, auction_owner_kp],
    )
    .await
    .expect("failed to initialize auction account");
}

async fn trade(
    ctx: &mut ProgramTestContext,
    payer: &Keypair,
    token_kp: &Keypair,
    auction_pk: &Pubkey,
    auction_token_owner_pk: &Pubkey,
    auction_token_pk: &Pubkey,
    auction_owner_kp: &Keypair,
    auction_owner_token_pk: &Pubkey,
    customer_kp: &Keypair,
    customer_token_pk: &Pubkey,
) {
    send_tx(
        ctx,
        &[create_associated_token_account(
            &payer.pubkey(),
            &customer_kp.pubkey(),
            &token_kp.pubkey(),
        )],
        &[payer],
    )
    .await
    .expect("failed to create customer account for tokens");

    macro_rules! assert_error {
        ($fut:expr, $expected:expr) => {
            let error = $fut.await.expect_err("expected error");
            match error {
                TransportError::TransactionError(TransactionError::InstructionError(
                    _,
                    InstructionError::Custom(x),
                )) => {
                    assert_eq!(
                        AuctionError::decode_custom_error_to_enum(x),
                        Some($expected)
                    );
                }
                other => panic!("Unexpected error: {:?}", other),
            };
        };
    }

    // Should fail because we delayed auction
    assert_error!(
        send_tx(
            ctx,
            &[auction_instruction::make_bid(
                &auction_pk,
                &payer.pubkey(),
                &token_kp.pubkey(),
                auction_token_pk,
                auction_token_owner_pk,
                customer_token_pk,
                1,
            )
            .expect("failed to create MakeBid instruction")],
            &[payer],
        ),
        AuctionError::NotStarted
    );

    move_forward(ctx, TIME_STEP).await;

    // Buy 1 Token
    send_tx(
        ctx,
        &[auction_instruction::make_bid(
            &auction_pk,
            &payer.pubkey(),
            &token_kp.pubkey(),
            auction_token_pk,
            auction_token_owner_pk,
            customer_token_pk,
            1,
        )
        .expect("failed to create MakeBid instruction")],
        &[payer],
    )
    .await
    .expect("failed to make a bid");

    move_forward(ctx, TIME_STEP).await;

    // Cann't withdraw Tokes because auction still alive
    assert_error!(
        send_tx(
            ctx,
            &[auction_instruction::withdraw_tokens(
                &auction_pk,
                &auction_owner_kp.pubkey(),
                &token_kp.pubkey(),
                auction_token_pk,
                auction_token_owner_pk,
                auction_owner_token_pk,
            )
            .expect("failed to create WithdrawToken")],
            &[payer, auction_owner_kp],
        ),
        AuctionError::NotFinished
    );

    // Cann't withdraw SOL because auction still alive
    move_to_next_slot(ctx).await;
    assert_error!(
        send_tx(
            ctx,
            &[auction_instruction::withdraw_sol(
                &auction_pk,
                &auction_owner_kp.pubkey(),
                &token_kp.pubkey(),
                auction_token_owner_pk,
                &auction_owner_kp.pubkey(),
            )
            .expect("failed to create WithdrawToken")],
            &[payer, auction_owner_kp],
        ),
        AuctionError::NotFinished
    );

    // Buy everything rest
    send_tx(
        ctx,
        &[auction_instruction::make_bid(
            &auction_pk,
            &payer.pubkey(),
            &token_kp.pubkey(),
            auction_token_pk,
            auction_token_owner_pk,
            customer_token_pk,
            TOKEN_AMOUNT,
        )
        .expect("failed to create MakeBid instruction")],
        &[payer],
    )
    .await
    .expect("failed to make a bid");

    // Should fail because we sold everything
    assert_error!(
        send_tx(
            ctx,
            &[auction_instruction::make_bid(
                &auction_pk,
                &payer.pubkey(),
                &token_kp.pubkey(),
                auction_token_pk,
                auction_token_owner_pk,
                customer_token_pk,
                1,
            )
            .expect("failed to create MakeBid instruction")],
            &[payer],
        ),
        AuctionError::EverythingSoldOut
    );

    let mut steps = PRICE_START.div_euclid(PRICE_STEP) as i64;
    if PRICE_START.rem_euclid(PRICE_STEP) > 0 {
        steps += 1;
    }
    move_forward(ctx, TIME_STEP * (steps - 1)).await;

    // Auction finished
    assert_error!(
        send_tx(
            ctx,
            &[auction_instruction::make_bid(
                &auction_pk,
                &payer.pubkey(),
                &token_kp.pubkey(),
                auction_token_pk,
                auction_token_owner_pk,
                customer_token_pk,
                1,
            )
            .expect("failed to create MakeBid instruction")],
            &[payer],
        ),
        AuctionError::Finished
    );

    // Withdraw Tokens
    send_tx(
        ctx,
        &[auction_instruction::withdraw_tokens(
            &auction_pk,
            &auction_owner_kp.pubkey(),
            &token_kp.pubkey(),
            auction_token_pk,
            auction_token_owner_pk,
            auction_owner_token_pk,
        )
        .expect("failed to create WithdrawToken")],
        &[payer, auction_owner_kp],
    )
    .await
    .expect("failed to withdraw tokens");

    // Withdraw SOL
    send_tx(
        ctx,
        &[auction_instruction::withdraw_sol(
            &auction_pk,
            &auction_owner_kp.pubkey(),
            &token_kp.pubkey(),
            auction_token_owner_pk,
            &auction_owner_kp.pubkey(),
        )
        .expect("failed to create WithdrawToken")],
        &[payer, auction_owner_kp],
    )
    .await
    .expect("failed to withdraw tokens");
}

async fn verify(
    ctx: &mut ProgramTestContext,
    rent: &Rent,
    auction_owner_kp: &Pubkey,
    auction_token_owner_pk: &Pubkey,
    customer_token_pk: &Pubkey,
) {
    // Verify auction SOL balance
    let fut = ctx.banks_client.get_balance(*auction_token_owner_pk);
    let balance = fut.await.expect("get_balance failed");
    assert_eq!(balance, 0);

    // Verify auction owner balance in SOL
    let fut = ctx.banks_client.get_balance(*auction_owner_kp);
    let balance = fut.await.expect("get_balance failed");
    assert_eq!(
        balance,
        PRICE_START * 1 + (PRICE_START - PRICE_STEP) * (TOKEN_AMOUNT - 1) + rent.minimum_balance(0)
    );

    // Verify Token balance on customer account
    let acc = get_account(ctx, customer_token_pk.clone()).await;
    let data = TokenAccount::unpack(&acc.data()).expect("Valid packed data");
    assert_eq!(data.amount, TOKEN_AMOUNT);
}

async fn get_unix_timestamp(ctx: &mut ProgramTestContext) -> UnixTimestamp {
    let account = ctx
        .banks_client
        .get_account(sysvar::clock::id())
        .await
        .expect("failed to call get_account")
        .expect("Clock sysvar not precent");
    from_account::<Clock, _>(&account)
        .expect("failed to deserialize Clock sysvar")
        .unix_timestamp
}

async fn move_forward(ctx: &mut ProgramTestContext, shift: UnixTimestamp) {
    let required_time = get_unix_timestamp(ctx).await + shift;
    while required_time > get_unix_timestamp(ctx).await {
        move_to_next_slot(ctx).await;
    }
}

async fn move_to_next_slot(ctx: &mut ProgramTestContext) {
    let slot = ctx.banks_client.get_root_slot().await;
    let new_slot = slot.expect("failed to get Slot") + 1;
    ctx.warp_to_slot(new_slot + 1).expect("failed to warp");
}

async fn print_account<T>(ctx: &mut ProgramTestContext, name: &str, key: Pubkey)
where
    T: std::fmt::Debug + IsInitialized + Pack,
{
    println!("---");
    let acc = get_account(ctx, key).await;
    println!("{} address {:?}: {:?}", name, key, acc);
    println!("Unpacked data: {:?}", T::unpack(&acc.data()));
}

async fn get_account(ctx: &mut ProgramTestContext, key: Pubkey) -> Account {
    ctx.banks_client
        .get_account(key)
        .await
        .expect("failed to call get_account")
        .expect("account not found")
}

#[derive(Debug)]
struct EmptyData;

impl IsInitialized for EmptyData {
    fn is_initialized(&self) -> bool {
        true
    }
}

impl Pack for EmptyData {
    const LEN: usize = 0;

    fn pack_into_slice(&self, _dst: &mut [u8]) {}

    fn unpack_from_slice(_src: &[u8]) -> Result<Self, ProgramError> {
        Ok(EmptyData)
    }
}

impl Sealed for EmptyData {}
