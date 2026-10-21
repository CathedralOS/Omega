//! Native coordinates of an already-authenticated core profile carrier.
//!
//! Declaration positions identify core contract roles, as in the Terminal
//! reader. Retained field identities determine geometry; field names, assumed
//! Rust layouts and sample-specific offsets do not participate.

use super::{SelectedInstructionError, failure};
use crate::structural_inputs::structural_reference_input;
use semantic_vocabulary::{IntegerSign, ScalarType, StructuralTypeId};
use terminal_psi::{
    StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    find_structural_type,
};

#[derive(Clone, Copy)]
pub(super) struct Scalar {
    pub(super) offset: u32,
    pub(super) width: u32,
}

#[derive(Clone, Copy)]
pub(super) struct Counted {
    pub(super) offset: u32,
    pub(super) stride: u32,
    pub(super) capacity: u64,
    pub(super) count: Scalar,
}

pub(super) struct TransferLayout {
    pub(super) width: Scalar,
    pub(super) alignment: Scalar,
    pub(super) operations: Option<[Scalar; 12]>,
}

pub(super) struct ProfileLayout {
    pub(super) bytes: u32,
    pub(super) regions: Counted,
    pub(super) start: Scalar,
    pub(super) length: Scalar,
    pub(super) peer: Scalar,
    pub(super) stable: Scalar,
    pub(super) external: Scalar,
    pub(super) external_read: Scalar,
    pub(super) external_write: Scalar,
    pub(super) external_rules: Counted,
    pub(super) external_transfer: TransferLayout,
    pub(super) atomic: Scalar,
    pub(super) atomic_rules: Counted,
    pub(super) atomic_transfer: TransferLayout,
    pub(super) reach: Counted,
    pub(super) service: Scalar,
}

fn invalid() -> SelectedInstructionError {
    failure("resource profile lost its authenticated complete core carrier geometry")
}

struct Node {
    field_type: StructuralFieldType,
    offset: u32,
    width: u32,
}

impl Node {
    fn root(
        declarations: &[StructuralTypeDeclaration],
        root: StructuralTypeId,
    ) -> Result<Self, SelectedInstructionError> {
        let shape = structural_reference_input::shape(root, declarations)
            .ok_or_else(|| failure("resource profile carrier has no complete native shape"))?;
        Ok(Self {
            field_type: StructuralFieldType::Structural(root),
            offset: 0,
            width: shape.byte_size,
        })
    }

    fn declaration<'types>(
        &self,
        declarations: &'types [StructuralTypeDeclaration],
    ) -> Result<&'types StructuralTypeDeclaration, SelectedInstructionError> {
        find_structural_type(
            declarations,
            self.field_type.as_structural().ok_or_else(invalid)?,
        )
        .ok_or_else(invalid)
    }

    fn record<const COUNT: usize>(
        &self,
        declarations: &[StructuralTypeDeclaration],
    ) -> Result<[Node; COUNT], SelectedInstructionError> {
        let declaration = self.declaration(declarations)?;
        let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
            return Err(failure(
                "resource profile expected a core record declaration",
            ));
        };
        if fields.len() != COUNT || fields.iter().any(|field| field.relevance.is_erased()) {
            return Err(failure(
                "resource profile record changed its complete field roster",
            ));
        }
        fields
            .iter()
            .map(|field| {
                // These are native owned values, including tagged sums. The
                // portable reference-byte query intentionally does not assign
                // a native discriminator layout to those sums.
                let (byte_offset, byte_length) = if let Some(scalar) =
                    field.field_type.scalar_type()
                {
                    structural_reference_input::field_read(
                        declaration.id,
                        &[],
                        field.id,
                        scalar,
                        declarations,
                    )
                    .ok_or_else(|| {
                        failure("resource profile scalar field has no native storage projection")
                    })?
                } else {
                    let (selected, offset) = structural_reference_input::project(
                        declaration.id,
                        &[StructuralPathSegment::Field(field.identity)],
                        declarations,
                    )
                    .ok_or_else(|| {
                        failure(
                            "resource profile structural field has no native storage projection",
                        )
                    })?;
                    if Some(selected) != field.field_type.as_structural() {
                        return Err(failure(
                            "resource profile field projection changed its carrier identity",
                        ));
                    }
                    let shape =
                        structural_reference_input::field_storage_shape(field, declarations)
                            .ok_or_else(|| {
                                failure(
                                    "resource profile structural field has no native storage shape",
                                )
                            })?;
                    (offset, shape.byte_size)
                };
                Ok(Node {
                    field_type: field.field_type.clone(),
                    offset: super::super::offset(self.offset, u64::from(byte_offset))?,
                    width: byte_length,
                })
            })
            .collect::<Result<Vec<_>, SelectedInstructionError>>()?
            .try_into()
            .map_err(|_| invalid())
    }

    fn sum<const CASES: usize>(
        &self,
        declarations: &[StructuralTypeDeclaration],
    ) -> Result<(Scalar, [Vec<Node>; CASES]), SelectedInstructionError> {
        let declaration = self.declaration(declarations)?;
        let StructuralTypeShape::Sum { cases } = &declaration.shape else {
            return Err(failure("resource profile expected a core sum declaration"));
        };
        if cases.len() != CASES {
            return Err(failure("resource profile sum changed its case roster"));
        }
        let layout = structural_reference_input::case_layout(declaration.id, declarations)
            .ok_or_else(invalid)?;
        let payloads = cases
            .iter()
            .zip(&layout.cases)
            .map(|(case, layout)| {
                if case.fields.len() != layout.fields.len()
                    || case.fields.iter().any(|field| field.relevance.is_erased())
                {
                    return Err(invalid());
                }
                case.fields
                    .iter()
                    .zip(&layout.fields)
                    .map(|(field, layout)| {
                        Ok(Node {
                            field_type: field.field_type.clone(),
                            offset: super::super::offset(
                                self.offset,
                                u64::from(layout.byte_offset),
                            )?,
                            width: layout.shape.byte_size,
                        })
                    })
                    .collect::<Result<Vec<_>, SelectedInstructionError>>()
            })
            .collect::<Result<Vec<_>, SelectedInstructionError>>()?;
        Ok((
            Scalar {
                offset: super::super::offset(self.offset, u64::from(layout.tag_byte_offset))?,
                width: layout.tag_shape.byte_size,
            },
            payloads.try_into().map_err(|_| invalid())?,
        ))
    }

    fn tag<const CASES: usize>(
        &self,
        declarations: &[StructuralTypeDeclaration],
    ) -> Result<Scalar, SelectedInstructionError> {
        let (tag, payloads) = self.sum::<CASES>(declarations)?;
        if payloads.iter().any(|payload| !payload.is_empty()) {
            return Err(failure(
                "resource profile enum unexpectedly carries payload fields",
            ));
        }
        Ok(tag)
    }

    fn scalar(
        &self,
        declarations: &[StructuralTypeDeclaration],
        boolean: bool,
    ) -> Result<Scalar, SelectedInstructionError> {
        let scalar = match &self.field_type {
            StructuralFieldType::Structural(_) => {
                let StructuralTypeShape::PrimitiveScalar(scalar) =
                    self.declaration(declarations)?.shape
                else {
                    return Err(invalid());
                };
                scalar
            }
            field => field.scalar_type().ok_or_else(invalid)?,
        };
        let valid = if boolean {
            scalar == ScalarType::Boolean && self.width == 1
        } else {
            matches!(scalar, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Unsigned && integer.bits() == 64)
                && self.width == 8
        };
        if !valid {
            return Err(failure(
                "resource profile scalar differs from its core Boolean or u64 carrier",
            ));
        }
        Ok(Scalar {
            offset: self.offset,
            width: self.width,
        })
    }

    fn counted(
        &self,
        declarations: &[StructuralTypeDeclaration],
        capacity: u64,
    ) -> Result<(Counted, Node), SelectedInstructionError> {
        let [entries, count] = self.record(declarations)?;
        let StructuralTypeShape::FixedArray { element, length } =
            entries.declaration(declarations)?.shape
        else {
            return Err(failure(
                "resource profile counted carrier has no fixed array",
            ));
        };
        if length != capacity {
            return Err(failure(
                "resource profile counted array changed its core capacity",
            ));
        }
        let element = Self::root(declarations, element)?;
        let stride = element.width;
        if u64::from(stride).checked_mul(capacity) != Some(u64::from(entries.width)) {
            return Err(failure(
                "resource profile fixed array size differs from its element stride",
            ));
        }
        Ok((
            Counted {
                offset: entries.offset,
                stride,
                capacity,
                count: count.scalar(declarations, false)?,
            },
            element,
        ))
    }
}

impl ProfileLayout {
    pub(super) fn derive(
        declarations: &[StructuralTypeDeclaration],
        root: StructuralTypeId,
    ) -> Result<Self, SelectedInstructionError> {
        let profile = Node::root(declarations, root)?;
        let (regions, region) = profile.counted(declarations, 32)?;
        let [range, peer, stable, external, atomic, reach] = region.record(declarations)?;
        let [start, length] = range.record(declarations)?;
        let (external_tag, [external_none, external_access]) = external.sum(declarations)?;
        let (atomic_tag, [atomic_none, atomic_access]) = atomic.sum(declarations)?;
        if !external_none.is_empty() || !atomic_none.is_empty() {
            return Err(invalid());
        }
        let [external_read, external_write, external_rules]: [Node; 3] =
            external_access.try_into().map_err(|_| invalid())?;
        let [atomic_rules]: [Node; 1] = atomic_access.try_into().map_err(|_| invalid())?;
        let (external_rules, transfer) = external_rules.counted(declarations, 16)?;
        let [width, alignment] = transfer.record(declarations)?;
        let external_transfer = TransferLayout {
            width: width.scalar(declarations, false)?,
            alignment: alignment.scalar(declarations, false)?,
            operations: None,
        };
        let (atomic_rules, atomic_transfer) = atomic_rules.counted(declarations, 16)?;
        let [transfer, operations] = atomic_transfer.record(declarations)?;
        let [width, alignment] = transfer.record(declarations)?;
        let operations = operations
            .record::<12>(declarations)?
            .iter()
            .map(|operation| operation.scalar(declarations, true))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| invalid())?;
        let atomic_transfer = TransferLayout {
            width: width.scalar(declarations, false)?,
            alignment: alignment.scalar(declarations, false)?,
            operations: Some(operations),
        };
        let (reach, service) = reach.counted(declarations, 32)?;
        Ok(Self {
            bytes: profile.width,
            regions,
            start: start.scalar(declarations, false)?,
            length: length.scalar(declarations, false)?,
            peer: peer.tag::<2>(declarations)?,
            stable: stable.tag::<4>(declarations)?,
            external: external_tag,
            external_read: external_read.tag::<3>(declarations)?,
            external_write: external_write.scalar(declarations, true)?,
            external_rules,
            external_transfer,
            atomic: atomic_tag,
            atomic_rules,
            atomic_transfer,
            reach,
            service: service.scalar(declarations, false)?,
        })
    }
}
