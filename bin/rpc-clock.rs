use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::Instruction, pubkey::Pubkey, signature::Signer,
    signer::keypair::read_keypair_file, transaction::Transaction,
};

fn main() {
    let program_id = "HgjMgB2M5X6N2kiWhnGcJnAgANRV6Bwz47i1MUqa4Q2r"
        .parse::<Pubkey>()
        .expect("valid pubkey");

    let config_file = solana_cli_config::CONFIG_FILE
        .clone()
        .expect("config file exists");
    let config = solana_cli_config::Config::load(&config_file).expect("load config");

    let rpc = RpcClient::new(config.json_rpc_url.clone());
    let keypair = read_keypair_file(config.keypair_path).expect("keypair loaded");
    let pubkey = keypair.pubkey();

    let balance = rpc.get_balance(&pubkey).expect("get_balance");
    println!("Account {:?} with balance {:?}", pubkey, balance);

    let recent_blockhash = rpc.get_recent_blockhash().expect("get_recent_blockhash").0;

    let transaction = Transaction::new_signed_with_payer(
        &[Instruction::new_with_bincode(program_id, &[0], vec![])],
        Some(&pubkey),
        &[&keypair],
        recent_blockhash,
    );
    let signature = rpc
        .send_transaction(&transaction)
        .expect("send_transaction");
    println!("Transaction executed: {:?}", signature);
}
