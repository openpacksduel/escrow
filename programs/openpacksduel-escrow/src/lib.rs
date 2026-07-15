use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

const DUEL_SEED: &[u8] = b"duel";
const VAULT_SEED: &[u8] = b"vault";
const MAX_FEE_BPS: u16 = 1_000;
const MIN_DUEL_DURATION_SECONDS: i64 = 60;
const MAX_DUEL_DURATION_SECONDS: i64 = 7 * 24 * 60 * 60;

#[program]
pub mod openpacksduel_escrow {
    use super::*;

    pub fn initialize_duel(ctx: Context<InitializeDuel>, args: InitializeDuelArgs) -> Result<()> {
        let clock = Clock::get()?;
        validate_initialization(&args, ctx.accounts.creator.key(), clock.unix_timestamp)?;

        let duel = &mut ctx.accounts.duel;
        duel.version = 1;
        duel.bump = ctx.bumps.duel;
        duel.vault_bump = ctx.bumps.vault;
        duel.status = DuelStatus::Waiting;
        duel.creator = ctx.accounts.creator.key();
        duel.opponent = args.opponent.unwrap_or_default();
        duel.payment_mint = ctx.accounts.payment_mint.key();
        duel.vault = ctx.accounts.vault.key();
        duel.fee_recipient = args.fee_recipient;
        duel.provider_signer = args.provider_signer;
        duel.nonce = args.nonce;
        duel.stake_amount = args.stake_amount;
        duel.fee_bps = args.fee_bps;
        duel.created_at = clock.unix_timestamp;
        duel.expires_at = args.expires_at;
        duel.creator_deposited = false;
        duel.opponent_deposited = false;
        duel.valuation_policy_hash = args.valuation_policy_hash;

        emit!(DuelInitialized {
            duel: duel.key(),
            creator: duel.creator,
            opponent: duel.opponent,
            payment_mint: duel.payment_mint,
            stake_amount: duel.stake_amount,
            expires_at: duel.expires_at,
        });

        Ok(())
    }

    pub fn fund_duel(ctx: Context<FundDuel>) -> Result<()> {
        require!(
            Clock::get()?.unix_timestamp < ctx.accounts.duel.expires_at,
            EscrowError::DuelExpired
        );
        let player = ctx.accounts.player.key();
        let role = ctx.accounts.duel.depositor_role(player)?;
        let stake_amount = ctx.accounts.duel.stake_amount;

        let cpi_accounts = TransferChecked {
            from: ctx.accounts.player_source.to_account_info(),
            mint: ctx.accounts.payment_mint.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.player.to_account_info(),
        };
        token::transfer_checked(
            CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts),
            stake_amount,
            ctx.accounts.payment_mint.decimals,
        )?;

        let duel = &mut ctx.accounts.duel;
        match role {
            DepositorRole::Creator => duel.creator_deposited = true,
            DepositorRole::Opponent => {
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
            amount: stake_amount,
            status: duel.status,
        });

        Ok(())
    }

    pub fn cancel_unmatched(ctx: Context<CancelUnmatched>) -> Result<()> {
        require_eq!(
            ctx.accounts.duel.status,
            DuelStatus::Waiting,
            EscrowError::InvalidStatus
        );
        require!(
            !ctx.accounts.duel.opponent_deposited,
            EscrowError::OpponentAlreadyJoined
        );

        if ctx.accounts.duel.creator_deposited {
            transfer_from_vault(
                &ctx.accounts.duel,
                &ctx.accounts.vault,
                &ctx.accounts.payment_mint,
                &ctx.accounts.creator_destination,
                &ctx.accounts.token_program,
                ctx.accounts.duel.stake_amount,
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

    pub fn refund_expired(ctx: Context<RefundExpired>, player: Pubkey) -> Result<()> {
        let clock = Clock::get()?;
        require!(
            matches!(
                ctx.accounts.duel.status,
                DuelStatus::Waiting | DuelStatus::Funded
            ),
            EscrowError::InvalidStatus
        );
        require!(
            clock.unix_timestamp >= ctx.accounts.duel.expires_at,
            EscrowError::DuelNotExpired
        );
        require_keys_eq!(
            ctx.accounts.destination.owner,
            player,
            EscrowError::InvalidDestinationOwner
        );

        let is_creator = player == ctx.accounts.duel.creator;
        let is_opponent = player == ctx.accounts.duel.opponent;
        require!(is_creator || is_opponent, EscrowError::InvalidPlayer);
        require!(
            (is_creator && ctx.accounts.duel.creator_deposited)
                || (is_opponent && ctx.accounts.duel.opponent_deposited),
            EscrowError::DepositNotFound
        );

        transfer_from_vault(
            &ctx.accounts.duel,
            &ctx.accounts.vault,
            &ctx.accounts.payment_mint,
            &ctx.accounts.destination,
            &ctx.accounts.token_program,
            ctx.accounts.duel.stake_amount,
        )?;

        let duel = &mut ctx.accounts.duel;
        if is_creator {
            duel.creator_deposited = false;
        } else {
            duel.opponent_deposited = false;
        }
        if !duel.creator_deposited && !duel.opponent_deposited {
            duel.status = DuelStatus::Refunded;
        }

        emit!(DepositRefunded {
            duel: duel.key(),
            player,
            amount: duel.stake_amount,
            status: duel.status,
        });

        Ok(())
    }
}

fn transfer_from_vault<'info>(
    duel: &Account<'info, Duel>,
    vault: &Account<'info, TokenAccount>,
    payment_mint: &Account<'info, Mint>,
    destination: &Account<'info, TokenAccount>,
    token_program: &Program<'info, Token>,
    amount: u64,
) -> Result<()> {
    let creator = duel.creator;
    let nonce = duel.nonce.to_le_bytes();
    let bump = [duel.bump];
    let signer_seeds = [DUEL_SEED, creator.as_ref(), nonce.as_ref(), bump.as_ref()];

    let cpi_accounts = TransferChecked {
        from: vault.to_account_info(),
        mint: payment_mint.to_account_info(),
        to: destination.to_account_info(),
        authority: duel.to_account_info(),
    };
    token::transfer_checked(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            cpi_accounts,
            &[&signer_seeds],
        ),
        amount,
        payment_mint.decimals,
    )
}

fn validate_initialization(args: &InitializeDuelArgs, creator: Pubkey, now: i64) -> Result<()> {
    require!(args.stake_amount > 0, EscrowError::InvalidStakeAmount);
    require!(args.fee_bps <= MAX_FEE_BPS, EscrowError::InvalidFeeRate);
    require!(
        args.provider_signer != Pubkey::default(),
        EscrowError::InvalidProviderSigner
    );
    require!(
        args.fee_recipient != Pubkey::default(),
        EscrowError::InvalidFeeRecipient
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
        seeds = [VAULT_SEED, duel.key().as_ref()],
        bump,
        token::mint = payment_mint,
        token::authority = duel,
    )]
    pub vault: Account<'info, TokenAccount>,
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
        has_one = vault,
    )]
    pub duel: Account<'info, Duel>,
    #[account(
        mut,
        token::mint = payment_mint,
        token::authority = player,
    )]
    pub player_source: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [VAULT_SEED, duel.key().as_ref()],
        bump = duel.vault_bump,
        token::mint = payment_mint,
        token::authority = duel,
    )]
    pub vault: Account<'info, TokenAccount>,
    pub payment_mint: Account<'info, Mint>,
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
        has_one = vault,
    )]
    pub duel: Account<'info, Duel>,
    #[account(
        mut,
        seeds = [VAULT_SEED, duel.key().as_ref()],
        bump = duel.vault_bump,
        token::mint = payment_mint,
        token::authority = duel,
    )]
    pub vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        token::mint = payment_mint,
        token::authority = creator,
    )]
    pub creator_destination: Account<'info, TokenAccount>,
    pub payment_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct RefundExpired<'info> {
    pub caller: Signer<'info>,
    #[account(
        mut,
        seeds = [DUEL_SEED, duel.creator.as_ref(), duel.nonce.to_le_bytes().as_ref()],
        bump = duel.bump,
        has_one = payment_mint,
        has_one = vault,
    )]
    pub duel: Account<'info, Duel>,
    #[account(
        mut,
        seeds = [VAULT_SEED, duel.key().as_ref()],
        bump = duel.vault_bump,
        token::mint = payment_mint,
        token::authority = duel,
    )]
    pub vault: Account<'info, TokenAccount>,
    #[account(mut, token::mint = payment_mint)]
    pub destination: Account<'info, TokenAccount>,
    pub payment_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitializeDuelArgs {
    pub nonce: u64,
    pub opponent: Option<Pubkey>,
    pub stake_amount: u64,
    pub fee_bps: u16,
    pub expires_at: i64,
    pub provider_signer: Pubkey,
    pub fee_recipient: Pubkey,
    pub valuation_policy_hash: [u8; 32],
}

#[account]
#[derive(InitSpace)]
pub struct Duel {
    pub version: u8,
    pub bump: u8,
    pub vault_bump: u8,
    pub status: DuelStatus,
    pub creator: Pubkey,
    pub opponent: Pubkey,
    pub payment_mint: Pubkey,
    pub vault: Pubkey,
    pub fee_recipient: Pubkey,
    pub provider_signer: Pubkey,
    pub nonce: u64,
    pub stake_amount: u64,
    pub fee_bps: u16,
    pub created_at: i64,
    pub expires_at: i64,
    pub creator_deposited: bool,
    pub opponent_deposited: bool,
    pub valuation_policy_hash: [u8; 32],
}

impl Duel {
    fn depositor_role(&self, player: Pubkey) -> Result<DepositorRole> {
        require_eq!(self.status, DuelStatus::Waiting, EscrowError::InvalidStatus);

        if player == self.creator {
            require!(!self.creator_deposited, EscrowError::AlreadyDeposited);
            return Ok(DepositorRole::Creator);
        }

        require!(
            self.opponent == Pubkey::default() || player == self.opponent,
            EscrowError::InvalidPlayer
        );
        require!(!self.opponent_deposited, EscrowError::AlreadyDeposited);
        Ok(DepositorRole::Opponent)
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Eq, InitSpace, PartialEq)]
pub enum DuelStatus {
    Waiting,
    Funded,
    Cancelled,
    Refunded,
}

enum DepositorRole {
    Creator,
    Opponent,
}

#[event]
pub struct DuelInitialized {
    pub duel: Pubkey,
    pub creator: Pubkey,
    pub opponent: Pubkey,
    pub payment_mint: Pubkey,
    pub stake_amount: u64,
    pub expires_at: i64,
}

#[event]
pub struct DuelFunded {
    pub duel: Pubkey,
    pub player: Pubkey,
    pub amount: u64,
    pub status: DuelStatus,
}

#[event]
pub struct DuelCancelled {
    pub duel: Pubkey,
    pub creator: Pubkey,
}

#[event]
pub struct DepositRefunded {
    pub duel: Pubkey,
    pub player: Pubkey,
    pub amount: u64,
    pub status: DuelStatus,
}

#[error_code]
pub enum EscrowError {
    #[msg("Stake amount must be greater than zero")]
    InvalidStakeAmount,
    #[msg("Fee rate exceeds the protocol maximum")]
    InvalidFeeRate,
    #[msg("Duel expiry is outside the allowed window")]
    InvalidExpiry,
    #[msg("Opponent must be a non-default wallet distinct from the creator")]
    InvalidOpponent,
    #[msg("Provider signer must be configured")]
    InvalidProviderSigner,
    #[msg("Fee recipient must be configured")]
    InvalidFeeRecipient,
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
    #[msg("No refundable deposit exists for this player")]
    DepositNotFound,
    #[msg("Refund destination is not owned by the player")]
    InvalidDestinationOwner,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waiting_duel() -> Duel {
        Duel {
            version: 1,
            bump: 1,
            vault_bump: 2,
            status: DuelStatus::Waiting,
            creator: Pubkey::new_from_array([1; 32]),
            opponent: Pubkey::default(),
            payment_mint: Pubkey::new_from_array([2; 32]),
            vault: Pubkey::new_from_array([3; 32]),
            fee_recipient: Pubkey::new_from_array([4; 32]),
            provider_signer: Pubkey::new_from_array([5; 32]),
            nonce: 7,
            stake_amount: 1_000_000,
            fee_bps: 250,
            created_at: 100,
            expires_at: 200,
            creator_deposited: false,
            opponent_deposited: false,
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
    fn funded_match_rejects_new_deposits() {
        let mut duel = waiting_duel();
        duel.status = DuelStatus::Funded;
        assert!(duel.depositor_role(duel.creator).is_err());
    }
}
