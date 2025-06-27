use {
    crate::{
        api::routes::Error,
        domain::{auction, eth, order},
    },
    dto::auction::*,
};

/// Converts a data transfer object into its domain object representation.
pub fn to_domain(auction: &Auction) -> Result<auction::Auction, Error> {
    Ok(auction::Auction {
        id: match auction.id {
            Some(id) => auction::Id::Solve(id),
            None => auction::Id::Quote,
        },
        tokens: auction::Tokens(
            auction
                .tokens
                .iter()
                .map(|(address, token)| {
                    (
                        eth::TokenAddress(*address),
                        auction::Token {
                            decimals: token.decimals,
                            reference_price: token
                                .reference_price
                                .map(eth::Ether)
                                .map(auction::Price),
                            available_balance: token.available_balance,
                            trusted: token.trusted,
                        },
                    )
                })
                .collect(),
        ),
        orders: auction
            .orders
            .iter()
            .map(|order| order::Order {
                uid: order::Uid(order.uid),
                sell: eth::Asset {
                    token: eth::TokenAddress(order.sell_token),
                    amount: order.sell_amount,
                },
                buy: eth::Asset {
                    token: eth::TokenAddress(order.buy_token),
                    amount: order.buy_amount,
                },
                side: match order.kind {
                    Kind::Buy => order::Side::Buy,
                    Kind::Sell => order::Side::Sell,
                },
                class: match order.class {
                    Class::Market => order::Class::Market,
                    Class::Limit => order::Class::Limit,
                },
                partially_fillable: order.partially_fillable,
            })
            .collect(),
        gas_price: auction::GasPrice(eth::Ether(auction.effective_gas_price)),
        deadline: auction::Deadline(auction.deadline),
    })
}
