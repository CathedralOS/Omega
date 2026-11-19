/*
Bounded recovery of canonical rows, source envelopes, and inert policy.

We validate canonical row framing for local reconstruction and own both
directions of the public recovered-row envelope. We do not produce
package-review comparison rows; that belongs to `encode`. `policy/` restores
receipt-free typed policy without compiler replay.
*/

mod policy;

pub use policy::PackagePolicyRecoveryLimits;
pub(crate) use policy::{PackagePolicyRecoveryError, PackagePolicyRecoveryUsage};
