use std::str::FromStr as _;

use borsh::{BorshDeserialize, BorshSerialize};
use hyperlane_core::H256;
use hyperlane_sealevel_mailbox::instruction::{Instruction as MailboxInstruction, OutboxDispatch};
use hyperlane_sealevel_mailbox::{mailbox_message_dispatch_authority_pda_seeds, spl_noop};
use solana_program::account_info::{next_account_info, AccountInfo};
use solana_program::entrypoint::ProgramResult;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::program::{get_return_data, invoke_signed};
use solana_program::program_error::ProgramError;
use solana_program::pubkey::{Pubkey, PUBKEY_BYTES};
use solana_program::{msg, pubkey};

solana_program::declare_id!("4KdqVph6eMnS2omUBLBH2u4G6wwqxG5hzesZpsFcSWod");
solana_program::entrypoint!(process_instruction);

pub enum RegisterError {
    InvalidMailbox,
    InvalidRecipient,
}

impl From<RegisterError> for ProgramError {
    fn from(value: RegisterError) -> Self {
        ProgramError::Custom(value as u32)
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct RegisterMessage {
    /// The destination domain id
    pub destination: u32,
    /// The pubkey of the embedded users wallet
    pub embedded_user: Pubkey,
    /// Recipient warp route in hex or base58 encoding
    pub recipient: String,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum HyperlaneRegisterInstruction {
    SendRegister(RegisterMessage),
}

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = HyperlaneRegisterInstruction::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    match instruction {
        HyperlaneRegisterInstruction::SendRegister(register_message) => {
            register(program_id, accounts, register_message)
        }
    }
}

pub fn register(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    register_message: RegisterMessage,
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();
    let mailbox_program = next_account_info(accounts_iter)?;

    if *mailbox_program.key != trusted_mailbox() {
        return Err(RegisterError::InvalidMailbox.into());
    }

    // Account 2: Outbox PDA.
    let mailbox_outbox_account = next_account_info(accounts_iter)?;
    // Account 3: Dispatch authority.
    let dispatch_authority_account = next_account_info(accounts_iter)?;
    let (expected_dispatch_authority_key, expected_dispatch_authority_bump) =
        Pubkey::find_program_address(mailbox_message_dispatch_authority_pda_seeds!(), program_id);

    if dispatch_authority_account.key != &expected_dispatch_authority_key {
        return Err(ProgramError::InvalidArgument);
    }

    // Account 4: System program.
    let system_program_account = next_account_info(accounts_iter)?;

    if system_program_account.key != &solana_program::system_program::id() {
        return Err(ProgramError::InvalidArgument);
    }

    // Account 5: SPL Noop program.
    let spl_noop = next_account_info(accounts_iter)?;

    if spl_noop.key != &spl_noop::id() {
        return Err(ProgramError::InvalidArgument);
    }

    // Account 6: Payer.
    let payer_info = next_account_info(accounts_iter)?;
    // Account 7: Unique message account.
    // Defer to the checks in the Mailbox / IGP, no need to verify anything here.
    let unique_message_account = next_account_info(accounts_iter)?;
    // Account 8: Dispatched message PDA.
    // Similarly defer to the checks in the Mailbox to ensure account validity.
    let dispatched_message_account = next_account_info(accounts_iter)?;
    let accounts = vec![
        AccountMeta::new(*mailbox_outbox_account.key, false),
        AccountMeta::new_readonly(*dispatch_authority_account.key, true),
        AccountMeta::new_readonly(*system_program_account.key, false),
        AccountMeta::new_readonly(*spl_noop.key, false),
        AccountMeta::new(*payer_info.key, true),
        AccountMeta::new_readonly(*unique_message_account.key, true),
        AccountMeta::new(*dispatched_message_account.key, false),
    ];
    let account_infos = &[
        mailbox_outbox_account.clone(),
        dispatch_authority_account.clone(),
        system_program_account.clone(),
        spl_noop.clone(),
        payer_info.clone(),
        unique_message_account.clone(),
        dispatched_message_account.clone(),
    ];

    // TODO: IGP support?

    let dispatch_authority_seeds: &[&[u8]] =
        mailbox_message_dispatch_authority_pda_seeds!(expected_dispatch_authority_bump);
    let mut message_body = Vec::with_capacity(PUBKEY_BYTES * 2);
    message_body.extend_from_slice(&payer_info.key.to_bytes());
    message_body.extend_from_slice(&register_message.embedded_user.to_bytes());

    let recipient =
        H256::from_str(&register_message.recipient).map_err(|_| RegisterError::InvalidRecipient)?;

    let dispatch_instruction = MailboxInstruction::OutboxDispatch(OutboxDispatch {
        sender: *program_id,
        destination_domain: register_message.destination,
        recipient,
        message_body,
    });
    let mailbox_ixn = Instruction {
        program_id: *mailbox_program.key,
        data: dispatch_instruction.into_instruction_data()?,
        accounts,
    };
    // Call the Mailbox program to dispatch the message.
    invoke_signed(&mailbox_ixn, account_infos, &[dispatch_authority_seeds])?;

    // Parse the message ID from the return data from the prior dispatch.
    let (returning_program_id, returned_data) =
        get_return_data().ok_or(ProgramError::InvalidArgument)?;

    // The mailbox itself doesn't make any CPIs, but as a sanity check we confirm
    // that the return data is from the mailbox.
    if returning_program_id != *mailbox_program.key {
        return Err(ProgramError::InvalidArgument);
    }

    let message_id = H256::try_from_slice(&returned_data).expect("Mailbox returned invalid H256");
    msg!("message_id {}", message_id);

    Ok(())
}

fn trusted_mailbox() -> Pubkey {
    if cfg!(feature = "test-utils") {
        // pubkey returned by `hyperlane_test_utils::mailbox_id`
        // the id of the mailbox in tests
        pubkey!("692KZJaoe2KRcD6uhCQDLLXnLNA5ZLnfvdqjE4aX9iu1")
    } else {
        // Solana testnet mailbox
        pubkey!("75HBBLae3ddeneJVrZeyrDfv6vb7SMC3aCpBucSXS5aR")
    }
}
