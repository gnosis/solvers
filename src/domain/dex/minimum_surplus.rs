//! Minimum surplus requirements for DEX swaps.

use {
    crate::{
        domain::{auction, eth},
        util::conv,
    },
    bigdecimal::BigDecimal,
    ethereum_types::U256,
    num::{BigUint, Integer, Zero},
    std::cmp,
};

/// DEX swap minimum surplus limits. The actual minimum surplus required for 
/// a swap is bounded by a relative amount, and an absolute Ether value. These 
/// limits are used to determine the minimum surplus requirement for a 
/// particular asset (i.e. token and amount).
#[derive(Clone, Debug)]
pub struct Limits {
    relative: BigDecimal,
    absolute: Option<eth::Ether>,
}

impl Limits {
    /// Creates a new [`Limits`] instance. Returns `None` if the `relative`
    /// minimum surplus limit is negative.
    pub fn new(relative: BigDecimal, absolute: Option<eth::Ether>) -> Option<Self> {
        (relative >= Zero::zero()).then_some(Self { relative, absolute })
    }

    /// Computes the actual minimum surplus requirement to use for an asset using the
    /// specified reference prices.
    pub fn relative(&self, asset: &eth::Asset, tokens: &auction::Tokens) -> MinimumSurplus {
        if let (Some(absolute), Some(price)) =
            (&self.absolute, tokens.reference_price(&asset.token))
        {
            let absolute = conv::ether_to_decimal(absolute);
            let amount = conv::ether_to_decimal(&eth::Ether(asset.amount))
                * conv::ether_to_decimal(&price.0);

            let absolute_as_relative = absolute / amount;
            let requirement = cmp::max(absolute_as_relative, self.relative.clone());

            MinimumSurplus(requirement)
        } else {
            MinimumSurplus(self.relative.clone())
        }
    }
}

/// A relative minimum surplus requirement.
///
/// Relative minimum surplus has saturating semantics. I.e. if adding minimum surplus to a
/// token amount would overflow a `U256`, then `U256::max_value()` is returned
/// instead.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct MinimumSurplus(BigDecimal);

impl MinimumSurplus {
    /// Adds minimum surplus requirement to the specified token amount. This can be used to 
    /// compute the minimum required buy amount.
    pub fn add(&self, amount: U256) -> U256 {
        amount.saturating_add(self.abs(&amount))
    }

    /// Subtracts minimum surplus requirement from the specified token amount. This can be used to 
    /// compute the maximum allowed sell amount.
    pub fn sub(&self, amount: U256) -> U256 {
        amount.saturating_sub(self.abs(&amount))
    }

    /// Returns the absolute minimum surplus amount.
    fn abs(&self, amount: &U256) -> U256 {
        let amount = conv::u256_to_biguint(amount);
        let (int, exp) = self.0.as_bigint_and_exponent();

        let numer = amount * int.to_biguint().expect("positive by construction");
        let denom = BigUint::from(10_u8).pow(exp.unsigned_abs().try_into().unwrap_or(u32::MAX));

        let abs = numer.div_ceil(&denom);
        conv::biguint_to_u256(&abs).unwrap_or_else(U256::max_value)
    }

    /// Returns the relative minimum surplus as a `BigDecimal` factor.
    pub fn as_factor(&self) -> &BigDecimal {
        &self.0
    }

    /// Rounds a relative minimum surplus value to the specified decimal precision.
    pub fn round(&self, arg: u32) -> Self {
        Self(self.0.round(arg as _))
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::domain::auction};

    #[test]
    fn minimum_surplus_requirement() {
        let token = |t: &str| eth::TokenAddress(t.parse().unwrap());
        let ether = |e: &str| conv::decimal_to_ether(&e.parse().unwrap()).unwrap();
        let price = |e: &str| auction::Token {
            decimals: Default::default(),
            symbol: Default::default(),
            reference_price: Some(auction::Price(ether(e))),
            available_balance: Default::default(),
            trusted: Default::default(),
        };

        let tokens = auction::Tokens(
            [
                // WETH
                (
                    token("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
                    price("1.0"),
                ),
                // USDC
                (
                    token("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
                    price("589783000.0"),
                ),
            ]
            .into_iter()
            .collect(),
        );
        let minimum_surplus = Limits {
            relative: "0.01".parse().unwrap(), // 1%
            absolute: Some(ether("0.02")),
        };

        for (asset, relative, min_buy) in [
            // Small amount: absolute minimum surplus dominates
            // 0.5 WETH * 0.02/0.5 = 0.02 WETH absolute = 4% > 1% relative
            (
                eth::Asset {
                    token: token("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
                    amount: 500_000_000_000_000_000_u128.into(), // 0.5 WETH
                },
                "0.04",
                520_000_000_000_000_000_u128,
            ),
            // Medium amount: relative minimum surplus dominates  
            // 5 WETH * 1% = 0.05 WETH > 0.02 WETH absolute
            (
                eth::Asset {
                    token: token("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
                    amount: 5_000_000_000_000_000_000_u128.into(), // 5 WETH
                },
                "0.01", 
                5_050_000_000_000_000_000_u128,
            ),
            // For USDC: relative dominates for this amount
            (
                eth::Asset {
                    token: token("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
                    amount: 10_000_000_000_u128.into(), // 10K USDC
                },
                "0.01",
                10_100_000_000_u128,
            ),
        ] {
            let relative = MinimumSurplus(relative.parse().unwrap());
            let min_buy = U256::from(min_buy);

            let computed = minimum_surplus.relative(&asset, &tokens);

            assert_eq!(computed.round(9), relative);
            assert_eq!(computed.add(asset.amount), min_buy);
        }
    }
}