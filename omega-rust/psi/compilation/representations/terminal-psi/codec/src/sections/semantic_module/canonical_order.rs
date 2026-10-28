//! Canonical ordering and recursive term-validation policy for module bytes.
//!
//! This module owns exact collection ordering, crash-route canonicality,
//! proposition operand/set ordering, and scalar/content nesting checks. Closed
//! structural-foundation validation remains in the parent codec.

use std::collections::BTreeSet;

use semantic_vocabulary::{ContentTerm, IntegerMathTerm, Proposition, PropositionId, ScalarTerm};
use terminal_psi::{
    CrashRouteBucket, CrashRouteGuard, OperationKind, ProofPropositionId, ProofValueId,
    StructuralParameterDeclaration, StructuralTypeShape, TerminalModule, Terminator,
};

use super::proposition_wire::encode_proposition;
use super::scalar_term_wire::encode_scalar_term;
use super::wire::{Writer, require_increasing, strictly_increasing};
use super::{CodecError, MAX_CONTENT_TERM_DEPTH, MAX_PROPOSITION_DEPTH, MAX_SCALAR_TERM_DEPTH};

pub(crate) fn validate_canonical_order(module: &TerminalModule) -> Result<(), CodecError> {
    require_increasing(
        module
            .structural_referents
            .iter()
            .map(|referent| referent.id),
        "structural referent identities",
    )?;
    require_increasing(
        module
            .callback_applications
            .iter()
            .map(|row| row.application_index),
        "callback applications",
    )?;
    for application in &module.callback_applications {
        if application.requirement.is_empty()
            || application.published_contract == [0; 32]
            || application.body.contract == [0; 32]
        {
            return Err(CodecError::MalformedStructuralFoundation(
                "callback application lacks its requirement or contract",
            ));
        }
    }
    require_increasing(
        module.boundary_application_candidates.iter(),
        "boundary application candidates",
    )?;
    require_increasing(
        module.task_activation_demands.demands.iter().map(|demand| {
            (
                demand.report_fingerprint,
                demand.machine_callable_identity,
                demand.entry,
            )
        }),
        "task activation demands by report fingerprint, machine, and entry",
    )?;
    require_increasing(
        module
            .task_activation_demands
            .structural_referents
            .iter()
            .map(|row| row.id),
        "task structural referents",
    )?;
    require_increasing(
        module
            .task_activation_demands
            .target_layout_applications
            .iter()
            .map(|row| row.id),
        "task target layout applications",
    )?;
    for application in &module.task_activation_demands.target_layout_applications {
        require_increasing(
            application
                .realizations
                .iter()
                .map(|row| row.target.as_str()),
            "task target layout alternatives",
        )?;
    }
    for demand in &module.task_activation_demands.demands {
        require_increasing(
            demand.frames.iter().map(|frame| frame.identity),
            "task activation frames",
        )?;
        require_increasing(
            demand.crossings.iter().map(|crossing| crossing.identity),
            "task activation crossings",
        )?;
        for crossing in &demand.crossings {
            if crossing
                .live_carry
                .iter()
                .any(|live| !strictly_increasing(live.claims.iter().copied()))
            {
                return Err(CodecError::NonCanonicalOrder(
                    "task activation live carry claims",
                ));
            }
        }
    }
    require_increasing(
        module
            .task_activation_demands
            .structural_types
            .iter()
            .map(|declaration| declaration.id),
        "task activation structural types by StructuralTypeId",
    )?;
    require_increasing(
        module
            .boundary_application_demands
            .iter()
            .map(|demand| demand.terminal_operation()),
        "boundary application demands by operation",
    )?;
    for candidate in &module.boundary_application_candidates {
        if candidate.selection_identity.is_absent()
            || candidate.target.as_ref().is_some_and(String::is_empty)
            || candidate.body.contract == [0; 32]
            || candidate
                .body
                .specialization
                .as_ref()
                .is_some_and(|row| row.commitment == [0; 32])
        {
            return Err(CodecError::MalformedStructuralFoundation(
                "empty boundary candidate identity or commitment",
            ));
        }
    }
    for application in module
        .boundary_application_demands
        .iter()
        .map(|demand| demand.application())
        .chain(
            module
                .boundary_application_candidates
                .iter()
                .map(|candidate| &candidate.application),
        )
    {
        match application {
            terminal_psi::BoundaryApplication::Empty => {}
            terminal_psi::BoundaryApplication::Exact(arguments) => {
                if arguments.is_empty() {
                    return Err(CodecError::MalformedStructuralFoundation(
                        "nonempty boundary application has no arguments",
                    ));
                }
                for (ordinal, argument) in arguments.iter().enumerate() {
                    if usize::try_from(argument.binder_ordinal()).ok() != Some(ordinal) {
                        return Err(CodecError::NonCanonicalOrder(
                            "boundary application binders",
                        ));
                    }
                    if let terminal_psi::BoundaryApplicationArgument::Const {
                        value_type,
                        value_encoding,
                        ..
                    } = argument
                        && (value_type.is_empty() || value_encoding.is_empty())
                    {
                        return Err(CodecError::MalformedStructuralFoundation(
                            "boundary const application has an empty canonical value identity",
                        ));
                    }
                }
            }
        }
    }
    require_increasing(
        module
            .target_machine_families
            .iter()
            .map(|family| family.canonical),
        "target-machine families by canonical callable",
    )?;
    for family in &module.target_machine_families {
        require_increasing(
            family.members.iter().map(|member| member.target.as_str()),
            "target-family members by deployment tag",
        )?;
    }
    require_increasing(
        module
            .scalar_block_invariants
            .iter()
            .map(|invariant| (invariant.machine, invariant.header)),
        "scalar block invariants by machine and header",
    )?;
    for invariant in &module.scalar_block_invariants {
        validate_canonical_proposition(&invariant.predicate, 0)?;
        require_increasing(
            invariant.arrivals.iter().map(|arrival| arrival.edge),
            "scalar block invariant arrivals by EdgeId",
        )?;
    }
    require_increasing(
        module
            .operation_crash_contracts
            .iter()
            .map(|contract| (contract.machine, contract.operation)),
        "operation crash contracts by machine and operation",
    )?;
    for contract in &module.operation_crash_contracts {
        if contract.published_routes.is_empty()
            || !crash_routes_are_canonical(&contract.published_routes)
        {
            return Err(CodecError::NonCanonicalOrder(
                "operation crash contract published route buckets",
            ));
        }
        validate_crash_route_predicates(&contract.published_routes)?;
        if !crash_routes_are_canonical(&contract.crash_continuations) {
            return Err(CodecError::NonCanonicalOrder(
                "operation crash contract continuation buckets",
            ));
        }
        validate_crash_route_predicates(&contract.crash_continuations)?;
    }
    require_increasing(
        module
            .suspension_call_sites
            .iter()
            .map(|site| (site.operation, site.crossing)),
        "suspension call sites by operation and crossing",
    )?;
    require_increasing(
        module
            .suspension_call_plans
            .iter()
            .map(|plan| (plan.operation, plan.crossing)),
        "suspension call plans by operation and crossing",
    )?;
    for plan in &module.suspension_call_plans {
        // Storage identity is not value-occurrence identity. A repeated actual
        // still occupies two formal positions, and each row retains its own
        // claims and policy. This roster is an ordered multiset of demands;
        // equality is valid here, unlike declaration/claim identity tables.
        if !plan
            .live_values
            .iter()
            .map(|live| (&live.place, live.storage, live.value_type, live.effective))
            .is_sorted()
        {
            return Err(CodecError::NonCanonicalOrder(
                "suspension live values by place, storage, type, and policy",
            ));
        }
        if plan
            .live_values
            .iter()
            .any(|live| !strictly_increasing(live.claims.iter().copied()))
        {
            return Err(CodecError::NonCanonicalOrder(
                "suspension live claims by ClaimId",
            ));
        }
    }
    require_increasing(
        module
            .structural_types
            .iter()
            .map(|declaration| declaration.id),
        "structural types by StructuralTypeId",
    )?;
    require_increasing(
        module
            .proof_output_calls
            .iter()
            .map(|invocation| (invocation.caller, invocation.ordinal)),
        "evidence package invocations by caller and ordinal",
    )?;
    require_increasing(
        module
            .closed_conformance_applications
            .iter()
            .map(|application| {
                (
                    application.owner,
                    module
                        .identities
                        .text(application.declaration_identity)
                        .unwrap_or_default(),
                    application.report_fingerprint,
                )
            }),
        "closed conformance applications by owner, declaration, and fingerprint",
    )?;
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
        if let Some(terminal_psi::StructuralFieldType::RefinedScalar(refinement)) =
            &declaration.scalar_restriction
        {
            validate_canonical_proposition(refinement.predicate(), 0)?;
        }
        let (fields, cases) = match &declaration.shape {
            StructuralTypeShape::Record { fields, .. } => (fields.as_slice(), &[][..]),
            StructuralTypeShape::Sum { cases } => (&[][..], cases.as_slice()),
            StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases.as_slice()),
            _ => (&[][..], &[][..]),
        };
        for field in fields
            .iter()
            .chain(cases.iter().flat_map(|case| &case.fields))
        {
            if let terminal_psi::StructuralFieldType::RefinedScalar(refinement) = &field.field_type
            {
                validate_canonical_proposition(refinement.predicate(), 0)?;
            }
        }
        match &declaration.shape {
            StructuralTypeShape::Record { fields, .. }
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
    require_increasing(
        module
            .structural_domains
            .iter()
            .map(|declaration| declaration.id),
        "structural domains by StructuralDomainId",
    )?;
    require_increasing(
        module.services.iter().map(|declaration| declaration.id),
        "services by ServiceId",
    )?;
    if module
        .services
        .iter()
        .any(|declaration| !strictly_increasing(declaration.parents.iter().copied()))
    {
        return Err(CodecError::NonCanonicalOrder(
            "service parents by ServiceId",
        ));
    }
    require_increasing(
        module.root_service_reach.concrete.iter().copied(),
        "concrete root service reach by ServiceId",
    )?;
    require_increasing(
        module
            .root_service_reach
            .installation_dependencies
            .iter()
            .map(|dependency| {
                module
                    .identities
                    .text(dependency.requirement_identity)
                    .unwrap_or_default()
            }),
        "installation reach dependencies by requirement identity",
    )?;
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
    require_increasing(
        module
            .boundary_machines
            .iter()
            .map(|declaration| declaration.id),
        "boundary machines by BoundaryMachineId",
    )?;
    for declaration in &module.boundary_machines {
        if let terminal_psi::BoundaryMachineResult::Structural(result) = &declaration.result {
            require_increasing(
                result.qualifications.iter().copied(),
                "boundary result qualifications by StructuralDomainId",
            )?;
            if !strictly_increasing(result.projected_qualifications.iter()) {
                return Err(CodecError::NonCanonicalOrder(
                    "boundary result projected qualifications by path and domain",
                ));
            }
        }
        if !crash_routes_are_canonical(&declaration.crash_routes) {
            return Err(CodecError::NonCanonicalOrder(
                "boundary crash route buckets",
            ));
        }
        validate_crash_route_predicates(&declaration.crash_routes)?;
        validate_parameter_order(&declaration.structural_parameters)?;
        require_increasing(
            declaration.requires.iter().copied(),
            "boundary requirements by argument index and domain",
        )?;
        require_increasing(
            declaration.normal_qualifications.iter().copied(),
            "boundary normal parameter qualifications",
        )?;
        require_increasing(
            declaration.scalar_domain_requirements.iter().copied(),
            "boundary scalar domain requirements by argument index and domain",
        )?;
        for proposition in &declaration.scalar_requires {
            validate_canonical_proposition(proposition, 0)?;
        }
        if !canonical_propositions_strictly_increase(&declaration.scalar_requires)? {
            return Err(CodecError::NonCanonicalOrder(
                "boundary scalar requires propositions",
            ));
        }
        for proposition in &declaration.scalar_ensures {
            validate_canonical_proposition(proposition, 0)?;
        }
        if !canonical_propositions_strictly_increase(&declaration.scalar_ensures)? {
            return Err(CodecError::NonCanonicalOrder(
                "boundary scalar ensures propositions",
            ));
        }
        require_increasing(
            declaration.published_service_ceiling.iter().copied(),
            "boundary published service ceiling by ServiceId",
        )?;
        require_increasing(
            declaration.fixed_service_reach.iter().copied(),
            "boundary fixed service reach by ServiceId",
        )?;
    }
    require_increasing(
        module.provider_candidates.iter().map(|candidate| {
            (
                candidate.boundary,
                module
                    .identities
                    .text(candidate.provider_identity)
                    .unwrap_or_default(),
                module
                    .identities
                    .text(candidate.provider_declaration.identity())
                    .unwrap_or_default(),
                module
                    .identities
                    .text(candidate.candidate_identity)
                    .unwrap_or_default(),
                candidate.candidate,
            )
        }),
        "provider candidates by exact conformance identity",
    )?;
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
        if let terminal_psi::FloatMeaningSource::TransitionalInput(source) = &projection.source
            && !float_projection_sources.contains(&source.id)
        {
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
    require_increasing(
        module
            .proposition_declarations
            .iter()
            .cloned()
            .map(|mut declaration| {
                declaration.id = PropositionId::new(1).expect("one is nonzero");
                declaration
            }),
        "proposition declarations by semantic identity",
    )?;
    require_increasing(
        module
            .proposition_declarations
            .iter()
            .map(|declaration| declaration.id),
        "proposition declarations by PropositionId",
    )?;
    require_increasing(
        module
            .proposition_applications
            .iter()
            .cloned()
            .map(|mut application| {
                application.id = PropositionId::new(1).expect("one is nonzero");
                application
            }),
        "proposition applications by semantic identity",
    )?;
    require_increasing(
        module
            .proposition_applications
            .iter()
            .map(|application| application.id),
        "proposition applications by PropositionId",
    )?;
    require_increasing(
        module
            .evidence_terms
            .iter()
            .map(|term| (term.proposition, &term.interface, term.id)),
        "evidence terms by proposition and EvidenceTermId",
    )?;
    require_increasing(
        module.evidence_terms.iter().map(|term| term.id),
        "evidence terms by EvidenceTermId",
    )?;
    require_increasing(
        module
            .evidence_contract_lanes
            .iter()
            .map(|lane| (lane.machine, lane.kind, lane.position)),
        "evidence contract lanes by machine, kind, and position",
    )?;
    require_increasing(
        module.machines.iter().map(|machine| machine.id),
        "machines by MachineId",
    )?;
    for machine in &module.machines {
        validate_parameter_order(&machine.structural_parameters)?;
        for block in &machine.blocks {
            if let Some(invocation) = block.terminator.module_invocation() {
                require_increasing(
                    invocation.claim_transfers.iter(),
                    "invocation claim transfers by claim and argument index",
                )?;
                if !crash_routes_are_canonical(invocation.crash_continuations) {
                    return Err(CodecError::NonCanonicalOrder(
                        "invocation crash continuation buckets",
                    ));
                }
                validate_crash_route_predicates(invocation.crash_continuations)?;
            }
            if let Terminator::ExternalCompletion {
                frontier_lower_bound,
                ..
            } = &block.terminator
                && !strictly_increasing(frontier_lower_bound.iter().copied())
            {
                return Err(CodecError::NonCanonicalOrder(
                    "external completion frontier lower bound by ClaimId",
                ));
            }
            validate_parameter_order(&block.structural_parameters)?;
        }
        require_increasing(
            machine.entry_claims.iter().map(|claim| claim.claim),
            "entry claims by ClaimId",
        )?;
        require_increasing(
            machine.block_entry_claims.iter().map(|claim| claim.claim),
            "block entry claims by ClaimId",
        )?;
        require_increasing(
            machine
                .loan_completions
                .iter()
                .map(|completion| (completion.operation, completion.place, &completion.path)),
            "loan completions by call and owned region",
        )?;
        for completion in &machine.loan_completions {
            require_increasing(
                completion.claims.iter().map(|claim| claim.claim),
                "loan completion claims by ClaimId",
            )?;
        }
        require_increasing(
            machine.published_service_ceiling.iter().copied(),
            "machine published service ceiling by ServiceId",
        )?;
        if !crash_routes_are_canonical(&machine.contract.crash_routes) {
            return Err(CodecError::NonCanonicalOrder("crash route buckets"));
        }
        validate_crash_route_predicates(&machine.contract.crash_routes)?;
        require_increasing(
            machine.blocks.iter().map(|block| block.id),
            "blocks by BlockId",
        )?;
        for operation in machine.operations() {
            if let Some(result) = operation.result.as_structural() {
                require_increasing(
                    result.qualifications.iter().copied(),
                    "structural operation result qualifications by StructuralDomainId",
                )?;
                require_increasing(
                    result.claims.iter().map(|binding| binding.claim),
                    "structural operation result claims by ClaimId",
                )?;
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
                } if !strictly_increasing(claim_transfers.iter()) => {
                    return Err(CodecError::NonCanonicalOrder(
                        "call claim transfers by claim and argument index",
                    ));
                }
                terminal_psi::call_structural_pair_pattern!(selected_evidence, ..)
                    if !strictly_increasing(selected_evidence.iter().map(|binding| {
                        (
                            binding.guard,
                            binding.position,
                            binding.output_field.as_str(),
                            binding.output,
                        )
                    })) || selected_evidence.iter().any(|binding| {
                        !strictly_increasing(binding.argument_substitutions.iter().cloned())
                            || !strictly_increasing(
                                binding.validity.proposition_dependencies.iter().copied(),
                            )
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
                } if !strictly_increasing(completion_receipts.iter()) => {
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
            if let Terminator::Jump {
                cleanup_actions, ..
            } = &block.terminator
            {
                let mut places_and_paths = BTreeSet::new();
                if cleanup_actions
                    .iter()
                    .filter_map(|action| match action {
                        terminal_psi::TerminalAffineCleanupAction::DiscardResidual(discard) => {
                            Some(discard)
                        }
                        _ => None,
                    })
                    .any(|discard| {
                        !places_and_paths.insert((discard.place, discard.path.as_slice()))
                    })
                {
                    return Err(CodecError::NonCanonicalOrder(
                        "partial affine residual discards are unique",
                    ));
                }
            }
        }
        require_increasing(
            machine.structural_places.iter().map(|place| place.id),
            "structural places by PlaceId",
        )?;
        require_increasing(
            machine
                .content_entry_claims
                .iter()
                .map(|binding| binding.claim),
            "content entry claims by ClaimId",
        )?;
        if machine
            .content_entry_claims
            .iter()
            .any(|binding| !strictly_increasing(binding.projections.iter()))
        {
            return Err(CodecError::NonCanonicalOrder(
                "entry-claim content projections by identity and algebra",
            ));
        }
        validate_identity_reshuffles(&machine.content_identity_reshuffles)?;
        require_increasing(
            machine.content_partition_compositions.iter(),
            "content partition compositions",
        )?;
        for composition in &machine.content_partition_compositions {
            require_increasing(
                composition
                    .source_structural_places
                    .iter()
                    .map(|place| place.id),
                "partition source structural places by PlaceId",
            )?;
            require_increasing(
                composition.input_claims.iter().copied(),
                "partition input claims by ClaimId",
            )?;
            require_increasing(
                composition.substitutions.iter(),
                "partition place substitutions",
            )?;
        }
        require_increasing(
            machine
                .contract
                .ensures
                .iter()
                .map(|clause| clause.obligation),
            "ensures by ObligationId",
        )?;
        let propositions = machine
            .contract
            .requires
            .iter()
            .chain(machine.contract.source_checked_requires.iter())
            .chain(
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
        if !canonical_propositions_strictly_increase(&machine.contract.source_checked_requires)? {
            return Err(CodecError::NonCanonicalOrder(
                "source-checked requires propositions",
            ));
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
    if parameters
        .iter()
        .any(|parameter| !strictly_increasing(parameter.projected_qualifications.iter()))
    {
        return Err(CodecError::NonCanonicalOrder(
            "projected structural qualifications by path and StructuralDomainId",
        ));
    }
    Ok(())
}

pub(crate) fn crash_routes_are_canonical(routes: &[CrashRouteBucket]) -> bool {
    strictly_increasing(routes.iter().map(|bucket| bucket.cause))
        && routes.iter().all(|bucket| {
            !bucket.alternatives.is_empty()
                && strictly_increasing(bucket.alternatives.iter())
                && (!bucket.alternatives.contains(&CrashRouteGuard::Truth)
                    || bucket.alternatives == [CrashRouteGuard::Truth])
        })
}

pub(crate) fn validate_crash_route_predicates(
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
    // The proposition depth guard walks its tree through an explicit worklist
    // rather than the call stack, matching `validate_scalar_term_depth` and
    // `validate_integer_math_term_depth` below: a deeply nested conjunction,
    // disjunction, or implication must reach the depth bound and reject instead
    // of overflowing the stack on hosts with small thread stacks.
    enum Step<'a> {
        Validate(&'a Proposition, usize),
        StrictlyIncreasing(&'a [Proposition], &'static str),
    }
    let mut pending = vec![Step::Validate(proposition, depth)];
    while let Some(step) = pending.pop() {
        match step {
            Step::Validate(proposition, depth) => {
                if depth > MAX_PROPOSITION_DEPTH {
                    return Err(CodecError::PropositionNestingTooDeep);
                }
                match proposition {
                    Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => {}
                    Proposition::Equal(left, right) => {
                        validate_scalar_term_depth(left)?;
                        validate_scalar_term_depth(right)?;
                        if canonical_scalar_term_bytes(left)? > canonical_scalar_term_bytes(right)?
                        {
                            return Err(CodecError::NonCanonicalOrder("equality operands"));
                        }
                    }
                    Proposition::LessThan(left, right) | Proposition::LessOrEqual(left, right) => {
                        validate_scalar_term_depth(left)?;
                        validate_scalar_term_depth(right)?;
                    }
                    Proposition::IntegerMathEqual(left, right) => {
                        validate_integer_math_term_depth(left)?;
                        validate_integer_math_term_depth(right)?;
                        if left > right {
                            return Err(CodecError::NonCanonicalOrder(
                                "mathematical integer equality operands",
                            ));
                        }
                    }
                    Proposition::IntegerMathLessThan(left, right)
                    | Proposition::IntegerMathLessOrEqual(left, right) => {
                        validate_integer_math_term_depth(left)?;
                        validate_integer_math_term_depth(right)?;
                    }
                    Proposition::IeeeFloatComparison {
                        kind, left, right, ..
                    } => {
                        if kind.is_symmetric() && left > right {
                            return Err(CodecError::NonCanonicalOrder("IEEE equality operands"));
                        }
                    }
                    Proposition::ScalarIeeeFloatComparison {
                        kind, left, right, ..
                    } => {
                        validate_scalar_term_depth(left)?;
                        validate_scalar_term_depth(right)?;
                        if kind.is_symmetric() && left > right {
                            return Err(CodecError::NonCanonicalOrder(
                                "scalar IEEE equality operands",
                            ));
                        }
                    }
                    Proposition::ByteSequenceEqual { left, right } => {
                        if left > right {
                            return Err(CodecError::NonCanonicalOrder(
                                "byte-sequence equality operands",
                            ));
                        }
                    }
                    Proposition::StructuralCaseMembership { .. } => {}
                    Proposition::StructuralIdentity { left, right } => {
                        if left > right {
                            return Err(CodecError::NonCanonicalOrder(
                                "structural identity operands",
                            ));
                        }
                    }
                    Proposition::Conjunction(conjuncts) => {
                        if conjuncts
                            .iter()
                            .any(|conjunct| matches!(conjunct, Proposition::Conjunction(_)))
                        {
                            return Err(CodecError::NestedConjunction);
                        }
                        // The ordering check runs after every conjunct is
                        // validated, matching the recursive version's order.
                        pending.push(Step::StrictlyIncreasing(
                            conjuncts,
                            "conjunction propositions",
                        ));
                        for conjunct in conjuncts.iter().rev() {
                            pending.push(Step::Validate(conjunct, depth + 1));
                        }
                    }
                    Proposition::Disjunction(disjuncts) => {
                        if disjuncts
                            .iter()
                            .any(|disjunct| matches!(disjunct, Proposition::Disjunction(_)))
                        {
                            return Err(CodecError::NestedDisjunction);
                        }
                        pending.push(Step::StrictlyIncreasing(
                            disjuncts,
                            "disjunction propositions",
                        ));
                        for disjunct in disjuncts.iter().rev() {
                            pending.push(Step::Validate(disjunct, depth + 1));
                        }
                    }
                    Proposition::Implication {
                        premise,
                        conclusion,
                    } => {
                        // `premise` is visited before `conclusion`, matching the
                        // recursive version's operand order.
                        pending.push(Step::Validate(conclusion, depth + 1));
                        pending.push(Step::Validate(premise, depth + 1));
                    }
                    Proposition::ContentConservation(conservation) => {
                        validate_content_term_depth(conservation.left())?;
                        validate_content_term_depth(conservation.right())?;
                    }
                }
            }
            Step::StrictlyIncreasing(propositions, order_label) => {
                if !canonical_propositions_strictly_increase(propositions)? {
                    return Err(CodecError::NonCanonicalOrder(order_label));
                }
            }
        }
    }
    Ok(())
}

fn validate_integer_math_term_depth(term: &IntegerMathTerm) -> Result<(), CodecError> {
    let mut pending = vec![(term, 0_usize)];
    while let Some((term, depth)) = pending.pop() {
        if depth > MAX_SCALAR_TERM_DEPTH {
            return Err(CodecError::ScalarTermNestingTooDeep);
        }
        match term {
            IntegerMathTerm::MathValue { .. } | IntegerMathTerm::IntegerLiteral(_) => {}
            IntegerMathTerm::Embed(term) => validate_scalar_term_depth_from(term, depth + 1)?,
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

/// Canonical bytewise ordering key for one scalar term. Equality producers
/// use the same operand order as canonical validation, not Rust enum order or
/// numeric ordering of identities embedded in the term.
pub fn canonical_scalar_term_order_key(term: &ScalarTerm) -> Result<Vec<u8>, CodecError> {
    canonical_scalar_term_bytes(term)
}

fn canonical_scalar_term_bytes(term: &ScalarTerm) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    encode_scalar_term(&mut writer, term, 0)?;
    Ok(writer.finish())
}

fn validate_scalar_term_depth(term: &ScalarTerm) -> Result<(), CodecError> {
    validate_scalar_term_depth_from(term, 0)
}

fn validate_scalar_term_depth_from(term: &ScalarTerm, depth: usize) -> Result<(), CodecError> {
    let mut pending = vec![(term, depth)];
    while let Some((term, depth)) = pending.pop() {
        if depth > MAX_SCALAR_TERM_DEPTH {
            return Err(CodecError::ScalarTermNestingTooDeep);
        }
        for operand in term.operands() {
            pending.push((operand, depth + 1));
        }
    }
    Ok(())
}

fn validate_content_term_depth(term: &ContentTerm) -> Result<(), CodecError> {
    let mut pending = vec![(term, 0_usize)];
    while let Some((term, depth)) = pending.pop() {
        if depth > MAX_CONTENT_TERM_DEPTH {
            return Err(CodecError::ContentTermNestingTooDeep);
        }
        if let ContentTerm::Separate(terms) = term {
            for term in terms {
                pending.push((term, depth + 1));
            }
        }
    }
    Ok(())
}

/// Receiving canonicality keeps one input claim across exclusive destinations.
fn validate_identity_reshuffles(
    rows: &[terminal_psi::ContentIdentityReshuffle],
) -> Result<(), CodecError> {
    if !strictly_increasing(
        rows.iter()
            .map(|reshuffle| (reshuffle.claim, &reshuffle.output)),
    ) || rows.iter().enumerate().any(|(position, row)| {
        rows[..position].iter().any(|previous| {
            previous.claim == row.claim
                && (previous.input != row.input
                    || previous.projections != row.projections
                    || !previous.output.is_case_alternative(&row.output))
        })
    }) {
        return Err(CodecError::NonCanonicalOrder(
            "content identity reshuffles by claim and exclusive output case",
        ));
    }
    if rows
        .iter()
        .any(|reshuffle| !strictly_increasing(reshuffle.projections.iter()))
    {
        return Err(CodecError::NonCanonicalOrder(
            "claim content projections by identity and algebra",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod content_identity_tests {
    use super::*;
    use semantic_vocabulary::{
        ClaimId, ContentPlaceSegment, ContentPlaceVersion, ContentStructuralPlace, PlaceId,
    };
    use terminal_psi::ContentIdentityReshuffle;

    fn alternative_rows() -> Vec<ContentIdentityReshuffle> {
        ["Accepted", "Rejected"]
            .into_iter()
            .map(|case| ContentIdentityReshuffle {
                claim: ClaimId::new(1).unwrap(),
                input: ContentStructuralPlace {
                    root: PlaceId::new(1).unwrap(),
                    version: ContentPlaceVersion::Entry,
                    segments: vec![],
                },
                output: ContentStructuralPlace {
                    root: PlaceId::new(2).unwrap(),
                    version: ContentPlaceVersion::Current,
                    segments: vec![
                        ContentPlaceSegment::Case(case.into()),
                        ContentPlaceSegment::Field("capacity".into()),
                    ],
                },
                projections: vec![],
            })
            .collect()
    }

    #[test]
    fn one_claim_can_return_through_exclusive_cases() {
        assert_eq!(validate_identity_reshuffles(&alternative_rows()), Ok(()));
    }

    #[test]
    fn coexisting_claim_destinations_and_substituted_inputs_refuse() {
        let mut rows = alternative_rows();
        rows[0].output.segments = vec![ContentPlaceSegment::Field("left".into())];
        rows[1].output.segments = vec![ContentPlaceSegment::Field("right".into())];
        assert!(validate_identity_reshuffles(&rows).is_err());
        let mut rows = alternative_rows();
        rows[1].input.root = PlaceId::new(3).unwrap();
        assert!(validate_identity_reshuffles(&rows).is_err());
        let mut rows = alternative_rows();
        rows.swap(0, 1);
        assert!(validate_identity_reshuffles(&rows).is_err());
        let mut rows = alternative_rows();
        rows[1] = rows[0].clone();
        assert!(validate_identity_reshuffles(&rows).is_err());
    }
}
