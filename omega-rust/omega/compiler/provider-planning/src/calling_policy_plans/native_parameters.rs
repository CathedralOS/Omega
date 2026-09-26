//! Borrowed compiler metadata for exact semantic/native telescope projection.

use super::{
    BoundaryCallbackBinder, BoundaryDirectCallbackParameter, BoundaryNativeParameter,
    BoundaryNativeParameterOrigin, BoundaryNativeParameterShape, MaterializedBoundarySignature,
};
use calling_conventions::{
    CallbackRequirementId, NativeCallbackDemand, NativeParameterId, StaticMachineBinderId,
};
use symbols::SymbolHandle;

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

impl BoundaryNativeParameter {
    pub const fn identity(&self) -> NativeParameterId {
        self.identity
    }

    pub const fn native_ordinal(&self) -> u32 {
        self.native_ordinal
    }

    pub const fn shape(&self) -> BoundaryNativeParameterShape {
        self.shape
    }

    pub const fn origin(&self) -> BoundaryNativeParameterOrigin {
        self.origin
    }

    pub const fn layout_data_symbol(&self) -> SymbolHandle {
        self.layout_data_symbol
    }
}

impl BoundaryDirectCallbackParameter {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn identity(&self) -> NativeParameterId {
        self.identity
    }

    pub const fn native_ordinal(&self) -> u32 {
        self.native_ordinal
    }

    pub const fn binder(&self) -> StaticMachineBinderId {
        self.binder
    }

    pub const fn requirement(&self) -> CallbackRequirementId {
        self.requirement
    }
}
