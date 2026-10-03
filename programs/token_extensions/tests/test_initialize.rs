use {
    anchor_lang::prelude::{Clock, Pubkey},
    anchor_spl::token_2022::{
        spl_token_2022::{
            extension::{
                confidential_transfer::instruction::{
                    initialize_mint as initialize_confidential_transfer_mint, ProofContextState,
                    PubkeyValidityProofContext,
                },
                confidential_transfer_fee::instruction::initialize_confidential_transfer_fee_config,
                default_account_state::instruction::initialize_default_account_state,
                metadata_pointer::instruction::initialize as initialize_metadata_pointer,
                transfer_fee::{
                    instruction::{initialize_transfer_fee_config, transfer_checked_with_fee},
                    TransferFeeConfig,
                },
                BaseStateWithExtensions as ABaseStateWithExtensions, ExtensionType,
                StateWithExtensions as AStateWithExtensions,
            },
            instruction::{
                initialize_account3, initialize_mint2, initialize_mint_close_authority,
                initialize_permanent_delegate, mint_to, thaw_account,
            },
            state::{Account as TokenAccount, AccountState, Mint as MintState},
        },
        ID as TOKEN_PROGRAM_ID,
    },
    litesvm::LiteSVM,
    solana_address::Address,
    solana_keypair::Keypair,
    solana_message::Instruction,
    solana_signer::Signer,
    solana_system_interface::instruction::create_account,
    solana_transaction::{versioned::VersionedTransaction, Message, VersionedMessage},
    solana_zk_elgamal_proof_interface::instruction::{ContextStateInfo, ProofInstruction},
    solana_zk_sdk::{
        encryption::{
            auth_encryption::AeKey,
            elgamal::{ElGamalCiphertext, ElGamalKeypair},
        },
        zk_elgamal_proof_program::pubkey_validity::build_pubkey_validity_proof_data,
    },
    spl_token_2022_interface::{
        extension::{
            confidential_transfer::{
                instruction::{apply_pending_balance, approve_account, configure_account, deposit},
                ConfidentialTransferAccount,
            },
            BaseStateWithExtensions, StateWithExtensions,
        },
        state::Account as TokenAccountState,
    },
    spl_token_confidential_transfer_proof_extraction::instruction::ProofLocation,
    std::{fmt::Debug, str::FromStr},
};

struct TestHelper {
    svm: LiteSVM,
    admin: Keypair,
    user: Keypair,
    user_ata: Keypair,
    user2_ata: Keypair,
}

impl Debug for TestHelper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestHelper")
            .field("admin", &self.admin.pubkey())
            .field("user", &self.user.pubkey())
            .field("user_ata", &self.user_ata.pubkey())
            .field("user2_ata", &self.user2_ata.pubkey())
            .finish()
    }
}

impl TestHelper {
    fn new() -> TestHelper {
        let mut h = TestHelper {
            svm: LiteSVM::new(),
            admin: Keypair::new(),
            user: Keypair::new(),
            user_ata: Keypair::new(),
            user2_ata: Keypair::new(),
        };
        println!("{:?}", h);

        h.svm.airdrop(&h.admin.pubkey(), 1_000_000_000).unwrap();

        let space = ExtensionType::try_calculate_account_len::<TokenAccount>(&[
            ExtensionType::TransferFeeAmount,
            ExtensionType::ConfidentialTransferAccount,
            ExtensionType::ConfidentialTransferFeeAmount,
        ])
        .unwrap();
        let rent = h.svm.minimum_balance_for_rent_exemption(space);
        let (user_ata, user2_ata) = (h.user_ata.insecure_clone(), h.user2_ata.insecure_clone());
        h.send(
            &[
                create_account(
                    &h.admin.pubkey(),
                    &h.user_ata.pubkey(),
                    rent,
                    space as u64,
                    &TOKEN_PROGRAM_ID,
                ),
                create_account(
                    &h.admin.pubkey(),
                    &h.user2_ata.pubkey(),
                    rent,
                    space as u64,
                    &TOKEN_PROGRAM_ID,
                ),
            ],
            &[&user_ata, &user2_ata],
        );

        return h;
    }
    fn send(&mut self, ixs: &[Instruction], signers: &[&Keypair]) {
        let blockhash = self.svm.latest_blockhash();
        let msg = Message::new_with_blockhash(&ixs, Some(&self.admin.pubkey()), &blockhash);
        let mut keys = vec![&self.admin];
        keys.extend_from_slice(signers);
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &keys).unwrap();

        let res = self.svm.send_transaction(tx);
        match res {
            Ok(e) => assert!(true),
            Err(e) => {
                println!("{} {}", e.meta.logs.join("\n"), e.err);
                assert!(false);
            }
        };
    }
}
const SUPPORTED_EXTENSIONS: [ExtensionType; 7] = [
    ExtensionType::TransferFeeConfig,
    ExtensionType::MetadataPointer,
    ExtensionType::DefaultAccountState,
    ExtensionType::MintCloseAuthority,
    ExtensionType::ConfidentialTransferMint,
    ExtensionType::ConfidentialTransferFeeConfig,
    ExtensionType::PermanentDelegate,
];
#[test]
fn test_initialize() {
    let mut h = TestHelper::new();
    let program_id = token_extensions::id();

    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/token_extensions.so"
    ));
    h.svm.add_program(program_id, bytes).unwrap();
    let mint = Keypair::new();

    let withdraw_elgamal = ElGamalKeypair::new_rand();
    let withdraw_elgamal_pubkey: [u8; 32] = withdraw_elgamal.pubkey().into();
    let space =
        ExtensionType::try_calculate_account_len::<MintState>(&SUPPORTED_EXTENSIONS).unwrap();
    let rent = h.svm.minimum_balance_for_rent_exemption(space);
    let ixs = [
        create_account(
            &h.admin.pubkey(),
            &mint.pubkey(),
            rent,
            space as u64,
            &TOKEN_PROGRAM_ID,
        ),
        initialize_transfer_fee_config(
            &TOKEN_PROGRAM_ID,
            &mint.pubkey(),
            Some(&h.admin.pubkey()),
            Some(&h.admin.pubkey()),
            100,
            10_000,
        )
        .unwrap(),
        initialize_metadata_pointer(
            &TOKEN_PROGRAM_ID,
            &mint.pubkey(),
            Some(h.admin.pubkey()),
            Some(mint.pubkey()),
        )
        .unwrap(),
        initialize_default_account_state(&TOKEN_PROGRAM_ID, &mint.pubkey(), &AccountState::Frozen)
            .unwrap(),
        initialize_mint_close_authority(&TOKEN_PROGRAM_ID, &mint.pubkey(), Some(&h.admin.pubkey()))
            .unwrap(),
        initialize_confidential_transfer_mint(
            &TOKEN_PROGRAM_ID,
            &mint.pubkey(),
            Some(h.admin.pubkey()),
            false,
            None,
        )
        .unwrap(),
        initialize_confidential_transfer_fee_config(
            &TOKEN_PROGRAM_ID,
            &mint.pubkey(),
            Some(h.admin.pubkey()),
            &withdraw_elgamal_pubkey.into(),
        )
        .unwrap(),
        initialize_permanent_delegate(&TOKEN_PROGRAM_ID, &mint.pubkey(), &h.admin.pubkey())
            .unwrap(),
        initialize_mint2(
            &TOKEN_PROGRAM_ID,
            &mint.pubkey(),
            &h.admin.pubkey(),
            Some(&h.admin.pubkey()),
            9,
        )
        .unwrap(),
        initialize_account3(
            &TOKEN_PROGRAM_ID,
            &h.user_ata.pubkey(),
            &mint.pubkey(),
            &h.user.pubkey(),
        )
        .unwrap(),
        initialize_account3(
            &TOKEN_PROGRAM_ID,
            &h.user2_ata.pubkey(),
            &mint.pubkey(),
            &h.user.pubkey(),
        )
        .unwrap(),
    ];

    h.send(&ixs, &[&mint]);

    let raw = h.svm.get_account(&mint.pubkey()).unwrap();
    let data = raw.data;

    let state = AStateWithExtensions::<MintState>::unpack(&data).unwrap();

    // A transfer fee means the amount credited is not the amount debited.
    // Any accounting that assumes otherwise is wrong against this mint, so
    // read the live fee rather than assuming zero.
    //
    // Fees are epoch scheduled: `newer_transfer_fee` may not be in force
    // yet, which is why `get_epoch_fee` takes the current epoch.

    let mint_amount = 10_0_000;
    let transfer_amount = 10_000;

    let epoch = h.svm.get_sysvar::<Clock>().epoch;
    let fee = match state.get_extension::<TransferFeeConfig>() {
        Ok(config) => config.calculate_epoch_fee(epoch, transfer_amount).unwrap(),
        Err(_) => 0,
    };

    let user_key = h.user.insecure_clone();

    h.send(
        &[
            thaw_account(
                &TOKEN_PROGRAM_ID,
                &h.user_ata.pubkey(),
                &mint.pubkey(),
                &h.admin.pubkey(),
                &[&h.admin.pubkey()],
            )
            .unwrap(),
            thaw_account(
                &TOKEN_PROGRAM_ID,
                &h.user2_ata.pubkey(),
                &mint.pubkey(),
                &h.admin.pubkey(),
                &[&h.admin.pubkey()],
            )
            .unwrap(),
            mint_to(
                &TOKEN_PROGRAM_ID,
                &mint.pubkey(),
                &h.user_ata.pubkey(),
                &h.admin.pubkey(),
                &[&h.admin.pubkey()],
                mint_amount,
            )
            .unwrap(),
        ],
        &[],
    );
    let transfer_ix = transfer_checked_with_fee(
        &TOKEN_PROGRAM_ID,
        &h.user_ata.pubkey(),
        &mint.pubkey(),
        &h.user2_ata.pubkey(),
        &h.admin.pubkey(),
        &[&user_key.pubkey()],
        transfer_amount,
        9,
        fee,
    )
    .unwrap();
    h.send(&[transfer_ix], &[&user_key]);

    let el = ElGamalKeypair::new_rand();
    let aes = AeKey::new_rand();
    let proof_data = build_pubkey_validity_proof_data(&el).unwrap();
    let space = std::mem::size_of::<ProofContextState<PubkeyValidityProofContext>>();
    let proof_rent = h.svm.minimum_balance_for_rent_exemption(space);
    let proof_acc = Keypair::new();
    let admin_key = h.admin.insecure_clone();
    let ZK_PROOF_PROGRAM_ID: Address =
        Address::from_str("ZkE1Gama1Proof11111111111111111111111111111").unwrap();
    h.send(
        &[create_account(
            &admin_key.pubkey(),
            &proof_acc.pubkey(),
            proof_rent,
            space as u64,
            &ZK_PROOF_PROGRAM_ID,
        )],
        &[&proof_acc],
    );
    let pk_bytes: [u8; 32] = el.pubkey().into();
    println!("ELGAMAL PK HEAD: {:?}", &pk_bytes[..8]);
    let verify_ix = ProofInstruction::VerifyPubkeyValidity.encode_verify_proof(
        Some(ContextStateInfo {
            context_state_account: &proof_acc.pubkey(),
            context_state_authority: &h.user.pubkey(),
        }),
        &proof_data,
    );
    h.send(&[verify_ix], &[]);
    h.send(
        &configure_account(
            &TOKEN_PROGRAM_ID,
            &h.user_ata.pubkey(),
            &mint.pubkey(),
            &AeKey::encrypt(&aes, 0).into(),
            65536,
            &user_key.pubkey(),
            &[],
            ProofLocation::ContextStateAccount(&proof_acc.pubkey()),
        )
        .unwrap(),
        &[&user_key],
    );

    h.send(
        &[approve_account(
            &TOKEN_PROGRAM_ID,
            &h.user_ata.pubkey(),
            &mint.pubkey(),
            &admin_key.pubkey(),
            &[&admin_key.pubkey()],
        )
        .unwrap()],
        &[],
    );

    h.send(
        &[deposit(
            &TOKEN_PROGRAM_ID,
            &h.user_ata.pubkey(),
            &mint.pubkey(),
            1000,
            9,
            &user_key.pubkey(),
            &[],
        )
        .unwrap()],
        &[&user_key],
    );

    let raw = h.svm.get_account(&h.user_ata.pubkey()).unwrap();
    let state = StateWithExtensions::<TokenAccountState>::unpack(&raw.data).unwrap();
    let ct = state
        .get_extension::<ConfidentialTransferAccount>()
        .unwrap();

    // pending is split lo(16-bit)/hi(32-bit) for cheap discrete-log decrypt
    let lo: ElGamalCiphertext = ct.pending_balance_lo.try_into().unwrap();
    let hi: ElGamalCiphertext = ct.pending_balance_hi.try_into().unwrap();
    let avail: ElGamalCiphertext = ct.available_balance.try_into().unwrap();
    let total = el.secret().decrypt_u32(&avail).unwrap()
        + el.secret().decrypt_u32(&lo).unwrap()
        + (el.secret().decrypt_u32(&hi).unwrap() << 16);

    let counter: u64 = ct.pending_balance_credit_counter.into();
    let new_decryptable = aes.encrypt(total); // AeCiphertext
    h.send(
        &[apply_pending_balance(
            &TOKEN_PROGRAM_ID,
            &h.user_ata.pubkey(),
            1,
            &new_decryptable.into(),
            &user_key.pubkey(),
            &[],
        )
        .unwrap()],
        &[&user_key],
    );
}
