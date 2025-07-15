use std::str::FromStr;
use std::sync::Arc;

use hyperlane_register_module::{config, SolanaRegistration};
use sov_hyperlane_integration::warp::{Admin, TokenKind};
use sov_hyperlane_integration::{
    HyperlaneAddress, InterchainGasPaymaster, Ism, Mailbox as RawMailbox, MerkleTreeHook, Message,
    Warp, WarpCallMessage, WarpEvent,
};
use sov_modules_api::execution_mode::Native;
use sov_modules_api::macros::config_value;
use sov_modules_api::{Base58Address, HexHash, HexString, SafeVec};
use sov_test_utils::runtime::genesis::zk::config::HighLevelZkGenesisConfig;
use sov_test_utils::runtime::TestRunner;
use sov_test_utils::{
    generate_runtime, AsUser, MockDaSpec, MockZkvm, TestUser, TransactionTestCase,
};

use crate::spec::SolanaSpec;

pub type Mailbox<S> = RawMailbox<S, SolanaRegistration<S>>;
pub type S = SolanaSpec<MockDaSpec, MockZkvm, MockZkvm, Native>;
pub type RT = TestRuntime<S>;
type WarpRouteId = HexHash;

generate_runtime! {
    name: TestRuntime,
    modules: [mailbox: Mailbox<S>, warp: Warp<S>, merkle_tree_hooks: MerkleTreeHook<S>, interchain_gas_paymaster: InterchainGasPaymaster<S>, solana_register: SolanaRegistration<S>],
    operating_mode: sov_modules_api::runtime::OperatingMode::Zk,
    minimal_genesis_config_type: sov_test_utils::runtime::genesis::zk::config::MinimalZkGenesisConfig<S>,
    runtime_trait_impl_bounds: [S::Address: HyperlaneAddress],
    kernel_type: sov_test_utils::runtime::BasicKernel<'a, S>,
    auth_type: sov_modules_api::capabilities::RollupAuthenticator<S, TestRuntime<S>>,
    auth_call_wrapper: |call| call,
}

pub fn generate_with_additional_accounts(num_accounts: usize) -> HighLevelZkGenesisConfig<S> {
    HighLevelZkGenesisConfig::generate_with_additional_accounts_and_code_commitments(
        num_accounts,
        Default::default(),
        Default::default(),
    )
}

#[allow(clippy::type_complexity)]
pub fn setup() -> (
    TestRunner<TestRuntime<S>, S>,
    TestUser<S>,
    TestUser<S>,
    TestUser<S>,
) {
    let genesis_config = generate_with_additional_accounts(3);

    let admin_account = genesis_config.additional_accounts()[0].clone();
    let extra_account = genesis_config.additional_accounts()[1].clone();
    let relayer_account = genesis_config.additional_accounts()[1].clone();

    let genesis =
        GenesisConfig::from_minimal_config(genesis_config.clone().into(), (), (), (), (), ());

    (
        TestRunner::new_with_genesis(genesis.into_genesis_params(), Default::default()),
        admin_account,
        extra_account,
        relayer_account,
    )
}

pub fn register_basic_warp_route(
    runner: &mut TestRunner<RT, S>,
    user: &TestUser<S>,
) -> WarpRouteId {
    register_warp_route_with_ism_and_token_source(runner, user, Ism::AlwaysTrust, TokenKind::Native)
}

pub fn register_warp_route_with_ism_and_token_source(
    runner: &mut TestRunner<RT, S>,
    user: &TestUser<S>,
    ism: Ism,
    token_source: TokenKind,
) -> WarpRouteId {
    // The borrow checker doesn't know that the closure runs before the end of execute transaction, so it complains about lifetimes
    // if we don't Arc the warp route id
    let warp_route_id = Arc::new(std::sync::Mutex::new(HexString([0; 32])));
    let id_ref = warp_route_id.clone();
    runner.execute_transaction(TransactionTestCase {
        input: user.create_plain_message::<RT, Warp<S>>(WarpCallMessage::Register {
            admin: Admin::InsecureOwner(user.address()),
            token_source,
            ism,
            remote_routers: SafeVec::new(),
        }),
        assert: Box::new(move |result, _| {
            assert!(
                result.tx_receipt.is_successful(),
                "Recipient was not registered successfully"
            );
            for event in result.events {
                if let TestRuntimeEvent::Warp(WarpEvent::RouteRegistered { route_id, .. }) = event {
                    *id_ref.lock().unwrap() = route_id;
                }
            }
        }),
    });
    let id = *warp_route_id.lock().unwrap();
    assert!(id != HexString([0; 32]), "Warp route was not registered");
    id
}

pub fn make_message(
    nonce: u32,
    origin: u32,
    sender: HexHash,
    destination: u32,
    recipient: HexHash,
    body: HexString,
) -> Message {
    Message {
        version: 3,
        nonce,
        origin_domain: origin,
        sender,
        dest_domain: destination,
        recipient,
        body,
    }
}

pub fn make_invalid_message(nonce: u32, recipient: HexHash, body: HexString) -> Message {
    let program_b58 = Base58Address::from_str(config::SOLANA_PROGRAM_ID).unwrap();
    let program_id = HexHash::new(program_b58.0);

    make_message(
        nonce,
        0, // wrong origin domain
        program_id,
        config_value!("HYPERLANE_BRIDGE_DOMAIN"),
        recipient,
        body,
    )
}

pub fn make_valid_message(nonce: u32, recipient: HexHash, body: HexString) -> Message {
    let program_b58 = Base58Address::from_str(config::SOLANA_PROGRAM_ID).unwrap();
    let program_id = HexHash::new(program_b58.0);

    make_message(
        nonce,
        config::HYPERLANE_SOLANA_CHAIN_ID,
        program_id,
        config_value!("HYPERLANE_BRIDGE_DOMAIN"),
        recipient,
        body,
    )
}
