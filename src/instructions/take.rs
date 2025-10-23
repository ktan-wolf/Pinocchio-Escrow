use std::fmt::Pointer;

use pinocchio::{
    account_info::{self, AccountInfo},
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::create_program_address,
    ProgramResult,
};
use pinocchio_system::instructions::CreateAccountWithSeed;
use pinocchio_token::{
    instructions::{CloseAccount, Transfer},
    state::TokenAccount,
};
//use pinocchio_token::state::TokenAccount;

use crate::Escrow;

use super::helper;

pub struct TakeAccounts<'a> {
    pub taker: &'a AccountInfo,
    pub maker: &'a AccountInfo,
    pub escrow: &'a AccountInfo,
    pub mint_a: &'a AccountInfo,
    pub mint_b: &'a AccountInfo,
    pub vault: &'a AccountInfo,
    pub taker_ata_a: &'a AccountInfo,
    pub taker_ata_b: &'a AccountInfo,
    pub maker_ata_b: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for TakeAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [taker, maker, escrow, mint_a, mint_b, vault, taker_ata_a, taker_ata_b, maker_ata_b, system_program, token_program, _] =
            accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        helper::SignerAccount::check(taker)?;
        helper::ProgramAccount::check(escrow)?;
        helper::MintInterface::check(mint_a)?;
        helper::MintInterface::check(mint_b)?;
        helper::AssociatedToken::check(taker_ata_a, taker, mint_b, token_program)?;
        helper::AssociatedToken::check(vault, escrow, mint_a, token_program)?;

        Ok(Self {
            taker,
            maker,
            escrow,
            mint_a,
            mint_b,
            taker_ata_a,
            taker_ata_b,
            maker_ata_b,
            vault,
            system_program,
            token_program,
        })
    }
}

pub struct Take<'a> {
    pub accounts: TakeAccounts<'a>,
}

impl<'a> TryFrom<&'a [AccountInfo]> for Take<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let accounts = TakeAccounts::try_from(accounts)?;

        helper::AssociatedToken::init_if_needed(
            accounts.taker_ata_a,
            accounts.mint_a,
            accounts.taker,
            accounts.taker,
            accounts.system_program,
            accounts.token_program,
        )?;

        helper::AssociatedToken::init_if_needed(
            accounts.maker_ata_b,
            accounts.mint_b,
            accounts.taker,
            accounts.maker_ata_b,
            accounts.system_program,
            accounts.token_program,
        )?;

        Ok(Self { accounts })
    }
}

// impl<'a> Take<'a> {
//     pub const DISCRIMINATOR: &'a u8 = &1;
//
//     pub fn process(&mut self) -> ProgramResult {
//         let data = self.accounts.escrow.try_borrow_data()?;
//
//         let escrow = Escrow::load(&data)?;
//
//         let escrow_key = create_program_address(
//             &[b"escrow", self.accounts.maker.key(), &escrow.bump],
//             &crate::ID,
//         )?;
//
//         if &escrow_key != self.accounts.escrow.key() {
//             return Err(ProgramError::InvalidAccountOwner);
//         }
//
//         let seed_binding = escrow.seed.to_le_bytes();
//         let bump_binding = escrow.bump;
//         let escrow_seeds = [
//             Seed::from(b"escrow"),
//             Seed::from(self.accounts.maker.key().as_ref()),
//             Seed::from(&seed_binding),
//             Seed::from(&bump_binding),
//         ];
//
//         let signer = Signer::from(&escrow_seeds);
//
//         let amount = TokenAccount::get_amount(self.accounts.vault);
//
//         Transfer {
//             from: self.accounts.taker_ata_b,
//             to: self.accounts.maker_ata_b,
//             authority: self.accounts.taker,
//             amount: escrow.receive,
//         }
//         .invoke()?;
//
//         //escrow closing
//         drop(data);
//         helper::ProgramAccount::close(self.accounts.escrow, self.accounts.taker)?;
//
//         Ok(())
//     }
// }
//
impl<'a> Take<'a> {
    pub const DISCRIMINATOR: &'a u8 = &1;

    pub fn process(&mut self) -> ProgramResult {
        let data = self.accounts.escrow.try_borrow_data()?;
        let escrow = Escrow::load(&data)?;

        // Check if the escrow is valid
        let escrow_key = create_program_address(
            &[
                b"escrow",
                self.accounts.maker.key(),
                &escrow.seed.to_le_bytes(),
                &escrow.bump,
            ],
            &crate::ID,
        )?;
        if &escrow_key != self.accounts.escrow.key() {
            return Err(ProgramError::InvalidAccountOwner);
        }

        let seed_binding = escrow.seed.to_le_bytes();
        let bump_binding = escrow.bump;
        let escrow_seeds = [
            Seed::from(b"escrow"),
            Seed::from(self.accounts.maker.key().as_ref()),
            Seed::from(&seed_binding),
            Seed::from(&bump_binding),
        ];
        let signer = Signer::from(&escrow_seeds);

        let amount = TokenAccount::get_amount(self.accounts.vault);

        // Transfer from the Vault to the Taker
        Transfer {
            from: self.accounts.vault,
            to: self.accounts.taker_ata_a,
            authority: self.accounts.escrow,
            amount,
        }
        .invoke_signed(&[signer.clone()])?;

        // Close the Vault
        CloseAccount {
            account: self.accounts.vault,
            destination: self.accounts.maker,
            authority: self.accounts.escrow,
        }
        .invoke_signed(&[signer.clone()])?;

        // Transfer from the Taker to the Maker
        Transfer {
            from: self.accounts.taker_ata_b,
            to: self.accounts.maker_ata_b,
            authority: self.accounts.taker,
            amount: escrow.receive,
        }
        .invoke()?;

        // Close the Escrow
        drop(data);
        helper::ProgramAccount::close(self.accounts.escrow, self.accounts.taker)?;

        Ok(())
    }
}
