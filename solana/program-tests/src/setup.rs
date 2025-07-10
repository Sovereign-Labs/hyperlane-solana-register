use hyperlane_sealevel_mailbox::protocol_fee::ProtocolFee;
use hyperlane_test_utils::mailbox_id;
use solana_program_test::{processor, BanksClient, ProgramTest};
use solana_sdk::{pubkey::Pubkey, signature::Keypair};

const LOCAL_DOMAIN: u32 = 13775;
const REMOTE_DOMAIN: u32 = 69420;
const PROTOCOL_FEE: u64 = 1_000_000_000;
const MAX_PROTOCOL_FEE: u64 = 1_000_000_001;

pub fn protocol_fee_config() -> ProtocolFee {
    ProtocolFee {
        fee: PROTOCOL_FEE,
        beneficiary: Pubkey::new_unique(),
    }
}

pub async fn init_env() -> (BanksClient, Keypair) {
    let program_id = mailbox_id();
    let mut program_test = ProgramTest::new(
        "hyperlane_sealevel_mailbox",
        program_id,
        processor!(hyperlane_sealevel_mailbox::processor::process_instruction),
    );

    program_test.add_program("spl_noop", spl_noop::id(), processor!(spl_noop::noop));

    // idk why this is added twice or if it needs to be
    let mailbox_program_id = mailbox_id();
    program_test.add_program(
        "hyperlane_sealevel_mailbox",
        mailbox_program_id,
        processor!(hyperlane_sealevel_mailbox::processor::process_instruction),
    );

    // Maybe not needed, is ISM only used for incoming messages or also outgoing
    //     program_test.add_program(
    //     "hyperlane_sealevel_test_ism",
    //     hyperlane_sealevel_test_ism::id(),
    //     processor!(hyperlane_sealevel_test_ism::program::process_instruction),
    // );

    let (banks_client, payer, _recent_blockhash) = program_test.start().await;

    (banks_client, payer)
}
