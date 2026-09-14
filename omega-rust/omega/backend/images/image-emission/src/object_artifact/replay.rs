//! Evidence replay passes for object construction. Every retained
//! machine-code record is independently re-decoded from final bytes here;
//! no producer-supplied offset, peak, or shape crosses into the artifact
//! unverified. Installation-record validation reuses the same passes.

pub(crate) mod boundary;
pub(crate) mod dynamic;
pub(crate) mod instruction_loads;
pub(crate) mod scalar;
pub(crate) mod structural;
pub(crate) mod unit;
pub(crate) mod x86_fma;
