//! Reconstruct complete record construction from the current executable inputs.
//!
//! Earlier operations own evaluation order. This operation binds their finished
//! values to declaration identities and transfers selected children atomically.
//! Source admission alone cannot authorize substituted fields or child homes in
//! a rewritten optimization unit; dominance and current ownership are checked
//! separately against the same operands.
use crate::unit_validation::operation_contracts::structural_access::structural_source_contract;
use crate::unit_validation::structural_catalog::resolve_structural_path;
use abstract_operations::AbstractOperation as O;
use abstract_operations::AbstractStructuralCallResult;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{PlaceId, StructuralPlaceKind, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    ByteSequenceCarrier, RecordFieldValue, StructuralAccess, StructuralFieldType,
    StructuralMultiplicity, StructuralTypeShape,
};

/// A plain record result is governed by its signature and custody, independent
/// of the operations used to produce it. Argument access, current availability,
/// and the callee's return operands are validated by their ordinary owners.
pub(crate) fn plain_record_call(
    operation: &O,
    callee: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    let O::CallStructural {
        result:
            AbstractStructuralCallResult::Structural {
                result,
                returned_claim_transfers,
                selected_evidence,
            },
        claim_transfers,
        requirement_obligations,
        crash_continuations,
        ..
    } = operation
    else {
        return false;
    };
    let Some(signature) = callee.result.structural() else {
        return false;
    };
    let Some(contract) = &callee.verified_contract else {
        return false;
    };
    matches!(
        signature.multiplicity,
        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
    ) && signature.qualifications.is_empty()
        && signature.projected_qualifications.is_empty()
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && claim_transfers.is_empty()
        && returned_claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && *crash_continuations == contract.crash_routes
        && selected_evidence.is_empty()
        && callee.entry_claims.is_empty()
        && callee.entry_claim_declarations.is_empty()
        && callee.content_entry_claims.is_empty()
        && callee.evidence_contract_lanes.is_empty()
        && contract.requires.is_empty()
        && contract.ensures.is_empty()
        && contract.outcome_specific_ensures.is_empty()
        && callee.structural_parameters.iter().all(|parameter| {
            matches!(
                parameter.multiplicity,
                StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
            ) && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
        })
        && plain_record(types, signature.structural_type)
}

/// Zero initialization publishes the same owned result as explicit aggregate
/// construction, but owes whole-shape zero validity instead of field operands.
/// Reconstruct that judgment from the current type catalog: prior Terminal
/// admission cannot authorize a rewritten result with a different shape.
pub(crate) fn zeroed_establishment_matches(
    function: &PsiOptimizationFunction,
    operation: &O,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    let O::EstablishZeroedStructural {
        psi_operation,
        result,
    } = operation
    else {
        return false;
    };
    function.structural_places.iter().any(|place| {
        place.id == result.place
            && matches!(place.kind, StructuralPlaceKind::OperationResult { producer, structural_type }
                if producer == *psi_operation && structural_type == result.structural_type)
    }) && matches!(result.multiplicity, StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted)
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.qualification_establishments.is_empty()
        && result.claims.is_empty()
        && terminal_semantics::zeroed_structural_shape(types.values().copied(), result.structural_type)
}

pub(crate) fn record_establishment_matches(
    function: &PsiOptimizationFunction,
    operation: &O,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    domains: &BTreeMap<
        semantic_vocabulary::StructuralDomainId,
        &terminal_psi::StructuralDomainDeclaration,
    >,
) -> bool {
    let O::EstablishRecord {
        psi_operation,
        result,
        fields,
    } = operation
    else {
        return false;
    };
    let exact_result_place = function.structural_places.iter().any(|place| {
        place.id == result.place
            && matches!(place.kind, StructuralPlaceKind::OperationResult { producer, structural_type }
                if producer == *psi_operation && structural_type == result.structural_type)
    });
    if !exact_result_place
        || !matches!(
            result.multiplicity,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
        )
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !constructible_record(types, result.structural_type)
    {
        return false;
    }
    let Some(terminal_psi::StructuralTypeDeclaration {
        shape: StructuralTypeShape::Record {
            fields: declarations,
        },
        ..
    }) = types.get(&result.structural_type).copied()
    else {
        return false;
    };
    field_initializers_match(function, result, declarations, fields, types, domains)
}

/// Select the same owned subtree that current partial-custody replay consumes.
/// A projected constructor never substitutes its root type for the selected field.
fn constructor_argument_contract<'a>(
    function: &'a PsiOptimizationFunction,
    argument: &terminal_psi::StructuralArgument,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> Option<super::structural_access::StructuralSourceContract<'a>> {
    let mut source = structural_source_contract(function, argument.place)?;
    if !argument.path.is_empty() {
        if argument.access != StructuralAccess::Owned
            || !argument.path.iter().all(|segment| {
                matches!(
                    segment,
                    terminal_psi::StructuralPathSegment::Field(_)
                        | terminal_psi::StructuralPathSegment::FixedIndex(_)
                )
            })
        {
            return None;
        }
        let root = crate::current_ownership::partial_affine_root_type(function, argument.place)?;
        source.structural_type = resolve_structural_path(types, root, &argument.path)?;
    }
    Some(source)
}

pub(crate) fn field_initializers_match(
    function: &PsiOptimizationFunction,
    result: &terminal_psi::StructuralOperationResult,
    declarations: &[terminal_psi::StructuralFieldDeclaration],
    fields: &[terminal_psi::RecordFieldInitializer],
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    domains: &BTreeMap<
        semantic_vocabulary::StructuralDomainId,
        &terminal_psi::StructuralDomainDeclaration,
    >,
) -> bool {
    let declarations = declarations
        .iter()
        .filter(|field| !field.relevance.is_erased());
    if declarations.clone().count() != fields.len() {
        return false;
    }
    declarations.zip(fields).all(|(declaration, initializer)| {
        if declaration.id != initializer.field || declaration.relevance.is_erased() {
            return false;
        }
        match (&declaration.field_type, &initializer.value) {
            (StructuralFieldType::Structural(expected), RecordFieldValue::Structural(argument)) => {
                structural_initializer_matches(function, result, *expected, argument, types)
            }
            (field_type, RecordFieldValue::Structural(argument))
                if field_type.byte_sequence_carrier().is_some() =>
            {
                // Owned bytes and shared descriptors have different backing
                // custody, but both bind the current completed value. Domain
                // wrappers do not select another constructor path.
                let Some(source) = structural_source_contract(function, argument.place) else {
                    return false;
                };
                let Some(declared_source) = types.get(&source.structural_type) else {
                    return false;
                };
                let carrier_matches = match field_type.byte_sequence_carrier() {
                    Some(carrier @ ByteSequenceCarrier::BoundedOwned { .. }) => {
                        source.access == StructuralAccess::Owned
                            && source.multiplicity == StructuralMultiplicity::Unrestricted
                            && declared_source.shape == StructuralTypeShape::ByteSequence(carrier)
                    }
                    Some(ByteSequenceCarrier::BorrowedView {
                        access: None | Some(StructuralAccess::SharedBorrow),
                    }) => {
                        matches!(
                            source.access,
                            StructuralAccess::Owned | StructuralAccess::SharedBorrow
                        ) && matches!(
                            declared_source.shape,
                            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                                access: None | Some(StructuralAccess::SharedBorrow),
                            })
                        )
                    }
                    _ => false,
                };
                argument.access == StructuralAccess::Owned
                    && argument.path.is_empty()
                    && argument.place != result.place
                    && carrier_matches
                    && source.declared_byte_predicates(domains).is_some()
                    && !source_has_claims(function, argument.place)
                    && matches!(
                        source.multiplicity,
                        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
                    )
                    && (result.multiplicity != StructuralMultiplicity::Unrestricted
                        || source.multiplicity == StructuralMultiplicity::Unrestricted)
                    && (field_type.byte_predicates().is_empty()
                        || crate::unit_validation::function_structure::whole_source_predicates(
                            types.values().copied(),
                            domains.values().copied(),
                            function,
                            argument.place,
                        )
                        .is_some_and(|available| available.implies(field_type.byte_predicates())))
            }
            (
                field_type,
                RecordFieldValue::Scalar {
                    value,
                    range_obligation,
                },
            ) => {
                field_type.scalar_type().is_some_and(|scalar_type| {
                    function
                        .parameters
                        .iter()
                        .chain(function.blocks.iter().flat_map(|block| {
                            block
                                .parameters
                                .iter()
                                .chain(block.nodes.iter().flat_map(|node| &node.definitions))
                        }))
                        .any(|definition| {
                            definition.value == *value && definition.scalar_type == scalar_type
                        })
                }) && terminal_semantics::scalar_field_requires_evidence(field_type)
                    == range_obligation.is_some()
            }
            _ => false,
        }
    })
}

fn source_has_claims(function: &PsiOptimizationFunction, place: PlaceId) -> bool {
    function
        .entry_claim_declarations
        .iter()
        .any(|claim| claim.input == place)
        || function
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == place)
        || function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .any(|node| {
                let result = match &node.operation {
                    O::EstablishScalarArray { result, .. }
                    | O::EstablishStructuralArray { result, .. }
                    | O::EstablishCase { result, .. }
                    | O::EstablishRecord { result, .. }
                    | O::EstablishZeroedStructural { result, .. }
                    | O::EstablishReference { result, .. }
                    | O::MoveStructuralField { result, .. }
                    | O::CallStructural {
                        result: AbstractStructuralCallResult::Structural { result, .. },
                        ..
                    }
                    | O::BoundaryCall {
                        result: abstract_operations::AbstractBoundaryResult::Structural(result),
                        ..
                    } => result,
                    _ => return false,
                };
                result.place == place && !result.claims.is_empty()
            })
}

fn plain_record(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
) -> bool {
    record_type(types, root, false, false)
}

/// Local construction can relocate checked reference carriers. Keep this
/// separate from plain payload classification: plain calls and returns do not
/// thereby acquire recursive reference transfer semantics.
pub(crate) fn constructible_record(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
) -> bool {
    record_type(types, root, true, true)
}

/// Whether a member's shape is a shared-view descriptor (`&[T]` element view
/// or a borrowed byte view): the record stores a loan, not owned bytes.
fn shared_view_member(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    child: StructuralTypeId,
) -> bool {
    types.get(&child).is_some_and(|declaration| {
        matches!(
            declaration.shape,
            StructuralTypeShape::ElementView { .. }
                | StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                )
        )
    })
}

fn record_type(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
    references: bool,
    shared_view_members: bool,
) -> bool {
    let mut pending = vec![(root, false)];
    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    while let Some((current, exiting)) = pending.pop() {
        if exiting {
            active.remove(&current);
            complete.insert(current);
            continue;
        }
        if complete.contains(&current) {
            continue;
        }
        if !active.insert(current) {
            return false;
        }
        // A record may embed a completed sum or mixed member as a whole
        // field — the walk descends through every member-bearing shape.
        // Fixed arrays carry every element as a member slot.
        let fields: Vec<&terminal_psi::StructuralFieldDeclaration> = match types
            .get(&current)
            .copied()
            .map(|declaration| &declaration.shape)
        {
            Some(StructuralTypeShape::Record { fields }) => fields.iter().collect(),
            Some(StructuralTypeShape::Sum { cases }) => {
                cases.iter().flat_map(|case| case.fields.iter()).collect()
            }
            Some(StructuralTypeShape::Mixed { fields, cases }) => fields
                .iter()
                .chain(cases.iter().flat_map(|case| case.fields.iter()))
                .collect(),
            Some(StructuralTypeShape::FixedArray { element, .. }) => {
                pending.push((current, true));
                pending.push((*element, false));
                continue;
            }
            _ => return false,
        };
        pending.push((current, true));
        for field in fields {
            // An erased proof member carries its declaration only: no runtime
            // storage, content or cleanup, as Terminal's record validation
            // treats it. Any other erased relevance stays refused.
            if field.relevance.is_erased() {
                if matches!(field.field_type, StructuralFieldType::Erased { .. }) {
                    continue;
                }
                return false;
            }
            match field.field_type {
                StructuralFieldType::Scalar(_)
                | StructuralFieldType::Atomic(_)
                | StructuralFieldType::IeeeFloat(_)
                | StructuralFieldType::RefinedScalar(_)
                | StructuralFieldType::BoundedInteger(_) => {}
                StructuralFieldType::Structural(child)
                    if references
                        && crate::unit_validation::references::referent(types, child).is_some() => {
                }
                // A shared-view member stores a relocated descriptor the same
                // way a borrowed byte view does: the binding roster checks
                // the loan source instead of owning referent cells.
                StructuralFieldType::Structural(child)
                    if shared_view_members && shared_view_member(types, child) => {}
                // Predicates constrain the descriptor's current content, not
                // the containing record's storage-loan permission. Both field
                // presentations retain their backing through descriptor replay;
                // field-store validation independently checks the predicates.
                StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                )
                | StructuralFieldType::PredicateByteSequence {
                    carrier: terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                    ..
                } if references || shared_view_members => {}
                // Primitive arrays retain owned scalar storage as record fields.
                // Replay the complete element chain, including below zero extents.
                StructuralFieldType::Structural(child)
                    if terminal_semantics::scalar_array_leaf_shape(
                        types.values().copied(),
                        child,
                    )
                    .is_some() => {}
                StructuralFieldType::Structural(child) => pending.push((child, false)),
                StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                )
                | StructuralFieldType::PredicateByteSequence {
                    carrier: terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                    ..
                } => {}
                _ => return false,
            }
        }
    }
    true
}

/// A locally constructible whole-carrier type: records, sums, and mixed sums
/// compose scalar leaves and bare primitive-reference leaves. Other owned
/// containers have no relocation vocabulary yet.
pub(crate) fn constructible(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
) -> bool {
    let mut pending = vec![root];
    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if complete.contains(&current) {
            continue;
        }
        if !active.insert(current) {
            return false;
        }
        let Some(declaration) = types.get(&current).copied() else {
            return false;
        };
        // Fixed arrays carry every element as a member slot.
        let matching: Vec<&terminal_psi::StructuralFieldDeclaration> = match &declaration.shape {
            StructuralTypeShape::Record { fields } => fields.iter().collect(),
            StructuralTypeShape::Sum { cases } => {
                cases.iter().flat_map(|case| case.fields.iter()).collect()
            }
            StructuralTypeShape::Mixed { fields, cases } => fields
                .iter()
                .chain(cases.iter().flat_map(|case| case.fields.iter()))
                .collect(),
            StructuralTypeShape::FixedArray { element, .. } => {
                pending.push(*element);
                Vec::new()
            }
            _ => return false,
        };
        for field in matching {
            if field.relevance.is_erased() {
                return false;
            }
            match field.field_type {
                StructuralFieldType::Scalar(_)
                | StructuralFieldType::Atomic(_)
                | StructuralFieldType::IeeeFloat(_)
                | StructuralFieldType::RefinedScalar(_)
                | StructuralFieldType::BoundedInteger(_) => {}
                StructuralFieldType::Structural(child)
                    if crate::unit_validation::references::referent(types, child).is_some() => {}
                StructuralFieldType::Structural(child) => pending.push(child),
                _ => return false,
            }
        }
        active.remove(&current);
        complete.insert(current);
    }
    true
}

/// The completed-value join shared by named fields and fixed-array elements.
pub(super) fn structural_initializer_matches(
    function: &PsiOptimizationFunction,
    result: &terminal_psi::StructuralOperationResult,
    expected: StructuralTypeId,
    argument: &terminal_psi::StructuralArgument,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    let Some(source) = constructor_argument_contract(function, argument, types) else {
        return false;
    };
    // A view-descriptor member stores its descriptor with the
    // loan's own access, the element-view sibling of the
    // borrowed byte view arm below: the record relocates the
    // descriptor, the referent stays borrowed.
    argument.access == StructuralAccess::Owned
        && (source.access == StructuralAccess::Owned
            || (source.access == StructuralAccess::SharedBorrow
                && shared_view_member(types, expected)))
        && source.structural_type == expected
        && source.is_unqualified()
        && !source_has_claims(function, argument.place)
        && argument.place != result.place
        && matches!(
            source.multiplicity,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
        )
        && (result.multiplicity != StructuralMultiplicity::Unrestricted
            || source.multiplicity == StructuralMultiplicity::Unrestricted)
}
