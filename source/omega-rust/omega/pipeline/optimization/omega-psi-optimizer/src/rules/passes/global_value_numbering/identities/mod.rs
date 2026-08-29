//! Obligation-free total-scalar identity entrance.
//!
//! Every catalog row descends into one identically named folder. Its `mod.rs`
//! owns the exact contract and proposal join; `laws.rs` owns only that rule's
//! closed semantic partition. Shared candidate mechanics remain here.

mod bitwise_absorbing;
mod bitwise_neutral;
mod contract;
mod model;
mod proposal;
mod saturating_multiply_zero;
mod saturating_neutral;
mod wrapping_multiply_zero;
mod wrapping_neutral;
mod wrapping_shift_zero_count;

pub use bitwise_absorbing::BitwiseAbsorbingLiteralIdentityRule;
pub use bitwise_neutral::BitwiseNeutralLiteralIdentityRule;
pub use saturating_multiply_zero::SaturatingMultiplyZeroAnnihilationRule;
pub use saturating_neutral::SaturatingNeutralArithmeticIdentityRule;
pub use wrapping_multiply_zero::WrappingMultiplyZeroAnnihilationRule;
pub use wrapping_neutral::WrappingNeutralArithmeticIdentityRule;
pub use wrapping_shift_zero_count::WrappingShiftZeroCountIdentityRule;

use model::TotalScalarIdentityShape;
