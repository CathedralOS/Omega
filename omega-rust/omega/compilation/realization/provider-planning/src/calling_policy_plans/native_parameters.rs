//! Borrowed compiler metadata for exact semantic/native telescope projection.

use super::{
    BoundaryCallbackBinder, BoundaryDirectCallbackParameter, BoundaryNativeParameter,
    MaterializedBoundarySignature,
};
use calling_conventions::NativeCallbackDemand;

impl MaterializedBoundarySignature {
    /// Authored ABI order; compact identities remain compiler-only join keys.
    pub fn native_parameters(&self) -> &[BoundaryNativeParameter] {
        &self.native_parameters
    }

    pub fn callback_binders(&self) -> &[BoundaryCallbackBinder] {
        &self.callback_binders
    }

    pub fn callback_demands(&self) -> &[NativeCallbackDemand] {
        &self.callback_demands
    }

    pub fn direct_callback_parameters(&self) -> &[BoundaryDirectCallbackParameter] {
        &self.direct_callback_parameters
    }
}
