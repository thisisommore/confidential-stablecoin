use {
    anchor_lang::{
        prelude::{Clock, Pubkey},
        Key,
    },
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
            solana_zk_sdk::encryption::pod::auth_encryption::PodAeCiphertext,
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
            auth_encryption::{AeCiphertext, AeKey},
            elgamal::{ElGamalCiphertext, ElGamalKeypair, ElGamalPubkey},
        },
        zk_elgamal_proof_program::pubkey_validity::build_pubkey_validity_proof_data,
    },
    spl_token_2022_interface::{
        extension::{
            confidential_transfer::{
                instruction::{
                    apply_pending_balance, approve_account, configure_account, deposit,
                    transfer_with_fee, withdraw,
                },
                ConfidentialTransferAccount,
            },
            BaseStateWithExtensions, StateWithExtensions,
        },
        state::Account as TokenAccountState,
    },
    spl_token_confidential_transfer_proof_extraction::instruction::ProofLocation,
    spl_token_confidential_transfer_proof_generation::{
        transfer_with_fee::transfer_with_fee_split_proof_data, withdraw::withdraw_proof_data,
    },
    std::{
        fmt::Debug,
        num::{NonZero, NonZeroI8},
        str::FromStr,
    },
};

struct TestHelper {
    svm: LiteSVM,
    mint: Keypair,
    payer: Keypair,
}

struct User {
    pub key: Keypair,
    pub elgamal: ElGamalKeypair,
    pub aes: AeKey,
    pub ata: Keypair,
}

impl User {
    fn pubkey(&self) -> Pubkey {
        self.key.pubkey()
    }
}

impl User {
    fn new() -> User {
        User {
            key: Keypair::new(),
            elgamal: ElGamalKeypair::new_rand(),
            aes: AeKey::new_rand(),
            ata: Keypair::new(),
        }
    }
}

struct Mint<'info> {
    key: Keypair,
    helper: &'info mut TestHelper,
    decimals: u8,
}

impl Debug for TestHelper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestHelper")
            .field("mint", &self.mint.pubkey())
            .finish()
    }
}

impl TestHelper {
    fn ata(&mut self, user: &User) {
        let space = ExtensionType::try_calculate_account_len::<TokenAccount>(&[
            ExtensionType::TransferFeeAmount,
            ExtensionType::ConfidentialTransferAccount,
            ExtensionType::ConfidentialTransferFeeAmount,
        ])
        .unwrap();
        self.new_account_from_key(space, &TOKEN_PROGRAM_ID, &user.ata);
        self.send(
            &[initialize_account3(
                &TOKEN_PROGRAM_ID,
                &user.ata.pubkey(),
                &self.mint.pubkey(),
                &user.pubkey(),
            )
            .unwrap()],
            &[],
        );
    }
    fn thaw(&mut self, user: &User, admin: &User) {
        self.send(
            &[thaw_account(
                &TOKEN_PROGRAM_ID,
                &user.ata.pubkey(),
                &self.mint.pubkey(),
                &admin.pubkey(),
                &[&admin.pubkey()],
            )
            .unwrap()],
            &[&admin.key],
        )
    }
    fn transfer(&mut self, user_src: &User, user_to: &User, admin: &User, transfer_amount: u64) {
        let raw = self.svm.get_account(&self.mint.pubkey()).unwrap();
        let state = AStateWithExtensions::<MintState>::unpack(&raw.data).unwrap();
        let epoch = self.svm.get_sysvar::<Clock>().epoch;
        let fee = match state.get_extension::<TransferFeeConfig>() {
            Ok(config) => config.calculate_epoch_fee(epoch, transfer_amount).unwrap(),
            Err(_) => 0,
        };

        let transfer_ix = transfer_checked_with_fee(
            &TOKEN_PROGRAM_ID,
            &user_src.ata.pubkey(),
            &self.mint.pubkey(),
            &user_to.ata.pubkey(),
            &user_src.pubkey(),
            &[&user_src.pubkey()],
            transfer_amount,
            9,
            fee,
        )
        .unwrap();
        self.send(&[transfer_ix], &[&user_src.key]);
    }
    fn enable_ct(&mut self, user: &User, admin: &User) {
        let el = &user.elgamal;
        let aes = &user.aes;
        let proof_data = build_pubkey_validity_proof_data(el).unwrap();
        let space = std::mem::size_of::<ProofContextState<PubkeyValidityProofContext>>();
        let proof_rent = self.svm.minimum_balance_for_rent_exemption(space);
        let admin_key = admin.key.insecure_clone();
        let ZK_PROOF_PROGRAM_ID: Address =
            Address::from_str("ZkE1Gama1Proof11111111111111111111111111111").unwrap();

        let proof_acc = self.new_account(space, &ZK_PROOF_PROGRAM_ID);

        let pk_bytes: [u8; 32] = user.elgamal.pubkey().into();
        println!("ELGAMAL PK HEAD: {:?}", &pk_bytes[..8]);
        let verify_ix = ProofInstruction::VerifyPubkeyValidity.encode_verify_proof(
            Some(ContextStateInfo {
                context_state_account: &proof_acc.pubkey(),
                context_state_authority: &user.pubkey(),
            }),
            &proof_data,
        );
        self.send(&[verify_ix], &[]);
        self.send(
            &configure_account(
                &TOKEN_PROGRAM_ID,
                &user.ata.pubkey(),
                &self.mint.pubkey(),
                &AeKey::encrypt(aes, 0).into(),
                65536,
                &user.pubkey(),
                &[],
                ProofLocation::ContextStateAccount(&proof_acc.pubkey()),
            )
            .unwrap(),
            &[&user.key],
        );

        self.send(
            &[approve_account(
                &TOKEN_PROGRAM_ID,
                &user.ata.pubkey(),
                &self.mint.pubkey(),
                &admin_key.pubkey(),
                &[&admin_key.pubkey()],
            )
            .unwrap()],
            &[&admin.key],
        );
    }
    fn apply(&mut self, user: &User) {
        let el = &user.elgamal;
        let aes = &user.aes;
        let raw = self.svm.get_account(&user.ata.pubkey()).unwrap();
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
        self.send(
            &[apply_pending_balance(
                &TOKEN_PROGRAM_ID,
                &user.ata.pubkey(),
                counter,
                &new_decryptable.into(),
                &user.pubkey(),
                &[],
            )
            .unwrap()],
            &[&user.key],
        );
    }
    fn deposit(&mut self, user: &User, amount: u64) {
        let user_ata = &user.ata.pubkey();
        self.send(
            &[deposit(
                &TOKEN_PROGRAM_ID,
                user_ata,
                &self.mint.pubkey(),
                amount,
                9,
                &user.pubkey(),
                &[],
            )
            .unwrap()],
            &[&user.key],
        );
    }

    fn withdraw(&mut self, user: &User, amount: u64) {
        let raw_acount = self.svm.get_account(&user.ata.pubkey()).unwrap();
        let st = StateWithExtensions::<TokenAccountState>::unpack(&raw_acount.data).unwrap();
        let ct = st.get_extension::<ConfidentialTransferAccount>().unwrap();
        let available_balance: ElGamalCiphertext = ct.available_balance.try_into().unwrap();
        let decryptable_balance: AeCiphertext =
            ct.decryptable_available_balance.try_into().unwrap();

        let balance = decryptable_balance.decrypt(&user.aes).unwrap();

        let proofs =
            withdraw_proof_data(&available_balance, balance, amount, &user.elgamal).unwrap();
        let new_decryptable = user.aes.encrypt(balance - amount).into();

        let user_ata = &user.ata.pubkey();
        self.send(
            &withdraw(
                &TOKEN_PROGRAM_ID,
                user_ata,
                &self.mint.pubkey(),
                amount,
                9,
                &new_decryptable,
                &user.pubkey(),
                &[],
                ProofLocation::InstructionOffset(
                    NonZeroI8::new(1).unwrap(),
                    &proofs.equality_proof_data,
                ),
                ProofLocation::InstructionOffset(
                    NonZeroI8::new(2).unwrap(),
                    &proofs.range_proof_data,
                ),
            )
            .unwrap(),
            &[&user.key],
        );
    }

    fn mint_tokens(&mut self, user: &User, admin: &User, mint_amount: u64) {
        let user_ata = &user.ata.pubkey();
        self.send(
            &[mint_to(
                &TOKEN_PROGRAM_ID,
                &self.mint.pubkey(),
                user_ata,
                &admin.pubkey(),
                &[&admin.pubkey()],
                mint_amount,
            )
            .unwrap()],
            &[&admin.key],
        );
    }

    fn send(&mut self, ixs: &[Instruction], signers: &[&Keypair]) {
        let blockhash = self.svm.latest_blockhash();
        let msg = Message::new_with_blockhash(&ixs, Some(&self.payer.pubkey()), &blockhash);
        let mut signers_final = vec![&self.payer];
        signers_final.extend_from_slice(signers);
        let tx =
            VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &signers_final).unwrap();

        let res = self.svm.send_transaction(tx);
        match res {
            Ok(e) => assert!(true),
            Err(e) => {
                println!("{} {}", e.meta.logs.join("\n"), e.err);
                assert!(false);
            }
        };
    }
    fn new_account(&mut self, space: usize, owner: &Pubkey) -> Keypair {
        let rent = self.svm.minimum_balance_for_rent_exemption(space);
        let a = Keypair::new();
        self.send(
            &[create_account(
                &self.payer.pubkey(),
                &a.pubkey(),
                rent,
                space as u64,
                owner,
            )],
            &[&a],
        );
        a
    }

    fn units(&self, amount: u64) -> u64 {
        amount * 10u64.pow(9)
    }

    fn new_account_from_key(&mut self, space: usize, owner: &Pubkey, keypair: &Keypair) {
        let rent = self.svm.minimum_balance_for_rent_exemption(space);

        self.send(
            &[create_account(
                &self.payer.pubkey(),
                &keypair.pubkey(),
                rent,
                space as u64,
                owner,
            )],
            &[keypair],
        );
    }

    fn setup(admin: &User) -> TestHelper {
        let withdraw_elgamal_pubkey = admin.elgamal.pubkey();
        let mut th = TestHelper {
            svm: LiteSVM::new(),
            mint: Keypair::new(),
            payer: Keypair::new(),
        };

        th.svm.airdrop(&th.payer.pubkey(), 10_000_000_000);
        let space =
            ExtensionType::try_calculate_account_len::<MintState>(&SUPPORTED_EXTENSIONS).unwrap();
        let mint = th.new_account(space, &TOKEN_PROGRAM_ID);
        th.mint = mint;
        let MINT: &Pubkey = &th.mint.pubkey();
        let ADMIN: Pubkey = admin.key.pubkey();
        let some_admin_ref = Some(&ADMIN);
        let some_admin = Some(ADMIN);
        let withdraw_elgamal_pubkey_raw: [u8; 32] = withdraw_elgamal_pubkey.into();
        let ixs = [
            initialize_transfer_fee_config(
                &TOKEN_PROGRAM_ID,
                MINT,
                some_admin_ref,
                some_admin_ref,
                100,
                10_000,
            )
            .unwrap(),
            initialize_metadata_pointer(
                &TOKEN_PROGRAM_ID,
                MINT,
                some_admin,
                Some(th.mint.pubkey()),
            )
            .unwrap(),
            initialize_default_account_state(&TOKEN_PROGRAM_ID, MINT, &AccountState::Frozen)
                .unwrap(),
            initialize_mint_close_authority(&TOKEN_PROGRAM_ID, MINT, some_admin_ref).unwrap(),
            initialize_confidential_transfer_mint(&TOKEN_PROGRAM_ID, MINT, some_admin, false, None)
                .unwrap(),
            initialize_confidential_transfer_fee_config(
                &TOKEN_PROGRAM_ID,
                MINT,
                some_admin,
                &withdraw_elgamal_pubkey_raw.into(),
            )
            .unwrap(),
            initialize_permanent_delegate(&TOKEN_PROGRAM_ID, MINT, &ADMIN).unwrap(),
            initialize_mint2(&TOKEN_PROGRAM_ID, MINT, &ADMIN, some_admin_ref, 9).unwrap(),
        ];

        th.send(&ixs, &[]);
        th
    }

    fn transfer_ct(&mut self, user_src: &User, user_dst: &User, admin: &User, amount: u64) {
        let user_src_ata = &user_src.ata.pubkey();
        let user_to_ata = &user_dst.ata.pubkey();
        let aes = &user_src.aes;
        let sender_el = &user_src.elgamal;
        let dest_el = &user_dst.elgamal;

        let ata_raw = self.svm.get_account(user_src_ata).unwrap();
        let state = StateWithExtensions::<TokenAccountState>::unpack(&ata_raw.data).unwrap();
        let confidential_transfer_account = state
            .get_extension::<ConfidentialTransferAccount>()
            .unwrap();

        let available_balance: ElGamalCiphertext = confidential_transfer_account
            .available_balance
            .try_into()
            .unwrap();
        let decryptable_available_balance: AeCiphertext = confidential_transfer_account
            .decryptable_available_balance
            .try_into()
            .unwrap();

        let balance = decryptable_available_balance.decrypt(aes).unwrap();

        let mint_raw = self.svm.get_account(&self.mint.pubkey()).unwrap();
        let mint_state = AStateWithExtensions::<MintState>::unpack(&mint_raw.data).unwrap();
        let epoch = self.svm.get_sysvar::<Clock>().epoch;
        let fee_config = mint_state.get_extension::<TransferFeeConfig>().unwrap();
        let fee = fee_config.calculate_epoch_fee(epoch, amount).unwrap();

        let proofs = transfer_with_fee_split_proof_data(
            &available_balance,
            &decryptable_available_balance,
            amount,
            sender_el,
            aes,
            dest_el.pubkey(),
            None,
            admin.elgamal.pubkey(),
            100,
            10_000,
        )
        .unwrap();

        let new_decryptable = aes.encrypt(balance - amount - fee).into();

        let ixs = transfer_with_fee(
            &TOKEN_PROGRAM_ID,
            user_src_ata,
            &self.mint.pubkey(),
            user_to_ata,
            &new_decryptable,
            &proofs
                .transfer_amount_ciphertext_validity_proof_data_with_ciphertext
                .ciphertext_lo,
            &proofs
                .transfer_amount_ciphertext_validity_proof_data_with_ciphertext
                .ciphertext_hi,
            &user_src.pubkey(),
            &[],
            ProofLocation::InstructionOffset(
                NonZeroI8::new(1).unwrap(),
                &proofs.equality_proof_data,
            ),
            ProofLocation::InstructionOffset(
                NonZeroI8::new(2).unwrap(),
                &proofs
                    .transfer_amount_ciphertext_validity_proof_data_with_ciphertext
                    .proof_data,
            ),
            ProofLocation::InstructionOffset(
                NonZeroI8::new(3).unwrap(),
                &proofs.percentage_with_cap_proof_data,
            ),
            ProofLocation::InstructionOffset(
                NonZeroI8::new(4).unwrap(),
                &proofs.fee_ciphertext_validity_proof_data,
            ),
            ProofLocation::InstructionOffset(NonZeroI8::new(5).unwrap(), &proofs.range_proof_data),
        )
        .unwrap();

        self.send(&ixs, &[&user_src.key]);
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
    let admin = User::new();

    let mut mint = TestHelper::setup(&admin);
    mint.svm
        .airdrop(&admin.key.pubkey(), 1_000_000_000)
        .unwrap();

    let user_a = User::new();
    let user_b = User::new();

    mint.ata(&user_a);
    mint.ata(&user_b);
    mint.thaw(&user_a, &admin);
    mint.mint_tokens(&user_a, &admin, mint.units(200));
    mint.thaw(&user_b, &admin);

    mint.transfer(&user_a, &user_b, &admin, mint.units(10));
    mint.enable_ct(&user_a, &admin);
    mint.deposit(&user_a, mint.units(10));
    mint.apply(&user_a);

    mint.enable_ct(&user_b, &admin);

    mint.transfer_ct(&user_a, &user_b, &admin, mint.units(9));
    mint.apply(&user_b);
    // mint.withdraw(&user_b, mint.units(9));
}
