use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

declare_id!("Co198eFfQcmn1WzZRnHV6jxcSLBDCv1qNfPfiBYdCLfS");

const DUEL_SEED: &[u8] = b"duel";
const PAYMENT_VAULT_SEED: &[u8] = b"vault";
const CARD_VAULT_SEED: &[u8] = b"card-vault";
const CREATOR_CARD_SEED: &[u8] = b"creator";
const OPPONENT_CARD_SEED: &[u8] = b"opponent";
const RESULT_SEED: &[u8] = b"result";
const MIN_DUEL_DURATION_SECONDS: i64 = 60;
const MAX_DUEL_DURATION_SECONDS: i64 = 7 * 24 * 60 * 60;
const MAX_PROVIDER_CLOCK_SKEW_SECONDS: i64 = 30;

#[program]
pub mod openpacksduel_escrow {
    use super::*;

    pub fn initialize_duel(ctx: Context<InitializeDuel>, args: InitializeDuelArgs) -> Result<()> {
        let clock = Clock::get()?;
        validate_initialization(&args, ctx.accounts.creator.key(), clock.unix_timestamp)?;

        let duel = &mut ctx.accounts.duel;
        duel.version = 2;
        duel.bump = ctx.bumps.duel;
        duel.payment_vault_bump = ctx.bumps.payment_vault;
        duel.status = DuelStatus::Waiting;
        duel.creator = ctx.accounts.creator.key();
        duel.opponent = args.opponent.unwrap_or_default();
        duel.payment_mint = ctx.accounts.payment_mint.key();
        duel.payment_vault = ctx.accounts.payment_vault.key();
        duel.fee_recipient = args.fee_recipient;
        duel.provider_signer = args.provider_signer;
        duel.nonce = args.nonce;
        duel.fee_amount = args.fee_amount;
        duel.created_at = clock.unix_timestamp;
        duel.expires_at = args.expires_at;
        duel.creator_deposited = false;
        duel.opponent_deposited = false;
        duel.creator_card_deposited = false;
        duel.opponent_card_deposited = false;
        duel.creator_card_mint = Pubkey::default();
        duel.opponent_card_mint = Pubkey::default();
        duel.creator_card_vault = Pubkey::default();
        duel.opponent_card_vault = Pubkey::default();
        duel.result_commitment = Pubkey::default();
        duel.valuation_policy_hash = args.valuation_policy_hash;

        emit!(DuelInitialized {
            duel: duel.key(),
            creator: duel.creator,
            opponent: duel.opponent,
            payment_mint: duel.payment_mint,
            fee_amount: duel.fee_amount,
            expires_at: duel.expires_at,
            provider_signer: duel.provider_signer,
            valuation_policy_hash: duel.valuation_policy_hash,
        });

        Ok(())
    }

    pub fn fund_duel(ctx: Context<FundDuel>) -> Result<()> {
        require_before_expiry(&ctx.accounts.duel)?;
        let player = ctx.accounts.player.key();
        let role = ctx.accounts.duel.depositor_role(player)?;
        let fee_amount = ctx.accounts.duel.fee_amount;

        transfer_checked(
            &ctx.accounts.player_source,
            &ctx.accounts.payment_mint,
            &ctx.accounts.payment_vault,
            &ctx.accounts.player,
            &ctx.accounts.token_program,
            fee_amount,
        )?;

        let duel = &mut ctx.accounts.duel;
        match role {
            PlayerRole::Creator => duel.creator_deposited = true,
            PlayerRole::Opponent => {
                if duel.opponent == Pubkey::default() {
                    duel.opponent = player;
                }
                duel.opponent_deposited = true;
            }
        }

        if duel.creator_deposited && duel.opponent_deposited {
            duel.status = DuelStatus::Funded;
        }

        emit!(DuelFunded {
            duel: duel.key(),
            player,
            role,
            amount: fee_amount,
            status: duel.status,
        });

        Ok(())
    }

    pub fn deposit_card_asset(
        ctx: Context<DepositCardAsset>,
        args: DepositCardAssetArgs,
    ) -> Result<()> {
        require_before_expiry(&ctx.accounts.duel)?;
        require!(
            matches!(
                ctx.accounts.duel.status,
                DuelStatus::Funded | DuelStatus::AwaitingResult
            ),
            EscrowError::InvalidStatus
        );
        require!(
            args.asset_kind == AssetKind::LegacySplNft,
            EscrowError::UnsupportedAssetStandard
        );
        require!(
            ctx.accounts.card_mint.decimals == 0 && ctx.accounts.card_mint.supply == 1,
            EscrowError::InvalidCardMint
        );
        require!(
            ctx.accounts.depositor.key() == ctx.accounts.duel.player_for_role(args.role)
                || ctx.accounts.depositor.key() == ctx.accounts.duel.provider_signer,
            EscrowError::InvalidCardDepositor
        );
        require!(
            !ctx.accounts.duel.card_deposited(args.role),
            EscrowError::CardAlreadyDeposited
        );

        transfer_checked(
            &ctx.accounts.depositor_source,
            &ctx.accounts.card_mint,
            &ctx.accounts.card_vault,
            &ctx.accounts.depositor,
            &ctx.accounts.token_program,
            1,
        )?;

        let duel = &mut ctx.accounts.duel;
        duel.record_card_deposit(
            args.role,
            ctx.accounts.card_mint.key(),
            ctx.accounts.card_vault.key(),
        );
        if duel.creator_card_deposited && duel.opponent_card_deposited {
            duel.status = DuelStatus::AwaitingResult;
        }

        emit!(CardAssetDeposited {
            duel: duel.key(),
            role: args.role,
            player: duel.player_for_role(args.role),
            depositor: ctx.accounts.depositor.key(),
            mint: ctx.accounts.card_mint.key(),
            vault: ctx.accounts.card_vault.key(),
            asset_kind: args.asset_kind,
        });

        Ok(())
    }

    pub fn submit_result(ctx: Context<SubmitResult>, args: SubmitResultArgs) -> Result<()> {
        let clock = Clock::get()?;
        let duel_key = ctx.accounts.duel.key();
        let duel = &ctx.accounts.duel;

        require!(
            duel.status == DuelStatus::AwaitingResult,
            EscrowError::InvalidStatus
        );
        require!(
            clock.unix_timestamp < duel.expires_at,
            EscrowError::DuelExpired
        );
        require!(args.duel == duel_key, EscrowError::ResultDuelMismatch);
        require_keys_eq!(
            args.creator,
            duel.creator,
            EscrowError::ResultPlayerMismatch
        );
        require_keys_eq!(
            args.opponent,
            duel.opponent,
            EscrowError::ResultPlayerMismatch
        );
        require_keys_eq!(
            args.creator_card_mint,
            duel.creator_card_mint,
            EscrowError::ResultAssetMismatch
        );
        require_keys_eq!(
            args.opponent_card_mint,
            duel.opponent_card_mint,
            EscrowError::ResultAssetMismatch
        );
        require!(
            args.valuation_policy_hash == duel.valuation_policy_hash,
            EscrowError::ValuationPolicyMismatch
        );
        require!(
            args.creator_asset_kind == AssetKind::LegacySplNft
                && args.opponent_asset_kind == AssetKind::LegacySplNft,
            EscrowError::UnsupportedAssetStandard
        );
        require!(
            args.provider_request_id != [0; 32],
            EscrowError::InvalidProviderRequest
        );
        require!(
            args.opened_at >= duel.created_at
                && args.opened_at
                    <= clock
                        .unix_timestamp
                        .saturating_add(MAX_PROVIDER_CLOCK_SKEW_SECONDS),
            EscrowError::InvalidResultTimestamp
        );

        let outcome = determine_outcome(args.creator_value, args.opponent_value);
        let result = &mut ctx.accounts.result_commitment;
        result.version = 1;
        result.bump = ctx.bumps.result_commitment;
        result.duel = duel_key;
        result.provider_signer = ctx.accounts.provider_signer.key();
        result.provider_request_id = args.provider_request_id;
        result.creator = args.creator;
        result.opponent = args.opponent;
        result.creator_card_mint = args.creator_card_mint;
        result.opponent_card_mint = args.opponent_card_mint;
        result.creator_asset_kind = args.creator_asset_kind;
        result.opponent_asset_kind = args.opponent_asset_kind;
        result.valuation_policy_hash = args.valuation_policy_hash;
        result.creator_value = args.creator_value;
        result.opponent_value = args.opponent_value;
        result.opened_at = args.opened_at;
        result.committed_at = clock.unix_timestamp;
        result.outcome = outcome;
        result.settled = false;

        let duel = &mut ctx.accounts.duel;
        duel.result_commitment = result.key();
        duel.status = DuelStatus::ReadyToSettle;

        emit!(ResultCommitted {
            duel: duel_key,
            result_commitment: result.key(),
            provider_signer: result.provider_signer,
            provider_request_id: result.provider_request_id,
            creator: result.creator,
            opponent: result.opponent,
            creator_card_mint: result.creator_card_mint,
            opponent_card_mint: result.opponent_card_mint,
            creator_asset_kind: result.creator_asset_kind,
            opponent_asset_kind: result.opponent_asset_kind,
            valuation_policy_hash: result.valuation_policy_hash,
            creator_value: result.creator_value,
            opponent_value: result.opponent_value,
            outcome,
        });

        Ok(())
    }

    pub fn settle_duel(ctx: Context<SettleDuel>) -> Result<()> {
        let duel = &ctx.accounts.duel;
        let result = &ctx.accounts.result_commitment;
        require!(
            duel.status == DuelStatus::ReadyToSettle,
            EscrowError::InvalidStatus
        );
        require!(!result.settled, EscrowError::ResultAlreadySettled);
        validate_settlement_accounts(&ctx)?;

        match result.outcome {
            DuelOutcome::CreatorWins => {
                transfer_platform_fees(
                    duel,
                    &ctx.accounts.payment_vault,
                    &ctx.accounts.payment_mint,
                    &ctx.accounts.fee_destination,
                    &ctx.accounts.token_program,
                )?;
                transfer_card_from_vault(
                    duel,
                    &ctx.accounts.creator_card_vault,
                    &ctx.accounts.creator_card_mint,
                    &ctx.accounts.creator_card_destination,
                    &ctx.accounts.token_program,
                )?;
                transfer_card_from_vault(
                    duel,
                    &ctx.accounts.opponent_card_vault,
                    &ctx.accounts.opponent_card_mint,
                    &ctx.accounts.opponent_card_destination,
                    &ctx.accounts.token_program,
                )?;
            }
            DuelOutcome::OpponentWins => {
                transfer_platform_fees(
                    duel,
                    &ctx.accounts.payment_vault,
                    &ctx.accounts.payment_mint,
                    &ctx.accounts.fee_destination,
                    &ctx.accounts.token_program,
                )?;
                transfer_card_from_vault(
                    duel,
                    &ctx.accounts.creator_card_vault,
                    &ctx.accounts.creator_card_mint,
                    &ctx.accounts.creator_card_destination,
                    &ctx.accounts.token_program,
                )?;
                transfer_card_from_vault(
                    duel,
                    &ctx.accounts.opponent_card_vault,
                    &ctx.accounts.opponent_card_mint,
                    &ctx.accounts.opponent_card_destination,
                    &ctx.accounts.token_program,
                )?;
            }
            DuelOutcome::Tie => {
                transfer_from_duel_vault(
                    duel,
                    &ctx.accounts.payment_vault,
                    &ctx.accounts.payment_mint,
                    &ctx.accounts.creator_payment_destination,
                    &ctx.accounts.token_program,
                    duel.fee_amount,
                )?;
                transfer_from_duel_vault(
                    duel,
                    &ctx.accounts.payment_vault,
                    &ctx.accounts.payment_mint,
                    &ctx.accounts.opponent_payment_destination,
                    &ctx.accounts.token_program,
                    duel.fee_amount,
                )?;
                transfer_card_from_vault(
                    duel,
                    &ctx.accounts.creator_card_vault,
                    &ctx.accounts.creator_card_mint,
                    &ctx.accounts.creator_card_destination,
                    &ctx.accounts.token_program,
                )?;
                transfer_card_from_vault(
                    duel,
                    &ctx.accounts.opponent_card_vault,
                    &ctx.accounts.opponent_card_mint,
                    &ctx.accounts.opponent_card_destination,
                    &ctx.accounts.token_program,
                )?;
            }
        }

        let fee_amount = if result.outcome == DuelOutcome::Tie {
            0
        } else {
            total_fee_deposits(duel.fee_amount)?
        };
        let duel = &mut ctx.accounts.duel;
        duel.creator_deposited = false;
        duel.opponent_deposited = false;
        duel.creator_card_deposited = false;
        duel.opponent_card_deposited = false;
        duel.status = DuelStatus::Settled;
        ctx.accounts.result_commitment.settled = true;

        emit!(DuelSettled {
            duel: duel.key(),
            result_commitment: ctx.accounts.result_commitment.key(),
            outcome: ctx.accounts.result_commitment.outcome,
            winner: ctx.accounts.result_commitment.outcome.winner(duel),
            creator_value: ctx.accounts.result_commitment.creator_value,
            opponent_value: ctx.accounts.result_commitment.opponent_value,
            fee_amount,
        });

        Ok(())
    }

    pub fn cancel_unmatched(ctx: Context<CancelUnmatched>) -> Result<()> {
        require!(
            ctx.accounts.duel.status == DuelStatus::Waiting,
            EscrowError::InvalidStatus
        );
        require!(
            !ctx.accounts.duel.opponent_deposited,
            EscrowError::OpponentAlreadyJoined
        );

        if ctx.accounts.duel.creator_deposited {
            transfer_from_duel_vault(
                &ctx.accounts.duel,
                &ctx.accounts.payment_vault,
                &ctx.accounts.payment_mint,
                &ctx.accounts.creator_destination,
                &ctx.accounts.token_program,
                ctx.accounts.duel.fee_amount,
            )?;
        }

        let duel = &mut ctx.accounts.duel;
        duel.creator_deposited = false;
        duel.status = DuelStatus::Cancelled;

        emit!(DuelCancelled {
            duel: duel.key(),
            creator: duel.creator,
        });

        Ok(())
    }

    pub fn refund_expired_payment(
        ctx: Context<RefundExpiredPayment>,
        player: Pubkey,
    ) -> Result<()> {
        require_refundable(&ctx.accounts.duel)?;
        require_keys_eq!(
            ctx.accounts.destination.owner,
            player,
            EscrowError::InvalidDestinationOwner
        );

        let role = ctx.accounts.duel.role_for_player(player)?;
        require!(
            ctx.accounts.duel.payment_deposited(role),
            EscrowError::DepositNotFound
        );

        transfer_from_duel_vault(
            &ctx.accounts.duel,
            &ctx.accounts.payment_vault,
            &ctx.accounts.payment_mint,
            &ctx.accounts.destination,
            &ctx.accounts.token_program,
            ctx.accounts.duel.fee_amount,
        )?;

        let duel = &mut ctx.accounts.duel;
        duel.clear_payment_deposit(role);
        duel.update_refund_status();

        emit!(PaymentRefunded {
            duel: duel.key(),
            player,
            role,
            amount: duel.fee_amount,
            status: duel.status,
        });

        Ok(())
    }

    pub fn refund_expired_card(ctx: Context<RefundExpiredCard>, role: PlayerRole) -> Result<()> {
        require_refundable(&ctx.accounts.duel)?;
        require!(
            ctx.accounts.duel.card_deposited(role),
            EscrowError::CardDepositNotFound
        );
        require_keys_eq!(
            ctx.accounts.card_mint.key(),
            ctx.accounts.duel.card_mint(role),
            EscrowError::ResultAssetMismatch
        );
        require_keys_eq!(
            ctx.accounts.card_vault.key(),
            ctx.accounts.duel.card_vault(role),
            EscrowError::InvalidCardVault
        );
        require_keys_eq!(
            ctx.accounts.destination.owner,
            ctx.accounts.duel.player_for_role(role),
            EscrowError::InvalidDestinationOwner
        );

        transfer_card_from_vault(
            &ctx.accounts.duel,
            &ctx.accounts.card_vault,
            &ctx.accounts.card_mint,
            &ctx.accounts.destination,
            &ctx.accounts.token_program,
        )?;

        let player = ctx.accounts.duel.player_for_role(role);
        let duel = &mut ctx.accounts.duel;
        duel.clear_card_deposit(role);
        duel.update_refund_status();

        emit!(CardAssetRefunded {
            duel: duel.key(),
            player,
            role,
            mint: ctx.accounts.card_mint.key(),
            status: duel.status,
        });

        Ok(())
    }
}

fn transfer_checked<'info>(
    from: &Account<'info, TokenAccount>,
    mint: &Account<'info, Mint>,
    to: &Account<'info, TokenAccount>,
    authority: &Signer<'info>,
    token_program: &Program<'info, Token>,
    amount: u64,
) -> Result<()> {
    token::transfer_checked(
        CpiContext::new(
            token_program.key(),
            TransferChecked {
                from: from.to_account_info(),
                mint: mint.to_account_info(),
                to: to.to_account_info(),
                authority: authority.to_account_info(),
            },
        ),
        amount,
        mint.decimals,
    )
}

fn transfer_from_duel_vault<'info>(
    duel: &Account<'info, Duel>,
    vault: &Account<'info, TokenAccount>,
    mint: &Account<'info, Mint>,
    destination: &Account<'info, TokenAccount>,
    token_program: &Program<'info, Token>,
    amount: u64,
) -> Result<()> {
    let creator = duel.creator;
    let nonce = duel.nonce.to_le_bytes();
    let bump = [duel.bump];
    let signer_seeds = [DUEL_SEED, creator.as_ref(), nonce.as_ref(), bump.as_ref()];

    token::transfer_checked(
        CpiContext::new_with_signer(
            token_program.key(),
            TransferChecked {
                from: vault.to_account_info(),
                mint: mint.to_account_info(),
                to: destination.to_account_info(),
                authority: duel.to_account_info(),
            },
            &[&signer_seeds],
        ),
        amount,
        mint.decimals,
    )
}

fn transfer_card_from_vault<'info>(
    duel: &Account<'info, Duel>,
    vault: &Account<'info, TokenAccount>,
    mint: &Account<'info, Mint>,
    destination: &Account<'info, TokenAccount>,
    token_program: &Program<'info, Token>,
) -> Result<()> {
    transfer_from_duel_vault(duel, vault, mint, destination, token_program, 1)
}

fn transfer_platform_fees<'info>(
    duel: &Account<'info, Duel>,
    payment_vault: &Account<'info, TokenAccount>,
    payment_mint: &Account<'info, Mint>,
    fee_destination: &Account<'info, TokenAccount>,
    token_program: &Program<'info, Token>,
) -> Result<()> {
    let fee = total_fee_deposits(duel.fee_amount)?;
    transfer_from_duel_vault(
        duel,
        payment_vault,
        payment_mint,
        fee_destination,
        token_program,
        fee,
    )?;
    Ok(())
}

fn total_fee_deposits(fee_amount: u64) -> Result<u64> {
    let total = fee_amount
        .checked_mul(2)
        .ok_or(EscrowError::ArithmeticOverflow)?;
    Ok(total)
}

fn determine_outcome(creator_value: u64, opponent_value: u64) -> DuelOutcome {
    match creator_value.cmp(&opponent_value) {
        std::cmp::Ordering::Greater => DuelOutcome::CreatorWins,
        std::cmp::Ordering::Less => DuelOutcome::OpponentWins,
        std::cmp::Ordering::Equal => DuelOutcome::Tie,
    }
}

fn require_before_expiry(duel: &Account<Duel>) -> Result<()> {
    require!(
        Clock::get()?.unix_timestamp < duel.expires_at,
        EscrowError::DuelExpired
    );
    Ok(())
}

fn require_refundable(duel: &Account<Duel>) -> Result<()> {
    require!(
        matches!(
            duel.status,
            DuelStatus::Waiting
                | DuelStatus::Funded
                | DuelStatus::AwaitingResult
                | DuelStatus::Refunding
        ),
        EscrowError::InvalidStatus
    );
    require!(
        Clock::get()?.unix_timestamp >= duel.expires_at,
        EscrowError::DuelNotExpired
    );
    Ok(())
}

fn validate_initialization(args: &InitializeDuelArgs, creator: Pubkey, now: i64) -> Result<()> {
    require!(args.fee_amount > 0, EscrowError::InvalidFeeAmount);
    require!(
        args.provider_signer != Pubkey::default(),
        EscrowError::InvalidProviderSigner
    );
    require!(
        args.fee_recipient != Pubkey::default(),
        EscrowError::InvalidFeeRecipient
    );
    require!(
        args.valuation_policy_hash != [0; 32],
        EscrowError::InvalidValuationPolicy
    );
    if let Some(opponent) = args.opponent {
        require_keys_neq!(opponent, creator, EscrowError::InvalidOpponent);
        require!(opponent != Pubkey::default(), EscrowError::InvalidOpponent);
    }

    let duration = args
        .expires_at
        .checked_sub(now)
        .ok_or(EscrowError::InvalidExpiry)?;
    require!(
        (MIN_DUEL_DURATION_SECONDS..=MAX_DUEL_DURATION_SECONDS).contains(&duration),
        EscrowError::InvalidExpiry
    );

    Ok(())
}

fn validate_settlement_accounts(ctx: &Context<SettleDuel>) -> Result<()> {
    let duel = &ctx.accounts.duel;
    let result = &ctx.accounts.result_commitment;
    require_keys_eq!(result.duel, duel.key(), EscrowError::ResultDuelMismatch);
    require_keys_eq!(
        duel.result_commitment,
        result.key(),
        EscrowError::ResultDuelMismatch
    );
    require_keys_eq!(
        result.creator,
        duel.creator,
        EscrowError::ResultPlayerMismatch
    );
    require_keys_eq!(
        result.opponent,
        duel.opponent,
        EscrowError::ResultPlayerMismatch
    );
    require_keys_eq!(
        result.creator_card_mint,
        duel.creator_card_mint,
        EscrowError::ResultAssetMismatch
    );
    require_keys_eq!(
        result.opponent_card_mint,
        duel.opponent_card_mint,
        EscrowError::ResultAssetMismatch
    );
    require!(
        result.valuation_policy_hash == duel.valuation_policy_hash,
        EscrowError::ValuationPolicyMismatch
    );
    require_keys_eq!(
        ctx.accounts.creator_card_vault.key(),
        duel.creator_card_vault,
        EscrowError::InvalidCardVault
    );
    require_keys_eq!(
        ctx.accounts.opponent_card_vault.key(),
        duel.opponent_card_vault,
        EscrowError::InvalidCardVault
    );

    let creator_card_owner = match result.outcome {
        DuelOutcome::CreatorWins | DuelOutcome::Tie => duel.creator,
        DuelOutcome::OpponentWins => duel.opponent,
    };
    let opponent_card_owner = match result.outcome {
        DuelOutcome::CreatorWins => duel.creator,
        DuelOutcome::OpponentWins | DuelOutcome::Tie => duel.opponent,
    };
    require_keys_eq!(
        ctx.accounts.creator_card_destination.owner,
        creator_card_owner,
        EscrowError::InvalidDestinationOwner
    );
    require_keys_eq!(
        ctx.accounts.opponent_card_destination.owner,
        opponent_card_owner,
        EscrowError::InvalidDestinationOwner
    );
    require_keys_eq!(
        ctx.accounts.creator_payment_destination.owner,
        duel.creator,
        EscrowError::InvalidDestinationOwner
    );
    require_keys_eq!(
        ctx.accounts.opponent_payment_destination.owner,
        duel.opponent,
        EscrowError::InvalidDestinationOwner
    );
    require_keys_eq!(
        ctx.accounts.fee_destination.owner,
        duel.fee_recipient,
        EscrowError::InvalidFeeDestination
    );
    Ok(())
}

#[derive(Accounts)]
#[instruction(args: InitializeDuelArgs)]
pub struct InitializeDuel<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,
    #[account(
        init,
        payer = creator,
        space = 8 + Duel::INIT_SPACE,
        seeds = [DUEL_SEED, creator.key().as_ref(), args.nonce.to_le_bytes().as_ref()],
        bump,
    )]
    pub duel: Account<'info, Duel>,
    #[account(
        init,
        payer = creator,
        seeds = [PAYMENT_VAULT_SEED, duel.key().as_ref()],
        bump,
        token::mint = payment_mint,
        token::authority = duel,
    )]
    pub payment_vault: Account<'info, TokenAccount>,
    pub payment_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FundDuel<'info> {
    #[account(mut)]
    pub player: Signer<'info>,
    #[account(
        mut,
        seeds = [DUEL_SEED, duel.creator.as_ref(), duel.nonce.to_le_bytes().as_ref()],
        bump = duel.bump,
        has_one = payment_mint,
        has_one = payment_vault,
    )]
    pub duel: Account<'info, Duel>,
    #[account(mut, token::mint = payment_mint, token::authority = player)]
    pub player_source: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [PAYMENT_VAULT_SEED, duel.key().as_ref()],
        bump = duel.payment_vault_bump,
        token::mint = payment_mint,
        token::authority = duel,
    )]
    pub payment_vault: Account<'info, TokenAccount>,
    pub payment_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(args: DepositCardAssetArgs)]
pub struct DepositCardAsset<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,
    #[account(
        mut,
        seeds = [DUEL_SEED, duel.creator.as_ref(), duel.nonce.to_le_bytes().as_ref()],
        bump = duel.bump,
    )]
    pub duel: Account<'info, Duel>,
    #[account(mut, token::mint = card_mint, token::authority = depositor)]
    pub depositor_source: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = depositor,
        seeds = [CARD_VAULT_SEED, duel.key().as_ref(), args.role.seed()],
        bump,
        token::mint = card_mint,
        token::authority = duel,
    )]
    pub card_vault: Account<'info, TokenAccount>,
    pub card_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(args: SubmitResultArgs)]
pub struct SubmitResult<'info> {
    #[account(mut, address = duel.provider_signer)]
    pub provider_signer: Signer<'info>,
    #[account(
        mut,
        seeds = [DUEL_SEED, duel.creator.as_ref(), duel.nonce.to_le_bytes().as_ref()],
        bump = duel.bump,
    )]
    pub duel: Account<'info, Duel>,
    #[account(
        init,
        payer = provider_signer,
        space = 8 + ResultCommitment::INIT_SPACE,
        seeds = [RESULT_SEED, provider_signer.key().as_ref(), args.provider_request_id.as_ref()],
        bump,
    )]
    pub result_commitment: Account<'info, ResultCommitment>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SettleDuel<'info> {
    pub caller: Signer<'info>,
    #[account(
        mut,
        seeds = [DUEL_SEED, duel.creator.as_ref(), duel.nonce.to_le_bytes().as_ref()],
        bump = duel.bump,
        has_one = payment_mint,
        has_one = payment_vault,
    )]
    pub duel: Account<'info, Duel>,
    #[account(mut, has_one = duel)]
    pub result_commitment: Account<'info, ResultCommitment>,
    #[account(
        mut,
        seeds = [PAYMENT_VAULT_SEED, duel.key().as_ref()],
        bump = duel.payment_vault_bump,
        token::mint = payment_mint,
        token::authority = duel,
    )]
    pub payment_vault: Account<'info, TokenAccount>,
    pub payment_mint: Account<'info, Mint>,
    #[account(mut, token::mint = payment_mint)]
    pub creator_payment_destination: Account<'info, TokenAccount>,
    #[account(mut, token::mint = payment_mint)]
    pub opponent_payment_destination: Account<'info, TokenAccount>,
    #[account(mut, token::mint = payment_mint)]
    pub fee_destination: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [CARD_VAULT_SEED, duel.key().as_ref(), CREATOR_CARD_SEED],
        bump,
        token::mint = creator_card_mint,
        token::authority = duel,
    )]
    pub creator_card_vault: Account<'info, TokenAccount>,
    pub creator_card_mint: Account<'info, Mint>,
    #[account(mut, token::mint = creator_card_mint)]
    pub creator_card_destination: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [CARD_VAULT_SEED, duel.key().as_ref(), OPPONENT_CARD_SEED],
        bump,
        token::mint = opponent_card_mint,
        token::authority = duel,
    )]
    pub opponent_card_vault: Account<'info, TokenAccount>,
    pub opponent_card_mint: Account<'info, Mint>,
    #[account(mut, token::mint = opponent_card_mint)]
    pub opponent_card_destination: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CancelUnmatched<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,
    #[account(
        mut,
        seeds = [DUEL_SEED, duel.creator.as_ref(), duel.nonce.to_le_bytes().as_ref()],
        bump = duel.bump,
        has_one = creator,
        has_one = payment_mint,
        has_one = payment_vault,
    )]
    pub duel: Account<'info, Duel>,
    #[account(
        mut,
        seeds = [PAYMENT_VAULT_SEED, duel.key().as_ref()],
        bump = duel.payment_vault_bump,
        token::mint = payment_mint,
        token::authority = duel,
    )]
    pub payment_vault: Account<'info, TokenAccount>,
    #[account(mut, token::mint = payment_mint, token::authority = creator)]
    pub creator_destination: Account<'info, TokenAccount>,
    pub payment_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct RefundExpiredPayment<'info> {
    pub caller: Signer<'info>,
    #[account(
        mut,
        seeds = [DUEL_SEED, duel.creator.as_ref(), duel.nonce.to_le_bytes().as_ref()],
        bump = duel.bump,
        has_one = payment_mint,
        has_one = payment_vault,
    )]
    pub duel: Account<'info, Duel>,
    #[account(
        mut,
        seeds = [PAYMENT_VAULT_SEED, duel.key().as_ref()],
        bump = duel.payment_vault_bump,
        token::mint = payment_mint,
        token::authority = duel,
    )]
    pub payment_vault: Account<'info, TokenAccount>,
    #[account(mut, token::mint = payment_mint)]
    pub destination: Account<'info, TokenAccount>,
    pub payment_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(role: PlayerRole)]
pub struct RefundExpiredCard<'info> {
    pub caller: Signer<'info>,
    #[account(
        mut,
        seeds = [DUEL_SEED, duel.creator.as_ref(), duel.nonce.to_le_bytes().as_ref()],
        bump = duel.bump,
    )]
    pub duel: Account<'info, Duel>,
    #[account(
        mut,
        seeds = [CARD_VAULT_SEED, duel.key().as_ref(), role.seed()],
        bump,
        token::mint = card_mint,
        token::authority = duel,
    )]
    pub card_vault: Account<'info, TokenAccount>,
    pub card_mint: Account<'info, Mint>,
    #[account(mut, token::mint = card_mint)]
    pub destination: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitializeDuelArgs {
    pub nonce: u64,
    pub opponent: Option<Pubkey>,
    pub fee_amount: u64,
    pub expires_at: i64,
    pub provider_signer: Pubkey,
    pub fee_recipient: Pubkey,
    pub valuation_policy_hash: [u8; 32],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DepositCardAssetArgs {
    pub role: PlayerRole,
    pub asset_kind: AssetKind,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SubmitResultArgs {
    pub duel: Pubkey,
    pub provider_request_id: [u8; 32],
    pub creator: Pubkey,
    pub opponent: Pubkey,
    pub creator_card_mint: Pubkey,
    pub opponent_card_mint: Pubkey,
    pub creator_asset_kind: AssetKind,
    pub opponent_asset_kind: AssetKind,
    pub valuation_policy_hash: [u8; 32],
    pub creator_value: u64,
    pub opponent_value: u64,
    pub opened_at: i64,
}

#[account]
#[derive(InitSpace)]
pub struct Duel {
    pub version: u8,
    pub bump: u8,
    pub payment_vault_bump: u8,
    pub status: DuelStatus,
    pub creator: Pubkey,
    pub opponent: Pubkey,
    pub payment_mint: Pubkey,
    pub payment_vault: Pubkey,
    pub fee_recipient: Pubkey,
    pub provider_signer: Pubkey,
    pub nonce: u64,
    pub fee_amount: u64,
    pub created_at: i64,
    pub expires_at: i64,
    pub creator_deposited: bool,
    pub opponent_deposited: bool,
    pub creator_card_deposited: bool,
    pub opponent_card_deposited: bool,
    pub creator_card_mint: Pubkey,
    pub opponent_card_mint: Pubkey,
    pub creator_card_vault: Pubkey,
    pub opponent_card_vault: Pubkey,
    pub result_commitment: Pubkey,
    pub valuation_policy_hash: [u8; 32],
}

impl Duel {
    fn depositor_role(&self, player: Pubkey) -> Result<PlayerRole> {
        require!(
            self.status == DuelStatus::Waiting,
            EscrowError::InvalidStatus
        );

        if player == self.creator {
            require!(!self.creator_deposited, EscrowError::AlreadyDeposited);
            return Ok(PlayerRole::Creator);
        }

        require!(
            self.opponent == Pubkey::default() || player == self.opponent,
            EscrowError::InvalidPlayer
        );
        require!(!self.opponent_deposited, EscrowError::AlreadyDeposited);
        Ok(PlayerRole::Opponent)
    }

    fn role_for_player(&self, player: Pubkey) -> Result<PlayerRole> {
        if player == self.creator {
            return Ok(PlayerRole::Creator);
        }
        require_keys_eq!(player, self.opponent, EscrowError::InvalidPlayer);
        Ok(PlayerRole::Opponent)
    }

    fn player_for_role(&self, role: PlayerRole) -> Pubkey {
        match role {
            PlayerRole::Creator => self.creator,
            PlayerRole::Opponent => self.opponent,
        }
    }

    fn payment_deposited(&self, role: PlayerRole) -> bool {
        match role {
            PlayerRole::Creator => self.creator_deposited,
            PlayerRole::Opponent => self.opponent_deposited,
        }
    }

    fn clear_payment_deposit(&mut self, role: PlayerRole) {
        match role {
            PlayerRole::Creator => self.creator_deposited = false,
            PlayerRole::Opponent => self.opponent_deposited = false,
        }
    }

    fn card_deposited(&self, role: PlayerRole) -> bool {
        match role {
            PlayerRole::Creator => self.creator_card_deposited,
            PlayerRole::Opponent => self.opponent_card_deposited,
        }
    }

    fn card_mint(&self, role: PlayerRole) -> Pubkey {
        match role {
            PlayerRole::Creator => self.creator_card_mint,
            PlayerRole::Opponent => self.opponent_card_mint,
        }
    }

    fn card_vault(&self, role: PlayerRole) -> Pubkey {
        match role {
            PlayerRole::Creator => self.creator_card_vault,
            PlayerRole::Opponent => self.opponent_card_vault,
        }
    }

    fn record_card_deposit(&mut self, role: PlayerRole, mint: Pubkey, vault: Pubkey) {
        match role {
            PlayerRole::Creator => {
                self.creator_card_deposited = true;
                self.creator_card_mint = mint;
                self.creator_card_vault = vault;
            }
            PlayerRole::Opponent => {
                self.opponent_card_deposited = true;
                self.opponent_card_mint = mint;
                self.opponent_card_vault = vault;
            }
        }
    }

    fn clear_card_deposit(&mut self, role: PlayerRole) {
        match role {
            PlayerRole::Creator => self.creator_card_deposited = false,
            PlayerRole::Opponent => self.opponent_card_deposited = false,
        }
    }

    fn update_refund_status(&mut self) {
        self.status = if self.creator_deposited
            || self.opponent_deposited
            || self.creator_card_deposited
            || self.opponent_card_deposited
        {
            DuelStatus::Refunding
        } else {
            DuelStatus::Refunded
        };
    }
}

#[account]
#[derive(InitSpace)]
pub struct ResultCommitment {
    pub version: u8,
    pub bump: u8,
    pub duel: Pubkey,
    pub provider_signer: Pubkey,
    pub provider_request_id: [u8; 32],
    pub creator: Pubkey,
    pub opponent: Pubkey,
    pub creator_card_mint: Pubkey,
    pub opponent_card_mint: Pubkey,
    pub creator_asset_kind: AssetKind,
    pub opponent_asset_kind: AssetKind,
    pub valuation_policy_hash: [u8; 32],
    pub creator_value: u64,
    pub opponent_value: u64,
    pub opened_at: i64,
    pub committed_at: i64,
    pub outcome: DuelOutcome,
    pub settled: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Eq, InitSpace, PartialEq)]
pub enum DuelStatus {
    Waiting,
    Funded,
    AwaitingResult,
    ReadyToSettle,
    Refunding,
    Settled,
    Cancelled,
    Refunded,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Eq, InitSpace, PartialEq)]
pub enum PlayerRole {
    Creator,
    Opponent,
}

impl PlayerRole {
    fn seed(&self) -> &'static [u8] {
        match self {
            Self::Creator => CREATOR_CARD_SEED,
            Self::Opponent => OPPONENT_CARD_SEED,
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Eq, InitSpace, PartialEq)]
pub enum AssetKind {
    LegacySplNft,
    ProgrammableNft,
    CompressedNft,
    Token2022,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Eq, InitSpace, PartialEq)]
pub enum DuelOutcome {
    CreatorWins,
    OpponentWins,
    Tie,
}

impl DuelOutcome {
    fn winner(&self, duel: &Duel) -> Pubkey {
        match self {
            Self::CreatorWins => duel.creator,
            Self::OpponentWins => duel.opponent,
            Self::Tie => Pubkey::default(),
        }
    }
}

#[event]
pub struct DuelInitialized {
    pub duel: Pubkey,
    pub creator: Pubkey,
    pub opponent: Pubkey,
    pub payment_mint: Pubkey,
    pub fee_amount: u64,
    pub expires_at: i64,
    pub provider_signer: Pubkey,
    pub valuation_policy_hash: [u8; 32],
}

#[event]
pub struct DuelFunded {
    pub duel: Pubkey,
    pub player: Pubkey,
    pub role: PlayerRole,
    pub amount: u64,
    pub status: DuelStatus,
}

#[event]
pub struct CardAssetDeposited {
    pub duel: Pubkey,
    pub role: PlayerRole,
    pub player: Pubkey,
    pub depositor: Pubkey,
    pub mint: Pubkey,
    pub vault: Pubkey,
    pub asset_kind: AssetKind,
}

#[event]
pub struct ResultCommitted {
    pub duel: Pubkey,
    pub result_commitment: Pubkey,
    pub provider_signer: Pubkey,
    pub provider_request_id: [u8; 32],
    pub creator: Pubkey,
    pub opponent: Pubkey,
    pub creator_card_mint: Pubkey,
    pub opponent_card_mint: Pubkey,
    pub creator_asset_kind: AssetKind,
    pub opponent_asset_kind: AssetKind,
    pub valuation_policy_hash: [u8; 32],
    pub creator_value: u64,
    pub opponent_value: u64,
    pub outcome: DuelOutcome,
}

#[event]
pub struct DuelSettled {
    pub duel: Pubkey,
    pub result_commitment: Pubkey,
    pub outcome: DuelOutcome,
    pub winner: Pubkey,
    pub creator_value: u64,
    pub opponent_value: u64,
    pub fee_amount: u64,
}

#[event]
pub struct DuelCancelled {
    pub duel: Pubkey,
    pub creator: Pubkey,
}

#[event]
pub struct PaymentRefunded {
    pub duel: Pubkey,
    pub player: Pubkey,
    pub role: PlayerRole,
    pub amount: u64,
    pub status: DuelStatus,
}

#[event]
pub struct CardAssetRefunded {
    pub duel: Pubkey,
    pub player: Pubkey,
    pub role: PlayerRole,
    pub mint: Pubkey,
    pub status: DuelStatus,
}

#[error_code]
pub enum EscrowError {
    #[msg("Per-player fee deposit must be greater than zero")]
    InvalidFeeAmount,
    #[msg("Duel expiry is outside the allowed window")]
    InvalidExpiry,
    #[msg("Opponent must be a non-default wallet distinct from the creator")]
    InvalidOpponent,
    #[msg("Provider signer must be configured")]
    InvalidProviderSigner,
    #[msg("Fee recipient must be configured")]
    InvalidFeeRecipient,
    #[msg("Valuation policy hash must be configured")]
    InvalidValuationPolicy,
    #[msg("Instruction is not valid for the duel's current status")]
    InvalidStatus,
    #[msg("Wallet is not a participant in this duel")]
    InvalidPlayer,
    #[msg("This wallet has already deposited")]
    AlreadyDeposited,
    #[msg("The opponent has already joined")]
    OpponentAlreadyJoined,
    #[msg("The duel has not expired")]
    DuelNotExpired,
    #[msg("The duel is already expired")]
    DuelExpired,
    #[msg("No refundable payment deposit exists for this player")]
    DepositNotFound,
    #[msg("No refundable card deposit exists for this role")]
    CardDepositNotFound,
    #[msg("Refund or settlement destination is not owned by the required player")]
    InvalidDestinationOwner,
    #[msg("Fee destination is not owned by the committed fee recipient")]
    InvalidFeeDestination,
    #[msg("Only legacy SPL NFTs are supported by the devnet MVP")]
    UnsupportedAssetStandard,
    #[msg("Card mint must be a zero-decimal, single-supply legacy SPL mint")]
    InvalidCardMint,
    #[msg("Card depositor must be the bound player or provider signer")]
    InvalidCardDepositor,
    #[msg("A card has already been deposited for this role")]
    CardAlreadyDeposited,
    #[msg("Card vault does not match the vault committed to the duel")]
    InvalidCardVault,
    #[msg("Result commitment is bound to a different duel")]
    ResultDuelMismatch,
    #[msg("Result participant does not match the duel participant")]
    ResultPlayerMismatch,
    #[msg("Result asset does not match the card in custody")]
    ResultAssetMismatch,
    #[msg("Result valuation policy does not match the precommitted policy")]
    ValuationPolicyMismatch,
    #[msg("Provider request ID must be non-zero and globally unique per provider")]
    InvalidProviderRequest,
    #[msg("Provider result timestamp is outside the accepted duel window")]
    InvalidResultTimestamp,
    #[msg("Result has already been settled")]
    ResultAlreadySettled,
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waiting_duel() -> Duel {
        Duel {
            version: 2,
            bump: 1,
            payment_vault_bump: 2,
            status: DuelStatus::Waiting,
            creator: Pubkey::new_from_array([1; 32]),
            opponent: Pubkey::default(),
            payment_mint: Pubkey::new_from_array([2; 32]),
            payment_vault: Pubkey::new_from_array([3; 32]),
            fee_recipient: Pubkey::new_from_array([4; 32]),
            provider_signer: Pubkey::new_from_array([5; 32]),
            nonce: 7,
            fee_amount: 50_000,
            created_at: 100,
            expires_at: 200,
            creator_deposited: false,
            opponent_deposited: false,
            creator_card_deposited: false,
            opponent_card_deposited: false,
            creator_card_mint: Pubkey::default(),
            opponent_card_mint: Pubkey::default(),
            creator_card_vault: Pubkey::default(),
            opponent_card_vault: Pubkey::default(),
            result_commitment: Pubkey::default(),
            valuation_policy_hash: [9; 32],
        }
    }

    #[test]
    fn open_match_accepts_creator_and_first_opponent() {
        let duel = waiting_duel();
        assert!(duel.depositor_role(duel.creator).is_ok());
        assert!(duel.depositor_role(Pubkey::new_from_array([6; 32])).is_ok());
    }

    #[test]
    fn direct_match_rejects_third_wallet() {
        let mut duel = waiting_duel();
        duel.opponent = Pubkey::new_from_array([6; 32]);
        assert!(duel
            .depositor_role(Pubkey::new_from_array([7; 32]))
            .is_err());
    }

    #[test]
    fn result_outcome_is_deterministic_and_ties_do_not_reroll() {
        assert_eq!(determine_outcome(101, 100), DuelOutcome::CreatorWins);
        assert_eq!(determine_outcome(100, 101), DuelOutcome::OpponentWins);
        assert_eq!(determine_outcome(100, 100), DuelOutcome::Tie);
    }

    #[test]
    fn settled_platform_fee_is_exactly_both_disclosed_deposits() {
        assert_eq!(total_fee_deposits(50_000).unwrap(), 100_000);
        assert!(total_fee_deposits(u64::MAX).is_err());
    }

    #[test]
    fn refund_is_terminal_only_after_every_custodied_asset_is_cleared() {
        let mut duel = waiting_duel();
        duel.creator_deposited = true;
        duel.creator_card_deposited = true;
        duel.update_refund_status();
        assert_eq!(duel.status, DuelStatus::Refunding);

        duel.creator_deposited = false;
        duel.creator_card_deposited = false;
        duel.update_refund_status();
        assert_eq!(duel.status, DuelStatus::Refunded);
    }

    #[test]
    fn funded_match_rejects_new_payment_deposits() {
        let mut duel = waiting_duel();
        duel.status = DuelStatus::Funded;
        assert!(duel.depositor_role(duel.creator).is_err());
    }
}
