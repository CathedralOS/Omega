//! Joins backend callback layouts to semantic and native boundary parameters.
//!
//! Layout owns nominal field paths and target geometry; this catalog retains
//! their exact association with the calling signature and ordered demands.

mod validation;
pub(super) use validation::validate;

use super::{BoundaryNativeParameter, BoundaryNativeParameterOrigin};
use calling_conventions::NativePlace;
use layout::{
    PrivateCallbackInlineField, PrivateCallbackLayout, TargetClosedPlanLaidDataLayoutIdentity,
    TargetClosedPrivateCallbackDemand, TargetClosedTwoHopPrivateCallbackPath,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallbackLayoutEntry {
    formal_ordinal: u32,
    native_ordinal: u32,
    destination: NativePlace,
    layout: PrivateCallbackLayout,
}

impl BoundaryCallbackLayoutEntry {
    pub const fn formal_ordinal(&self) -> u32 {
        self.formal_ordinal
    }

    pub const fn native_ordinal(&self) -> u32 {
        self.native_ordinal
    }

    /// Exact compiler-only join to one materialization in the validated plan.
    pub const fn destination(&self) -> &NativePlace {
        &self.destination
    }

    pub const fn root_layout(&self) -> &TargetClosedPlanLaidDataLayoutIdentity {
        self.layout.root_layout()
    }

    pub const fn inline_field(&self) -> Option<&PrivateCallbackInlineField> {
        self.layout.inline_field()
    }

    /// Includes the complete typed slot conformance application. Its compact
    /// report suffix is never parsed to recover semantic declaration identity.
    pub const fn terminal_slot(&self) -> &TargetClosedPrivateCallbackDemand {
        self.layout.terminal_slot()
    }

    pub const fn composed_offset(&self) -> usize {
        self.layout.composed_offset()
    }

    pub(super) fn direct(
        parameter: &BoundaryNativeParameter,
        layouts: &[TargetClosedPlanLaidDataLayoutIdentity],
        demand: &TargetClosedPrivateCallbackDemand,
    ) -> Result<Self, String> {
        let formal_ordinal = semantic_formal(parameter)?;
        let layout = PrivateCallbackLayout::direct(layouts, demand)?;
        if layout.root_layout().data_symbol != parameter.layout_data_symbol {
            return Err(
                "private callback demand has an ambiguous or foreign root layout".to_owned(),
            );
        }
        Ok(Self {
            formal_ordinal,
            native_ordinal: parameter.native_ordinal,
            destination: demand.native_demand(parameter.identity).destination,
            layout,
        })
    }

    pub(super) fn two_hop(
        parameter: &BoundaryNativeParameter,
        path: &TargetClosedTwoHopPrivateCallbackPath,
    ) -> Result<Self, String> {
        let formal_ordinal = semantic_formal(parameter)?;
        if path.root_layout.data_symbol != parameter.layout_data_symbol {
            return Err(
                "private callback path does not join its named root and child layouts".to_owned(),
            );
        }
        Ok(Self {
            formal_ordinal,
            native_ordinal: parameter.native_ordinal,
            destination: path.native_demand(parameter.identity).destination,
            layout: PrivateCallbackLayout::two_hop(path)?,
        })
    }
}

fn semantic_formal(parameter: &BoundaryNativeParameter) -> Result<u32, String> {
    match parameter.origin {
        BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal }
            if parameter.layout_data_symbol.is_valid() =>
        {
            Ok(formal_ordinal)
        }
        _ => Err("private layout callback must belong to one semantic formal".to_owned()),
    }
}
