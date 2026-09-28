//! Retained native callback field layouts and their target-geometry checks.
//!
//! Layout construction copies the exact named root/child/slot relationship;
//! validation replays its nominal path, containment and pointer geometry.
//! Source-formal association remains with the caller. These are local layout
//! records, not a portable wire format or conformance authorization.

mod geometry;
#[cfg(test)]
mod tests;

use crate::{
    TargetClosedPlanLaidDataLayoutIdentity, TargetClosedPrivateCallbackDemand,
    TargetClosedTwoHopPrivateCallbackPath,
};
use calling_conventions::{
    NativePlace, callback_layout_field_slot_id, callback_layout_slot_id, callback_requirement_id,
};
use std::sync::Arc;
use symbols::SymbolHandle;
use target::NativeTarget;

/// Retains a direct or inline-child callback layout for later validation.
/// Construction establishes named ownership; `validate` checks the native
/// destination and selected target before the caller uses the layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateCallbackLayout {
    root_layout: TargetClosedPlanLaidDataLayoutIdentity,
    inline_field: Option<PrivateCallbackInlineField>,
    terminal_slot: TargetClosedPrivateCallbackDemand,
    composed_offset: usize,
}

/// Named inline field connecting a root layout to the terminal slot's owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateCallbackInlineField {
    symbol: SymbolHandle,
    identity: Arc<str>,
    offset: usize,
    extent: usize,
    alignment: usize,
    child_layout: TargetClosedPlanLaidDataLayoutIdentity,
}

impl PrivateCallbackLayout {
    pub const fn root_layout(&self) -> &TargetClosedPlanLaidDataLayoutIdentity {
        &self.root_layout
    }

    pub const fn inline_field(&self) -> Option<&PrivateCallbackInlineField> {
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

    /// Retains the demand's unique named root, rejecting missing or ambiguous
    /// roots. The root's nominal layout ID can differ from the terminal's
    /// legacy layout ID.
    pub fn direct(
        layouts: &[TargetClosedPlanLaidDataLayoutIdentity],
        demand: &TargetClosedPrivateCallbackDemand,
    ) -> Result<Self, String> {
        let mut roots = layouts.iter().filter(|root| {
            root.data_symbol == demand.data_symbol
                && root.layout_subject_identity == demand.layout_subject_identity
        });
        let root = roots
            .next()
            .ok_or_else(|| "private callback demand has no exact named root layout".to_owned())?;
        if roots.next().is_some() {
            return Err(
                "private callback demand has an ambiguous or foreign root layout".to_owned(),
            );
        }
        Ok(Self {
            root_layout: root.clone(),
            inline_field: None,
            terminal_slot: demand.clone(),
            composed_offset: demand.offset,
        })
    }

    /// Retains one root-to-child path, rejecting a terminal owned by a different
    /// named child. Target geometry is checked by `validate`.
    pub fn two_hop(path: &TargetClosedTwoHopPrivateCallbackPath) -> Result<Self, String> {
        if path.child_layout.data_symbol != path.terminal_demand.data_symbol
            || path.child_layout.layout_subject_identity
                != path.terminal_demand.layout_subject_identity
        {
            return Err(
                "private callback path does not join its named root and child layouts".to_owned(),
            );
        }
        Ok(Self {
            root_layout: path.root_layout.clone(),
            inline_field: Some(PrivateCallbackInlineField {
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

    /// Replays nominal field identity and target geometry, not the source proof
    /// that admitted the conformance or the signature's parameter association.
    pub fn validate(&self, destination: &NativePlace, target: NativeTarget) -> Result<(), String> {
        let NativePlace::Field { parameter, .. } = destination else {
            return Err("callback layout catalog contains a non-field destination".to_owned());
        };
        let terminal = &self.terminal_slot;
        if terminal.slot_identity.is_empty()
            || terminal.callback_requirement_identity.is_empty()
            || terminal.slot != callback_layout_slot_id(terminal.layout, &terminal.slot_identity)
            || terminal.requirement
                != callback_requirement_id(&terminal.callback_requirement_identity)
            || !terminal.slot_application.declaration.is_valid()
            || !terminal.slot_application.trait_definition.is_valid()
            || terminal.slot_application.commitment.is_zero()
        {
            return Err("callback layout catalog lost its named slot application".to_owned());
        }
        let expected = if let Some(field) = &self.inline_field {
            NativePlace::Field {
                parameter: *parameter,
                layout: self.root_layout.layout,
                field_path: vec![
                    callback_layout_field_slot_id(self.root_layout.layout, &field.identity),
                    terminal.slot,
                ],
            }
        } else {
            terminal.native_demand(*parameter).destination
        };
        if *destination != expected {
            return Err("callback layout catalog changed its exact ordered field path".to_owned());
        }
        geometry::validate(self, target)
    }
}

impl PrivateCallbackInlineField {
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
