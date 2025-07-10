use std::str::FromStr as _;

use sov_modules_api::{
    Base58Address, Context, CredentialId, Error as ModuleError, HexHash, HexString, Module,
    ModuleId, ModuleInfo, ModuleRestApi, Spec, TxState,
};

use sov_hyperlane_integration::{HyperlaneAddress, Ism, Recipient, Warp};

#[derive(Clone, ModuleInfo, ModuleRestApi)]
pub struct SolanaRegistration<S: Spec>
where
    S::Address: HyperlaneAddress,
{
    /// The ID of the module.
    #[id]
    pub id: ModuleId,

    /// The inner module that we will fall back to if the origin domain is not the configured
    /// solana domain or the sender is not the configured solana program id.
    #[module]
    warp: Warp<S>,

    #[module]
    accounts: sov_accounts::Accounts<S>,
}

impl<S: Spec> SolanaRegistration<S>
where
    S::Address: HyperlaneAddress,
{
    fn should_handle(&self, origin: u32, sender: HexHash) -> bool {
        let program_id = Base58Address::from_str(config::SOLANA_PROGRAM_ID).unwrap();
        origin == config::HYPERLANE_SOLANA_CHAIN_ID && sender == HexString(program_id.0)
    }

    fn unpack_body(&self, body: &[u8]) -> anyhow::Result<([u8; 32], [u8; 32])> {
        if body.len() < 64 {
            anyhow::bail!("Register message body malformed")
        } else {
            let user_pubkey: [u8; 32] = body[0..32].try_into()?;
            let embedded_pubkey: [u8; 32] = body[32..64].try_into()?;
            Ok((user_pubkey, embedded_pubkey))
        }
    }

    fn register(&mut self, body: HexString, state: &mut impl TxState<S>) -> anyhow::Result<()> {
        let (user_pubkey, embedded_pubkey) = self.unpack_body(body.as_ref())?;
        let credential_id = CredentialId::from(embedded_pubkey);
        let address = S::Address::try_from(&user_pubkey)?;
        let resolved_address =
            self.accounts
                .resolve_sender_address(&address, &credential_id, state)?;

        anyhow::ensure!(
            address == resolved_address,
            "Embedded pubkey already registered to different address"
        );

        Ok(())
    }
}

impl<S: Spec> Module for SolanaRegistration<S>
where
    S::Address: HyperlaneAddress,
{
    type Spec = S;

    type Config = ();

    type CallMessage = ();

    type Event = ();

    fn call(
        &mut self,
        _message: Self::CallMessage,
        _context: &Context<Self::Spec>,
        _state: &mut impl TxState<Self::Spec>,
    ) -> Result<(), ModuleError> {
        Err(anyhow::anyhow!("Module doesn't support calls").into())
    }
}

impl<S: Spec> Recipient<S> for SolanaRegistration<S>
where
    S::Address: HyperlaneAddress,
{
    fn ism(&self, recipient: &HexHash, state: &mut impl TxState<S>) -> anyhow::Result<Option<Ism>> {
        self.warp.ism(recipient, state)
    }

    fn default_ism(&self, state: &mut impl TxState<S>) -> anyhow::Result<Option<Ism>> {
        self.warp.default_ism(state)
    }

    fn handle(
        &mut self,
        origin: u32,
        sender: HexHash,
        recipient: &HexHash,
        body: HexString,
        state: &mut impl TxState<S>,
    ) -> anyhow::Result<()> {
        if self.should_handle(origin, sender) {
            self.register(body, state)
        } else {
            self.warp.handle(origin, sender, recipient, body, state)
        }
    }
}

mod config {
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}
