use ethereum_types::U256;

/// A supported Ethereum Chain ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChainId {
    Mainnet = 1,
    Goerli = 5,
    Gnosis = 100,
    Base = 8453,
    ArbitrumOne = 42161,
}

impl ChainId {
    pub fn new(value: U256) -> Result<Self, UnsupportedChain> {
        // Check to avoid panics for large `U256` values, as there is no checked
        // conversion API available and we don't support chains with IDs greater
        // than `u64::MAX` anyway.
        if value > U256::from(u64::MAX) {
            return Err(UnsupportedChain);
        }

        match value.as_u64() {
            1 => Ok(Self::Mainnet),
            5 => Ok(Self::Goerli),
            100 => Ok(Self::Gnosis),
            8453 => Ok(Self::Base),
            42161 => Ok(Self::ArbitrumOne),
            _ => Err(UnsupportedChain),
        }
    }

    /// Returns the network ID for the chain.
    pub fn network_id(self) -> &'static str {
        match self {
            ChainId::Mainnet => "1",
            ChainId::Goerli => "5",
            ChainId::Gnosis => "100",
            ChainId::Base => "8453",
            ChainId::ArbitrumOne => "42161",
        }
    }

    /// Returns the chain ID as a numeric value.
    pub fn value(self) -> U256 {
        U256::from(self as u64)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unsupported chain")]
pub struct UnsupportedChain;
