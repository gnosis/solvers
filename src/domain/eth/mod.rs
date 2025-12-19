mod chain;

pub use {
    self::chain::ChainId,
    alloy::primitives::{Address, U256},
};

/// A contract address.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContractAddress(pub Address);

/// An ERC20 token address.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TokenAddress(pub Address);

impl From<Address> for TokenAddress {
    fn from(inner: Address) -> Self {
        Self(inner)
    }
}

/// An asset on the Ethereum blockchain. Represents a particular amount of a
/// particular token.
#[derive(Debug, Clone, Copy)]
pub struct Asset {
    pub amount: U256,
    pub token: TokenAddress,
}

/// An Ether amount in wei.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Ether(pub U256);

impl From<U256> for Ether {
    fn from(value: U256) -> Self {
        Self(value)
    }
}

/// Gas amount.
#[derive(Clone, Copy, Debug, Default)]
pub struct Gas(pub U256);

impl std::ops::Add for Gas {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

/// An arbitrary ethereum interaction that is required for the settlement
/// execution.
#[derive(Debug)]
pub struct Interaction {
    pub target: Address,
    pub value: Ether,
    pub calldata: Vec<u8>,
}
