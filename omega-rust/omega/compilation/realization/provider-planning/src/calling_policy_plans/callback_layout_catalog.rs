//! Joins backend callback layouts to semantic and native boundary parameters.
//!
//! Layout owns nominal field paths and target geometry; this catalog retains
//! their exact association with the calling signature and ordered demands.

mod validation;
pub(super) use validation::validate;

use super::{BoundaryNativeParameter, BoundaryNativeParameterOrigin};
use calling_conventions::NativePlace;
use layout::{
    PrivateCallbackInlineField, PrivateCallbackLayout, TargetClosedDataLayoutIdentity,
    TargetClosedPrivateCallbackDemand,
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

    pub const fn root_layout(&self) -> &TargetClosedDataLayoutIdentity {
        self.layout.root_layout()
    }

    pub fn inline_fields(&self) -> Result<&[PrivateCallbackInlineField], String> {
        self.layout.inline_fields()
    }

    pub const fn terminal_slot(&self) -> &TargetClosedPrivateCallbackDemand {
        self.layout.terminal_slot()
    }

    pub const fn composed_offset(&self) -> usize {
        self.layout.composed_offset()
    }

    pub(crate) fn source_free(
        &self,
    ) -> Result<calling_conventions::BoundaryCallbackLayout, String> {
        fn data(
            value: &TargetClosedDataLayoutIdentity,
        ) -> calling_conventions::CallbackDataGeometry {
            calling_conventions::CallbackDataGeometry {
                identity: value.data_identity.to_string(),
                policy: value.layout_subject_identity.as_deref().map(str::to_owned),
                layout: value.layout,
                size: value.physical.size as u64,
                alignment: value.physical.alignment as u64,
            }
        }
        let fields = self
            .inline_fields()?
            .iter()
            .map(|field| calling_conventions::CallbackInlineFieldGeometry {
                identity: field.identity().to_owned(),
                offset: field.offset() as u64,
                child: data(field.child_layout()),
            })
            .collect::<Vec<_>>();
        let terminal = self.terminal_slot();
        Ok(calling_conventions::BoundaryCallbackLayout::new(
            self.formal_ordinal,
            self.native_ordinal,
            self.destination.clone(),
            data(self.root_layout()),
            &fields,
            calling_conventions::CallbackSlotGeometry {
                identity: terminal.slot_identity.to_string(),
                policy: terminal.layout_subject_identity.to_string(),
                requirement_identity: terminal.callback_requirement_identity.to_string(),
                layout: terminal.layout,
                slot: terminal.slot,
                requirement: terminal.requirement,
                conformance_commitment: terminal.slot_application.commitment.as_bytes(),
                offset: terminal.offset as u64,
                size: terminal.byte_size as u64,
                alignment: terminal.alignment as u64,
            },
        ))
    }

    pub(super) fn from_layout(
        parameter: &BoundaryNativeParameter,
        layout: &PrivateCallbackLayout,
    ) -> Result<Self, String> {
        let formal_ordinal = semantic_formal(parameter)?;
        if layout.root_layout().data_symbol != parameter.layout_data_symbol {
            return Err(
                "private callback path does not join its exact parameter carrier".to_owned(),
            );
        }
        Ok(Self {
            formal_ordinal,
            native_ordinal: parameter.native_ordinal,
            destination: layout.native_demand(parameter.identity)?.destination,
            layout: layout.clone(),
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
