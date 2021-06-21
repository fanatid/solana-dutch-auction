# Dutch auction on Solana blockchain

https://en.wikipedia.org/wiki/Dutch_auction

This not code for production usage, need more tests and knowledge for verification. Just interesting task for learning some things from [Solana](https://solana.com/).

Usage example in [tests/auction.rs](tests/auction.rs) (can be testsed only for target `bpf`):

```
cargo test-bpf --test auction -- --nocapture
```

### Install Solana Tool Suite

Easiest way to install tools is to use `install` tool: https://docs.solana.com/cli/install-solana-cli-tools#use-solanas-install-tool

Unfortunately on M1 I got error for `1.7.1`:

```bash
$ solana-test-validator --help
dyld: Library not loaded: /usr/local/opt/openssl@1.1/lib/libssl.1.1.dylib
  Referenced from: /Users/kirill/.local/share/solana/install/active_release/bin/solana-test-validator
  Reason: image not found
zsh: abort      solana-test-validator --help
``` 

But we also can build binaries from source code:

https://docs.solana.com/cli/install-solana-cli-tools#build-from-source

Before run `cargo-install-all.sh` we need:

- Checkout to `v1.7.1`
- Comment line with `spl-token-cli` installation, because it's not compilable right now due to `ring` crate: https://github.com/solana-labs/solana/blob/v1.7.1/scripts/cargo-install-all.sh#L135
- Cherry-pick code from https://github.com/solana-labs/solana/pull/17632

Still error on M1 on deploying [helloword](https://github.com/solana-labs/example-helloworld) example:

```
Error: Deploying program failed: unable to confirm transaction. This can happen in situations such as transaction expiration and insufficient fee-payer funds
```

We sill would need x86_64 MacOS/Linux for deploying program to local validator 😑.

At least we can run tests on M1.

Unfortunately, little later I was found that my tests are not work on M1 and switched to x86_64 Linux.

### Explore tokens management

First I was need to explore how to work with Tokens in Solana. There was few unclear things mostly related with accounts deriviation and selecting with which keys I should work. But I was able to mint, approve, transfer tokens in [ProgramTest](https://docs.rs/solana-program-test/1.7.2/solana_program_test/struct.ProgramTest.html), as result I added test to [tests/token.rs](tests/token.rs).

### Move time in ProgramTest forward

New day — new problem. Our auction depends from time. I found that we can get time in our program from [Clock](https://docs.rs/solana-program/1.7.2/solana_program/clock/struct.Clock.html) (by calling `Clock::get`, it's possible because `Clock` implement [Sysvar](https://docs.rs/solana-program/1.7.2/solana_program/sysvar/trait.Sysvar.html#method.get) trait). What I was not able to understand is how to forward time in my tests. I even deployed my small progmram to test validator first time and run it first time, for checking that time is changed. For communicating with validator I added binary [bin/rpc-clock.rs](bin/rpc-clock.rs) (it's sad that methods in RPC client is not async 😞). Finally I found that we can move forward with [ProgramTestContext::warp_to_slot](https://docs.rs/solana-program-test/1.7.1/solana_program_test/struct.ProgramTestContext.html#method.warp_to_slot).

### Cross-program invocation

Our contract we will need move Tokens and SOL between accounts. [Calling Between Programs](https://docs.solana.com/developing/programming-model/calling-between-programs) in docs says that we need to use [solana_program::program::invoke](https://docs.rs/solana-program/1.7.2/solana_program/program/fn.invoke.html) function. [solana-program-library](https://github.com/solana-labs/solana-program-library) even have example [cross-program-invocation](https://github.com/solana-labs/solana-program-library/tree/master/examples/rust/cross-program-invocation).

I tried to call `invoke` in same way as I would call it outside smart contract, but my test fail with: `Program id TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA wasn't found in account_infos`. I quickly found that this came from [SyscallStubs](https://docs.rs/solana-program-test/1.7.1/src/solana_program_test/lib.rs.html#260). Looking on example I noticed feature `test-bpf` and tried run my test with `cargo test-bpf`. Error again. There also important note that we are not able to build `bpf` target once have [solana-sdk](https://crates.io/crates/solana-sdk) or other related crate in `[dependencies]` section. We should use workspaces. TIL.

On next day morning I thought what if I pass called program [AccountInfo](https://docs.rs/solana-program/1.7.1/solana_program/account_info/struct.AccountInfo.html) to `invoke`? Success. Looks like I did not read docs carefully and this was really unclear for me that we need pass extra `AccountInfo`.

### Program Derived Address

For simplicity initially I used `spl_toke::transfer_checked` but anybody should able to call our program and program should able to send Tokens. Obviously we can not reveal auction secret key. This lead us to section [Program Derive Address](https://docs.solana.com/developing/programming-model/calling-between-programs#program-derived-addresses). From docs we know that [Pubkey](https://docs.rs/solana-program/1.7.1/solana_program/pubkey/struct.Pubkey.html) have two methods for deriving address: `create_with_seed` and `create_program_address`.

Which we should use? In [solana-program-test](https://docs.rs/solana-program-test/1.7.1/src/solana_program_test/lib.rs.html#314) I found that we need to use `create_program_address`.

Now we need to move Tokens controlled by contract. Initially I tried use derived address as Tokens store. This block me for few hours because derived address is account which control account with Tokens (which can be created with [spl_associated_token_account::create_associated_token_account](https://docs.rs/spl-associated-token-account/1.0.2/spl_associated_token_account/fn.create_associated_token_account.html)). As result we will have 3 accounts: auction account, account for tokens, auction derived account which control account with tokens and store SOL from customers.

And for call transfer with new additional sign in program we need to use [invoke_signed](https://docs.rs/solana-program/1.7.2/solana_program/program/fn.invoke_signed.html).

### Auction

Source code in [src](src) directory.
