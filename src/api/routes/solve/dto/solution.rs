use {
    crate::domain::{eth, solution},
    dto::solution::*,
};

/// Creates a new solution DTO from its domain object.
pub fn from_domain(solutions: &[solution::Solution]) -> super::Solutions {
    super::Solutions {
        solutions: solutions
            .iter()
            .map(|solution| Solution {
                id: solution.id.0,
                prices: solution
                    .prices
                    .0
                    .iter()
                    .map(|(token, price)| (token.0, *price))
                    .collect(),
                trades: solution
                    .trades
                    .iter()
                    .map(|trade| match trade {
                        solution::Trade::Fulfillment(trade) => Trade::Fulfillment(Fulfillment {
                            order: OrderUid(trade.order().uid.0),
                            executed_amount: trade.executed().amount,
                            fee: trade.surplus_fee().map(|fee| fee.amount),
                        }),
                    })
                    .collect(),
                pre_interactions: interaction_data_from_domain(&solution.pre_interactions),
                post_interactions: interaction_data_from_domain(&solution.post_interactions),
                interactions: solution
                    .interactions
                    .iter()
                    .map(|interaction| match interaction {
                        solution::Interaction::Custom(interaction) => {
                            Interaction::Custom(CustomInteraction {
                                target: interaction.target,
                                value: interaction.value.0,
                                calldata: interaction.calldata.clone(),
                                internalize: interaction.internalize,
                                allowances: interaction
                                    .allowances
                                    .iter()
                                    .map(|allowance| Allowance {
                                        token: allowance.asset.token.0,
                                        amount: allowance.asset.amount,
                                        spender: allowance.spender,
                                    })
                                    .collect(),
                                inputs: interaction
                                    .inputs
                                    .iter()
                                    .map(|i| Asset {
                                        token: i.token.0,
                                        amount: i.amount,
                                    })
                                    .collect(),
                                outputs: interaction
                                    .outputs
                                    .iter()
                                    .map(|o| Asset {
                                        token: o.token.0,
                                        amount: o.amount,
                                    })
                                    .collect(),
                            })
                        }
                    })
                    .collect(),
                gas: solution.gas.map(|gas| gas.0.as_u64()),
                flashloans: None,
                wrappers: Default::default(),
            })
            .collect(),
    }
}

fn interaction_data_from_domain(interaction_data: &[eth::Interaction]) -> Vec<Call> {
    interaction_data
        .iter()
        .map(|interaction| Call {
            target: interaction.target.0,
            value: interaction.value.0,
            calldata: interaction.calldata.clone(),
        })
        .collect()
}
