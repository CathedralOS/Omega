//! Facts follow the owned contents transferred at one completed operation.
//!
//! Scalar record leaves retain their exact equality equations. Byte predicates
//! describe an entire owned byte leaf, so copy their live formulas onto the new
//! leaf now, before a later source write can invalidate the originals. Rewriting
//! an implication never detaches its premise. Borrowed descriptors and reference
//! referents are not copied contents, and byte recasts do not preserve the source
//! declaration's field namespace. None of these rules establishes routed domains.

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, PlaceId, Proposition, ScalarTerm, ScalarType, StructuralTypeId,
};
use terminal_psi::{
    ByteSequenceCarrier, Operation, OperationKind, StructuralTypeShape, TerminalMachine,
    TerminalModule,
};

/// The operation's validation owns access, liveness and copyability. Its exact
/// logical subtree, rather than native layout compatibility, owns substitution.
pub(super) fn append_copy(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    axioms: &mut Vec<Proposition>,
) {
    let Some(result) = operation.result.structural() else {
        return;
    };
    let Some(declaration) = module
        .structural_types
        .iter()
        .find(|row| row.id == result.structural_type)
    else {
        return;
    };
    if matches!(
        declaration.shape,
        StructuralTypeShape::Reference { .. } | StructuralTypeShape::ElementView { .. }
    ) || declaration.shape.is_borrowed_byte_carrier()
    {
        return;
    }
    // A repeated operation establishes fresh contents at its result identity.
    axioms.retain(|fact| !crate::validation::proposition_observes_places(fact, &[result.place]));
    let (source, path) = match &operation.kind {
        OperationKind::StructuralLeafCopy { source, path } => {
            let Some(path) =
                crate::validation::canonical_field_path(module, machine, *source, path)
            else {
                return;
            };
            (*source, path)
        }
        OperationKind::StructuralCaseLeafCopy { source, path } => (*source, path.clone()),
        _ => return,
    };
    let Some(source_type) =
        crate::validation::structural::operations::caller_structural_root_type(machine, source)
    else {
        return;
    };
    let same_type = terminal_semantics::canonical_structural_path_tip(
        module.structural_types.iter(),
        source_type,
        &path,
    )
    .is_some_and(|selected| selected.id == result.structural_type);
    // An inline owned-byte field carries its capacity directly rather than a
    // structural type ID. The same exact carrier is still the same byte value.
    let same_bytes = match declaration.shape {
        StructuralTypeShape::ByteSequence(carrier @ ByteSequenceCarrier::BoundedOwned { .. }) => {
            owned_byte_carrier(module, source_type, &path) == Some(carrier)
        }
        _ => false,
    };
    if !same_type && !same_bytes {
        return;
    }
    append(
        module,
        &terminal_psi::StructuralFieldType::Structural(result.structural_type),
        result.place,
        Vec::new(),
        source,
        path,
        axioms,
    );
}

/// Shared by whole copies and record, case, or array child establishment. Each
/// caller supplies the child's admitted destination type and exact value path.
pub(super) fn append(
    module: &TerminalModule,
    contents: &terminal_psi::StructuralFieldType,
    destination: PlaceId,
    destination_path: Vec<CanonicalStructuralPathSegment>,
    source: PlaceId,
    source_path: Vec<CanonicalStructuralPathSegment>,
    axioms: &mut Vec<Proposition>,
) {
    // Scan only facts that exist at this transfer. A nested copy does not
    // enumerate the possible elements of a large fixed array.
    let original_count = axioms.len();
    for index in 0..original_count {
        let mut copied = axioms[index].clone();
        let mut changed = false;
        super::super::super::observation_substitution::substitute(&mut copied, &mut |term| {
            let ScalarTerm::ByteSequencePredicate {
                predicate,
                root,
                path,
            } = term
            else {
                return None;
            };
            if *root != source {
                return None;
            }
            let suffix = path.strip_prefix(source_path.as_slice())?;
            match contents {
                terminal_psi::StructuralFieldType::Structural(structural_type) => {
                    owned_byte_carrier(module, *structural_type, suffix)?;
                }
                field
                    if suffix.is_empty()
                        && matches!(
                            field.byte_sequence_carrier(),
                            Some(ByteSequenceCarrier::BoundedOwned { .. })
                        ) => {}
                _ => return None,
            }
            let mut path = destination_path.clone();
            path.extend_from_slice(suffix);
            changed = true;
            Some(ScalarTerm::ByteSequencePredicate {
                predicate: *predicate,
                root: destination,
                path,
            })
        });
        if changed && !axioms.contains(&copied) {
            axioms.push(copied);
        }
    }
    if let terminal_psi::StructuralFieldType::Structural(structural_type) = contents {
        append_scalar_fields(
            module,
            *structural_type,
            destination,
            destination_path,
            source,
            source_path,
            axioms,
        );
    }
}

/// Only owned inline payload paths qualify. The canonical type walker cannot
/// cross a Reference or ElementView, and the final carrier must own its bytes.
fn owned_byte_carrier(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<ByteSequenceCarrier> {
    if let Some(tip) = terminal_semantics::canonical_structural_path_tip(
        module.structural_types.iter(),
        structural_type,
        path,
    ) && let StructuralTypeShape::ByteSequence(
        carrier @ ByteSequenceCarrier::BoundedOwned { .. },
    ) = tip.shape
    {
        return Some(carrier);
    }
    let (CanonicalStructuralPathSegment::Field(field), parent) = path.split_last()? else {
        return None;
    };
    let fields =
        if let Some((CanonicalStructuralPathSegment::Case(case), prefix)) = parent.split_last() {
            let tip = terminal_semantics::canonical_structural_path_tip(
                module.structural_types.iter(),
                structural_type,
                prefix,
            )?;
            let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                &tip.shape
            else {
                return None;
            };
            &cases.iter().find(|row| row.id == *case)?.fields
        } else {
            let tip = terminal_semantics::canonical_structural_path_tip(
                module.structural_types.iter(),
                structural_type,
                parent,
            )?;
            let (StructuralTypeShape::Record { fields }
            | StructuralTypeShape::Mixed { fields, .. }) = &tip.shape
            else {
                return None;
            };
            fields
        };
    let selected = fields
        .iter()
        .find(|row| row.id == *field && !row.relevance.is_erased())?;
    match selected.field_type.byte_sequence_carrier()? {
        carrier @ ByteSequenceCarrier::BoundedOwned { .. } => Some(carrier),
        _ => None,
    }
}

fn append_scalar_fields(
    module: &TerminalModule,
    structural_type: semantic_vocabulary::StructuralTypeId,
    destination: semantic_vocabulary::PlaceId,
    destination_path: Vec<CanonicalStructuralPathSegment>,
    source: semantic_vocabulary::PlaceId,
    source_path: Vec<CanonicalStructuralPathSegment>,
    axioms: &mut Vec<Proposition>,
) {
    let Some(declaration) = module
        .structural_types
        .iter()
        .find(|item| item.id == structural_type)
    else {
        return;
    };
    let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape else {
        return;
    };
    for field in fields {
        let mut target = destination_path.clone();
        target.push(CanonicalStructuralPathSegment::Field(field.id));
        let mut origin = source_path.clone();
        origin.push(CanonicalStructuralPathSegment::Field(field.id));
        if let terminal_psi::StructuralFieldType::Structural(child) = field.field_type {
            append_scalar_fields(module, child, destination, target, source, origin, axioms);
        } else {
            let equation = match field.field_type.scalar_type() {
                Some(ScalarType::Integer(integer)) => Some((
                    ScalarTerm::integer_field_path(destination, target, integer),
                    ScalarTerm::integer_field_path(source, origin, integer),
                )),
                Some(ScalarType::Boolean) => Some((
                    ScalarTerm::boolean_field_path(destination, target),
                    ScalarTerm::boolean_field_path(source, origin),
                )),
                Some(ScalarType::IeeeFloat(format)) => Some((
                    ScalarTerm::ieee_float_field_path(destination, target, format),
                    ScalarTerm::ieee_float_field_path(source, origin, format),
                )),
                _ => None,
            };
            if let Some((left, right)) = equation {
                axioms.push(Proposition::Equal(left, right));
            }
        }
    }
}
