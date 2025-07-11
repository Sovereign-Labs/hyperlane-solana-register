use std::str::FromStr as _;

use borsh::{BorshDeserialize, BorshSerialize};
use hyperlane_core::H256;
use hyperlane_sealevel_mailbox::instruction::{Instruction as MailboxInstruction, OutboxDispatch};
use hyperlane_sealevel_mailbox::{mailbox_message_dispatch_authority_pda_seeds, spl_noop};
use solana_program::account_info::{next_account_info, AccountInfo};
use solana_program::entrypoint::ProgramResult;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::msg;
use solana_program::program::{get_return_data, invoke_signed};
use solana_program::program_error::ProgramError;
use solana_program::pubkey::{Pubkey, PUBKEY_BYTES};

solana_program::declare_id!("4KdqVph6eMnS2omUBLBH2u4G6wwqxG5hzesZpsFcSWod");
solana_program::entrypoint!(process_instruction);

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct RegisterMessage {
    pub destination: u32,
    pub embedded_user: Pubkey,
    pub recipient: String, // recipient warp route in hex or base58 encoding
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
    msg!("getting instruction");
    let instruction = HyperlaneRegisterInstruction::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    msg!("executing instruction");
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

    // we have a trusted mailbox
    // if *mailbox_program.key != pubkey!("75HBBLae3ddeneJVrZeyrDfv6vb7SMC3aCpBucSXS5aR") {
    //     return Err(ProgramError::InvalidArgument);
    // }

    msg!("getting first account");
    // Account 2: Outbox PDA.
    let mailbox_outbox_account = next_account_info(accounts_iter)?;

    // Account 3: Dispatch authority.
    let dispatch_authority_account = next_account_info(accounts_iter)?;
    msg!("got dispatch auth account");
    let (expected_dispatch_authority_key, expected_dispatch_authority_bump) =
        Pubkey::find_program_address(mailbox_message_dispatch_authority_pda_seeds!(), program_id);
    msg!("finding expected dispatch auth");
    if dispatch_authority_account.key != &expected_dispatch_authority_key {
        msg!("dispatch authority account didnt match expected");
        return Err(ProgramError::InvalidArgument);
    }

    msg!("getting system account");
    // Account 4: System program.
    let system_program_account = next_account_info(accounts_iter)?;
    if system_program_account.key != &solana_program::system_program::id() {
        return Err(ProgramError::InvalidArgument);
    }

    msg!("getting spl noop program");
    // Account 5: SPL Noop program.
    let spl_noop = next_account_info(accounts_iter)?;
    if spl_noop.key != &spl_noop::id() {
        return Err(ProgramError::InvalidArgument);
    }

    msg!("getting payer account");
    // Account 6: Payer.
    let payer_info = next_account_info(accounts_iter)?;

    msg!("getting unique message account");
    // Account 7: Unique message account.
    // Defer to the checks in the Mailbox / IGP, no need to verify anything here.
    let unique_message_account = next_account_info(accounts_iter)?;

    msg!("getting dispatched message account");
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

    msg!("about to send instruction");
    let dispatch_instruction = MailboxInstruction::OutboxDispatch(OutboxDispatch {
        sender: *program_id,
        destination_domain: register_message.destination,
        recipient: H256::from_str(&register_message.recipient).unwrap(),
        message_body,
    });
    let mailbox_ixn = Instruction {
        program_id: *mailbox_program.key,
        data: dispatch_instruction.into_instruction_data()?,
        accounts,
    };
    msg!("invoking.....");
    // Call the Mailbox program to dispatch the message.
    invoke_signed(&mailbox_ixn, account_infos, &[dispatch_authority_seeds])?;
    msg!("invokded");

    // Parse the message ID from the return data from the prior dispatch.
    let (returning_program_id, returned_data) =
        get_return_data().ok_or(ProgramError::InvalidArgument)?;
    msg!("got returned data");
    // The mailbox itself doesn't make any CPIs, but as a sanity check we confirm
    // that the return data is from the mailbox.
    if returning_program_id != *mailbox_program.key {
        return Err(ProgramError::InvalidArgument);
    }
    msg!("validated returning program id");
    let message_id: H256 =
        H256::try_from_slice(&returned_data).map_err(|_| ProgramError::InvalidArgument)?;
    msg!("got message id");

    msg!("register {}", message_id);

    Ok(())
}
