//! Replay the current byte contents required by a UTF-8 primitive replacement.
//! Scalar receiving evidence proves the new byte; this separate all-path walk
//! proves that replacing one byte cannot split an existing multibyte scalar.
use super::{byte_field_freshness, structural_freshness};
use abstract_operations::{
    AbstractOperation as O, StructuralScalarStoreDestination as Destination,
};
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    ByteSequencePredicate as Predicate, StructuralPathSegment as Segment, StructuralTypeDeclaration,
};

pub(super) fn requires_ascii(
    unit: &PsiOptimizationUnit,
    destination: &Destination,
    path: &[Segment],
) -> Option<bool> {
    let (whole, projected) = match destination {
        Destination::Parameter(parameter) | Destination::OwnedBlockParameter { parameter, .. } => (
            &parameter.qualifications,
            &parameter.projected_qualifications,
        ),
        Destination::OwnedResult { result, .. } => {
            (&result.qualifications, &result.projected_qualifications)
        }
    };
    let predicates = terminal_semantics::primitive_store_byte_predicates(
        unit.structural_types.iter(),
        unit.structural_domains.iter(),
        destination.structural_type(),
        whole,
        projected,
        path,
    )?;
    Some(
        predicates.predicates().any(|p| p == Predicate::ValidUtf8)
            && !predicates.predicates().any(|p| p == Predicate::AsciiOnly),
    )
}

pub(super) fn holds(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    use_site: (usize, usize),
    root: PlaceId,
    path: &[Segment],
    budget: usize,
) -> bool {
    if budget == 0 {
        return false;
    }
    structural_freshness::established_on_every_path_with_entry(
        function,
        predecessors,
        use_site,
        root,
        |position, operation, root| {
            match operation {
            O::EstablishZeroedStructural { result, .. } if result.place == root => true,
            O::EstablishScalarArray { result, elements, .. } if result.place == root && path.is_empty() =>
                elements.iter().all(|element| function.block_nodes().any(|node| matches!(node.operation,
                    O::IntegerConstant { result, value: semantic_vocabulary::IntegerValue::Unsigned(value), .. }
                    if result == *element && value <= 127))),
            O::StoreStructuralField { destination, path: written, field, value, .. }
                if destination.place() == root => {
                    let written = full_path(function, types, root, written, *field);
                    written.as_deref() == Some(path)
                        && holds(unit, function, types, predecessors, position, value.place, &value.path, budget - 1)
                }
            O::StructuralLeafCopy { source, path: abstract_operations::StructuralCopyPath::Ordinary(copied), result, .. }
                if result.place == root => {
                    let mut selected = copied.clone();
                    selected.extend_from_slice(path);
                    holds(unit, function, types, predecessors, position, *source, &selected, budget - 1)
                }
            operation => operation.structural_result().is_some_and(|(_, result)|
                result.place == root && declared_ascii(unit, result.structural_type,
                    &result.qualifications, &result.projected_qualifications, path)),
        }
        },
        |_, _, _, _| false,
        |root| {
            function
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == root)
                .is_some_and(|parameter| {
                    declared_ascii(
                        unit,
                        parameter.structural_type,
                        &parameter.qualifications,
                        &parameter.projected_qualifications,
                        path,
                    )
                })
        },
        |operation, root| {
            match operation {
                O::WriteOnlyPrimitiveStore {
                    destination,
                    path: written,
                    value,
                    range_obligation,
                    ..
                } if destination.place() == root => {
                    if !terminal_semantics::structural_paths_may_overlap(path, written) {
                        return false;
                    }
                    let one_element = written.strip_prefix(path).is_some_and(|suffix| {
                        matches!(
                            suffix,
                            [Segment::FixedIndex(_) | Segment::RuntimeIndex { .. }]
                        )
                    });
                    // The catalog separately authenticates the receiving
                    // receipt. A literal ASCII byte needs no range receipt.
                    let literal_ascii = function.block_nodes().any(|node| matches!(node.operation,
                            O::IntegerConstant { result, value: semantic_vocabulary::IntegerValue::Unsigned(byte), .. }
                            if result == value.value && byte <= 127));
                    let receiving_ascii = range_obligation.is_some()
                        && requires_ascii(unit, destination, written) == Some(true);
                    return !(one_element && (literal_ascii || receiving_ascii));
                }
                O::StoreStructuralField {
                    destination,
                    path: written,
                    field,
                    ..
                } if destination.place() == root => {
                    return full_path(function, types, root, written, *field).is_none_or(
                        |written| terminal_semantics::structural_paths_may_overlap(path, &written),
                    );
                }
                // Scalar siblings cannot change this array; retain exact field
                // projection instead of invalidating the containing record.
                O::StructuralScalarFieldStore {
                    destination,
                    path: written,
                    field,
                    ..
                } if destination.place() == root => {
                    return full_path(function, types, root, written, Some(*field)).is_none_or(
                        |written| terminal_semantics::structural_paths_may_overlap(path, &written),
                    );
                }
                // A different place may name an alias of the same backing.
                // Without an exact disjoint footprint, no earlier byte-content
                // snapshot survives a store through that reference.
                O::WriteOnlyPrimitiveStore { .. }
                | O::StoreStructuralField { .. }
                | O::StructuralScalarFieldStore { .. }
                | O::StructuralByteSequenceFieldByteStore { .. }
                | O::StructuralByteSequenceFieldStore { .. }
                | O::ByteSequenceStore { .. }
                | O::ByteSequenceWrite { .. }
                | O::ElementViewWrite { .. } => return true,
                _ => {}
            }
            structural_freshness::structural_metadata_may_change(
                function, types, root, path, operation,
            )
        },
    )
}

fn full_path(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root: PlaceId,
    path: &[Segment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
) -> Option<Vec<Segment>> {
    field.map_or_else(
        || Some(path.to_vec()),
        |field| byte_field_freshness::field_path(function, types, root, path, field),
    )
}

fn declared_ascii(
    unit: &PsiOptimizationUnit,
    root: StructuralTypeId,
    whole: &[semantic_vocabulary::StructuralDomainId],
    projected: &[terminal_psi::StructuralPathQualification],
    path: &[Segment],
) -> bool {
    let Some(carrier) =
        terminal_semantics::runtime_structural_path_tip(unit.structural_types.iter(), root, path)
    else {
        return false;
    };
    whole
        .iter()
        .filter(|_| path.is_empty())
        .copied()
        .chain(
            projected
                .iter()
                .filter(|membership| membership.path == path)
                .map(|membership| membership.domain),
        )
        .any(|domain| {
            terminal_semantics::byte_qualification_predicates_in(
                unit.structural_domains.iter(),
                carrier.id,
                &[domain],
            )
            .is_some_and(|predicates| {
                predicates
                    .predicates()
                    .any(|predicate| predicate == Predicate::AsciiOnly)
            })
        })
}
