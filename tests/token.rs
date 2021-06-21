use solana_program_test::{tokio, ProgramTest};
use solana_sdk::{
    account::ReadableAccount, program_pack::Pack, signature::Signer, signer::keypair::Keypair,
    system_instruction, transaction::Transaction,
};
use spl_associated_token_account::{create_associated_token_account, get_associated_token_address};
use spl_token::{
    instruction::{
        approve_checked, initialize_mint, mint_to_checked, set_authority, transfer_checked,
        AuthorityType,
    },
    state::{Account, Mint},
};

#[tokio::test]
async fn nft_mint_transfer() {
    let token_decimals = 0;
    let token = Keypair::new();
    let token_authority = Keypair::new();
    let token_owner1 = Keypair::new();
    let token_owner2 = Keypair::new();
    let token_owner3 = Keypair::new();
    let token_delegate = Keypair::new();

    let token_account1 = get_associated_token_address(&token_owner1.pubkey(), &token.pubkey());
    let token_account2 = get_associated_token_address(&token_owner2.pubkey(), &token.pubkey());
    let token_account3 = get_associated_token_address(&token_owner3.pubkey(), &token.pubkey());

    let program_test = ProgramTest::default();
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    let rent = banks_client.get_rent().await.expect("failed to get a Rent");

    let transaction = Transaction::new_signed_with_payer(
        &[
            // Create token
            system_instruction::create_account(
                &payer.pubkey(),
                &token.pubkey(),
                rent.minimum_balance(Mint::LEN),
                Mint::LEN as u64,
                &spl_token::id(),
            ),
            initialize_mint(
                &spl_token::id(),
                &token.pubkey(),
                &token_authority.pubkey(),
                None,
                token_decimals,
            )
            .expect("create InitializeMint instruction"),
            // Create an account for holding tokens
            create_associated_token_account(
                &payer.pubkey(),
                &token_owner1.pubkey(),
                &token.pubkey(),
            ),
            // Mint token into account
            mint_to_checked(
                &spl_token::id(),
                &token.pubkey(),
                &token_account1,
                &token_authority.pubkey(),
                &[],
                5, // amount
                token_decimals,
            )
            .expect("create MintToChecked instruction"),
            // Disable future minting
            set_authority(
                &spl_token::id(),
                &token.pubkey(),
                None,
                AuthorityType::MintTokens,
                &token_authority.pubkey(),
                &[],
            )
            .expect("create SetAuthority instruction"),
        ],
        Some(&payer.pubkey()),
        &[&payer, &token, &token_authority],
        recent_blockhash,
    );
    banks_client
        .process_transaction(transaction)
        .await
        .expect("failed create a token and mint tokens");

    let transaction = Transaction::new_signed_with_payer(
        &[
            // Create an account for holding tokens
            create_associated_token_account(
                &payer.pubkey(),
                &token_owner2.pubkey(),
                &token.pubkey(),
            ),
            // Transfer 1 token
            transfer_checked(
                &spl_token::id(),
                &token_account1,
                &token.pubkey(),
                &token_account2,
                &token_owner1.pubkey(),
                &[],
                1,
                token_decimals,
            )
            .expect("create TransferChecked instruction"),
        ],
        Some(&payer.pubkey()),
        &[&payer, &token_owner1],
        recent_blockhash,
    );
    banks_client
        .process_transaction(transaction)
        .await
        .expect("failed transfer tokens");

    let transaction = Transaction::new_signed_with_payer(
        &[
            // Approve 1 token
            approve_checked(
                &spl_token::id(),
                &token_account1,
                &token.pubkey(),
                &token_delegate.pubkey(),
                &token_owner1.pubkey(),
                &[],
                2,
                token_decimals,
            )
            .expect("crate ApproveChecked instruction"),
        ],
        Some(&payer.pubkey()),
        &[&payer, &token_owner1],
        recent_blockhash,
    );
    banks_client
        .process_transaction(transaction)
        .await
        .expect("failed delegate  tokens");

    let transaction = Transaction::new_signed_with_payer(
        &[
            // Create an account for holding tokens
            create_associated_token_account(
                &payer.pubkey(),
                &token_owner3.pubkey(),
                &token.pubkey(),
            ),
            // Transfer 1 token
            transfer_checked(
                &spl_token::id(),
                &token_account1,
                &token.pubkey(),
                &token_account3,
                &token_delegate.pubkey(),
                &[],
                1,
                token_decimals,
            )
            .expect("create TransferChecked instruction"),
        ],
        Some(&payer.pubkey()),
        &[&payer, &token_delegate],
        recent_blockhash,
    );
    banks_client
        .process_transaction(transaction)
        .await
        .expect("failed transfer tokens");

    //
    let acc_token = banks_client
        .get_account(token.pubkey())
        .await
        .expect("get_account")
        .expect("not found");
    println!("Token: {:?} {:?}", token.pubkey(), acc_token);
    println!("Data: {:?}", Mint::unpack(&acc_token.data()));

    println!("Token authority: {:?}", token_authority.pubkey());

    println!("Token owner1: {:?}", token_owner1.pubkey());
    let acc_token_account1 = banks_client
        .get_account(token_account1)
        .await
        .expect("get_account")
        .expect("not found");
    println!(
        "Token account1: {:?} {:?}",
        token_account1, acc_token_account1
    );
    println!("Data: {:?}", Account::unpack(&acc_token_account1.data()));

    println!("Token owner2: {:?}", token_owner2.pubkey());
    let acc_token_account2 = banks_client
        .get_account(token_account2)
        .await
        .expect("get_account")
        .expect("not found");
    println!(
        "Token account2: {:?} {:?}",
        token_account2, acc_token_account2
    );
    println!("Data: {:?}", Account::unpack(&acc_token_account2.data()));

    println!("Token owner3: {:?}", token_owner3.pubkey());
    let acc_token_account3 = banks_client
        .get_account(token_account3)
        .await
        .expect("get_account")
        .expect("not found");
    println!(
        "Token account2: {:?} {:?}",
        token_account3, acc_token_account3
    );
    println!("Data: {:?}", Account::unpack(&acc_token_account3.data()));
}
