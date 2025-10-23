use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::find_program_address,
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_associated_token_account::instructions::Create;
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::state::TokenAccount;

pub struct SignerAccount<'a> {
    pub maker: &'a AccountInfo,
}

impl<'a> SignerAccount<'a> {
    pub fn check(maker: &'a AccountInfo) -> Result<Self, ProgramError> {
        if !maker.is_signer() {
            return Err(ProgramError::InvalidAccountOwner);
        }

        Ok(Self { maker })
    }
}

pub const TOKEN_2022_PROGRAM_ID: [u8; 32] = [
    0x06, 0xdd, 0xf6, 0xe1, 0xee, 0x75, 0x8f, 0xde, 0x18, 0x42, 0x5d, 0xbc, 0xe4, 0x6c, 0xcd, 0xda,
    0xb6, 0x1a, 0xfc, 0x4d, 0x83, 0xb9, 0x0d, 0x27, 0xfe, 0xbd, 0xf9, 0x28, 0xd8, 0xa1, 0x8b, 0xfc,
];

const TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET: usize = 165;
pub const TOKEN_2022_MINT_DISCRIMINATOR: u8 = 0x01;
pub const TOKEN_2022_TOKEN_ACCOUNT_DISCRIMINATOR: u8 = 0x02;

pub struct MintInterface<'a> {
    pub mint: &'a AccountInfo,
}

impl<'a> MintInterface<'a> {
    pub fn check(mint: &'a AccountInfo) -> Result<Self, ProgramError> {
        if !mint.is_owned_by(&TOKEN_2022_PROGRAM_ID) {
            if !mint.is_owned_by(&pinocchio_token::ID) {
                return Err(ProgramError::InvalidAccountOwner);
            } else {
                if mint.data_len().ne(&pinocchio_token::state::Mint::LEN) {
                    return Err(ProgramError::InvalidAccountData);
                }
            }
        } else {
            let data = mint.try_borrow_data()?;

            if data.len().ne(&pinocchio_token::state::Mint::LEN) {
                if data.len().le(&TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET) {
                    return Err(ProgramError::InvalidAccountData);
                }
                if data[TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET].ne(&TOKEN_2022_MINT_DISCRIMINATOR)
                {
                    return Err(ProgramError::InvalidAccountData);
                }
            }
        }

        Ok(Self { mint })
    }
}

pub struct AssociatedToken;

impl AssociatedToken {
    pub fn check(
        account: &AccountInfo,
        authority: &AccountInfo,
        mint: &AccountInfo,
        token_program: &AccountInfo,
    ) -> Result<(), ProgramError> {
        if !account.is_owned_by(&pinocchio_token::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }

        if account.data_len() != TokenAccount::LEN {
            return Err(ProgramError::InvalidAccountData);
        }

        if find_program_address(
            &[authority.key(), token_program.key(), mint.key()],
            &pinocchio_associated_token_account::ID,
        )
        .0
        .ne(account.key())
        {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(())
    }

    pub fn init(
        account: &AccountInfo,
        mint: &AccountInfo,
        payer: &AccountInfo,
        owner: &AccountInfo,
        system_program: &AccountInfo,
        token_program: &AccountInfo,
    ) -> Result<(), ProgramError> {
        Create {
            funding_account: payer,
            account,
            wallet: owner,
            mint,
            system_program,
            token_program,
        }
        .invoke()?;

        Ok(())
    }

    pub fn init_if_needed(
        account: &AccountInfo,
        mint: &AccountInfo,
        payer: &AccountInfo,
        owner: &AccountInfo,
        system_program: &AccountInfo,
        token_program: &AccountInfo,
    ) -> ProgramResult {
        match Self::check(account, payer, mint, token_program) {
            Ok(_) => Ok(()),
            Err(_) => Self::init(account, mint, payer, owner, system_program, token_program),
        }
    }
}

pub struct ProgramAccount;

impl ProgramAccount {
    pub fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if !account.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }

        Ok(())
    }

    pub fn init<'a, T: Sized>(
        payer: &AccountInfo,
        account: &AccountInfo,
        seeds: &[Seed<'a>],
        space: usize,
    ) -> ProgramResult {
        let lamports = Rent::get()?.minimum_balance(space);

        let signer = [Signer::from(seeds)];

        CreateAccount {
            from: payer,
            to: account,
            lamports,
            space: space as u64,
            owner: &crate::ID,
        }
        .invoke_signed(&signer)?;

        Ok(())
    }

    pub fn close(account: &AccountInfo, destination: &AccountInfo) -> ProgramResult {
        {
            let mut data = account.try_borrow_mut_data()?;
            data[0] = 0xff;
        }

        *destination.try_borrow_mut_lamports()? += *account.try_borrow_lamports()?;
        account.realloc(1, true)?;
        account.close()
    }
}
