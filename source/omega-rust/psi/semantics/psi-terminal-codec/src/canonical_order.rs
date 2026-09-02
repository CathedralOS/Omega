//! Canonical ordering and recursive term-validation policy for module bytes.
//!
//! This module owns exact collection ordering, crash-route canonicality,
//! proposition operand/set ordering, and scalar/content nesting checks. Closed
//! structural-foundation validation remains in the parent codec.

use std::collections::BTreeSet;

use psi_core::{ContentTerm, IntegerMathTerm, Proposition, PropositionId, ScalarTerm};
use psi_terminal::{
    CrashRouteBucket, CrashRouteGuard, OperationKind, ProofPropositionId, ProofValueId,
    StructuralParameterDeclaration, StructuralTypeShape, TerminalModule, Terminator,
};

use super::proposition_wire::encode_proposition;
use super::scalar_term_wire::encode_scalar_term;
use super::wire::Writer;
use super::{CodecError, MAX_CONTENT_TERM_DEPTH, MAX_PROPOSITION_DEPTH, MAX_SCALAR_TERM_DEPTH};

pub(super) fn validate_canonical_order(module: &TerminalModule) -> Result<(), CodecError> {
    if !strictly_increasing(
        module
            .dynamic_dispatch
            .parameters
            .iter()
            .map(|parameter| (parameter.owner, parameter.ordinal)),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "dynamic descriptor parameters by owner and ordinal",
        ));
    }
    if !strictly_increasing(module.dynamic_dispatch.arguments.iter().map(|argument| {
        (
            argument.owner,
            argument.operation,
            argument.parameter_ordinal,
        )
    })) {
        return Err(CodecError::NonCanonicalOrder(
            "dynamic descriptor arguments by owner, operation, and parameter ordinal",
        ));
    }
    if !strictly_increasing(
        module
            .structural_types
            .iter()
            .map(|declaration| declaration.id),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "structural types by StructuralTypeId",
        ));
    }
    if !strictly_increasing(
        module
            .proof_output_calls
            .iter()
            .map(|invocation| (invocation.caller, invocation.ordinal)),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "evidence package invocations by caller and ordinal",
        ));
    }
    if !strictly_increasing(
        module
            .closed_conformance_applications
            .iter()
            .map(|application| {
                (
                    application.owner,
                    application.declaration_identity.as_str(),
                    application.report_fingerprint,
                )
            }),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "closed conformance applications by owner, declaration, and fingerprint",
        ));
    }
    if !strictly_increasing(
        module
            .dynamic_dispatch
            .selections
            .iter()
            .map(|selection| (selection.owner, selection.ordinal)),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "dynamic conformance selections by owner and ordinal",
        ));
    }
    if !strictly_increasing(
        module
            .dynamic_dispatch
            .direct_dispatches
            .iter()
            .map(|dispatch| (dispatch.owner, dispatch.operation)),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "direct dynamic dispatches by owner and operation",
        ));
    }
    if !strictly_increasing(
        module
            .dynamic_dispatch
            .rebound_descriptors
            .iter()
            .map(|descriptor| (descriptor.owner, descriptor.ordinal)),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "rebound dynamic descriptors by owner and ordinal",
        ));
    }
    if !strictly_increasing(
        module
            .dynamic_dispatch
            .indirect_dispatches
            .iter()
            .map(|dispatch| (dispatch.owner, dispatch.operation)),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "indirect dynamic dispatches by owner and operation",
        ));
    }
    if !strictly_increasing(
        module
            .dynamic_dispatch
            .parameter_dispatches
            .iter()
            .map(|dispatch| (dispatch.owner, dispatch.operation)),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "parameter dynamic dispatches by owner and operation",
        ));
    }
    if !strictly_increasing(
        module
            .quotient_correspondences
            .iter()
            .map(|correspondence| correspondence.identity.0.as_slice()),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "quotient correspondences by canonical identity",
        ));
    }
    for invocation in &module.proof_output_calls {
        if invocation
            .outputs
            .iter()
            .enumerate()
            .any(|(position, output)| u32::try_from(position).ok() != Some(output.output_position))
        {
            return Err(CodecError::NonCanonicalOrder(
                "evidence package outputs by position",
            ));
        }
    }
    for declaration in &module.structural_types {
        match &declaration.shape {
            StructuralTypeShape::Record { fields }
                if !strictly_increasing(fields.iter().map(|field| field.id)) =>
            {
                return Err(CodecError::NonCanonicalOrder(
                    "structural fields by StructuralFieldId",
                ));
            }
            StructuralTypeShape::Sum { cases }
                if !strictly_increasing(cases.iter().map(|case| case.id)) =>
            {
                return Err(CodecError::NonCanonicalOrder(
                    "structural cases by StructuralCaseId",
                ));
            }
            StructuralTypeShape::Sum { cases }
                if cases
                    .iter()
                    .any(|case| !strictly_increasing(case.fields.iter().map(|field| field.id))) =>
            {
                return Err(CodecError::NonCanonicalOrder(
                    "structural case fields by StructuralFieldId",
                ));
            }
            StructuralTypeShape::Mixed { fields, .. }
                if !strictly_increasing(fields.iter().map(|field| field.id)) =>
            {
                return Err(CodecError::NonCanonicalOrder(
                    "mixed structural fields by StructuralFieldId",
                ));
            }
            StructuralTypeShape::Mixed { cases, .. }
                if !strictly_increasing(cases.iter().map(|case| case.id)) =>
            {
                return Err(CodecError::NonCanonicalOrder(
                    "mixed structural cases by StructuralCaseId",
                ));
            }
            StructuralTypeShape::Mixed { cases, .. }
                if cases
                    .iter()
                    .any(|case| !strictly_increasing(case.fields.iter().map(|field| field.id))) =>
            {
                return Err(CodecError::NonCanonicalOrder(
                    "mixed structural case fields by StructuralFieldId",
                ));
            }
            _ => {}
        }
    }
    if !strictly_increasing(
        module
            .structural_domains
            .iter()
            .map(|declaration| declaration.id),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "structural domains by StructuralDomainId",
        ));
    }
    if !strictly_increasing(module.services.iter().map(|declaration| declaration.id)) {
        return Err(CodecError::NonCanonicalOrder("services by ServiceId"));
    }
    if module
        .services
        .iter()
        .any(|declaration| !strictly_increasing(declaration.parents.iter().copied()))
    {
        return Err(CodecError::NonCanonicalOrder(
            "service parents by ServiceId",
        ));
    }
    if !strictly_increasing(module.root_service_reach.concrete.iter().copied()) {
        return Err(CodecError::NonCanonicalOrder(
            "concrete root service reach by ServiceId",
        ));
    }
    if !strictly_increasing(
        module
            .root_service_reach
            .installation_dependencies
            .iter()
            .map(|dependency| dependency.requirement_identity.as_str()),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "installation reach dependencies by requirement identity",
        ));
    }
    if module
        .root_service_reach
        .installation_dependencies
        .iter()
        .any(|dependency| !strictly_increasing(dependency.upper_bound.iter().copied()))
    {
        return Err(CodecError::NonCanonicalOrder(
            "installation reach upper bounds by ServiceId",
        ));
    }
    if module
        .float_meaning_equalities
        .iter()
        .enumerate()
        .any(|(index, proposition)| {
            let Ok(index) = u32::try_from(index) else {
                return true;
            };
            proposition.id != ProofPropositionId(index) || proposition.left > proposition.right
        })
    {
        return Err(CodecError::NonCanonicalOrder(
            "float-meaning equalities by dense proposition ID and ordered operands",
        ));
    }
    if !strictly_increasing(
        module
            .boundary_machines
            .iter()
            .map(|declaration| declaration.id),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "boundary machines by BoundaryMachineId",
        ));
    }
    for declaration in &module.boundary_machines {
        validate_parameter_order(&declaration.structural_parameters)?;
        if !strictly_increasing(declaration.requires.iter().copied()) {
            return Err(CodecError::NonCanonicalOrder(
                "boundary requirements by argument index and domain",
            ));
        }
        if !strictly_increasing(declaration.published_service_ceiling.iter().copied()) {
            return Err(CodecError::NonCanonicalOrder(
                "boundary published service ceiling by ServiceId",
            ));
        }
    }
    if !strictly_increasing(module.provider_candidates.iter().map(|candidate| {
        (
            candidate.boundary,
            candidate.provider_identity.as_str(),
            candidate.candidate_identity.as_str(),
            candidate.candidate,
        )
    })) {
        return Err(CodecError::NonCanonicalOrder(
            "provider candidates by exact conformance identity",
        ));
    }
    let mut float_projection_sources = Vec::new();
    for (index, projection) in module.float_meaning_projections.iter().enumerate() {
        let Ok(index) = u32::try_from(index) else {
            return Err(CodecError::NonCanonicalOrder(
                "float-meaning projections by dense proof value and first-use source IDs",
            ));
        };
        if projection.result.id != ProofValueId(index) {
            return Err(CodecError::NonCanonicalOrder(
                "float-meaning projections by dense proof value and first-use source IDs",
            ));
        }
        if let psi_terminal::FloatMeaningSource::TransitionalInput(source) = &projection.source {
            if !float_projection_sources.contains(&source.id) {
                let Ok(expected) = u32::try_from(float_projection_sources.len()) else {
                    return Err(CodecError::NonCanonicalOrder(
                        "float-meaning projections by dense proof value and first-use transitional source IDs",
                    ));
                };
                if source.id.0 != expected {
                    return Err(CodecError::NonCanonicalOrder(
                        "float-meaning projections by dense proof value and first-use transitional source IDs",
                    ));
                }
                float_projection_sources.push(source.id);
            }
        }
    }
    if !strictly_increasing(module.proposition_declarations.iter().cloned().map(
        |mut declaration| {
            declaration.id = PropositionId::new(1).expect("one is nonzero");
            declaration
        },
    )) {
        return Err(CodecError::NonCanonicalOrder(
            "proposition declarations by semantic identity",
        ));
    }
    if !strictly_increasing(
        module
            .proposition_declarations
            .iter()
            .map(|declaration| declaration.id),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "proposition declarations by PropositionId",
        ));
    }
    if !strictly_increasing(module.proposition_applications.iter().cloned().map(
        |mut application| {
            application.id = PropositionId::new(1).expect("one is nonzero");
            application
        },
    )) {
        return Err(CodecError::NonCanonicalOrder(
            "proposition applications by semantic identity",
        ));
    }
    if !strictly_increasing(
        module
            .proposition_applications
            .iter()
            .map(|application| application.id),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "proposition applications by PropositionId",
        ));
    }
    if !strictly_increasing(
        module
            .evidence_terms
            .iter()
            .map(|term| (term.proposition, &term.interface, term.id)),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "evidence terms by proposition and EvidenceTermId",
        ));
    }
    if !strictly_increasing(module.evidence_terms.iter().map(|term| term.id)) {
        return Err(CodecError::NonCanonicalOrder(
            "evidence terms by EvidenceTermId",
        ));
    }
    if !strictly_increasing(
        module
            .evidence_contract_lanes
            .iter()
            .map(|lane| (lane.machine, lane.kind, lane.position)),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "evidence contract lanes by machine, kind, and position",
        ));
    }
    if !strictly_increasing(module.machines.iter().map(|machine| machine.id)) {
        return Err(CodecError::NonCanonicalOrder("machines by MachineId"));
    }
    for machine in &module.machines {
        validate_parameter_order(&machine.structural_parameters)?;
        if !strictly_increasing(machine.entry_claims.iter().map(|claim| claim.claim)) {
            return Err(CodecError::NonCanonicalOrder("entry claims by ClaimId"));
        }
        if !strictly_increasing(machine.published_service_ceiling.iter().copied()) {
            return Err(CodecError::NonCanonicalOrder(
                "machine published service ceiling by ServiceId",
            ));
        }
        if !crash_routes_are_canonical(&machine.contract.crash_routes) {
            return Err(CodecError::NonCanonicalOrder("crash route buckets"));
        }
        validate_crash_route_predicates(&machine.contract.crash_routes)?;
        if !strictly_increasing(machine.blocks.iter().map(|block| block.id)) {
            return Err(CodecError::NonCanonicalOrder("blocks by BlockId"));
        }
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            if let psi_terminal::OperationResult::Structural(result) = &operation.result {
                if !strictly_increasing(result.qualifications.iter().copied()) {
                    return Err(CodecError::NonCanonicalOrder(
                        "structural operation result qualifications by StructuralDomainId",
                    ));
                }
                if !strictly_increasing(result.claims.iter().map(|binding| binding.claim)) {
                    return Err(CodecError::NonCanonicalOrder(
                        "structural operation result claims by ClaimId",
                    ));
                }
            }
            let crash_continuations = match &operation.kind {
                OperationKind::Call {
                    crash_continuations,
                    ..
                }
                | OperationKind::CallUnit {
                    crash_continuations,
                    ..
                }
                | OperationKind::CallStructuralScalar {
                    crash_continuations,
                    ..
                }
                | OperationKind::CallDynamicScalar {
                    crash_continuations,
                    ..
                }
                | OperationKind::CallDynamicParameterScalar {
                    crash_continuations,
                    ..
                }
                | OperationKind::CallStructural {
                    crash_continuations,
                    ..
                } => Some(crash_continuations),
                _ => None,
            };
            if let Some(crash_continuations) = crash_continuations {
                if !crash_routes_are_canonical(crash_continuations) {
                    return Err(CodecError::NonCanonicalOrder(
                        "call crash continuation buckets",
                    ));
                }
                validate_crash_route_predicates(crash_continuations)?;
            }
            match &operation.kind {
                OperationKind::CallUnit {
                    claim_transfers, ..
                }
                | OperationKind::CallStructuralScalar {
                    claim_transfers, ..
                }
                | OperationKind::CallStructural {
                    claim_transfers, ..
                } if !strictly_increasing(claim_transfers.iter().copied()) => {
                    return Err(CodecError::NonCanonicalOrder(
                        "call claim transfers by claim and argument index",
                    ));
                }
                OperationKind::CallStructural {
                    returned_claim_transfers,
                    ..
                } if !strictly_increasing(returned_claim_transfers.iter().copied()) => {
                    return Err(CodecError::NonCanonicalOrder(
                        "structural-call returned claim transfers",
                    ));
                }
                OperationKind::CallStructural {
                    selected_evidence, ..
                } if !strictly_increasing(selected_evidence.iter().map(|binding| {
                    (
                        binding.guard,
                        binding.position,
                        binding.output_field.as_str(),
                        binding.output,
                    )
                })) || selected_evidence.iter().any(|binding| {
                    !strictly_increasing(binding.validity.proposition_dependencies.iter().copied())
                        || !strictly_increasing(
                            binding.validity.interface_dependencies.iter().copied(),
                        )
                        || !strictly_increasing(binding.uses.iter().cloned())
                }) =>
                {
                    return Err(CodecError::NonCanonicalOrder(
                        "guarded-call selections or validity dependency roots",
                    ));
                }
                OperationKind::BoundaryCall {
                    completion_receipts,
                    ..
                } if !strictly_increasing(completion_receipts.iter().copied()) => {
                    return Err(CodecError::NonCanonicalOrder(
                        "boundary claim settlements by claim and argument index",
                    ));
                }
                _ => {}
            }
        }
        for block in &machine.blocks {
            if let Terminator::Crash { site_guard, .. } = &block.terminator {
                for predicate in site_guard {
                    validate_canonical_proposition(predicate.proposition(), 0)?;
                }
            }
            if let Terminator::ReturnUnitPartialAffine {
                residual_affine_discards,
                ..
            } = &block.terminator
            {
                let mut places_and_paths = BTreeSet::new();
                if residual_affine_discards.iter().any(|discard| {
                    !places_and_paths.insert((discard.place, discard.path.as_slice()))
                }) {
                    return Err(CodecError::NonCanonicalOrder(
                        "partial affine residual discards are unique",
                    ));
                }
            }
        }
        if !strictly_increasing(machine.structural_places.iter().map(|place| place.id)) {
            return Err(CodecError::NonCanonicalOrder(
                "structural places by PlaceId",
            ));
        }
        if !strictly_increasing(
            machine
                .content_entry_claims
                .iter()
                .map(|binding| binding.claim),
        ) {
            return Err(CodecError::NonCanonicalOrder(
                "content entry claims by ClaimId",
            ));
        }
        if machine
            .content_entry_claims
            .iter()
            .any(|binding| !strictly_increasing(binding.projections.iter()))
        {
            return Err(CodecError::NonCanonicalOrder(
                "entry-claim content projections by identity and algebra",
            ));
        }
        if !strictly_increasing(
            machine
                .content_identity_reshuffles
                .iter()
                .map(|reshuffle| reshuffle.claim),
        ) {
            return Err(CodecError::NonCanonicalOrder(
                "content identity reshuffles by ClaimId",
            ));
        }
        if machine
            .content_identity_reshuffles
            .iter()
            .any(|reshuffle| !strictly_increasing(reshuffle.projections.iter()))
        {
            return Err(CodecError::NonCanonicalOrder(
                "claim content projections by identity and algebra",
            ));
        }
        if !strictly_increasing(machine.content_partition_compositions.iter()) {
            return Err(CodecError::NonCanonicalOrder(
                "content partition compositions",
            ));
        }
        for composition in &machine.content_partition_compositions {
            if !strictly_increasing(
                composition
                    .source_structural_places
                    .iter()
                    .map(|place| place.id),
            ) {
                return Err(CodecError::NonCanonicalOrder(
                    "partition source structural places by PlaceId",
                ));
            }
            if !strictly_increasing(composition.input_claims.iter().copied()) {
                return Err(CodecError::NonCanonicalOrder(
                    "partition input claims by ClaimId",
                ));
            }
            if !strictly_increasing(composition.substitutions.iter()) {
                return Err(CodecError::NonCanonicalOrder(
                    "partition place substitutions",
                ));
            }
        }
        if !strictly_increasing(
            machine
                .contract
                .ensures
                .iter()
                .map(|clause| clause.obligation),
        ) {
            return Err(CodecError::NonCanonicalOrder("ensures by ObligationId"));
        }
        let propositions = machine.contract.requires.iter().chain(
            machine
                .contract
                .ensures
                .iter()
                .map(|clause| &clause.proposition),
        );
        for proposition in propositions {
            validate_canonical_proposition(proposition, 0)?;
        }
        if !canonical_propositions_strictly_increase(&machine.contract.requires)? {
            return Err(CodecError::NonCanonicalOrder("requires propositions"));
        }
    }
    Ok(())
}

fn validate_parameter_order(
    parameters: &[StructuralParameterDeclaration],
) -> Result<(), CodecError> {
    if parameters
        .iter()
        .enumerate()
        .any(|(index, parameter)| parameter.position != index as u32)
    {
        return Err(CodecError::NonCanonicalOrder(
            "structural parameters by dense position",
        ));
    }
    if parameters
        .iter()
        .any(|parameter| !strictly_increasing(parameter.qualifications.iter().copied()))
    {
        return Err(CodecError::NonCanonicalOrder(
            "structural parameter qualifications by StructuralDomainId",
        ));
    }
    if parameters.iter().any(|parameter| {
        parameter
            .projected_qualifications
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    }) {
        return Err(CodecError::NonCanonicalOrder(
            "projected structural qualifications by path and StructuralDomainId",
        ));
    }
    Ok(())
}

pub(super) fn crash_routes_are_canonical(routes: &[CrashRouteBucket]) -> bool {
    !routes.windows(2).any(|pair| pair[0].cause >= pair[1].cause)
        && routes.iter().all(|bucket| {
            !bucket.alternatives.is_empty()
                && !bucket
                    .alternatives
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                && (!bucket.alternatives.contains(&CrashRouteGuard::Truth)
                    || bucket.alternatives == [CrashRouteGuard::Truth])
        })
}

pub(super) fn validate_crash_route_predicates(
    routes: &[CrashRouteBucket],
) -> Result<(), CodecError> {
    for predicate in routes
        .iter()
        .flat_map(|bucket| &bucket.alternatives)
        .filter_map(|guard| match guard {
            CrashRouteGuard::Truth => None,
            CrashRouteGuard::Predicate(predicate) => Some(predicate),
        })
    {
        validate_canonical_proposition(predicate.proposition(), 0)?;
    }
    Ok(())
}

fn validate_canonical_proposition(
    proposition: &Proposition,
    depth: usize,
) -> Result<(), CodecError> {
    if depth > MAX_PROPOSITION_DEPTH {
        return Err(CodecError::PropositionNestingTooDeep);
    }
    match proposition {
        Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => Ok(()),
        Proposition::Equal(left, right) => {
            validate_scalar_term_depth(left)?;
            validate_scalar_term_depth(right)?;
            if canonical_scalar_term_bytes(left)? > canonical_scalar_term_bytes(right)? {
                return Err(CodecError::NonCanonicalOrder("equality operands"));
            }
            Ok(())
        }
        Proposition::LessThan(left, right) | Proposition::LessOrEqual(left, right) => {
            validate_scalar_term_depth(left)?;
            validate_scalar_term_depth(right)
        }
        Proposition::IntegerMathEqual(left, right) => {
            validate_integer_math_term_depth(left)?;
            validate_integer_math_term_depth(right)?;
            if left > right {
                return Err(CodecError::NonCanonicalOrder(
                    "mathematical integer equality operands",
                ));
            }
            Ok(())
        }
        Proposition::IntegerMathLessThan(left, right)
        | Proposition::IntegerMathLessOrEqual(left, right) => {
            validate_integer_math_term_depth(left)?;
            validate_integer_math_term_depth(right)
        }
        Proposition::IeeeFloatComparison { left, right, .. } => {
            if left > right {
                return Err(CodecError::NonCanonicalOrder("IEEE equality operands"));
            }
            Ok(())
        }
        Proposition::ByteSequenceEqual { left, right } => {
            if left > right {
                return Err(CodecError::NonCanonicalOrder(
                    "byte-sequence equality operands",
                ));
            }
            Ok(())
        }
        Proposition::StructuralCaseMembership { .. } => Ok(()),
        Proposition::Conjunction(conjuncts) => {
            if conjuncts
                .iter()
                .any(|conjunct| matches!(conjunct, Proposition::Conjunction(_)))
            {
                return Err(CodecError::NestedConjunction);
            }
            for conjunct in conjuncts {
                validate_canonical_proposition(conjunct, depth + 1)?;
            }
            if !canonical_propositions_strictly_increase(conjuncts)? {
                return Err(CodecError::NonCanonicalOrder("conjunction propositions"));
            }
            Ok(())
        }
        Proposition::Disjunction(disjuncts) => {
            if disjuncts
                .iter()
                .any(|disjunct| matches!(disjunct, Proposition::Disjunction(_)))
            {
                return Err(CodecError::NestedDisjunction);
            }
            for disjunct in disjuncts {
                validate_canonical_proposition(disjunct, depth + 1)?;
            }
            if !canonical_propositions_strictly_increase(disjuncts)? {
                return Err(CodecError::NonCanonicalOrder("disjunction propositions"));
            }
            Ok(())
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            validate_canonical_proposition(premise, depth + 1)?;
            validate_canonical_proposition(conclusion, depth + 1)
        }
        Proposition::ContentConservation(conservation) => {
            validate_content_term_depth(conservation.left(), 0)?;
            validate_content_term_depth(conservation.right(), 0)
        }
    }
}

fn validate_integer_math_term_depth(term: &IntegerMathTerm) -> Result<(), CodecError> {
    let mut pending = vec![(term, 0_usize)];
    while let Some((term, depth)) = pending.pop() {
        if depth > MAX_SCALAR_TERM_DEPTH {
            return Err(CodecError::ScalarTermNestingTooDeep);
        }
        match term {
            IntegerMathTerm::MathValue { .. } | IntegerMathTerm::IntegerLiteral(_) => {}
            IntegerMathTerm::Add(left, right)
            | IntegerMathTerm::Subtract(left, right)
            | IntegerMathTerm::Multiply(left, right) => {
                pending.push((left, depth + 1));
                pending.push((right, depth + 1));
            }
            IntegerMathTerm::ShiftLeft { value, count } => {
                pending.push((value, depth + 1));
                pending.push((count, depth + 1));
            }
        }
    }
    Ok(())
}

fn canonical_propositions_strictly_increase(
    propositions: &[Proposition],
) -> Result<bool, CodecError> {
    let mut previous = None;
    for proposition in propositions {
        let bytes = canonical_proposition_bytes(proposition)?;
        if previous.as_ref().is_some_and(|previous| previous >= &bytes) {
            return Ok(false);
        }
        previous = Some(bytes);
    }
    Ok(true)
}

fn canonical_proposition_bytes(proposition: &Proposition) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    encode_proposition(&mut writer, proposition, 0)?;
    Ok(writer.finish())
}

/// Canonical bytewise ordering key for one terminal proposition. Producers
/// that construct canonical sets use the codec-owned order rather than Rust
/// enum declaration order.
pub fn canonical_proposition_order_key(proposition: &Proposition) -> Result<Vec<u8>, CodecError> {
    canonical_proposition_bytes(proposition)
}

fn canonical_scalar_term_bytes(term: &ScalarTerm) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    encode_scalar_term(&mut writer, term, 0)?;
    Ok(writer.finish())
}

fn validate_scalar_term_depth(term: &ScalarTerm) -> Result<(), CodecError> {
    let mut pending = vec![(term, 0_usize)];
    while let Some((term, depth)) = pending.pop() {
        if depth > MAX_SCALAR_TERM_DEPTH {
            return Err(CodecError::ScalarTermNestingTooDeep);
        }
        match term {
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => {
                pending.push((operand, depth + 1));
            }
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. }
            | ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. }
            | ScalarTerm::ExactIntegerDivide { left, right, .. }
            | ScalarTerm::ExactIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerDivide { left, right, .. }
            | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
            | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
            | ScalarTerm::SaturatingIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
                pending.push((left, depth + 1));
                pending.push((right, depth + 1));
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                pending.push((value, depth + 1));
                pending.push((count, depth + 1));
            }
            ScalarTerm::Value { .. }
            | ScalarTerm::BooleanField { .. }
            | ScalarTerm::IntegerField { .. }
            | ScalarTerm::Boolean(_)
            | ScalarTerm::Integer { .. } => {}
        }
    }
    Ok(())
}

fn validate_content_term_depth(term: &ContentTerm, depth: usize) -> Result<(), CodecError> {
    if depth > MAX_CONTENT_TERM_DEPTH {
        return Err(CodecError::ContentTermNestingTooDeep);
    }
    if let ContentTerm::Separate(terms) = term {
        for term in terms {
            validate_content_term_depth(term, depth + 1)?;
        }
    }
    Ok(())
}

fn strictly_increasing<T: Ord>(values: impl IntoIterator<Item = T>) -> bool {
    let mut previous = None;
    for value in values {
        if previous.as_ref().is_some_and(|previous| previous >= &value) {
            return false;
        }
        previous = Some(value);
    }
    true
}
