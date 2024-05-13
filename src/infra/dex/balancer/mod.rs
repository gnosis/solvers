use {
    crate::{
        domain::{auction, dex, eth, order},
        util,
    },
    contracts::ethcontract::I256,
    ethereum_types::U256,
    ethrpc::current_block::CurrentBlockStream,
    std::sync::atomic::{self, AtomicU64},
    tracing::Instrument,
};

mod dto;
mod vault;

/// Bindings to the Balancer Smart Order Router (SOR) API.
pub struct Sor {
    client: super::Client,
    endpoint: reqwest::Url,
    vault: vault::Vault,
    settlement: eth::ContractAddress,
}

pub struct Config {
    /// Stream that yields every new block.
    pub block_stream: CurrentBlockStream,

    /// The URL for the Balancer SOR API.
    pub endpoint: reqwest::Url,

    /// The address of the Balancer Vault contract.
    pub vault: eth::ContractAddress,

    /// The address of the Settlement contract.
    pub settlement: eth::ContractAddress,
}

impl Sor {
    /// An approximate gas an individual Balancer swap uses.
    ///
    /// This value was determined heuristically using a Dune query that has been
    /// lost to time... See <https://github.com/cowprotocol/services/pull/171>.
    const GAS_PER_SWAP: u64 = 88_892;

    pub fn new(config: Config) -> Self {
        Self {
            client: super::Client::new(Default::default(), config.block_stream),
            endpoint: config.endpoint,
            vault: vault::Vault::new(config.vault),
            settlement: config.settlement,
        }
    }

    pub async fn swap(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
        gas_price: auction::GasPrice,
    ) -> Result<dex::Swap, Error> {
        let query = dto::Query::from_domain(order, gas_price);
        let quote = {
            // Set up a tracing span to make debugging of API requests easier.
            // Historically, debugging API requests to external DEXs was a bit
            // of a headache.
            static ID: AtomicU64 = AtomicU64::new(0);
            let id = ID.fetch_add(1, atomic::Ordering::Relaxed);
            self.quote(&query)
                .instrument(tracing::trace_span!("quote", id = %id))
                .await?
        };

        if quote.is_empty() {
            return Err(Error::NotFound);
        }

        let (input, output) = match order.side {
            order::Side::Buy => (quote.return_amount, quote.swap_amount),
            order::Side::Sell => (quote.swap_amount, quote.return_amount),
        };

        let (max_input, min_output) = match order.side {
            order::Side::Buy => (slippage.add(input), output),
            order::Side::Sell => (input, slippage.sub(output)),
        };

        let gas = U256::from(quote.swaps.len()) * Self::GAS_PER_SWAP;
        let call = {
            let kind = match order.side {
                order::Side::Sell => vault::SwapKind::GivenIn,
                order::Side::Buy => vault::SwapKind::GivenOut,
            } as _;
            let swaps = quote
                .swaps
                .into_iter()
                .map(|swap| vault::Swap {
                    pool_id: swap.pool_id,
                    asset_in_index: swap.asset_in_index.into(),
                    asset_out_index: swap.asset_out_index.into(),
                    amount: swap.amount,
                    user_data: swap.user_data,
                })
                .collect();
            let assets = quote.token_addresses.clone();
            let funds = vault::Funds {
                sender: self.settlement.0,
                from_internal_balance: false,
                recipient: self.settlement.0,
                to_internal_balance: false,
            };
            let limits = quote
                .token_addresses
                .iter()
                .map(|token| {
                    if *token == quote.token_in {
                        // Use positive swap limit for sell amounts (that is, maximum
                        // amount that can be transferred in).
                        I256::try_from(max_input).unwrap_or_default()
                    } else if *token == quote.token_out {
                        I256::try_from(min_output)
                            .unwrap_or_default()
                            .checked_neg()
                            .expect("positive integer can't overflow negation")
                    } else {
                        I256::zero()
                    }
                })
                .collect();
            // Sufficiently large value with as many 0's as possible for some
            // small gas savings.
            let deadline = U256::one() << 255;

            self.vault
                .batch_swap(kind, swaps, assets, funds, limits, deadline)
        };

        Ok(dex::Swap {
            call,
            input: eth::Asset {
                token: eth::TokenAddress(quote.token_in),
                amount: input,
            },
            output: eth::Asset {
                token: eth::TokenAddress(quote.token_out),
                amount: output,
            },
            allowance: dex::Allowance {
                spender: self.vault.address(),
                amount: dex::Amount::new(max_input),
            },
            gas: eth::Gas(gas),
        })
    }

    async fn quote(&self, query: &dto::Query) -> Result<dto::Quote, Error> {
        let quote = util::http::roundtrip!(
            <dto::Quote, util::serialize::Never>;
            self.client
                .request(reqwest::Method::POST, self.endpoint.clone())
                .json(query)
        )
        .await?;
        Ok(quote)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no valid swap interaction could be found")]
    NotFound,
    #[error("rate limited")]
    RateLimited,
    #[error(transparent)]
    Http(util::http::Error),
}

impl From<util::http::RoundtripError<util::serialize::Never>> for Error {
    fn from(err: util::http::RoundtripError<util::serialize::Never>) -> Self {
        match err {
            util::http::RoundtripError::Http(util::http::Error::Status(status_code, _))
                if status_code.as_u16() == 429 =>
            {
                Self::RateLimited
            }
            other_err => Self::Http(other_err.into()),
        }
    }
}
