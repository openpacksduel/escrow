#![allow(deprecated)]

use anchor_lang::{
    prelude::{Clock, Pubkey, Rent},
    solana_program::{program_option::COption, system_instruction, system_program},
    InstructionData, ToAccountMetas,
};
use openpacksduel_escrow::{
    accounts, instruction, AssetKind, DepositCardAssetArgs, InitializeDuelArgs, PlayerRole,
    SubmitResultArgs,
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

struct Fixture {
    creator: Keypair,
    opponent: Keypair,
    provider_signer: Keypair,
    fee_recipient: Pubkey,
    creator_payment_source: Pubkey,
    opponent_payment_source: Pubkey,
    creator_payment_destination: Pubkey,
    opponent_payment_destination: Pubkey,
    excess_destination: Pubkey,
    freezeable_card_mint: Pubkey,
    freezeable_card_source: Pubkey,
    immutable_card_mint: Pubkey,
    immutable_card_source: Pubkey,
    immutable_card_destination: Pubkey,
    opponent_immutable_card_mint: Pubkey,
    opponent_immutable_card_source: Pubkey,
    opponent_card_win_destination: Pubkey,
}

fn program_test() -> (ProgramTest, Fixture) {
    let creator = Keypair::new();
    let opponent = Keypair::new();
    let provider_signer = Keypair::new();
    let fee_recipient = Pubkey::new_unique();
    let creator_payment_source = Pubkey::new_unique();
    let opponent_payment_source = Pubkey::new_unique();
    let creator_payment_destination = Pubkey::new_unique();
    let opponent_payment_destination = Pubkey::new_unique();
    let excess_destination = Pubkey::new_unique();
    let freezeable_card_mint = Pubkey::new_unique();
    let freezeable_card_source = Pubkey::new_unique();
    let immutable_card_mint = Pubkey::new_unique();
    let immutable_card_source = Pubkey::new_unique();
    let immutable_card_destination = Pubkey::new_unique();
    let opponent_immutable_card_mint = Pubkey::new_unique();
    let opponent_immutable_card_source = Pubkey::new_unique();
    let opponent_card_win_destination = Pubkey::new_unique();
    let rent = Rent::default();

    let mut test = ProgramTest::new("openpacksduel_escrow", openpacksduel_escrow::id(), None);
    test.add_program(
        "spl_token",
        spl_token::id(),
        processor!(spl_token::processor::Processor::process),
    );
    test.add_account(creator.pubkey(), system_account(10_000_000_000));
    test.add_account(opponent.pubkey(), system_account(10_000_000_000));
    test.add_account(provider_signer.pubkey(), system_account(10_000_000_000));
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
        opponent_payment_destination,
        token_account(
            opponent.pubkey(),
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
    test.add_account(
        immutable_card_mint,
        mint_account(
            LegacyMint {
                mint_authority: COption::None,
                supply: 1,
                decimals: 0,
                is_initialized: true,
                freeze_authority: COption::None,
            },
            &rent,
        ),
    );
    test.add_account(
        immutable_card_source,
        token_account(creator.pubkey(), immutable_card_mint, 1, false, &rent),
    );
    test.add_account(
        immutable_card_destination,
        token_account(creator.pubkey(), immutable_card_mint, 0, false, &rent),
    );
    test.add_account(
        opponent_immutable_card_mint,
        mint_account(
            LegacyMint {
                mint_authority: COption::None,
                supply: 1,
                decimals: 0,
                is_initialized: true,
                freeze_authority: COption::None,
            },
            &rent,
        ),
    );
    test.add_account(
        opponent_immutable_card_source,
        token_account(
            opponent.pubkey(),
            opponent_immutable_card_mint,
            1,
            false,
            &rent,
        ),
    );
    test.add_account(
        opponent_card_win_destination,
        token_account(
            creator.pubkey(),
            opponent_immutable_card_mint,
            0,
            false,
            &rent,
        ),
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
            opponent_payment_destination,
            excess_destination,
            freezeable_card_mint,
            freezeable_card_source,
            immutable_card_mint,
            immutable_card_source,
            immutable_card_destination,
            opponent_immutable_card_mint,
            opponent_immutable_card_source,
            opponent_card_win_destination,
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
                provider_signer: fixture.provider_signer.pubkey(),
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
#[ignore = "runs against the SBF artifact in the Program release workflow"]
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
#[ignore = "runs against the SBF artifact in the Program release workflow"]
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

    let dust_transfer =
        system_instruction::transfer(&fixture.creator.pubkey(), &payment_vault, DUST_AMOUNT);
    send(&mut context, &[dust_transfer], &[&fixture.creator])
        .await
        .expect("unsolicited raw SOL transfer must succeed");

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

#[tokio::test]
#[ignore = "runs against the SBF artifact in the Program release workflow"]
async fn redeposited_terminal_card_returns_to_recorded_player_before_close() {
    let (test, fixture) = program_test();
    let mut context = test.start_with_context().await;
    let (duel, payment_vault) = duel_addresses(fixture.creator.pubkey());
    let (card_vault, _) = Pubkey::find_program_address(
        &[b"card-vault", duel.as_ref(), b"creator"],
        &openpacksduel_escrow::id(),
    );
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

    let deposit = anchor_lang::solana_program::instruction::Instruction {
        program_id: openpacksduel_escrow::id(),
        accounts: accounts::DepositCardAsset {
            depositor: fixture.creator.pubkey(),
            duel,
            depositor_source: fixture.immutable_card_source,
            card_vault,
            card_mint: fixture.immutable_card_mint,
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
    send(&mut context, &[deposit], &[&fixture.creator])
        .await
        .expect("immutable creator card must enter custody");

    let clock: Clock = context
        .banks_client
        .get_sysvar()
        .await
        .expect("clock query must succeed");
    context
        .warp_to_slot(clock.slot.saturating_add(200_000))
        .expect("clock warp must succeed");
    let expired_clock: Clock = context
        .banks_client
        .get_sysvar()
        .await
        .expect("expired clock query must succeed");
    assert!(expired_clock.unix_timestamp >= now + 600);

    let refund = anchor_lang::solana_program::instruction::Instruction {
        program_id: openpacksduel_escrow::id(),
        accounts: accounts::RefundExpiredCard {
            caller: context.payer.pubkey(),
            duel,
            card_vault,
            card_mint: fixture.immutable_card_mint,
            destination: fixture.immutable_card_destination,
            token_program: spl_token::id(),
        }
        .to_account_metas(None),
        data: instruction::RefundExpiredCard {
            role: PlayerRole::Creator,
        }
        .data(),
    };
    send(&mut context, &[refund], &[])
        .await
        .expect("expired card refund must clear tracked custody");

    let redeposit = spl_token::instruction::transfer_checked(
        &spl_token::id(),
        &fixture.immutable_card_destination,
        &fixture.immutable_card_mint,
        &card_vault,
        &fixture.creator.pubkey(),
        &[],
        1,
        0,
    )
    .expect("card redeposit instruction must encode");
    send(&mut context, &[redeposit], &[&fixture.creator])
        .await
        .expect("unsolicited card redeposit must succeed");

    let close = anchor_lang::solana_program::instruction::Instruction {
        program_id: openpacksduel_escrow::id(),
        accounts: accounts::CloseCardVault {
            caller: context.payer.pubkey(),
            duel,
            card_vault,
            card_mint: fixture.immutable_card_mint,
            rent_recipient: fixture.creator.pubkey(),
            recovery_destination: fixture.immutable_card_destination,
            token_program: spl_token::id(),
        }
        .to_account_metas(None),
        data: instruction::CloseCardVault {
            role: PlayerRole::Creator,
        }
        .data(),
    };
    send(&mut context, &[close], &[])
        .await
        .expect("permissionless card recovery and close must succeed");

    assert_eq!(
        token_amount(&mut context, fixture.immutable_card_destination).await,
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
#[ignore = "runs against the SBF artifact in the Program release workflow"]
async fn settled_losing_role_card_returns_to_winner_before_close() {
    let (test, fixture) = program_test();
    let mut context = test.start_with_context().await;
    let (duel, payment_vault) = duel_addresses(fixture.creator.pubkey());
    let (creator_card_vault, _) = Pubkey::find_program_address(
        &[b"card-vault", duel.as_ref(), b"creator"],
        &openpacksduel_escrow::id(),
    );
    let (opponent_card_vault, _) = Pubkey::find_program_address(
        &[b"card-vault", duel.as_ref(), b"opponent"],
        &openpacksduel_escrow::id(),
    );
    let provider_request_id = [7; 32];
    let (result_commitment, _) = Pubkey::find_program_address(
        &[
            b"result",
            fixture.provider_signer.pubkey().as_ref(),
            provider_request_id.as_ref(),
        ],
        &openpacksduel_escrow::id(),
    );
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

    let creator_deposit = anchor_lang::solana_program::instruction::Instruction {
        program_id: openpacksduel_escrow::id(),
        accounts: accounts::DepositCardAsset {
            depositor: fixture.creator.pubkey(),
            duel,
            depositor_source: fixture.immutable_card_source,
            card_vault: creator_card_vault,
            card_mint: fixture.immutable_card_mint,
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
    let opponent_deposit = anchor_lang::solana_program::instruction::Instruction {
        program_id: openpacksduel_escrow::id(),
        accounts: accounts::DepositCardAsset {
            depositor: fixture.opponent.pubkey(),
            duel,
            depositor_source: fixture.opponent_immutable_card_source,
            card_vault: opponent_card_vault,
            card_mint: fixture.opponent_immutable_card_mint,
            token_program: spl_token::id(),
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: instruction::DepositCardAsset {
            args: DepositCardAssetArgs {
                role: PlayerRole::Opponent,
                asset_kind: AssetKind::LegacySplNft,
            },
        }
        .data(),
    };
    send(
        &mut context,
        &[creator_deposit, opponent_deposit],
        &[&fixture.creator, &fixture.opponent],
    )
    .await
    .expect("both immutable cards must enter custody");

    let opened_clock: Clock = context
        .banks_client
        .get_sysvar()
        .await
        .expect("result clock query must succeed");
    let submit_result = anchor_lang::solana_program::instruction::Instruction {
        program_id: openpacksduel_escrow::id(),
        accounts: accounts::SubmitResult {
            provider_signer: fixture.provider_signer.pubkey(),
            duel,
            result_commitment,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: instruction::SubmitResult {
            args: SubmitResultArgs {
                duel,
                provider_request_id,
                creator: fixture.creator.pubkey(),
                opponent: fixture.opponent.pubkey(),
                creator_card_mint: fixture.immutable_card_mint,
                opponent_card_mint: fixture.opponent_immutable_card_mint,
                creator_asset_kind: AssetKind::LegacySplNft,
                opponent_asset_kind: AssetKind::LegacySplNft,
                valuation_policy_hash: [9; 32],
                creator_value: 2,
                opponent_value: 1,
                opened_at: opened_clock.unix_timestamp,
            },
        }
        .data(),
    };
    send(&mut context, &[submit_result], &[&fixture.provider_signer])
        .await
        .expect("provider result must commit a creator win");

    let settle = anchor_lang::solana_program::instruction::Instruction {
        program_id: openpacksduel_escrow::id(),
        accounts: accounts::SettleDuel {
            caller: context.payer.pubkey(),
            duel,
            result_commitment,
            payment_vault,
            payment_mint: spl_token::native_mint::id(),
            creator_payment_destination: fixture.creator_payment_destination,
            opponent_payment_destination: fixture.opponent_payment_destination,
            fee_destination: fixture.excess_destination,
            creator_card_vault,
            creator_card_mint: fixture.immutable_card_mint,
            creator_card_destination: fixture.immutable_card_destination,
            opponent_card_vault,
            opponent_card_mint: fixture.opponent_immutable_card_mint,
            opponent_card_destination: fixture.opponent_card_win_destination,
            token_program: spl_token::id(),
        }
        .to_account_metas(None),
        data: instruction::SettleDuel {}.data(),
    };
    send(&mut context, &[settle], &[])
        .await
        .expect("permissionless creator-win settlement must succeed");
    assert_eq!(
        token_amount(&mut context, fixture.opponent_card_win_destination).await,
        1
    );

    let redeposit = spl_token::instruction::transfer_checked(
        &spl_token::id(),
        &fixture.opponent_card_win_destination,
        &fixture.opponent_immutable_card_mint,
        &opponent_card_vault,
        &fixture.creator.pubkey(),
        &[],
        1,
        0,
    )
    .expect("winner card redeposit instruction must encode");
    send(&mut context, &[redeposit], &[&fixture.creator])
        .await
        .expect("winner must be able to redeposit the losing-role card");

    let close = anchor_lang::solana_program::instruction::Instruction {
        program_id: openpacksduel_escrow::id(),
        accounts: accounts::CloseCardVault {
            caller: context.payer.pubkey(),
            duel,
            card_vault: opponent_card_vault,
            card_mint: fixture.opponent_immutable_card_mint,
            rent_recipient: fixture.opponent.pubkey(),
            recovery_destination: fixture.opponent_card_win_destination,
            token_program: spl_token::id(),
        }
        .to_account_metas(None),
        data: instruction::CloseCardVault {
            role: PlayerRole::Opponent,
        }
        .data(),
    };
    send(&mut context, &[close], &[])
        .await
        .expect("terminal losing-role vault must return its card to the winner");

    assert_eq!(
        token_amount(&mut context, fixture.opponent_card_win_destination).await,
        1
    );
    assert!(context
        .banks_client
        .get_account(opponent_card_vault)
        .await
        .expect("vault query must succeed")
        .is_none());
}
