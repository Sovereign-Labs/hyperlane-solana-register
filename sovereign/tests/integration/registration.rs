use sov_hyperlane_integration::CallMessage;
use sov_modules_api::{CredentialId, HexString, SafeVec};
use sov_test_utils::{AsUser, TransactionTestCase};

use crate::setup::{
    make_invalid_message, make_valid_message, register_basic_warp_route, setup, Mailbox, RT, S,
};

#[test]
fn test_user_is_registered_correctly() {
    let (mut runner, admin, user, _relayer) = setup();
    let route_id = register_basic_warp_route(&mut runner, &admin);

    let payer = [1u8; 32];
    let embedded = [2u8; 32];
    let body = [payer, embedded].concat();
    let valid_message = make_valid_message(0, route_id, HexString::new(body));
    let message = HexString::new(SafeVec::try_from(valid_message.encode().0).unwrap());
    let credential = CredentialId::from(embedded);

    // Sanity check, ensure account definently doesnt already exist
    runner.query_state(|state| {
        let account = sov_accounts::Accounts::default().get_account(credential, state);
        assert!(matches!(account, sov_accounts::Response::AccountEmpty));
    });

    runner.execute_transaction(TransactionTestCase {
        input: user.create_plain_message::<RT, Mailbox<S>>(CallMessage::Process {
            metadata: HexString::new(SafeVec::new()),
            message,
        }),
        assert: Box::new(move |result, _| {
            assert!(
                result.tx_receipt.is_successful(),
                "Recipient was not registered successfully"
            );
        }),
    });

    runner.query_state(|state| {
        let account = sov_accounts::Accounts::default().get_account(credential, state);
        assert!(matches!(
            account,
            sov_accounts::Response::AccountExists { .. }
        ));
    });
}

#[test]
fn test_errors_if_user_already_registered() {
    let (mut runner, admin, user, _relayer) = setup();
    let route_id = register_basic_warp_route(&mut runner, &admin);

    let payer = [1u8; 32];
    let embedded = [2u8; 32];
    let body = [payer, embedded].concat();
    let valid_message = make_valid_message(0, route_id, HexString::new(body));
    let message = HexString::new(SafeVec::try_from(valid_message.encode().0).unwrap());

    runner.execute_transaction(TransactionTestCase {
        input: user.create_plain_message::<RT, Mailbox<S>>(CallMessage::Process {
            metadata: HexString::new(SafeVec::new()),
            message: message.clone(),
        }),
        assert: Box::new(move |result, _| {
            assert!(
                result.tx_receipt.is_successful(),
                "Recipient was not registered successfully"
            );
        }),
    });

    // payer is different so will try to register to different address
    let payer = [3u8; 32];
    let embedded = [2u8; 32];
    let body = [payer, embedded].concat();
    let valid_message = make_valid_message(1, route_id, HexString::new(body));
    let message = HexString::new(SafeVec::try_from(valid_message.encode().0).unwrap());

    runner.execute_transaction(TransactionTestCase {
        input: user.create_plain_message::<RT, Mailbox<S>>(CallMessage::Process {
            metadata: HexString::new(SafeVec::new()),
            message,
        }),
        assert: Box::new(move |result, _| match result.tx_receipt {
            sov_rollup_interface::stf::TxEffect::Reverted(contents) => {
                assert_eq!(
                    contents.reason.to_string(),
                    "Embedded pubkey already registered to different address".to_string()
                );
            }
            _ => panic!("Registration should have reverted: {:?}", result.tx_receipt),
        }),
    });
}

#[test]
fn test_handler_passes_through_if_not_register_message() {
    let (mut runner, admin, user, _relayer) = setup();
    let route_id = register_basic_warp_route(&mut runner, &admin);

    let payer = [1u8; 32];
    let embedded = [2u8; 32];
    let body = [payer, embedded].concat();
    let valid_message = make_invalid_message(0, route_id, HexString::new(body));
    let message = HexString::new(SafeVec::try_from(valid_message.encode().0).unwrap());
    // while the call fails, we just want to assert the handler is passed on to the warp route
    // if we get this error then we know it did because this error originates in the Warp module
    let expected_error = format!(
        "Remote router for route {route_id} and origin {} not found",
        valid_message.origin_domain
    );

    runner.execute_transaction(TransactionTestCase {
        input: user.create_plain_message::<RT, Mailbox<S>>(CallMessage::Process {
            metadata: HexString::new(SafeVec::new()),
            message,
        }),
        assert: Box::new(move |result, _| match result.tx_receipt {
            sov_rollup_interface::stf::TxEffect::Reverted(contents) => {
                assert_eq!(contents.reason.to_string(), expected_error);
            }
            _ => panic!(
                "Message should have been passed to Warp route: {:?}",
                result.tx_receipt
            ),
        }),
    });
}
