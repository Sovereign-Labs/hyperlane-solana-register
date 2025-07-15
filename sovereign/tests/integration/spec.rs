use anyhow::bail;
use borsh::{BorshDeserialize, BorshSerialize};
use sov_hyperlane_integration::HyperlaneAddress;
use sov_mock_zkvm::crypto::Ed25519PublicKey;
use sov_modules_api::{
    execution_mode::{Native, WitnessGeneration},
    higher_kinded_types::{Generic, HigherKindedHelper},
    BasicAddress, CredentialId, CryptoHelper, CryptoSpec, DaSpec, GasUnit, HexHash, HexString,
    Spec, ZkVerifier, Zkvm,
};
use sov_state::DefaultStorageSpec;

/// An address displayed in the Solana style base-58 encoding.
#[derive(
    Default,
    Ord,
    PartialOrd,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    BorshDeserialize,
    BorshSerialize,
    schemars::JsonSchema,
    sov_modules_api::macros::UniversalWallet,
)]
pub struct Base58Address(#[sov_wallet(display(base58))] pub [u8; 32]);

impl From<CredentialId> for Base58Address {
    fn from(credential_id: CredentialId) -> Self {
        Self(credential_id.0 .0)
    }
}

impl From<&Ed25519PublicKey> for Base58Address {
    fn from(key: &Ed25519PublicKey) -> Self {
        let credential_id = CredentialId::from(*key.bytes());
        Base58Address::from(credential_id)
    }
}

impl TryFrom<&[u8]> for Base58Address {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() != 32 {
            anyhow::bail!(
                "Invalid base58 address. Addresses are 32 bytes but only {} bytes could be decoded",
                value.len()
            );
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(value);
        Ok(Self(key))
    }
}

impl From<[u8; 32]> for Base58Address {
    fn from(value: [u8; 32]) -> Self {
        Self(value)
    }
}

impl AsRef<[u8]> for Base58Address {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl std::fmt::Display for Base58Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&bs58::encode(&self.0).into_string())?;
        Ok(())
    }
}

impl std::str::FromStr for Base58Address {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut output = [0u8; 32];
        let bytes_decoded = bs58::decode(s).onto(&mut output)?;
        if bytes_decoded != 32 {
            bail!(
                "Invalid base58 address. Addresses are 32 bytes but only {} bytes could be decoded",
                bytes_decoded
            );
        }
        Ok(Self(output))
    }
}

impl serde::Serialize for Base58Address {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(&self.to_string())
        } else {
            serde::Serialize::serialize(&self.0, serializer)
        }
    }
}

impl<'de> serde::Deserialize<'de> for Base58Address {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let s = <String as serde::Deserialize<'_>>::deserialize(deserializer)?;
            s.parse().map_err(serde::de::Error::custom)
        } else {
            let bytes = <[u8; 32] as serde::Deserialize<'_>>::deserialize(deserializer)?;
            Ok(Self(bytes))
        }
    }
}

impl std::fmt::Debug for Base58Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Use the Display implementation which already does base58 encoding
        write!(f, "{}", self)
    }
}

impl BasicAddress for Base58Address {}

impl HyperlaneAddress for Base58Address {
    /// Convert the address to a Hyperlane sender address.
    fn to_sender(&self) -> HexHash {
        HexString(self.0)
    }
    /// Convert a Hyperlane sender address back to the original..
    fn from_sender(recipient: HexHash) -> anyhow::Result<Self> {
        Ok(Self(recipient.0))
    }
}

/// A default implementation of the [`Spec`] trait. Used for testing but can also be a good
/// starting point for implementing a custom rollup.
#[derive(
    Default,
    serde::Serialize,
    serde::Deserialize,
    BorshDeserialize,
    BorshSerialize,
    schemars::JsonSchema,
)]
#[serde(bound = "")]
pub struct SolanaSpec<Da, InnerZkvm, OuterZkvm, Mode>(
    std::marker::PhantomData<(Da, InnerZkvm, OuterZkvm, Mode)>,
);

impl<Da, InnerZkvm: Zkvm, OuterZkvm: Zkvm, M> Generic for SolanaSpec<Da, InnerZkvm, OuterZkvm, M> {
    type With<K> = SolanaSpec<Da, InnerZkvm, OuterZkvm, K>;
}

impl<Da, InnerZkvm: Zkvm, OuterZkvm: Zkvm, M> HigherKindedHelper
    for SolanaSpec<Da, InnerZkvm, OuterZkvm, M>
{
    type Inner = M;
}

mod default_impls {
    use sov_rollup_interface::execution_mode::ExecutionMode;

    use super::SolanaSpec;

    impl<Da, InnerZkvm, OuterZkvm, Mode: ExecutionMode> Clone
        for SolanaSpec<Da, InnerZkvm, OuterZkvm, Mode>
    {
        fn clone(&self) -> Self {
            Self(std::marker::PhantomData)
        }
    }

    impl<Da, InnerZkvm, OuterZkvm, Mode: ExecutionMode> PartialEq<Self>
        for SolanaSpec<Da, InnerZkvm, OuterZkvm, Mode>
    {
        fn eq(&self, _other: &Self) -> bool {
            true
        }
    }

    impl<Da, InnerZkvm, OuterZkvm, Mode: ExecutionMode> Eq
        for SolanaSpec<Da, InnerZkvm, OuterZkvm, Mode>
    {
    }

    impl<Da, InnerZkvm, OuterZkvm, Mode: ExecutionMode> core::fmt::Debug
        for SolanaSpec<Da, InnerZkvm, OuterZkvm, Mode>
    {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(
                f,
                "SolanaSpec<{}>",
                std::any::type_name::<(Da, InnerZkvm, OuterZkvm, Mode)>()
            )
        }
    }
}

impl<Da: DaSpec, InnerZkvm: Zkvm, OuterZkvm: Zkvm> Spec
    for SolanaSpec<Da, InnerZkvm, OuterZkvm, WitnessGeneration>
where
    <InnerZkvm::Verifier as ZkVerifier>::CryptoSpec: sov_modules_api::CryptoSpecExt,
    Base58Address:
        for<'a> From<&'a <<<InnerZkvm as Zkvm>::Verifier as ZkVerifier>::CryptoSpec as CryptoHelper>::ExtendedPublicKey>,
{
    type Address = Base58Address;
    type Da = Da;
    type Gas = GasUnit<2>;

    type Storage =
        sov_state::ProverStorage<DefaultStorageSpec<<Self::CryptoSpec as CryptoSpec>::Hasher>>;

    type InnerZkvm = InnerZkvm;
    type OuterZkvm = OuterZkvm;

    type CryptoSpec = <InnerZkvm::Verifier as ZkVerifier>::CryptoSpec;
}

impl<Da: DaSpec, InnerZkvm: Zkvm, OuterZkvm: Zkvm> Spec
    for SolanaSpec<Da, InnerZkvm, OuterZkvm, Native>
where
    <InnerZkvm::Verifier as ZkVerifier>::CryptoSpec: sov_modules_api::CryptoSpecExt,
    Base58Address:
        for<'a> From<&'a <<<InnerZkvm as Zkvm>::Verifier as ZkVerifier>::CryptoSpec as CryptoHelper>::ExtendedPublicKey>,
{
    type Address = Base58Address;
    type Da = Da;
    type Gas = GasUnit<2>;

    // TODO: Replace ProverStorage with an optimized impl!
    type Storage =
        sov_state::ProverStorage<DefaultStorageSpec<<Self::CryptoSpec as CryptoSpec>::Hasher>>;

    type InnerZkvm = InnerZkvm;
    type OuterZkvm = OuterZkvm;

    type CryptoSpec = <InnerZkvm::Verifier as ZkVerifier>::CryptoSpec;
}
