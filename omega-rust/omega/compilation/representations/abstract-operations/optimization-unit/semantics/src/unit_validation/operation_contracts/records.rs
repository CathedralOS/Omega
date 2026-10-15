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
        terminal_psi::nonlinear_multiplicity_pair_pattern!()
    ) && signature.qualifications.is_empty()
        && signature.projected_qualifications.is_empty()
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && claim_transfers.is_empty()
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
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
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
    domains: &BTreeMap<
        semantic_vocabulary::StructuralDomainId,
        &terminal_psi::StructuralDomainDeclaration,
    >,
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
    }) && matches!(result.multiplicity,terminal_psi::nonlinear_multiplicity_pair_pattern!())
        && result.qualifications.is_empty()
        && terminal_semantics::zeroed_projected_byte_qualifications_hold(types.values().copied(), domains.values().copied(), result.structural_type, &result.projected_qualifications)
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
    // Reconstruct issuance from this complete constructor's current result:
    // one whole linear claim, or no claim for an affine/copyable root. The
    // field checks still own every child transfer and its declared type.
    let complete_root_custody = match result.multiplicity {
        StructuralMultiplicity::Linear => {
            matches!(result.claims.as_slice(), [claim] if claim.path.is_empty())
        }
        terminal_psi::nonlinear_multiplicity_pair_pattern!() => result.claims.is_empty(),
    };
    if !exact_result_place
        || !complete_root_custody
        || !constructor_qualifications_match(function, result, fields, types, domains, None)
        || !constructible_record(types, result.structural_type)
    {
        return false;
    }
    let Some(terminal_psi::StructuralTypeDeclaration {
        shape:
            StructuralTypeShape::Record {
                fields: declarations,
                ..
            },
        ..
    }) = types.get(&result.structural_type).copied()
    else {
        return false;
    };
    let fields_match = field_initializers_match(
        function,
        *psi_operation,
        result,
        declarations,
        fields,
        types,
        domains,
    );
    if !fields_match {
        return false;
    }
    // Relocation retains the complete operand roster. In particular a mutable
    // reference's preservation obligations cannot disappear inside a record.
    // Inline byte fields have their own predicate contract, not nominal rows.
    let mut projected = Vec::new();
    for declaration in declarations
        .iter()
        .filter(|field| !field.relevance.is_erased())
    {
        if !matches!(declaration.field_type, StructuralFieldType::Structural(_)) {
            continue;
        }
        let Some(RecordFieldValue::Structural(argument)) = fields
            .iter()
            .find(|binding| binding.field == declaration.id)
            .map(|binding| &binding.value)
        else {
            return false;
        };
        let Some(source) = structural_source_contract(function, argument.place) else {
            return false;
        };
        let (whole, children) = source.selected_qualifications(&argument.path);
        terminal_psi::StructuralPathQualification::append_under(
            &whole,
            &children,
            &[terminal_psi::StructuralPathSegment::Field(
                declaration.identity,
            )],
            &mut projected,
        );
    }
    projected.sort();
    projected.dedup();
    result.projected_qualifications == projected
}

/// Aggregate construction can carry a completed child's memberships below
/// that child's exact declaration identity. The receiving declaration and
/// constructor cannot mint memberships. A selected case contributes only its
/// own payload namespace; common fields retain their independent namespace.
pub(super) fn constructor_qualifications_match(
    function: &PsiOptimizationFunction,
    result: &terminal_psi::StructuralOperationResult,
    fields: &[terminal_psi::RecordFieldInitializer],
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    domains: &BTreeMap<
        semantic_vocabulary::StructuralDomainId,
        &terminal_psi::StructuralDomainDeclaration,
    >,
    selected_case: Option<semantic_vocabulary::StructuralCaseId>,
) -> bool {
    if !result.qualifications.is_empty()
        || !result.qualification_establishments.is_empty()
        || !crate::unit_validation::structural_catalog::structural_projected_qualifications_match(
            result.structural_type,
            &result.projected_qualifications,
            types,
            domains,
        )
    {
        return false;
    }
    let Some(declaration) = types.get(&result.structural_type) else {
        return false;
    };
    result.projected_qualifications.iter().all(|qualification| {
        use terminal_psi::StructuralPathSegment::{Case, Field};
        let (declarations, offset, identity, suffix) =
            match (&declaration.shape, qualification.path.as_slice()) {
                (
                    StructuralTypeShape::Record { fields, .. }
                    | StructuralTypeShape::Mixed { fields, .. },
                    [Field(identity), suffix @ ..],
                ) => (fields.as_slice(), 0, *identity, suffix),
                (
                    StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. },
                    [Case(case), Field(identity), suffix @ ..],
                ) if selected_case == Some(*case) => {
                    let Some(case) = terminal_psi::find_structural_case(cases, *case) else {
                        return false;
                    };
                    let offset = match &declaration.shape {
                        StructuralTypeShape::Mixed { fields, .. } => fields
                            .iter()
                            .filter(|field| !field.relevance.is_erased())
                            .count(),
                        _ => 0,
                    };
                    (case.fields.as_slice(), offset, *identity, suffix)
                }
                _ => return false,
            };
        // Field IDs are local to their namespace. Bind the exact live
        // declaration position, including common fields before case payloads.
        let Some(position) = declarations
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .position(|field| field.identity == identity)
        else {
            return false;
        };
        let Some(initializer) = fields.get(offset + position) else {
            return false;
        };
        let RecordFieldValue::Structural(argument) = &initializer.value else {
            return false;
        };
        let Some(source) = structural_source_contract(function, argument.place) else {
            return false;
        };
        let mut selected = argument.path.clone();
        selected.extend_from_slice(suffix);
        source.carries_qualification(&selected, qualification.domain)
    })
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
    constructor: semantic_vocabulary::OperationId,
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
                    Some(carrier) if carrier.is_shared_readable_view() => {
                        matches!(
                            source.access,
                            StructuralAccess::Owned | StructuralAccess::SharedBorrow
                        ) && declared_source.shape.is_shared_readable_byte_view()
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
                        terminal_psi::nonlinear_multiplicity_pair_pattern!()
                    )
                    && (result.multiplicity != StructuralMultiplicity::Unrestricted
                        || source.multiplicity == StructuralMultiplicity::Unrestricted)
                    && (field_type.byte_predicates().is_empty()
                        || constructed_field_predicates(
                            function,
                            constructor,
                            argument.place,
                            types,
                            domains,
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
                        .chain(
                            function.blocks.iter().flat_map(|block| {
                                block.parameters.iter().chain(block.definitions())
                            }),
                        )
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

/// The byte predicates a constructor operand supplies: those its whole source
/// replays to, plus the declared invariants of a pristine field snapshot this
/// constructor consumes. The destination field requests predicates; it never
/// supplies them.
fn constructed_field_predicates(
    function: &PsiOptimizationFunction,
    constructor: semantic_vocabulary::OperationId,
    source: PlaceId,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    domains: &BTreeMap<
        semantic_vocabulary::StructuralDomainId,
        &terminal_psi::StructuralDomainDeclaration,
    >,
) -> Option<terminal_psi::ByteSequencePredicateSet> {
    let whole = crate::unit_validation::function_structure::whole_source_predicates(
        types.values().copied(),
        domains.values().copied(),
        function,
        source,
    );
    let captured = crate::unit_validation::function_structure::copied_field_predicates_at_transfer(
        types.values().copied(),
        function,
        source,
        constructor,
    );
    match (whole, captured) {
        (Some(mut predicates), Some(captured)) => {
            for predicate in captured.predicates() {
                predicates.insert(predicate);
            }
            Some(predicates)
        }
        (whole, captured) => whole.or(captured),
    }
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
        || function.block_nodes().any(|node| {
            let result = match &node.operation {
                O::EstablishScalarArray { result, .. }
                | O::EstablishStructuralArray { result, .. }
                | abstract_operations::establish_case_or_record_result_pattern!(result)
                | O::EstablishZeroedStructural { result, .. }
                | O::EstablishReference { result, .. }
                | O::EstablishPlacedReference { result, .. }
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
        // Fields and array elements join the same declaration traversal.
        // Classify descriptor leaves here so inserting an array between a
        // record and its reference does not change construction custody.
        // Reference leaves stop at their carrier; they do not own referents.
        let fields: Vec<&terminal_psi::StructuralFieldDeclaration> = match types
            .get(&current)
            .copied()
            .map(|declaration| &declaration.shape)
        {
            Some(StructuralTypeShape::Record { fields, .. }) => fields.iter().collect(),
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
            Some(StructuralTypeShape::PrimitiveScalar(_)) => Vec::new(),
            Some(StructuralTypeShape::Reference { .. })
                if references
                    && crate::unit_validation::references::referent(types, current).is_some() =>
            {
                Vec::new()
            }
            Some(_) if shared_view_members && shared_view_member(types, current) => Vec::new(),
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

/// A locally constructible carrier: records, sums and fixed arrays compose
/// scalar fields and supported reference leaves. A reference owns its loan,
/// not the referent's storage; traversal stops at that carrier boundary.
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
            StructuralTypeShape::Record { fields, .. } => fields.iter().collect(),
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
            StructuralTypeShape::Reference { .. }
                if crate::unit_validation::references::referent(types, current).is_some() =>
            {
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
        && !source_has_claims(function, argument.place)
        && argument.place != result.place
        && matches!(
            source.multiplicity,
            terminal_psi::nonlinear_multiplicity_pair_pattern!()
        )
        && (result.multiplicity != StructuralMultiplicity::Unrestricted
            || source.multiplicity == StructuralMultiplicity::Unrestricted)
}
