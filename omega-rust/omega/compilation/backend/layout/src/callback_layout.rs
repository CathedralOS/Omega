//! Exact target-closed inline paths to compiler-private callback slots.
//! Natural and policy-laid records share this path owner. Each hop retains its
//! named declaration and native geometry; references never become inline hops.
//! The terminal slot retains the selected policy/conformance. Layout describes
//! storage only; registrar selection, access and lifetime remain separate.
mod geometry;
#[cfg(test)]
mod tests;

use crate::{TargetClosedDataLayoutIdentity, TargetClosedPrivateCallbackDemand};
use arena::{Arena, HandleSpan};
use calling_conventions::{
    NativeParameterId, NativePlace, callback_layout_field_slot_id, callback_layout_slot_id,
    callback_requirement_id,
};
use std::sync::Arc;
use symbols::SymbolHandle;
use target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateCallbackLayout {
    root_layout: TargetClosedDataLayoutIdentity,
    fields: Arena<PrivateCallbackInlineField>,
    path: HandleSpan<PrivateCallbackInlineField>,
    terminal_slot: TargetClosedPrivateCallbackDemand,
    composed_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateCallbackInlineField {
    pub(crate) symbol: SymbolHandle,
    pub(crate) field_layout: crate::FieldLayout,
    pub(crate) identity: Arc<str>,
    pub(crate) offset: usize,
    pub(crate) extent: usize,
    pub(crate) alignment: usize,
    pub(crate) child_layout: TargetClosedDataLayoutIdentity,
}

// Arena's zero-handle row is never part of a validated path. Its empty named
// identity deliberately cannot satisfy geometry validation.
impl Default for PrivateCallbackInlineField {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::default(),
            field_layout: crate::FieldLayout::default(),
            identity: Arc::from(""),
            offset: 0,
            extent: 0,
            alignment: 1,
            child_layout: TargetClosedDataLayoutIdentity {
                data_symbol: SymbolHandle::default(),
                data_identity: Arc::from(""),
                layout_subject_identity: None,
                layout: calling_conventions::LayoutPlanId::new(1).expect("nonzero arena dummy"),
                physical: crate::TypeLayout::default(),
            },
        }
    }
}

impl PrivateCallbackLayout {
    pub const fn root_layout(&self) -> &TargetClosedDataLayoutIdentity {
        &self.root_layout
    }
    pub fn inline_fields(&self) -> Result<&[PrivateCallbackInlineField], String> {
        self.fields
            .span(self.path)
            .ok_or_else(|| "callback inline path has stale field storage".to_owned())
    }
    pub const fn terminal_slot(&self) -> &TargetClosedPrivateCallbackDemand {
        &self.terminal_slot
    }
    pub const fn composed_offset(&self) -> usize {
        self.composed_offset
    }

    pub(crate) fn from_fields(
        root_layout: TargetClosedDataLayoutIdentity,
        inline_fields: &[PrivateCallbackInlineField],
        terminal_slot: TargetClosedPrivateCallbackDemand,
    ) -> Result<Self, String> {
        let mut fields = Arena::with_capacity(inline_fields.len());
        let mut path = HandleSpan::empty();
        let mut composed_offset = terminal_slot.offset;
        for field in inline_fields {
            composed_offset = composed_offset
                .checked_add(field.offset)
                .ok_or_else(|| "callback inline path offset overflowed".to_owned())?;
            fields.append_to_span(&mut path, field.clone());
        }
        Ok(Self {
            root_layout,
            fields,
            path,
            terminal_slot,
            composed_offset,
        })
    }

    pub fn native_demand(
        &self,
        parameter: NativeParameterId,
    ) -> Result<calling_conventions::NativeCallbackDemand, String> {
        let mut parent = &self.root_layout;
        let mut field_path = Vec::new();
        for field in self.inline_fields()? {
            field_path.push(callback_layout_field_slot_id(
                parent.layout,
                &field.identity,
            ));
            parent = &field.child_layout;
        }
        field_path.push(self.terminal_slot.slot);
        Ok(calling_conventions::NativeCallbackDemand {
            destination: NativePlace::Field {
                parameter,
                layout: self.root_layout.layout,
                field_path,
            },
            requirement: self.terminal_slot.requirement,
        })
    }

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
        if self.native_demand(*parameter)?.destination != *destination {
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
    pub const fn child_layout(&self) -> &TargetClosedDataLayoutIdentity {
        &self.child_layout
    }
}
