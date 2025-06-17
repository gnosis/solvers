//! Generic tolerance system for DEX operations.

use {
    crate::{
        domain::{auction, eth},
        util::conv,
    },
    bigdecimal::{BigDecimal, ToPrimitive},
    ethereum_types::U256,
    num::{BigUint, Integer, Zero},
    std::{cmp, marker::PhantomData},
};

/// Generic tolerance limits with both relative and absolute components.
/// The behavior is controlled by the `Policy` type parameter.
#[derive(Clone, Debug)]
pub struct Limits<Policy> {
    relative: BigDecimal,
    absolute: Option<eth::Ether>,
    _policy: PhantomData<Policy>,
}

impl<Policy> Limits<Policy> {
    /// Creates a new [`Limits`] instance. Returns `None` if the `relative`
    /// limit is outside the valid range.
    pub fn new(relative: BigDecimal, absolute: Option<eth::Ether>) -> Option<Self>
    where
        Policy: TolerancePolicy,
    {
        Policy::validate_relative(&relative).then_some(Self {
            relative,
            absolute,
            _policy: PhantomData,
        })
    }

    /// Computes the actual tolerance to use for an asset using the
    /// specified reference prices.
    pub fn relative(&self, asset: &eth::Asset, tokens: &auction::Tokens) -> Tolerance<Policy>
    where
        Policy: TolerancePolicy,
    {
        if let (Some(absolute), Some(price)) =
            (&self.absolute, tokens.reference_price(&asset.token))
        {
            let absolute = conv::ether_to_decimal(absolute);
            let amount = conv::ether_to_decimal(&eth::Ether(asset.amount))
                * conv::ether_to_decimal(&price.0);

            let absolute_as_relative = absolute / amount;
            let tolerance = Policy::combine(absolute_as_relative, self.relative.clone());

            Tolerance::new(tolerance)
        } else {
            Tolerance::new(self.relative.clone())
        }
    }
}

/// A tolerance value with saturating arithmetic semantics.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Tolerance<Policy> {
    value: BigDecimal,
    _policy: PhantomData<Policy>,
}

impl<Policy> Tolerance<Policy> {
    pub fn new(value: BigDecimal) -> Self {
        Self {
            value,
            _policy: PhantomData,
        }
    }

    /// Adds tolerance to the specified token amount.
    pub fn add(&self, amount: U256) -> U256 {
        amount.saturating_add(self.abs(&amount))
    }

    /// Subtracts tolerance from the specified token amount.
    pub fn sub(&self, amount: U256) -> U256 {
        amount.saturating_sub(self.abs(&amount))
    }

    /// Returns the absolute tolerance amount.
    fn abs(&self, amount: &U256) -> U256 {
        let amount = conv::u256_to_biguint(amount);
        let (int, exp) = self.value.as_bigint_and_exponent();

        let numer = amount * int.to_biguint().expect("positive by construction");
        let denom = BigUint::from(10_u8).pow(exp.unsigned_abs().try_into().unwrap_or(u32::MAX));

        let abs = numer.div_ceil(&denom);
        conv::biguint_to_u256(&abs).unwrap_or_else(U256::max_value)
    }

    /// Returns the tolerance as a `BigDecimal` factor.
    pub fn as_factor(&self) -> &BigDecimal {
        &self.value
    }

    /// Converts the tolerance factor into basis points.
    pub fn as_bps(&self) -> Option<u16> {
        let basis_points = self.as_factor() * BigDecimal::from(10000);
        basis_points.to_u16()
    }

    /// Rounds a tolerance value to the specified decimal precision.
    pub fn round(&self, arg: u32) -> Self {
        Self::new(self.value.round(arg as _))
    }

    /// Applies a tolerance factor to a U256.
    /// For a tolerance of 0.01 (1%), this returns value * 1.01
    pub fn apply(&self, value: eth::U256) -> eth::U256 {
        let factor = BigDecimal::from(1) + self.value.clone();
        let value_decimal = conv::u256_to_bigdecimal(&value);
        let result = value_decimal * factor;
        conv::bigdecimal_to_u256(&result).unwrap_or(eth::U256::max_value())
    }
}

/// Policy trait that defines how tolerance limits behave.
pub trait TolerancePolicy {
    /// Validates that a relative tolerance value is within acceptable bounds.
    fn validate_relative(relative: &BigDecimal) -> bool;

    /// Combines absolute and relative tolerance values according to the policy.
    fn combine(absolute_as_relative: BigDecimal, relative: BigDecimal) -> BigDecimal;
}

/// Policy for slippage tolerance - caps the relative tolerance with absolute.
#[derive(Clone, Debug, PartialEq)]
pub struct SlippagePolicy;

impl TolerancePolicy for SlippagePolicy {
    fn validate_relative(relative: &BigDecimal) -> bool {
        relative >= &Zero::zero() && relative <= &BigDecimal::from(1)
    }

    fn combine(absolute_as_relative: BigDecimal, relative: BigDecimal) -> BigDecimal {
        cmp::min(absolute_as_relative, relative)
    }
}

/// Policy for minimum surplus - ensures at least the higher of absolute or
/// relative.
#[derive(Clone, Debug, PartialEq)]
pub struct MinimumSurplusPolicy;

impl TolerancePolicy for MinimumSurplusPolicy {
    fn validate_relative(relative: &BigDecimal) -> bool {
        relative >= &Zero::zero()
    }

    fn combine(absolute_as_relative: BigDecimal, relative: BigDecimal) -> BigDecimal {
        cmp::max(absolute_as_relative, relative)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::domain::auction};

    #[test]
    fn slippage_tolerance() {
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
        let slippage = Limits::<SlippagePolicy> {
            relative: "0.01".parse().unwrap(), // 1%
            absolute: Some(ether("0.02")),
            _policy: PhantomData,
        };

        for (asset, relative, min, max) in [
            // tolerance defined by relative slippage
            (
                eth::Asset {
                    token: token("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
                    amount: 1_000_000_000_000_000_000_u128.into(),
                },
                "0.01",
                990_000_000_000_000_000_u128,
                1_010_000_000_000_000_000_u128,
            ),
            // tolerance capped by absolute slippage
            (
                eth::Asset {
                    token: token("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
                    amount: 100_000_000_000_000_000_000_u128.into(),
                },
                "0.0002",
                99_980_000_000_000_000_000_u128,
                100_020_000_000_000_000_000_u128,
            ),
        ] {
            let relative = Tolerance::<SlippagePolicy>::new(relative.parse().unwrap());
            let min = U256::from(min);
            let max = U256::from(max);

            let computed = slippage.relative(&asset, &tokens);

            assert_eq!(computed.round(9), relative);
            assert_eq!(computed.sub(asset.amount), min);
            assert_eq!(computed.add(asset.amount), max);
        }
    }

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
        let minimum_surplus = Limits::<MinimumSurplusPolicy> {
            relative: "0.01".parse().unwrap(), // 1%
            absolute: Some(ether("0.02")),
            _policy: PhantomData,
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
            let relative = Tolerance::<MinimumSurplusPolicy>::new(relative.parse().unwrap());
            let min_buy = U256::from(min_buy);

            let computed = minimum_surplus.relative(&asset, &tokens);

            assert_eq!(computed.round(9), relative);
            assert_eq!(computed.add(asset.amount), min_buy);
        }
    }
}
