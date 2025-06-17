//! Minimum surplus requirements for DEX swaps.

pub use crate::domain::dex::tolerance::{Limits, MinimumSurplusPolicy, Tolerance};

/// DEX swap minimum surplus limits.
pub type MinimumSurplusLimits = Limits<MinimumSurplusPolicy>;

/// A relative minimum surplus requirement.
pub type MinimumSurplus = Tolerance<MinimumSurplusPolicy>;
