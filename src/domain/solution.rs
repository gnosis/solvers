use {
    crate::{
        domain::{auction, eth, order},
        util,
    },
    ethereum_types::{Address, U256},
    std::collections::HashMap,
};

#[derive(Debug, Default, Copy, Clone)]
pub struct Id(pub u64);

impl From<u64> for Id {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

/// A solution to an auction.
#[derive(Debug, Default)]
pub struct Solution {
    pub id: Id,
    pub prices: ClearingPrices,
    pub trades: Vec<Trade>,
    pub pre_interactions: Vec<eth::Interaction>,
    pub interactions: Vec<Interaction>,
    pub post_interactions: Vec<eth::Interaction>,
    pub gas: Option<eth::Gas>,
}

impl Solution {
    /// Returns `self` with a new id.
    pub fn with_id(self, id: Id) -> Self {
        Self { id, ..self }
    }

    /// Returns `self` with eligible interactions internalized using the
    /// Settlement contract buffers.
    ///
    /// Currently, this internalizes all interactions with input/outputs where
    /// all input tokens are trusted and the settlement contract has sufficient
    /// balance to cover the output tokens.
    pub fn with_buffers_internalizations(mut self, tokens: &auction::Tokens) -> Self {
        let mut used_buffers = HashMap::new();
        for interaction in self.interactions.iter_mut() {
            let (inputs, outputs, internalize) = match interaction {
                Interaction::Custom(interaction) => (
                    &interaction.inputs[..],
                    &interaction.outputs[..],
                    &mut interaction.internalize,
                ),
            };

            let trusted_inputs = inputs.iter().all(|input| {
                matches!(
                    tokens.get(&input.token),
                    Some(auction::Token { trusted: true, .. })
                )
            });
            if inputs.is_empty() || outputs.is_empty() || !trusted_inputs {
                continue;
            }

            let Some(required_buffers) =
                outputs.iter().try_fold(HashMap::new(), |mut map, output| {
                    let amount = map.entry(output.token).or_default();
                    *amount = output.amount.checked_add(*amount)?;

                    let total = amount.checked_add(
                        used_buffers.get(&output.token).copied().unwrap_or_default(),
                    )?;
                    if total > tokens.get(&output.token)?.available_balance {
                        return None;
                    }

                    Some(map)
                })
            else {
                continue;
            };

            // Make sure to update the used buffers, this ensures that, if we
            // have two interactions that use the same token buffers, we don't
            // end up over-internalizing.
            for (token, amount) in required_buffers {
                let used = used_buffers.entry(token).or_default();
                *used = used.checked_add(amount).expect("overflow verified above");
            }

            *internalize = true;
        }

        self
    }
}

/// A solution for a settling a single order.
pub struct Single {
    /// The order included in this single order solution.
    pub order: order::Order,
    /// The total input to the swap for executing a single order.
    pub input: eth::Asset,
    /// The total output of the swap for executing a single order.
    pub output: eth::Asset,
    /// The swap interactions for the single order settlement.
    pub interactions: Vec<Interaction>,
    /// The estimated gas needed for swapping the sell amount to buy amount.
    pub gas: eth::Gas,
}

impl Single {
    /// Creates a full solution for a single order solution given gas and sell
    /// token prices.
    pub fn into_solution(
        self,
        gas_price: auction::GasPrice,
        sell_token: Option<auction::Price>,
        gas_offset: eth::Gas,
    ) -> Option<Solution> {
        let Self {
            order,
            input,
            output,
            interactions,
            gas: swap,
        } = self;

        if (order.sell.token, order.buy.token) != (input.token, output.token) {
            tracing::debug!(
                swap = ?(input.token, output.token),
                order = ?(order.sell.token, order.buy.token),
                "input/output tokens do not match",
            );
            return None;
        }

        let fee = if order.solver_determines_fee() {
            // TODO: If the order has signed `fee` amount already, we should
            // discount it from the surplus fee. ATM, users would pay both a
            // full order fee as well as a solver computed fee. Note that this
            // is fine for now, since there is no way to create limit orders
            // with non-zero fees.
            Fee::Surplus(
                sell_token?.ether_value(eth::Ether(
                    swap.0
                        .checked_add(gas_offset.0)?
                        .checked_mul(gas_price.0 .0)?,
                ))?,
            )
        } else {
            Fee::Protocol
        };
        let surplus_fee = fee.surplus().unwrap_or_default();

        // Compute total executed sell and buy amounts accounting for solver
        // fees. That is, the total amount of sell tokens transferred into the
        // contract and the total buy tokens transferred out of the contract.
        let (sell, buy) = match order.side {
            order::Side::Buy => (input.amount.checked_add(surplus_fee)?, output.amount),
            order::Side::Sell => {
                // We want to collect fees in the sell token, so we need to sell
                // `fee` more than the DEX swap. However, we don't allow
                // transferring more than `order.sell.amount` (guaranteed by the
                // Smart Contract), so we need to cap our executed amount to the
                // order's limit sell amount and compute the executed buy amount
                // accordingly.
                let sell = input
                    .amount
                    .checked_add(surplus_fee)?
                    .min(order.sell.amount);
                let buy = util::math::div_ceil(
                    sell.checked_sub(surplus_fee)?.checked_mul(output.amount)?,
                    input.amount,
                )?;
                (sell, buy)
            }
        };

        // Check order's limit price is satisfied accounting for solver
        // specified fees.
        if order.sell.amount.checked_mul(buy)? < order.buy.amount.checked_mul(sell)? {
            tracing::debug!(
                ?buy, ?sell, order = ?order,
                "order limit price not satisfied",
            );
            return None;
        }

        let executed = match order.side {
            order::Side::Buy => buy,
            order::Side::Sell => sell.checked_sub(surplus_fee)?,
        };
        Some(Solution {
            id: Default::default(),
            prices: ClearingPrices::new([
                (order.sell.token, buy),
                (order.buy.token, sell.checked_sub(surplus_fee)?),
            ]),
            pre_interactions: Default::default(),
            interactions,
            post_interactions: Default::default(),
            gas: Some(gas_offset + self.gas),
            trades: vec![Trade::Fulfillment(Fulfillment::new(order, executed, fee)?)],
        })
    }
}

/// A set of uniform clearing prices. They are represented as a mapping of token
/// addresses to price in an arbitrarily denominated price.
#[derive(Debug, Default)]
pub struct ClearingPrices(pub HashMap<eth::TokenAddress, U256>);

impl ClearingPrices {
    /// Creates a new set of clearing prices.
    pub fn new(prices: impl IntoIterator<Item = (eth::TokenAddress, U256)>) -> Self {
        Self(prices.into_iter().collect())
    }
}

/// A trade which executes an order as part of this solution.
#[derive(Debug)]
pub enum Trade {
    Fulfillment(Fulfillment),
}

/// A traded order within a solution.
#[derive(Debug)]
pub struct Fulfillment {
    order: order::Order,
    executed: U256,
    fee: Fee,
}

impl Fulfillment {
    /// Creates a new order filled to the specified amount. Returns `None` if
    /// the fill amount is incompatible with the order.
    pub fn new(order: order::Order, executed: U256, fee: Fee) -> Option<Self> {
        if matches!(fee, Fee::Surplus(_)) != order.solver_determines_fee() {
            tracing::debug!("incompatible fee type for order");
            return None;
        }

        let (full, fill) = match order.side {
            order::Side::Buy => (order.buy.amount, executed),
            order::Side::Sell => (
                order.sell.amount,
                executed.checked_add(fee.surplus().unwrap_or_default())?,
            ),
        };
        if (!order.partially_fillable && fill != full) || (order.partially_fillable && fill > full)
        {
            tracing::debug!(?fill, ?full, "invalid fill amount for orders full amount",);
            return None;
        }

        Some(Self {
            order,
            executed,
            fee,
        })
    }

    /// Get a reference to the traded order.
    pub fn order(&self) -> &order::Order {
        &self.order
    }

    /// Returns the trade execution as an asset (token address and amount).
    pub fn executed(&self) -> eth::Asset {
        let token = match self.order.side {
            order::Side::Buy => self.order.buy.token,
            order::Side::Sell => self.order.sell.token,
        };

        eth::Asset {
            token,
            amount: self.executed,
        }
    }

    /// Returns the solver computed fee that was charged to the order as an
    /// asset (token address and amount). Returns `None` if the fulfillment
    /// does not include a solver computed fee.
    pub fn surplus_fee(&self) -> Option<eth::Asset> {
        Some(eth::Asset {
            token: self.order.sell.token,
            amount: self.fee.surplus()?,
        })
    }
}

/// The fee that is charged to a user for executing an order.
#[derive(Clone, Copy, Debug)]
pub enum Fee {
    /// A protocol computed fee.
    ///
    /// That is, the fee is charged from the order's `fee_amount` that is
    /// included in the auction being solved.
    Protocol,

    /// An additional surplus fee that is charged by the solver.
    Surplus(U256),
}

impl Fee {
    /// Returns the dynamic component for the fee.
    pub fn surplus(&self) -> Option<U256> {
        match self {
            Fee::Protocol => None,
            Fee::Surplus(fee) => Some(*fee),
        }
    }
}

/// An interaction that is required to execute a solution by acquiring liquidity
/// or running some custom logic.
#[derive(Debug)]
pub enum Interaction {
    Custom(CustomInteraction),
}

/// An arbitrary interaction returned by the solver, which needs to be executed
/// to fulfill the trade.
#[derive(Debug)]
pub struct CustomInteraction {
    pub target: Address,
    pub value: eth::Ether,
    pub calldata: Vec<u8>,
    /// Indicated whether the interaction should be internalized (skips its
    /// execution as an optimization). This is only allowed under certain
    /// conditions.
    pub internalize: bool,
    /// Documents inputs of the interaction to determine whether internalization
    /// is actually legal.
    pub inputs: Vec<eth::Asset>,
    /// Documents outputs of the interaction to determine whether
    /// internalization is actually legal.
    pub outputs: Vec<eth::Asset>,
    /// Allowances required to successfully execute the interaction.
    pub allowances: Vec<Allowance>,
}

/// Approval required to make some `[CustomInteraction]` possible.
#[derive(Debug, Clone, Copy)]
pub struct Allowance {
    pub spender: Address,
    pub asset: eth::Asset,
}
