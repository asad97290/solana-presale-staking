use anchor_lang::prelude::*;
use anchor_lang::solana_program::system_instruction;
use anchor_spl::{associated_token::AssociatedToken, token::{transfer, Mint, Token, TokenAccount, Transfer}};
declare_id!("G7n94bhEkqKwBkgqVALJ2AzPrugaca5XH2pWw3xy88xB");

#[program]
pub mod omerta_presale {


    use super::*;

    pub fn start_presale(
        ctx: Context<StartPresale>,
        goal: u64,
        start_time: u64,
        end_time: u64,
        price_per_token: u64,
    ) -> Result<()> {
        let presale = &mut ctx.accounts.presale;

        presale.goal = goal;
        presale.start_time = start_time;
        presale.end_time = end_time;
        presale.price_per_token = price_per_token;
        presale.is_live = true;
        presale.amount_raised = 0;
        presale.authority = ctx.accounts.signer.key();
        Ok(())
    }
    pub fn stop_presale(ctx: Context<StopPresale>) -> Result<()> {
        
        let presale = &mut ctx.accounts.presale;
        require!(!presale.is_live, CustomError::PresaleAlreadyStopped); 

        presale.is_live = false;
        Ok(())
    }

    pub fn set_token_address(ctx: Context<SetTokenAddress>) -> Result<()>{
        let presale = &mut ctx.accounts.presale;
        presale.token_mint =  ctx.accounts.token_mint.key();

        Ok(())
    }
    
    pub fn invest_sol(ctx: Context<Invest>, value: u64) -> Result<()> {
        let presale_data = &mut ctx.accounts.presale;
        
        require!(presale_data.is_live, CustomError::PresaleNotLive);


        let cur_timestamp = u64::try_from(Clock::get()?.unix_timestamp).unwrap();

        
        require!(cur_timestamp > presale_data.start_time, CustomError::PresaleNotStarted);
        require!(cur_timestamp < presale_data.end_time, CustomError::PresaleHasEnd);

        
        ctx.accounts.data.amount += value;
        
        let number_of_tokens = value/presale_data.price_per_token;
        ctx.accounts.data.number_of_tokens += number_of_tokens;
        
        presale_data.amount_raised += value;

        let from_account = &ctx.accounts.from;
        let presale = presale_data.to_account_info();


        // Create the transfer instruction
        let transfer_instruction =
            system_instruction::transfer(from_account.key, presale.key, value);

        // Invoke the transfer instruction
        anchor_lang::solana_program::program::invoke(
            &transfer_instruction,
            &[
                from_account.to_account_info(),
                presale.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        Ok(())
    }

    pub fn claim_tokens(ctx: Context<ClaimTokens>) -> Result<()> {
        let presale_data = &ctx.accounts.presale;
        require!(presale_data.is_live, CustomError::PresaleNotLive);

        // Ensure the presale has ended before allowing token claims
        let cur_timestamp = u64::try_from(Clock::get()?.unix_timestamp).unwrap();
        require!(cur_timestamp > presale_data.end_time, CustomError::PresaleHasEnd);

        let tokens_to_claim = ctx.accounts.data.number_of_tokens;
        require!(tokens_to_claim > 0, CustomError::InsufficientFunds);

        // Reset the number of tokens to prevent double-claims
        ctx.accounts.data.number_of_tokens = 0;
        
        transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.presale_token_account.to_account_info(),
                    to: ctx.accounts.signer_token_account.to_account_info(),
                    authority: ctx.accounts.presale.to_account_info(),
                },
                &[&[PRESALE_SEED, &[ctx.bumps.presale]][..]],
            ),
            tokens_to_claim,
        )?;
        
        Ok(())
    }
 
    pub fn withdraw_sol(ctx: Context<WithdrawSol>) -> Result<()> {

        let presale = &mut ctx.accounts.presale.to_account_info();
        let recipient = &ctx.accounts.signer;
        
        let presale_balance = presale.lamports();
        
        require!(presale_balance > 0 , CustomError::InsufficientFunds);

        **presale.to_account_info().try_borrow_mut_lamports()? -= presale_balance;
        **recipient.to_account_info().try_borrow_mut_lamports()? += presale_balance;
    
        Ok(())
    }

}



pub const PRESALE_SEED:&[u8] = "omerta_presale".as_bytes();
pub const DATA_SEED:&[u8] = "my_data".as_bytes();

#[derive(Accounts)]
pub struct StopPresale<'info> {
    #[account(
        mut,
        seeds = [PRESALE_SEED],
        bump
    )]
    pub presale: Account<'info, PresaleInfo>,

    #[account(
        mut,
        constraint = signer.key() == presale.authority.key() @ CustomError::Unauthorized,
    )]
    pub signer: Signer<'info>,
    
}


#[derive(Accounts)]
pub struct SetTokenAddress<'info> {
    #[account(
        constraint = token_mint.is_initialized == true,
    )]
    pub token_mint: Account<'info, Mint>, // Token mint account

    #[account(
        mut,
        seeds = [PRESALE_SEED],
        bump
    )]
    pub presale: Account<'info, PresaleInfo>,

    #[account(
        mut,
        constraint = signer.key() == presale.authority.key() @ CustomError::Unauthorized,
    )]
    pub signer: Signer<'info>,
    
}



#[account]
pub struct PresaleInfo {
    pub goal: u64,
    pub token_mint: Pubkey,
    pub amount_raised: u64,
    pub start_time: u64,
    pub end_time: u64,
    pub price_per_token: u64,
    pub is_live:bool,
    pub authority:Pubkey
}

#[derive(Accounts)]
pub struct StartPresale<'info> {
    #[account(
        init_if_needed,
        payer = signer,
          /*
        Discriminator: 8 bytes
        PresaleInfo : size of PresaleInfo
         */
        space=8 + std::mem::size_of::<PresaleInfo>(),
        seeds = [PRESALE_SEED],
        bump
    )]
    pub presale: Account<'info, PresaleInfo>,
 

    #[account(mut)]
    pub signer: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,

}



#[account]
pub struct InvestmentData {
    pub amount: u64,
    pub number_of_tokens: u64,
}
#[derive(Accounts)]
pub struct Invest<'info> {
    #[account(
        init_if_needed,
        /*
        Discriminator: 8 bytes
        InvestmentData : size of InvestmentData
         */
        space=8 + std::mem::size_of::<InvestmentData>(),
        payer=from,
        seeds=[DATA_SEED,from.key().as_ref()],
        bump

    )]
    pub data: Account<'info, InvestmentData>,

    #[account(mut)]
    pub from: Signer<'info>,
    #[account(
        mut,
        seeds = [PRESALE_SEED],
        bump
    )]
    pub presale: Account<'info,PresaleInfo>,
    pub system_program: Program<'info, System>,
}



#[derive(Accounts)]
pub struct WithdrawSol<'info> {
 
    #[account(
        mut,
        constraint = signer.key() == presale.authority.key() @ CustomError::Unauthorized,
    )]
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [PRESALE_SEED],
        bump
    )]
    pub presale: Account<'info,PresaleInfo>,
    pub system_program: Program<'info, System>,
}


#[derive(Accounts)]
pub struct ClaimTokens<'info> {
    #[account(
        mut,
        seeds = [DATA_SEED, signer.key().as_ref()],
        bump,
    )]
    pub data: Account<'info, InvestmentData>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = presale.key()
    )]
    pub presale_token_account: Account<'info, TokenAccount>,
    
    #[account(
        init_if_needed,
        payer = signer,
        associated_token::mint = token_mint,
        associated_token::authority = signer,
    )]
    pub signer_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [PRESALE_SEED],
        bump,
    )]
    pub presale: Account<'info, PresaleInfo>,

    #[account(mut)]
    pub signer: Signer<'info>,


    #[account(mut)]
    pub token_mint: Account<'info, Mint>, 
   

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}


#[error_code]
pub enum CustomError {
    #[msg("Insufficient funds")]
    InsufficientFunds,
    #[msg("Presale not live")]
    PresaleNotLive,
    #[msg("Presale not started")]
    PresaleNotStarted,
    #[msg("Presale has end")]
    PresaleHasEnd,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Presale already stopped")]
    PresaleAlreadyStopped,
}

