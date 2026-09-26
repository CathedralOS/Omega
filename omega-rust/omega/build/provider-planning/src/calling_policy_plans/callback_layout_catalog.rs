//! Named callback layout semantics retained beside their native demand joins.
//!
//! These are compiler-owned inputs to later policy projection, not a wire
//! format: handles and compact native identities must not enter a lock.

mod validation;
#[cfg(test)]
pub(super) use validation::signature_fixture;
pub(super) use validation::validate;

use super::{BoundaryNativeParameter, BoundaryNativeParameterOrigin};
use calling_conventions::NativePlace;
use layout::{
    LayoutPlan, TargetClosedPlanLaidDataLayoutIdentity, TargetClosedPrivateCallbackDemand,
    TargetClosedTwoHopPrivateCallbackPath,
};
use std::sync::Arc;
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallbackLayoutEntry {
    formal_ordinal: u32,
    native_ordinal: u32,
    destination: NativePlace,
    root_layout: TargetClosedPlanLaidDataLayoutIdentity,
    inline_field: Option<BoundaryCallbackInlineField>,
    terminal_slot: TargetClosedPrivateCallbackDemand,
    composed_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallbackInlineField {
    symbol: SymbolHandle,
    identity: Arc<str>,
    offset: usize,
    extent: usize,
    alignment: usize,
    child_layout: TargetClosedPlanLaidDataLayoutIdentity,
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
        &self.root_layout
    }

    pub const fn inline_field(&self) -> Option<&BoundaryCallbackInlineField> {
        self.inline_field.as_ref()
    }

    /// Includes the complete typed slot conformance application. Its compact
    /// report suffix is never parsed to recover semantic declaration identity.
    pub const fn terminal_slot(&self) -> &TargetClosedPrivateCallbackDemand {
        &self.terminal_slot
    }

    pub const fn composed_offset(&self) -> usize {
        self.composed_offset
    }

    pub(super) fn direct(
        parameter: &BoundaryNativeParameter,
        layout_plan: &LayoutPlan,
        demand: &TargetClosedPrivateCallbackDemand,
    ) -> Result<Self, String> {
        let formal_ordinal = semantic_formal(parameter)?;
        let mut roots = layout_plan
            .plan_laid_layout_identities
            .iter()
            .filter(|root| {
                root.data_symbol == demand.data_symbol
                    && root.layout_subject_identity == demand.layout_subject_identity
            });
        let root = roots
            .next()
            .ok_or_else(|| "private callback demand has no exact named root layout".to_owned())?;
        if roots.next().is_some() || root.data_symbol != parameter.layout_data_symbol {
            return Err(
                "private callback demand has an ambiguous or foreign root layout".to_owned(),
            );
        }
        Ok(Self {
            formal_ordinal,
            native_ordinal: parameter.native_ordinal,
            destination: demand.native_demand(parameter.identity).destination,
            root_layout: root.clone(),
            inline_field: None,
            terminal_slot: demand.clone(),
            composed_offset: demand.offset,
        })
    }

    pub(super) fn two_hop(
        parameter: &BoundaryNativeParameter,
        path: &TargetClosedTwoHopPrivateCallbackPath,
    ) -> Result<Self, String> {
        let formal_ordinal = semantic_formal(parameter)?;
        if path.root_layout.data_symbol != parameter.layout_data_symbol
            || path.child_layout.data_symbol != path.terminal_demand.data_symbol
            || path.child_layout.layout_subject_identity
                != path.terminal_demand.layout_subject_identity
        {
            return Err(
                "private callback path does not join its named root and child layouts".to_owned(),
            );
        }
        Ok(Self {
            formal_ordinal,
            native_ordinal: parameter.native_ordinal,
            destination: path.native_demand(parameter.identity).destination,
            root_layout: path.root_layout.clone(),
            inline_field: Some(BoundaryCallbackInlineField {
                symbol: path.field_symbol,
                identity: path.field_identity.clone(),
                offset: path.field_relative_offset,
                extent: path.field_extent,
                alignment: path.field_alignment,
                child_layout: path.child_layout.clone(),
            }),
            terminal_slot: path.terminal_demand.clone(),
            composed_offset: path.composed_offset,
        })
    }
}

impl BoundaryCallbackInlineField {
    pub const fn symbol(&self) -> SymbolHandle {
        self.symbol
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub const fn offset(&self) -> usize {
        self.offset
    }

    pub const fn extent(&self) -> usize {
        self.extent
    }

    pub const fn alignment(&self) -> usize {
        self.alignment
    }

    pub const fn child_layout(&self) -> &TargetClosedPlanLaidDataLayoutIdentity {
        &self.child_layout
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
