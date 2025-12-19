//! The domain object representing a CoW Protocol order.

use {
    crate::{domain::eth, util},
    std::fmt::{self, Debug, Display, Formatter},
};

/// A CoW Protocol order in the auction.
#[derive(Debug, Clone)]
pub struct Order {
    pub uid: Uid,
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub side: Side,
    pub class: Class,
    pub partially_fillable: bool,
}

impl Order {
    /// Returns the order's owner address.
    pub fn owner(&self) -> eth::Address {
        eth::Address::from_slice(&self.uid.0[32..52])
    }

    /// Returns `true` if the order expects a solver-computed fee.
    pub fn solver_determines_fee(&self) -> bool {
        self.class == Class::Limit
    }
}

/// UID of an order.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct Uid(pub [u8; 56]);

impl Debug for Uid {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_tuple("Uid")
            .field(&util::fmt::Hex(&self.0))
            .finish()
    }
}

impl Display for Uid {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(&util::fmt::Hex(&self.0), f)
    }
}

/// The trading side of an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// An order with a fixed buy amount and maximum sell amount.
    Buy,
    /// An order with a fixed sell amount and a minimum buy amount.
    Sell,
}

/// The order classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Market,
    Limit,
}
