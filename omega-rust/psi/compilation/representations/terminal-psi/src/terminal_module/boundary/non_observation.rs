//! The explicit input authority an opaque realization must not silently assume.

use super::BoundaryMachineDeclaration;
use crate::{ByteSequenceCarrier, StructuralAccess, StructuralTypeDeclaration};
use crate::{StructuralTypeShape, structural_children};
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeSet;

impl BoundaryMachineDeclaration {
    /// Whether the admitted input declaration carries write-only authority,
    /// directly or in a reachable stored descriptor. A checked body proves
    /// non-observation through its ordinary call closure. An opaque import
    /// instead needs its own exact judgment; a visible checked alternative
    /// supplies no receipt for the selected foreign realization.
    ///
    /// This reads only declared access. It grants no authority, reconstructs no
    /// source, and treats reference cycles as finite declaration graphs.
    pub fn requires_checked_non_observation<'types>(
        &self,
        lookup: impl Fn(StructuralTypeId) -> Option<&'types StructuralTypeDeclaration>,
    ) -> bool {
        if self
            .structural_parameters
            .iter()
            .any(|parameter| parameter.access == StructuralAccess::WriteOnlyBorrow)
        {
            return true;
        }
        let mut pending: Vec<_> = self
            .structural_parameters
            .iter()
            .map(|parameter| parameter.structural_type)
            .collect();
        let mut visited = BTreeSet::new();
        while let Some(current) = pending.pop() {
            if !visited.insert(current) {
                continue;
            }
            let Some(declaration) = lookup(current) else {
                // An identity-only opaque referent publishes no stored access
                // declaration. No geometry or absence proof is derived here.
                continue;
            };
            let mut fields_require = |fields: &[crate::StructuralFieldDeclaration]| {
                for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                    pending.extend(structural_children(std::slice::from_ref(field)));
                    if field
                        .field_type
                        .byte_sequence_carrier()
                        .is_some_and(write_only_bytes)
                    {
                        return true;
                    }
                }
                false
            };
            match &declaration.shape {
                StructuralTypeShape::Reference { referent, access } => {
                    if *access == StructuralAccess::WriteOnlyBorrow {
                        return true;
                    }
                    pending.push(*referent);
                }
                StructuralTypeShape::ElementView { element, access } => {
                    if *access == Some(StructuralAccess::WriteOnlyBorrow) {
                        return true;
                    }
                    pending.push(*element);
                }
                StructuralTypeShape::ByteSequence(carrier) if write_only_bytes(*carrier) => {
                    return true;
                }
                StructuralTypeShape::Record { fields, .. } if fields_require(fields) => {
                    return true;
                }
                StructuralTypeShape::Sum { cases }
                    if cases.iter().any(|case| fields_require(&case.fields)) =>
                {
                    return true;
                }
                StructuralTypeShape::Mixed { fields, cases }
                    if fields_require(fields)
                        || cases.iter().any(|case| fields_require(&case.fields)) =>
                {
                    return true;
                }
                StructuralTypeShape::FixedArray { element, length }
                    if *length != crate::ArrayExtent::Literal(0) =>
                {
                    pending.push(*element);
                }
                _ => {}
            }
        }
        false
    }
}

fn write_only_bytes(carrier: ByteSequenceCarrier) -> bool {
    matches!(
        carrier,
        ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::WriteOnlyBorrow)
        } | ByteSequenceCarrier::BoundedView {
            access: Some(StructuralAccess::WriteOnlyBorrow),
            ..
        }
    )
}
