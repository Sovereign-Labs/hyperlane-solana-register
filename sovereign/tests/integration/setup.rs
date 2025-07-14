use std::collections::HashMap;
use std::sync::Arc;

// use sov_bank::Amount;
use sov_hyperlane_integration::igp::ExchangeRateAndGasPrice;
use sov_hyperlane_integration::warp::{Admin, TokenKind};
use sov_hyperlane_integration::{
    HyperlaneAddress, InterchainGasPaymaster, InterchainGasPaymasterCallMessage, Ism,
    Mailbox as RawMailbox, MerkleTreeHook, Warp, WarpCallMessage, WarpEvent,
};
use sov_modules_api::{HexHash, HexString, SafeVec, Spec};
use sov_test_utils::runtime::genesis::zk::config::HighLevelZkGenesisConfig;
use sov_test_utils::runtime::TestRunner;
use sov_test_utils::{generate_runtime, AsUser, TestSpec, TestUser, TransactionTestCase};

pub type Mailbox<S> = RawMailbox<S, Warp<S>>;
pub type S = TestSpec;
pub type RT = TestRuntime<S>;
type WarpRouteId = HexHash;

generate_runtime! {
    name: TestRuntime,
    modules: [mailbox: Mailbox<S>, warp: Warp<S>, merkle_tree_hooks: MerkleTreeHook<S>, interchain_gas_paymaster: InterchainGasPaymaster<S>],
    operating_mode: sov_modules_api::runtime::OperatingMode::Zk,
    minimal_genesis_config_type: sov_test_utils::runtime::genesis::zk::config::MinimalZkGenesisConfig<S>,
    runtime_trait_impl_bounds: [S::Address: HyperlaneAddress],
    kernel_type: sov_test_utils::runtime::BasicKernel<'a, S>,
    auth_type: sov_modules_api::capabilities::RollupAuthenticator<S, TestRuntime<S>>,
    auth_call_wrapper: |call| call,
}

#[allow(clippy::type_complexity)]
pub fn setup() -> (
    TestRunner<TestRuntime<S>, S>,
    TestUser<S>,
    TestUser<S>,
    TestUser<S>,
) {
    let genesis_config = HighLevelZkGenesisConfig::generate_with_additional_accounts(3);

    let admin_account = genesis_config.additional_accounts()[0].clone();
    let extra_account = genesis_config.additional_accounts()[1].clone();
    let relayer_account = genesis_config.additional_accounts()[1].clone();

    let genesis = GenesisConfig::from_minimal_config(genesis_config.clone().into(), (), (), (), ());

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
