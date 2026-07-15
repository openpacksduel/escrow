#![allow(deprecated)]

use anchor_lang::{
    prelude::{AccountInfo, Pubkey, Rent},
    solana_program::{entrypoint::ProgramResult, program_option::COption, system_program},
    InstructionData, ToAccountMetas,
};
use openpacksduel_escrow::{
    accounts, instruction, AssetKind, DepositCardAssetArgs, InitializeDuelArgs, PlayerRole,
};
use solana_account::Account;
use solana_keypair::Keypair;
use solana_program_pack::Pack;
use solana_program_test::{processor, ProgramTest, ProgramTestContext};
use solana_signer::Signer;
use solana_transaction::Transaction;
use spl_token::state::{Account as LegacyTokenAccount, AccountState, Mint as LegacyMint};

const FEE_AMOUNT: u64 = 1_000_000;
const DUST_AMOUNT: u64 = 41_000;
const NONCE: u64 = 7;

fn process_escrow_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let program_id = *program_id;
    let accounts = accounts.to_vec();
    let instruction_data = instruction_data.to_vec();

    openpacksduel_escrow::entry(&program_id, &accounts, &instruction_data)
}

struct Fixture {
    creator: Keypair,
    opponent: Keypair,
    provider_signer: Pubkey,
    fee_recipient: Pubkey,
    creator_payment_source: Pubkey,
    opponent_payment_source: Pubkey,
    creator_payment_destination: Pubkey,
    excess_destination: Pubkey,
    freezeable_card_mint: Pubkey,
    freezeable_card_source: Pubkey,
}

fn program_test() -> (ProgramTest, Fixture) {
    let creator = Keypair::new();
    let opponent = Keypair::new();
    let provider_signer = Pubkey::new_unique();
    let fee_recipient = Pubkey::new_unique();
    let creator_payment_source = Pubkey::new_unique();
    let opponent_payment_source = Pubkey::new_unique();
    let creator_payment_destination = Pubkey::new_unique();
    let excess_destination = Pubkey::new_unique();
    let freezeable_card_mint = Pubkey::new_unique();
    let freezeable_card_source = Pubkey::new_unique();
    let rent = Rent::default();

    let mut test = ProgramTest::new(
        "openpacksduel_escrow",
        openpacksduel_escrow::id(),
        processor!(process_escrow_instruction),
    );
    test.add_program(
        "spl_token",
        spl_token::id(),
        processor!(spl_token::processor::Processor::process),
    );
    test.add_account(creator.pubkey(), system_account(10_000_000_000));
    test.add_account(opponent.pubkey(), system_account(10_000_000_000));
    test.add_account(
        spl_token::native_mint::id(),
        mint_account(
            LegacyMint {
                mint_authority: COption::None,
                supply: 0,
                decimals: spl_token::native_mint::DECIMALS,
                is_initialized: true,
                freeze_authority: COption::None,
            },
            &rent,
        ),
    );
    test.add_account(
        creator_payment_source,
        token_account(
            creator.pubkey(),
            spl_token::native_mint::id(),
            FEE_AMOUNT + DUST_AMOUNT,
            true,
            &rent,
        ),
    );
    test.add_account(
        opponent_payment_source,
        token_account(
            opponent.pubkey(),
            spl_token::native_mint::id(),
            FEE_AMOUNT,
            true,
            &rent,
        ),
    );
    test.add_account(
        creator_payment_destination,
        token_account(
            creator.pubkey(),
            spl_token::native_mint::id(),
            0,
            true,
            &rent,
        ),
    );
    test.add_account(
        excess_destination,
        token_account(fee_recipient, spl_token::native_mint::id(), 0, true, &rent),
    );
    test.add_account(
        freezeable_card_mint,
        mint_account(
            LegacyMint {
                mint_authority: COption::None,
                supply: 1,
                decimals: 0,
                is_initialized: true,
                freeze_authority: COption::Some(creator.pubkey()),
            },
            &rent,
        ),
    );
    test.add_account(
        freezeable_card_source,
        token_account(creator.pubkey(), freezeable_card_mint, 1, false, &rent),
    );

    (
        test,
        Fixture {
            creator,
            opponent,
            provider_signer,
            fee_recipient,
            creator_payment_source,
            opponent_payment_source,
            creator_payment_destination,
            excess_destination,
            freezeable_card_mint,
            freezeable_card_source,
        },
    )
}

fn system_account(lamports: u64) -> Account {
    Account {
        lamports,
        data: Vec::new(),
        owner: system_program::ID,
        executable: false,
        rent_epoch: 0,
    }
}

fn mint_account(mint: LegacyMint, rent: &Rent) -> Account {
    let mut data = vec![0; LegacyMint::LEN];
    LegacyMint::pack(mint, &mut data).expect("mint fixture must pack");
    Account {
        lamports: rent.minimum_balance(LegacyMint::LEN),
        data,
        owner: spl_token::id(),
        executable: false,
        rent_epoch: 0,
    }
}

fn token_account(
    owner: Pubkey,
    mint: Pubkey,
    amount: u64,
    is_native: bool,
    rent: &Rent,
) -> Account {
    let reserve = rent.minimum_balance(LegacyTokenAccount::LEN);
    let mut data = vec![0; LegacyTokenAccount::LEN];
    LegacyTokenAccount::pack(
        LegacyTokenAccount {
            mint,
            owner,
            amount,
            delegate: COption::None,
            state: AccountState::Initialized,
            is_native: if is_native {
                COption::Some(reserve)
            } else {
                COption::None
            },
            delegated_amount: 0,
            close_authority: COption::None,
        },
        &mut data,
    )
    .expect("token fixture must pack");
    Account {
        lamports: reserve + if is_native { amount } else { 0 },
        data,
        owner: spl_token::id(),
        executable: false,
        rent_epoch: 0,
    }
}

fn duel_addresses(creator: Pubkey) -> (Pubkey, Pubkey) {
    let (duel, _) = Pubkey::find_program_address(
        &[b"duel", creator.as_ref(), &NONCE.to_le_bytes()],
        &openpacksduel_escrow::id(),
    );
    let (payment_vault, _) =
        Pubkey::find_program_address(&[b"vault", duel.as_ref()], &openpacksduel_escrow::id());
    (duel, payment_vault)
}

fn initialize_duel_instruction(
    fixture: &Fixture,
    duel: Pubkey,
    payment_vault: Pubkey,
    now: i64,
) -> anchor_lang::solana_program::instruction::Instruction {
    anchor_lang::solana_program::instruction::Instruction {
        program_id: openpacksduel_escrow::id(),
        accounts: accounts::InitializeDuel {
            creator: fixture.creator.pubkey(),
            duel,
            payment_vault,
            payment_mint: spl_token::native_mint::id(),
            token_program: spl_token::id(),
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: instruction::InitializeDuel {
            args: InitializeDuelArgs {
                nonce: NONCE,
                opponent: Some(fixture.opponent.pubkey()),
                fee_amount: FEE_AMOUNT,
                expires_at: now + 600,
                provider_signer: fixture.provider_signer,
                fee_recipient: fixture.fee_recipient,
                valuation_policy_hash: [9; 32],
            },
        }
        .data(),
    }
}

fn fund_duel_instruction(
    player: Pubkey,
    source: Pubkey,
    duel: Pubkey,
    payment_vault: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    anchor_lang::solana_program::instruction::Instruction {
        program_id: openpacksduel_escrow::id(),
        accounts: accounts::FundDuel {
            player,
            duel,
            player_source: source,
            payment_vault,
            payment_mint: spl_token::native_mint::id(),
            token_program: spl_token::id(),
        }
        .to_account_metas(None),
        data: instruction::FundDuel {}.data(),
    }
}

async fn send(
    context: &mut ProgramTestContext,
    instructions: &[anchor_lang::solana_program::instruction::Instruction],
    additional_signers: &[&Keypair],
) -> Result<(), solana_program_test::BanksClientError> {
    let latest_blockhash = context.banks_client.get_latest_blockhash().await?;
    let mut signers: Vec<&dyn Signer> = vec![&context.payer];
    signers.extend(
        additional_signers
            .iter()
            .map(|signer| *signer as &dyn Signer),
    );
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&context.payer.pubkey()),
        &signers,
        latest_blockhash,
    );
    context.banks_client.process_transaction(transaction).await
}

async fn token_amount(context: &mut ProgramTestContext, address: Pubkey) -> u64 {
    let account = context
        .banks_client
        .get_account(address)
        .await
        .expect("account query must succeed")
        .expect("token account must exist");
    LegacyTokenAccount::unpack(&account.data)
        .expect("token account must decode")
        .amount
}

#[tokio::test]
async fn freezeable_legacy_card_mint_is_rejected_without_moving_the_asset() {
    let (test, fixture) = program_test();
    let mut context = test.start_with_context().await;
    let (duel, payment_vault) = duel_addresses(fixture.creator.pubkey());
    let now = context.genesis_config().creation_time;

    send(
        &mut context,
        &[initialize_duel_instruction(
            &fixture,
            duel,
            payment_vault,
            now,
        )],
        &[&fixture.creator],
    )
    .await
    .expect("duel initialization must succeed");
    send(
        &mut context,
        &[
            fund_duel_instruction(
                fixture.creator.pubkey(),
                fixture.creator_payment_source,
                duel,
                payment_vault,
            ),
            fund_duel_instruction(
                fixture.opponent.pubkey(),
                fixture.opponent_payment_source,
                duel,
                payment_vault,
            ),
        ],
        &[&fixture.creator, &fixture.opponent],
    )
    .await
    .expect("both exact fee deposits must succeed");

    let (card_vault, _) = Pubkey::find_program_address(
        &[b"card-vault", duel.as_ref(), b"creator"],
        &openpacksduel_escrow::id(),
    );
    let deposit = anchor_lang::solana_program::instruction::Instruction {
        program_id: openpacksduel_escrow::id(),
        accounts: accounts::DepositCardAsset {
            depositor: fixture.creator.pubkey(),
            duel,
            depositor_source: fixture.freezeable_card_source,
            card_vault,
            card_mint: fixture.freezeable_card_mint,
            token_program: spl_token::id(),
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: instruction::DepositCardAsset {
            args: DepositCardAssetArgs {
                role: PlayerRole::Creator,
                asset_kind: AssetKind::LegacySplNft,
            },
        }
        .data(),
    };

    assert!(send(&mut context, &[deposit], &[&fixture.creator])
        .await
        .is_err());
    assert_eq!(
        token_amount(&mut context, fixture.freezeable_card_source).await,
        1
    );
    assert!(context
        .banks_client
        .get_account(card_vault)
        .await
        .expect("vault query must succeed")
        .is_none());
}

#[tokio::test]
async fn terminal_payment_dust_sweeps_to_committed_fee_recipient_before_close() {
    let (test, fixture) = program_test();
    let mut context = test.start_with_context().await;
    let (duel, payment_vault) = duel_addresses(fixture.creator.pubkey());
    let now = context.genesis_config().creation_time;

    send(
        &mut context,
        &[initialize_duel_instruction(
            &fixture,
            duel,
            payment_vault,
            now,
        )],
        &[&fixture.creator],
    )
    .await
    .expect("duel initialization must succeed");
    send(
        &mut context,
        &[fund_duel_instruction(
            fixture.creator.pubkey(),
            fixture.creator_payment_source,
            duel,
            payment_vault,
        )],
        &[&fixture.creator],
    )
    .await
    .expect("creator fee deposit must succeed");

    let dust_transfer = spl_token::instruction::transfer_checked(
        &spl_token::id(),
        &fixture.creator_payment_source,
        &spl_token::native_mint::id(),
        &payment_vault,
        &fixture.creator.pubkey(),
        &[],
        DUST_AMOUNT,
        spl_token::native_mint::DECIMALS,
    )
    .expect("dust transfer instruction must encode");
    send(&mut context, &[dust_transfer], &[&fixture.creator])
        .await
        .expect("unsolicited dust transfer must succeed");

    let cancel = anchor_lang::solana_program::instruction::Instruction {
        program_id: openpacksduel_escrow::id(),
        accounts: accounts::CancelUnmatched {
            creator: fixture.creator.pubkey(),
            duel,
            payment_vault,
            creator_destination: fixture.creator_payment_destination,
            payment_mint: spl_token::native_mint::id(),
            token_program: spl_token::id(),
        }
        .to_account_metas(None),
        data: instruction::CancelUnmatched {}.data(),
    };
    send(&mut context, &[cancel], &[&fixture.creator])
        .await
        .expect("tracked creator fee must refund on cancellation");

    let close = anchor_lang::solana_program::instruction::Instruction {
        program_id: openpacksduel_escrow::id(),
        accounts: accounts::ClosePaymentVault {
            caller: context.payer.pubkey(),
            duel,
            payment_vault,
            payment_mint: spl_token::native_mint::id(),
            rent_recipient: fixture.creator.pubkey(),
            excess_destination: fixture.excess_destination,
            token_program: spl_token::id(),
        }
        .to_account_metas(None),
        data: instruction::ClosePaymentVault {}.data(),
    };
    send(&mut context, &[close], &[])
        .await
        .expect("permissionless terminal sweep and close must succeed");

    assert_eq!(
        token_amount(&mut context, fixture.creator_payment_destination).await,
        FEE_AMOUNT
    );
    assert_eq!(
        token_amount(&mut context, fixture.excess_destination).await,
        DUST_AMOUNT
    );
    assert!(context
        .banks_client
        .get_account(payment_vault)
        .await
        .expect("vault query must succeed")
        .is_none());
}
