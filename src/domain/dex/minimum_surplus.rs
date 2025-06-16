//! Minimum surplus requirements for DEX swaps.

pub use crate::domain::dex::tolerance::{Limits, MinimumSurplus, Tolerance};

/// DEX swap minimum surplus limits.
pub type MinimumSurplusLimits = Limits<MinimumSurplus>;

/// A relative minimum surplus requirement.
pub type MinimumSurplus = Tolerance<MinimumSurplus>;
