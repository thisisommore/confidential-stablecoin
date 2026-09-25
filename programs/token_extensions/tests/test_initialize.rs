use litesvm_token::{CreateAccount, CreateMint};
use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{instruction::Instruction, system_program},
        InstructionData, ToAccountMetas,
    },
    anchor_spl::token_2022::ID as TOKEN_PROGRAM_ID,
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

pub fn mint(svm: &mut LiteSVM, payer: &Keypair) -> Pubkey {
    CreateMint::new(svm, payer)
        .decimals(9)
        .authority(&payer.pubkey())
        .send()
        .unwrap()
}

#[test]
fn test_initialize() {
    let program_id = token_extensions::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/token_extensions.so"
    ));
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    let decimals = 9;
    let mint = Keypair::new();

    let instruction = Instruction::new_with_bytes(
        program_id,
        &token_extensions::instruction::CreateMintWithFee {
            decimals: decimals,
            basis_points: 10,
            maximum_fee: 100,
        }
        .data(),
        token_extensions::accounts::CreateMintWithFee {
            payer: payer.pubkey(),
            system_program: system_program::ID,
            mint: mint.pubkey(),
            token_program: TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &mint]).unwrap();

    let res = svm.send_transaction(tx);
    match res {
        Ok(e) => assert!(true),
        Err(e) => {
            println!("{}", e.meta.logs.join("\n"));
            assert!(false);
        }
    };
    // assert!(res.is_ok());

    // let instruction = Instruction::new_with_bytes(
    //     program_id,
    //     &token_extensions::instruction::CreateMintWithFee {
    //         decimals: decimals,
    //         basis_points: 10,
    //         maximum_fee: 100,
    //     }
    //     .data(),
    //     token_extensions::accounts::CreateMintDeclaritive {
    //         payer: payer.pubkey(),
    //         counter,
    //         system_program: system_program::ID,
    //         mint: TOKEN_PROGRAM_ID,
    //         token_program: TOKEN_PROGRAM_ID,
    //     }
    //     .to_account_metas(None),
    // );

    // let blockhash = svm.latest_blockhash();
    // let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    // let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    // let res = svm.send_transaction(tx);
    // assert!(res.is_ok());
}
