//! Storage shapes for claim-free owned returns. Shape supplies no custody:
//! shared descriptor slots additionally require exact external-backing replay.
//! Access must be explicit in each byte carrier. An unannotated byte descriptor
//! or ElementView obtains access from its producer and cannot be classified as
//! shared from shape alone; ordinary references retain their own return contract.
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeSet;
use terminal_psi::{StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape};

pub fn plain_owned_shape(types: &[StructuralTypeDeclaration], root: StructuralTypeId) -> bool {
    owned_shape(types, root, false)
}

pub fn owned_shared_byte_descriptor_shape(
    types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
) -> bool {
    owned_shape(types, root, true)
}

fn owned_shape(
    types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
    shared_byte_slots: bool,
) -> bool {
    fn visit(
        types: &[StructuralTypeDeclaration],
        root: StructuralTypeId,
        active: &mut Vec<StructuralTypeId>,
        complete: &mut BTreeSet<StructuralTypeId>,
        shared_byte_slots: bool,
    ) -> bool {
        if complete.contains(&root) {
            return true;
        }
        if active.contains(&root) {
            return false;
        }
        let mut declarations = types.iter().filter(|declaration| declaration.id == root);
        let Some(declaration) = declarations.next() else {
            return false;
        };
        if declarations.next().is_some() {
            return false;
        }
        active.push(root);
        let mut field_is_owned = |field: &terminal_psi::StructuralFieldDeclaration| {
            // Erased declarations contribute no storage or loan authority.
            if field.relevance.is_erased() {
                return matches!(field.field_type, StructuralFieldType::Erased { .. });
            }
            match &field.field_type {
                StructuralFieldType::Scalar(_)
                | StructuralFieldType::Atomic(_)
                | StructuralFieldType::BoundedInteger(_)
        | StructuralFieldType::RefinedScalar(_)
                | StructuralFieldType::IeeeFloat(_) => true,
                StructuralFieldType::Structural(child) => {
                    visit(types, *child, active, complete, shared_byte_slots)
                }
                StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                )
                | StructuralFieldType::PredicateByteSequence {
                    carrier: terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                    ..
                } => true,
                StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                    },
                )
                | StructuralFieldType::PredicateByteSequence {
                    carrier:
                        terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                        },
                    ..
                } => shared_byte_slots,
                StructuralFieldType::ByteSequence(_)
                | StructuralFieldType::PredicateByteSequence { .. }
                | StructuralFieldType::Erased { .. } => false,
            }
        };
        let supported = match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_) => true,
            StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
            ) => true,
            StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView {
                    access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                },
            ) => shared_byte_slots,
            StructuralTypeShape::ByteSequence(_) => false,
            StructuralTypeShape::FusedServiceBinding => false,
            StructuralTypeShape::ElementView { .. } => false,
            StructuralTypeShape::Reference { .. } => false,
            StructuralTypeShape::Record { fields } => fields.iter().all(&mut field_is_owned),
            StructuralTypeShape::Sum { cases } => cases
                .iter()
                .flat_map(|case| &case.fields)
                .all(&mut field_is_owned),
            StructuralTypeShape::Mixed { fields, cases } => fields
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields))
                .all(&mut field_is_owned),
            StructuralTypeShape::FixedArray { element, .. } => {
                visit(types, *element, active, complete, shared_byte_slots)
            }
        };
        active.pop();
        if supported {
            complete.insert(root);
        }
        supported
    }
    visit(
        types,
        root,
        &mut Vec::new(),
        &mut BTreeSet::new(),
        shared_byte_slots,
    )
}
