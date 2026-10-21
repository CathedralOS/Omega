//! Physical coordinates of the retained core initialization outcome.
//!
//! Role identities were admitted with the lifecycle. Rejoin those types and
//! their native layouts; neither source names nor coincidentally equal sizes
//! choose the owner, returned value, or rejection payload.

use super::super::{
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, failure, offset,
};
use crate::{SelectedInstructionError, structural_inputs::structural_reference_input};
use calling_conventions::{PackedFieldLayout, ValueShape};
use terminal_psi::{
    StructuralCaseDeclaration, StructuralFieldDeclaration, TerminalPlacementLifecyclePlan,
};

pub(super) struct ResultLayout {
    pub shape: ValueShape,
    pub tag: u32,
    pub ready_owner: PackedFieldLayout,
    pub returned_owner: PackedFieldLayout,
    pub returned_value: PackedFieldLayout,
    pub failure_tag: u32,
    pub range: [u32; 2],
    pub alignment: [u32; 2],
}

fn declaration(
    types: &[StructuralTypeDeclaration],
    identity: StructuralTypeId,
) -> Result<&StructuralTypeDeclaration, SelectedInstructionError> {
    types
        .iter()
        .find(|item| item.id == identity)
        .ok_or_else(|| failure("initialization result lost a retained declaration"))
}

fn cases(
    types: &[StructuralTypeDeclaration],
    identity: StructuralTypeId,
) -> Result<&[StructuralCaseDeclaration], SelectedInstructionError> {
    match &declaration(types, identity)?.shape {
        StructuralTypeShape::Sum { cases } => Ok(cases),
        _ => Err(failure("initialization outcome must retain its core sum")),
    }
}

fn structural(
    field: &StructuralFieldDeclaration,
) -> Result<StructuralTypeId, SelectedInstructionError> {
    field
        .field_type
        .as_structural()
        .ok_or_else(|| failure("initialization role lost its structural type"))
}

impl ResultLayout {
    pub(super) fn derive(
        types: &[StructuralTypeDeclaration],
        result: StructuralTypeId,
        plan: &TerminalPlacementLifecyclePlan,
    ) -> Result<Self, SelectedInstructionError> {
        let [ready, rejected] = cases(types, result)? else {
            return Err(failure(
                "initialization outcome must retain Ready and Rejected",
            ));
        };
        let ([ready_owner], [returned, reason]) =
            (ready.fields.as_slice(), rejected.fields.as_slice())
        else {
            return Err(failure(
                "initialization outcome changed its custody or reason roles",
            ));
        };
        if structural(ready_owner)? != plan.storage.carrier {
            return Err(failure(
                "initialization Ready no longer retains the original owner",
            ));
        }
        let result_layout = structural_reference_input::case_layout(result, types)
            .ok_or_else(|| failure("initialization outcome has no native layout"))?;
        let [ready_layout, rejected_layout] = result_layout.cases.as_slice() else {
            return Err(failure("initialization outcome layout changed cases"));
        };
        let ([ready_owner], [returned_layout, reason_layout]) = (
            ready_layout.fields.as_slice(),
            rejected_layout.fields.as_slice(),
        ) else {
            return Err(failure("initialization outcome layout changed fields"));
        };
        let returned_type = structural(returned)?;
        let StructuralTypeShape::Record { fields, .. } = &declaration(types, returned_type)?.shape
        else {
            return Err(failure(
                "initialization return carrier is not its retained record",
            ));
        };
        let [owner, value] = fields.as_slice() else {
            return Err(failure("initialization return carrier lost an owned input"));
        };
        if structural(owner)? != plan.storage.carrier || structural(value)? != plan.schema {
            return Err(failure(
                "initialization returned input types differ from the lifecycle",
            ));
        }
        let returned_field = |field: &StructuralFieldDeclaration| {
            let region = super::super::field_region(types, returned_type, field)?;
            let shape = structural_reference_input::field_storage_shape(field, types)
                .ok_or_else(|| failure("initialization returned field has no storage shape"))?;
            Ok(PackedFieldLayout {
                shape,
                byte_offset: offset(returned_layout.byte_offset, u64::from(region.byte_offset))?,
            })
        };
        let reason_type = structural(reason)?;
        let reason_layouts = structural_reference_input::case_layout(reason_type, types)
            .ok_or_else(|| failure("initialization failure has no native layout"))?;
        let [range, alignment, incompatible] = reason_layouts.cases.as_slice() else {
            return Err(failure(
                "initialization failure changed its three core cases",
            ));
        };
        if !incompatible.fields.is_empty() {
            return Err(failure("IncompatibleSupply must have no runtime payload"));
        }
        let pair = |fields: &[PackedFieldLayout]| {
            let [left, right] = fields else {
                return Err(failure(
                    "initialization failure lost its two geometry words",
                ));
            };
            if left.shape.byte_size != 8 || right.shape.byte_size != 8 {
                return Err(failure(
                    "initialization failure geometry must be native u64 words",
                ));
            }
            Ok([
                offset(reason_layout.byte_offset, u64::from(left.byte_offset))?,
                offset(reason_layout.byte_offset, u64::from(right.byte_offset))?,
            ])
        };
        Ok(Self {
            shape: result_layout.shape,
            tag: result_layout.tag_byte_offset,
            ready_owner: *ready_owner,
            returned_owner: returned_field(owner)?,
            returned_value: returned_field(value)?,
            failure_tag: offset(
                reason_layout.byte_offset,
                u64::from(reason_layouts.tag_byte_offset),
            )?,
            range: pair(&range.fields)?,
            alignment: pair(&alignment.fields)?,
        })
    }
}
