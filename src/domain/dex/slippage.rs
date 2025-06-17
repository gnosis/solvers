//! Slippage tolerance computation for DEX swaps.

pub use crate::domain::dex::tolerance::{Limits, SlippagePolicy, Tolerance};

/// DEX swap slippage limits.
pub type SlippageLimits = Limits<SlippagePolicy>;

/// A relative slippage tolerance.
pub type Slippage = Tolerance<SlippagePolicy>;

impl Slippage {
    pub fn one_percent() -> Self {
        Tolerance::new("0.01".parse().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            domain::{auction, eth},
            util::conv,
        },
    };

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
                // COW
                (
                    token("0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB"),
                    price("0.000057"),
                ),
            ]
            .into_iter()
            .collect(),
        );
        let slippage = SlippageLimits::new(
            "0.01".parse().unwrap(), // 1%
            Some(ether("0.02")),
        )
        .unwrap();

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
            // tolerance defined by relative slippage
            (
                eth::Asset {
                    token: token("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
                    amount: 1_000_000_000_u128.into(), // 1K USDC
                },
                "0.01",
                990_000_000_u128,
                1_010_000_000_u128,
            ),
            // tolerance capped by absolute slippage
            // 0.02 WETH <=> 33.91 USDC, and ~0.0033910778% of 1M
            (
                eth::Asset {
                    token: token("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
                    amount: 1_000_000_000_000_u128.into(), // 1M USDC
                },
                "0.000033911",
                999_966_089_222_u128,
                1_000_033_910_778_u128,
            ),
            // tolerance defined by relative slippage
            (
                eth::Asset {
                    token: token("0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB"),
                    amount: 1_000_000_000_000_000_000_000_u128.into(), // 1K COW
                },
                "0.01",
                990_000_000_000_000_000_000_u128,
                1_010_000_000_000_000_000_000_u128,
            ),
            // tolerance capped by absolute slippage
            // 0.02 WETH <=> 350.88 COW, and ~0.0350877192982456140351% of 1M
            (
                eth::Asset {
                    token: token("0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB"),
                    amount: 1_000_000_000_000_000_000_000_000_u128.into(), // 1M COW
                },
                "0.000350877",
                999_649_122_807_017_543_859_649_u128,
                1_000_350_877_192_982_456_140_351_u128,
            ),
        ] {
            let relative = Slippage::new(relative.parse().unwrap());
            let min = ethereum_types::U256::from(min);
            let max = ethereum_types::U256::from(max);

            let computed = slippage.relative(&asset, &tokens);

            assert_eq!(computed.round(9), relative);
            assert_eq!(computed.sub(asset.amount), min);
            assert_eq!(computed.add(asset.amount), max);
        }
    }

    #[test]
    fn round_does_not_panic() {
        let slippage = Slippage::new(
            "42.115792089237316195423570985008687907853269984665640564039457584007913129639935"
                .parse()
                .unwrap(),
        );

        assert_eq!(slippage.round(4), Slippage::new("42.1158".parse().unwrap()));
    }
}
