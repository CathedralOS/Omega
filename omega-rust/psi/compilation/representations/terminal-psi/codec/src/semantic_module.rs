/*
Canonical `PSITERM\0` semantic-module entry points (`encode_module`, `decode_module`,
`semantic_fingerprint`, `terminal_psi_identity`), format marker (`FORMAT_MARKER = 262`), canonical
ordering validators (`canonical_order`), dynamic descriptor-result admission, and machine/provider
wire sections.
*/

pub(crate) use crate::block_wire;
pub(crate) use crate::foundation_validation as module_foundation_validation;
pub(crate) use crate::module_wire;
pub(crate) use crate::structural_wire::{
    structural_field_wire, structural_place_wire, structural_result_wire,
    structural_signature_wire, structural_type_wire,
};
pub(crate) use crate::term_wire::{
    content_wire, contract_wire, mathematical_certificate_wire, proof_declaration_wire,
    proof_term_wire, proposition_wire, scalar_qualification_wire, scalar_term_wire, scalar_wire,
};
pub(crate) use crate::wire_fields::{self, wire};

pub(crate) mod canonical_order {

    use std::collections::BTreeSet;

    use semantic_vocabulary::{Proposition, PropositionId, ScalarTerm};
    use terminal_psi::{
        BoundaryApplication, BoundaryApplicationArgument, BoundaryMachineResult,
        ContentIdentityReshuffle, CrashRouteBucket, CrashRouteGuard, FloatMeaningSource,
        OperationKind, ProofPropositionId, ProofValueId, StructuralFieldType,
        StructuralParameterDeclaration, StructuralTypeShape, TerminalAffineCleanupAction,
        TerminalModule, Terminator, call_structural_pair_pattern,
    };

    use super::CodecError;
    use super::proposition_wire::encode_proposition;
    use super::scalar_term_wire::encode_scalar_term;
    use super::wire::{Writer, require_increasing, strictly_increasing};

    macro_rules! noncanonical {
        ($reason:expr) => {
            CodecError::NonCanonicalOrder($reason)
        };
    }

    macro_rules! r {
        ($values:expr => $reason:expr) => {
            require_increasing($values, $reason)
        };
    }

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
        for application in module.callback_applications.iter().chain(
            module
                .installed_interrupt_registrations
                .iter()
                .flat_map(|registration| &registration.callbacks),
        ) {
            let exact_inbound = match application.boundary {
                Some(inbound) => module.boundary_machine(inbound).is_some_and(|boundary| {
                    module.identities.text(boundary.identity)
                        == Some(application.requirement.as_str())
                }),
                None => !application.entry_admitted,
            };
            if !exact_inbound {
                return Err(CodecError::MalformedStructuralFoundation(
                    "callback application lost its exact inbound requirement",
                ));
            }
            if application.requirement.is_empty()
                || application.calling_owner_application.is_empty()
                || application.published_contract == [0; 32]
                || application.body.contract == [0; 32]
            {
                return Err(CodecError::MalformedStructuralFoundation(
                    "callback application lacks its requirement or contract",
                ));
            }
        }
        r!(module.boundary_application_candidates.iter() => "boundary application candidates")?;
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
            r!(demand.frames.iter().map(|frame| frame.identity) => "task activation frames")?;
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
                    return Err(noncanonical!("task activation live carry claims"));
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
                BoundaryApplication::Empty => {}
                BoundaryApplication::Exact(arguments) => {
                    if arguments.is_empty() {
                        return Err(CodecError::MalformedStructuralFoundation(
                            "nonempty boundary application has no arguments",
                        ));
                    }
                    for (ordinal, argument) in arguments.iter().enumerate() {
                        if usize::try_from(argument.binder_ordinal()).ok() != Some(ordinal) {
                            return Err(noncanonical!("boundary application binders"));
                        }
                        if let BoundaryApplicationArgument::Const {
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
            validate_canonical_proposition(&invariant.predicate)?;
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
                return Err(noncanonical!(
                    "operation crash contract published route buckets"
                ));
            }
            validate_crash_route_predicates(&contract.published_routes)?;
            if !crash_routes_are_canonical(&contract.crash_continuations) {
                return Err(noncanonical!(
                    "operation crash contract continuation buckets"
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
            if !plan
                .live_values
                .iter()
                .map(|live| (&live.place, live.storage, live.value_type, live.effective))
                .is_sorted()
            {
                return Err(noncanonical!(
                    "suspension live values by place, storage, type, and policy"
                ));
            }
            if plan
                .live_values
                .iter()
                .any(|live| !strictly_increasing(live.claims.iter().copied()))
            {
                return Err(noncanonical!("suspension live claims by ClaimId"));
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
                .any(|(position, output)| {
                    u32::try_from(position).ok() != Some(output.output_position)
                })
            {
                return Err(noncanonical!("evidence package outputs by position"));
            }
        }
        for declaration in &module.structural_types {
            if let Some(StructuralFieldType::RefinedScalar(refinement)) =
                &declaration.scalar_restriction
            {
                validate_canonical_proposition(refinement.predicate())?;
            }
            // Authored order is the clauses' identity (a `DataDomain`
            // obligation names its clause by position), so only each
            // clause's own form is canonical, not their order.
            for clause in &declaration.requires {
                validate_canonical_proposition(clause)?;
            }
            let (fields, cases) = match &declaration.shape {
                StructuralTypeShape::Record { fields, .. } => (fields.as_slice(), &[][..]),
                StructuralTypeShape::Sum { cases } => (&[][..], cases.as_slice()),
                StructuralTypeShape::Mixed { fields, cases } => {
                    (fields.as_slice(), cases.as_slice())
                }
                _ => (&[][..], &[][..]),
            };
            for field in fields
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields))
            {
                if let StructuralFieldType::RefinedScalar(refinement) = &field.field_type {
                    validate_canonical_proposition(refinement.predicate())?;
                }
            }
            match &declaration.shape {
                StructuralTypeShape::Record { fields, .. }
                    if !strictly_increasing(fields.iter().map(|field| field.id)) =>
                {
                    return Err(noncanonical!("structural fields by StructuralFieldId"));
                }
                StructuralTypeShape::Sum { cases }
                    if !strictly_increasing(cases.iter().map(|case| case.id)) =>
                {
                    return Err(noncanonical!("structural cases by StructuralCaseId"));
                }
                StructuralTypeShape::Sum { cases }
                    if cases.iter().any(|case| {
                        !strictly_increasing(case.fields.iter().map(|field| field.id))
                    }) =>
                {
                    return Err(noncanonical!("structural case fields by StructuralFieldId"));
                }
                StructuralTypeShape::Mixed { fields, .. }
                    if !strictly_increasing(fields.iter().map(|field| field.id)) =>
                {
                    return Err(noncanonical!(
                        "mixed structural fields by StructuralFieldId"
                    ));
                }
                StructuralTypeShape::Mixed { cases, .. }
                    if !strictly_increasing(cases.iter().map(|case| case.id)) =>
                {
                    return Err(noncanonical!("mixed structural cases by StructuralCaseId"));
                }
                StructuralTypeShape::Mixed { cases, .. }
                    if cases.iter().any(|case| {
                        !strictly_increasing(case.fields.iter().map(|field| field.id))
                    }) =>
                {
                    return Err(noncanonical!(
                        "mixed structural case fields by StructuralFieldId"
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
        r!(module.services.iter().map(|declaration| declaration.id) => "services by ServiceId")?;
        if module
            .services
            .iter()
            .any(|declaration| !strictly_increasing(declaration.parents.iter().copied()))
        {
            return Err(noncanonical!("service parents by ServiceId"));
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
            return Err(noncanonical!(
                "installation reach upper bounds by ServiceId"
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
            return Err(noncanonical!(
                "float-meaning equalities by dense proposition ID and ordered operands"
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
            if let BoundaryMachineResult::Structural(result) = &declaration.result {
                require_increasing(
                    result.qualifications.iter().copied(),
                    "boundary result qualifications by StructuralDomainId",
                )?;
                if !strictly_increasing(result.projected_qualifications.iter()) {
                    return Err(noncanonical!(
                        "boundary result projected qualifications by path and domain"
                    ));
                }
            }
            if !crash_routes_are_canonical(&declaration.crash_routes) {
                return Err(noncanonical!("boundary crash route buckets"));
            }
            validate_crash_route_predicates(&declaration.crash_routes)?;
            validate_parameter_order(&declaration.structural_parameters)?;
            require_increasing(
                declaration.requires.iter(),
                "boundary requirements by argument index, path and domain",
            )?;
            r!(declaration.normal_qualifications.iter() => "boundary normal parameter qualifications")?;
            require_increasing(
                declaration.scalar_domain_requirements.iter().copied(),
                "boundary scalar domain requirements by argument index and domain",
            )?;
            for proposition in &declaration.scalar_requires {
                validate_canonical_proposition(proposition)?;
            }
            if !canonical_propositions_strictly_increase(&declaration.scalar_requires)? {
                return Err(noncanonical!("boundary scalar requires propositions"));
            }
            for proposition in &declaration.scalar_ensures {
                validate_canonical_proposition(proposition)?;
            }
            if !canonical_propositions_strictly_increase(&declaration.scalar_ensures)? {
                return Err(noncanonical!("boundary scalar ensures propositions"));
            }
            require_increasing(
                declaration.published_service_ceiling.iter().copied(),
                "boundary published service ceiling by ServiceId",
            )?;
            require_increasing(
                declaration.published_synchronous_entries.iter().copied(),
                "boundary published synchronous entries by ServiceId",
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
        require_increasing(
            module.provider_instance_requirements.iter().map(|row| {
                (
                    row.template,
                    row.package,
                    &row.selecting_declaration,
                    row.source_candidate_identity,
                )
            }),
            "provider instance requirement templates by callable and package",
        )?;
        for row in &module.provider_instance_requirements {
            require_increasing(
                row.required_instances.iter(),
                "closed provider requirement identities",
            )?;
        }
        let mut float_projection_sources = Vec::new();
        for (index, projection) in module.float_meaning_projections.iter().enumerate() {
            let Ok(index) = u32::try_from(index) else {
                return Err(noncanonical!(
                    "float-meaning projections by dense proof value and first-use source IDs"
                ));
            };
            if projection.result.id != ProofValueId(index) {
                return Err(noncanonical!(
                    "float-meaning projections by dense proof value and first-use source IDs"
                ));
            }
            if let FloatMeaningSource::TransitionalInput(source) = &projection.source
                && !float_projection_sources.contains(&source.id)
            {
                let Ok(expected) = u32::try_from(float_projection_sources.len()) else {
                    return Err(noncanonical!(
                        "float-meaning projections by dense proof value and first-use transitional source IDs"
                    ));
                };
                if source.id.0 != expected {
                    return Err(noncanonical!(
                        "float-meaning projections by dense proof value and first-use transitional source IDs"
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
        r!(module.evidence_terms.iter().map(|term| term.id) => "evidence terms by EvidenceTermId")?;
        require_increasing(
            module
                .evidence_contract_lanes
                .iter()
                .map(|lane| (lane.machine, lane.kind, lane.position)),
            "evidence contract lanes by machine, kind, and position",
        )?;
        require_increasing(
            module
                .declaration_contracts
                .iter()
                .map(|contract| contract.id),
            "declaration contracts by ContractId",
        )?;
        for contract in module
            .declaration_contracts
            .iter()
            .chain(module.machines.iter().map(|machine| &machine.contract))
        {
            validate_contract_order(contract)?;
        }
        r!(module.machines.iter().map(|machine| machine.id) => "machines by MachineId")?;
        for machine in &module.machines {
            for block in &machine.blocks {
                if let Some(invocation) = block.terminator.module_invocation() {
                    require_increasing(
                        invocation.claim_transfers.iter(),
                        "invocation claim transfers by claim and argument index",
                    )?;
                    if !crash_routes_are_canonical(invocation.crash_continuations) {
                        return Err(noncanonical!("invocation crash continuation buckets"));
                    }
                    validate_crash_route_predicates(invocation.crash_continuations)?;
                }
                if let Terminator::ExternalCompletion {
                    frontier_lower_bound,
                    ..
                } = &block.terminator
                    && !strictly_increasing(frontier_lower_bound.iter().copied())
                {
                    return Err(noncanonical!(
                        "external completion frontier lower bound by ClaimId"
                    ));
                }
                validate_parameter_order(&block.structural_parameters)?;
            }

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
            r!(machine.blocks.iter().map(|block| block.id) => "blocks by BlockId")?;
            for operation in machine.operations() {
                if let Some(result) = operation.result.as_structural() {
                    validate_input_observation_order(
                        &result.qualifications,
                        &result.projected_qualifications,
                        &result.additional_input_qualifications,
                    )?;
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
                        return Err(noncanonical!("call crash continuation buckets"));
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
                        return Err(noncanonical!(
                            "call claim transfers by claim and argument index"
                        ));
                    }
                    call_structural_pair_pattern!(selected_evidence, ..)
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
                        return Err(noncanonical!(
                            "guarded-call selections or validity dependency roots"
                        ));
                    }
                    OperationKind::BoundaryCall {
                        completion_receipts,
                        ..
                    } if !strictly_increasing(completion_receipts.iter()) => {
                        return Err(noncanonical!(
                            "boundary claim settlements by claim and argument index"
                        ));
                    }
                    _ => {}
                }
            }
            for block in &machine.blocks {
                if let Terminator::Crash { site_guard, .. } = &block.terminator {
                    for predicate in site_guard {
                        validate_canonical_proposition(predicate.proposition())?;
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
                            TerminalAffineCleanupAction::DiscardResidual(discard) => Some(discard),
                            _ => None,
                        })
                        .any(|discard| {
                            !places_and_paths.insert((discard.place, discard.path.as_slice()))
                        })
                    {
                        return Err(noncanonical!("partial affine residual discards are unique"));
                    }
                }
            }
            require_increasing(
                machine.structural_places.iter().map(|place| place.id),
                "structural places by PlaceId",
            )?;
            validate_identity_reshuffles(&machine.content_identity_reshuffles)?;
            r!(machine.content_partition_compositions.iter() => "content partition compositions")?;
            for composition in &machine.content_partition_compositions {
                require_increasing(
                    composition
                        .source_structural_places
                        .iter()
                        .map(|place| place.id),
                    "partition source structural places by PlaceId",
                )?;
                r!(composition.input_claims.iter().copied() => "partition input claims by ClaimId")?;
                r!(composition.substitutions.iter() => "partition place substitutions")?;
            }
        }
        Ok(())
    }

    // A requirement and a body publish the same formal and obligation
    // language. Canonical ordering must not depend on whether code exists.
    fn validate_contract_order(contract: &terminal_psi::MachineContract) -> Result<(), CodecError> {
        validate_parameter_order(&contract.structural_parameters)?;
        for row in [
            contract.declared_service_reach.as_deref(),
            contract.declared_synchronous_entries.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            require_increasing(row.iter().copied(), "authored effect ceiling by ServiceId")?;
        }
        if let Some(routes) = &contract.declared_crash_routes {
            if !crash_routes_are_canonical(routes) {
                return Err(noncanonical!("authored crash route buckets"));
            }
            validate_crash_route_predicates(routes)?;
        }

        r!(contract.entry_claims.iter().map(|claim| claim.claim) => "entry claims by ClaimId")?;
        require_increasing(
            contract.published_service_ceiling.iter().copied(),
            "machine published service ceiling by ServiceId",
        )?;
        require_increasing(
            contract.published_synchronous_entries.iter().copied(),
            "machine published synchronous entries by ServiceId",
        )?;
        if !crash_routes_are_canonical(&contract.crash_routes) {
            return Err(noncanonical!("crash route buckets"));
        }
        validate_crash_route_predicates(&contract.crash_routes)?;
        require_increasing(
            contract
                .content_entry_claims
                .iter()
                .map(|binding| binding.claim),
            "content entry claims by ClaimId",
        )?;
        if contract
            .content_entry_claims
            .iter()
            .any(|binding| !strictly_increasing(binding.projections.iter()))
        {
            return Err(noncanonical!(
                "entry-claim content projections by identity and algebra"
            ));
        }
        require_increasing(
            contract.ensures.iter().map(|clause| clause.obligation),
            "ensures by ObligationId",
        )?;
        let propositions = contract
            .requires
            .iter()
            .chain(contract.source_checked_requires.iter())
            .chain(contract.ensures.iter().map(|clause| &clause.proposition));
        for proposition in propositions {
            validate_canonical_proposition(proposition)?;
        }
        if !canonical_propositions_strictly_increase(&contract.requires)? {
            return Err(noncanonical!("requires propositions"));
        }
        if !canonical_propositions_strictly_increase(&contract.source_checked_requires)? {
            return Err(noncanonical!("source-checked requires propositions"));
        }
        Ok(())
    }

    fn validate_input_observation_order(
        whole: &[semantic_vocabulary::StructuralDomainId],
        projected: &[terminal_psi::StructuralPathQualification],
        observations: &[terminal_psi::StructuralPathQualification],
    ) -> Result<(), CodecError> {
        if !strictly_increasing(observations.iter())
            || observations.iter().any(|row| {
                if row.path.is_empty() {
                    !whole.contains(&row.domain)
                } else {
                    !projected.contains(row)
                }
            })
        {
            return Err(noncanonical!(
                "input observations by exact qualification subject"
            ));
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
            return Err(noncanonical!("structural parameters by dense position"));
        }
        if parameters
            .iter()
            .any(|parameter| !strictly_increasing(parameter.qualifications.iter().copied()))
        {
            return Err(noncanonical!(
                "structural parameter qualifications by StructuralDomainId"
            ));
        }
        if parameters
            .iter()
            .any(|parameter| !strictly_increasing(parameter.projected_qualifications.iter()))
        {
            return Err(noncanonical!(
                "projected structural qualifications by path and StructuralDomainId"
            ));
        }
        for parameter in parameters {
            validate_input_observation_order(
                &parameter.qualifications,
                &parameter.projected_qualifications,
                &parameter.additional_input_qualifications,
            )?;
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
            validate_canonical_proposition(predicate.proposition())?;
        }
        Ok(())
    }

    fn validate_canonical_proposition(proposition: &Proposition) -> Result<(), CodecError> {
        enum Step<'a> {
            Validate(&'a Proposition),
            StrictlyIncreasing(&'a [Proposition], &'static str),
        }
        let mut pending = vec![Step::Validate(proposition)];
        while let Some(step) = pending.pop() {
            match step {
                Step::Validate(proposition) => match proposition {
                    Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => {}
                    Proposition::Equal(left, right) => {
                        if canonical_scalar_term_bytes(left)? > canonical_scalar_term_bytes(right)?
                        {
                            return Err(noncanonical!("equality operands"));
                        }
                    }
                    Proposition::LessThan(_, _) | Proposition::LessOrEqual(_, _) => {}
                    Proposition::IntegerMathEqual(left, right) => {
                        if left > right {
                            return Err(noncanonical!("mathematical integer equality operands"));
                        }
                    }
                    Proposition::IntegerMathLessThan(_, _)
                    | Proposition::IntegerMathLessOrEqual(_, _) => {}
                    Proposition::IeeeFloatComparison {
                        kind, left, right, ..
                    } => {
                        if kind.is_symmetric() && left > right {
                            return Err(noncanonical!("IEEE equality operands"));
                        }
                    }
                    Proposition::ScalarIeeeFloatComparison {
                        kind, left, right, ..
                    } => {
                        if kind.is_symmetric() && left > right {
                            return Err(noncanonical!("scalar IEEE equality operands"));
                        }
                    }
                    Proposition::ByteSequenceEqual { left, right } => {
                        if left > right {
                            return Err(noncanonical!("byte-sequence equality operands"));
                        }
                    }
                    Proposition::StructuralCaseMembership { .. } => {}
                    Proposition::StructuralIdentity { left, right } => {
                        if left > right {
                            return Err(noncanonical!("structural identity operands"));
                        }
                    }
                    Proposition::Conjunction(conjuncts) => {
                        if conjuncts
                            .iter()
                            .any(|conjunct| matches!(conjunct, Proposition::Conjunction(_)))
                        {
                            return Err(CodecError::NestedConjunction);
                        }
                        pending.push(Step::StrictlyIncreasing(
                            conjuncts,
                            "conjunction propositions",
                        ));
                        for conjunct in conjuncts.iter().rev() {
                            pending.push(Step::Validate(conjunct));
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
                            pending.push(Step::Validate(disjunct));
                        }
                    }
                    Proposition::Implication {
                        premise,
                        conclusion,
                    } => {
                        pending.push(Step::Validate(conclusion));
                        pending.push(Step::Validate(premise));
                    }
                    Proposition::ContentConservation(_) => {}
                },
                Step::StrictlyIncreasing(propositions, order_label) => {
                    if !canonical_propositions_strictly_increase(propositions)? {
                        return Err(noncanonical!(order_label));
                    }
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
        encode_proposition(&mut writer, proposition)?;
        Ok(writer.finish())
    }

    pub fn canonical_proposition_order_key(
        proposition: &Proposition,
    ) -> Result<Vec<u8>, CodecError> {
        canonical_proposition_bytes(proposition)
    }

    pub fn canonical_scalar_term_order_key(term: &ScalarTerm) -> Result<Vec<u8>, CodecError> {
        canonical_scalar_term_bytes(term)
    }

    fn canonical_scalar_term_bytes(term: &ScalarTerm) -> Result<Vec<u8>, CodecError> {
        let mut writer = Writer::default();
        encode_scalar_term(&mut writer, term)?;
        Ok(writer.finish())
    }

    fn validate_identity_reshuffles(rows: &[ContentIdentityReshuffle]) -> Result<(), CodecError> {
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
            return Err(noncanonical!(
                "content identity reshuffles by claim and exclusive output case"
            ));
        }
        if rows
            .iter()
            .any(|reshuffle| !strictly_increasing(reshuffle.projections.iter()))
        {
            return Err(noncanonical!(
                "claim content projections by identity and algebra"
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
}
pub(crate) mod dynamic_result_admission {

    use crate::codec_error::{CodecError, malformed};
    use terminal_psi::{BoundaryMachineResult, TerminalModule};
    use terminal_semantics::descriptor_results::{DescriptorAnalysis, terminal_graph};

    pub(crate) fn validate_dynamic_descriptor_results(
        module: &TerminalModule,
    ) -> Result<(), CodecError> {
        let mut rows = Vec::new();
        for table in &module.dynamic_dispatch.tables {
            let interface = module.dynamic_dispatch.interface(table.interface).ok_or(
                CodecError::MalformedStructuralFoundation("dynamic table has no interface"),
            )?;
            if table.rows.len() != interface.slots.len() {
                return malformed("dynamic table row count disagrees with its interface");
            }
            for (row, slot) in table.rows.iter().zip(&interface.slots) {
                let Some(row) = row else {
                    continue;
                };
                let BoundaryMachineResult::Structural(result) = &slot.result else {
                    continue;
                };
                let leaves = terminal_semantics::borrowed_result_leaves(
                    module.structural_types.iter(),
                    result.structural_type,
                    result.multiplicity,
                    slot.result_reference_regions.len(),
                )
                .map_err(CodecError::MalformedStructuralFoundation)?;
                if leaves.iter().any(|leaf| {
                    matches!(leaf.kind, terminal_semantics::BorrowedResultLeafKind::View)
                }) {
                    rows.push((*row, slot));
                }
            }
        }
        if rows.is_empty() {
            return Ok(());
        }
        let graph = terminal_graph(module);
        let mut analysis = DescriptorAnalysis::new(&module.structural_types, &graph);
        for (row, slot) in rows {
            let callee = module
                .machine(row)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "dynamic table row has no machine",
                ))?;
            let result =
                callee
                    .result
                    .structural()
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "dynamic view result row has no structural result",
                    ))?;
            if !slot.result.matches_machine_result(&callee.result) {
                return malformed("dynamic view result row disagrees with its slot");
            }
            let slots =
                analysis
                    .result_slots(row)
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "dynamic row has unknown returned descriptor contents",
                    ))?;
            terminal_semantics::validate_descriptor_result_regions(
                &module.structural_types,
                &callee.structural_parameters,
                result,
                &slot.result_reference_regions,
                &slots,
            )
            .map_err(CodecError::MalformedStructuralFoundation)?;
        }
        Ok(())
    }
}
pub(crate) mod external_provider_wire {

    use super::CodecError;
    use super::machine_declaration_wire;
    use super::wire::{Reader, Writer};
    use super::wire_fields::{ExternalPackage, Field};
    use language_semantics::ExternalBindingIdentity;
    use terminal_psi::TerminalIdentityIndex;
    use terminal_psi::{
        ExternalProviderConformance, ExternalProviderOwner, NominalDeclarationIdentity,
    };

    pub(crate) fn encode(
        writer: &mut Writer,
        candidate: &ExternalProviderConformance,
    ) -> Result<(), CodecError> {
        writer.u32(candidate.requirement.identity().0);
        writer.u32(candidate.requirement_identity.0);
        ExternalPackage::write(writer, &candidate.requirement_package)?;
        writer.boolean(candidate.requirement_schema.is_some());
        if let Some(schema) = &candidate.requirement_schema {
            writer.u32(schema.identity.0);
            writer.u32(schema.method.0);
            writer.u32(schema.owner.0);
        }
        writer.boolean(candidate.numeric_intrinsic.is_some());
        if let Some(meaning) = candidate.numeric_intrinsic {
            writer.bytes(&meaning.canonical_encoding().ok_or(
                CodecError::MalformedStructuralFoundation(
                    "invalid external provider numeric meaning",
                ),
            )?);
        }
        writer.boolean(candidate.target.is_some());
        if let Some(target) = &candidate.target {
            writer.string("external provider target membership", target)?;
        }
        machine_declaration_wire::encode(writer, &candidate.realization)?;
        ExternalPackage::write(writer, &candidate.realization_package)?;
        writer.boolean(candidate.provider.is_some());
        if let Some(provider) = &candidate.provider {
            writer.u32(provider.declaration.identity().0);
            writer.u32(provider.identity.0);
            ExternalPackage::write(writer, &provider.package)?;
        }
        writer.u8(candidate.binding.mechanism().identity_tag());
        match &candidate.binding {
            ExternalBindingIdentity::CompilerIntrinsic => {}
            ExternalBindingIdentity::Syscall { number } => writer.bytes(&number.to_le_bytes()),
            ExternalBindingIdentity::VtableSlot { index } => writer.bytes(&index.to_le_bytes()),
            ExternalBindingIdentity::VtableField { field }
            | ExternalBindingIdentity::TableFunction { field } => {
                writer.string("external binding field", field)?
            }
        }
        Ok(())
    }

    pub(crate) fn decode(
        reader: &mut Reader<'_>,
    ) -> Result<ExternalProviderConformance, CodecError> {
        let requirement = NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?));
        let requirement_identity = TerminalIdentityIndex(reader.u32()?);
        let requirement_package = ExternalPackage::read(reader)?;
        let requirement_schema = if reader.boolean()? {
            Some(terminal_psi::ExternalRequirementSchema {
                identity: TerminalIdentityIndex(reader.u32()?),
                method: TerminalIdentityIndex(reader.u32()?),
                owner: TerminalIdentityIndex(reader.u32()?),
            })
        } else {
            None
        };
        let numeric_intrinsic = if reader.boolean()? {
            Some(
                language_semantics::numeric_intrinsics::NumericIntrinsic::from_canonical_encoding(
                    reader.array()?,
                )
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "invalid external provider numeric meaning",
                ))?,
            )
        } else {
            None
        };
        let target = if reader.boolean()? {
            Some(reader.string("external provider target membership")?)
        } else {
            None
        };
        let realization = machine_declaration_wire::decode(reader)?;
        let realization_package = ExternalPackage::read(reader)?;
        let provider = if reader.boolean()? {
            Some(ExternalProviderOwner {
                declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?)),
                identity: TerminalIdentityIndex(reader.u32()?),
                package: ExternalPackage::read(reader)?,
            })
        } else {
            None
        };
        let binding = match reader.u8()? {
            2 => ExternalBindingIdentity::Syscall {
                number: i64::from_le_bytes(reader.array()?),
            },
            3 => ExternalBindingIdentity::CompilerIntrinsic,
            4 => ExternalBindingIdentity::VtableSlot {
                index: i64::from_le_bytes(reader.array()?),
            },
            5 => ExternalBindingIdentity::VtableField {
                field: reader.string("external binding field")?,
            },
            6 => ExternalBindingIdentity::TableFunction {
                field: reader.string("external binding field")?,
            },
            tag => return Err(CodecError::InvalidTag("external provider binding", tag)),
        };
        Ok(ExternalProviderConformance {
            requirement,
            requirement_identity,
            requirement_package,
            requirement_schema,
            numeric_intrinsic,
            target,
            realization,
            realization_package,
            provider,
            binding,
        })
    }
}
pub(crate) mod machine_declaration_wire {

    use super::{CodecError, Reader, Writer, decode_counted};
    use terminal_psi::{
        CheckedMachineDeclaration, CheckedMachineSpecialization, NominalDeclarationIdentity,
        TerminalIdentityIndex,
    };

    pub(crate) fn encode(
        writer: &mut Writer,
        body: &CheckedMachineDeclaration,
    ) -> Result<(), CodecError> {
        validate(body)?;
        writer.u32(body.machine.identity().0);
        writer.u32(body.state.identity().0);
        writer.u32(body.semantic_declaration.0);
        writer.len("machine parameter identities", body.parameter_types.len())?;
        for identity in &body.parameter_types {
            writer.u32(identity.0);
        }
        writer.u32(body.callable.0);
        writer.u32(body.result_type.0);
        writer.bytes(&body.contract);
        writer.boolean(body.published_may_suspend);
        writer.boolean(body.published_may_block);
        writer.boolean(body.specialization.is_some());
        if let Some(specialization) = &body.specialization {
            writer.u32(specialization.template.identity().0);
            writer.u32(specialization.template_callable.0);
            writer.u32(specialization.template_semantic_declaration.0);
            writer.bytes(&specialization.commitment);
            writer.boolean(specialization.capture.is_some());
            if let Some(capture) = &specialization.capture {
                writer.u32(capture.template_identity.0);
                writer.bytes(&capture.template_commitment);
                writer.bytes(&capture.specialization_commitment);
            }
        }
        Ok(())
    }

    pub(crate) fn decode(reader: &mut Reader<'_>) -> Result<CheckedMachineDeclaration, CodecError> {
        let machine = NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?));
        let state = NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?));
        let semantic_declaration = TerminalIdentityIndex(reader.u32()?);
        let parameter_types =
            decode_counted(reader, |reader| Ok(TerminalIdentityIndex(reader.u32()?)))?;
        let callable = TerminalIdentityIndex(reader.u32()?);
        let result_type = TerminalIdentityIndex(reader.u32()?);
        let contract = reader.array()?;
        let published_may_suspend = reader.boolean()?;
        let published_may_block = reader.boolean()?;
        let specialization = if reader.boolean()? {
            Some(CheckedMachineSpecialization {
                template: NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?)),
                template_callable: TerminalIdentityIndex(reader.u32()?),
                template_semantic_declaration: TerminalIdentityIndex(reader.u32()?),
                commitment: reader.array()?,
                capture: if reader.boolean()? {
                    Some(terminal_psi::CheckedMachineCapture {
                        template_identity: TerminalIdentityIndex(reader.u32()?),
                        template_commitment: reader.array()?,
                        specialization_commitment: reader.array()?,
                    })
                } else {
                    None
                },
            })
        } else {
            None
        };
        let body = CheckedMachineDeclaration {
            machine,
            state,
            semantic_declaration,
            parameter_types,
            callable,
            result_type,
            contract,
            published_may_suspend,
            published_may_block,
            specialization,
        };
        validate(&body)?;
        Ok(body)
    }

    fn validate(body: &CheckedMachineDeclaration) -> Result<(), CodecError> {
        if body.contract == [0; 32]
            || body.specialization.as_ref().is_some_and(|row| {
                row.commitment == [0; 32]
                    || row.template_callable.is_absent()
                    || row.template_semantic_declaration.is_absent()
                    || row.capture.as_ref().is_some_and(|capture| {
                        capture.template_identity.is_absent()
                            || capture.template_commitment == [0; 32]
                            || capture.specialization_commitment == [0; 32]
                    })
            })
        {
            return Err(CodecError::MalformedStructuralFoundation(
                "empty checked machine commitment",
            ));
        }
        Ok(())
    }
}
pub(crate) mod machine_wire {

    #[cfg(test)]
    mod tests {
        use super::super::module_wire::encode_raw;
        use super::super::target_family_tests::{identity_table, unit_machine};
        use crate::{decode_module, encode_module, semantic_fingerprint};
        use semantic_vocabulary::{
            IntegerSign, IntegerType, MachineId, PlaceId, ScalarType, StructuralPlaceKind,
            StructuralTypeId, ValueId,
        };
        use terminal_psi::{
            BoundaryParameterKind::{Scalar, Structural},
            StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
            StructuralPlaceDeclaration, StructuralRecordStorage, StructuralTypeDeclaration,
            StructuralTypeShape, TerminalIdentityIndex, TerminalModule, ValueDeclaration,
        };

        fn module() -> TerminalModule {
            let identity = MachineId::new(1).unwrap();
            let mut module = TerminalModule::for_entry(identity);
            module.identities = identity_table(4);
            let mut machine = unit_machine(identity);
            machine.declaration.as_mut().unwrap().parameter_types =
                vec![TerminalIdentityIndex(3); 4];
            let scalar_type =
                ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
            machine.parameters = [1, 2]
                .map(|value| ValueDeclaration {
                    id: ValueId::new(value).unwrap(),
                    scalar_type,
                    qualifications: Default::default(),
                })
                .to_vec();
            let place = PlaceId::new(1).unwrap();
            let structural_type = StructuralTypeId::new(1).unwrap();
            machine
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place,
                    position: 0,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    additional_input_qualifications: Vec::new(),
                });
            machine.structural_places.push(StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::Parameter { position: 0 },
            });
            module.structural_types.push(StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                copyable: true,
                scalar_restriction: None,
                id: structural_type,
                identity: TerminalIdentityIndex(3),
                shape: StructuralTypeShape::Record {
                    fields: Vec::new(),
                    storage: StructuralRecordStorage::Packed,
                },
            });
            module.native_layouts.insert(
                structural_type,
                TerminalIdentityIndex(3),
                TerminalIdentityIndex::ABSENT,
                TerminalIdentityIndex::ABSENT,
                [],
                [],
            );
            machine.parameter_order = vec![Scalar, Structural, Scalar];
            module.machines.push(machine);
            module
        }

        #[test]
        fn machine_parameter_order_round_trip_and_bind_identity() {
            let original = module();
            let bytes = encode_module(&original).unwrap();
            let decoded = decode_module(&bytes).unwrap();
            assert_eq!(decoded, original);
            assert_eq!(
                decoded.machines[0].runtime_parameter_formals().unwrap(),
                [0, 2, 1]
            );
            let mut different = original.clone();
            different.machines[0].parameter_order = vec![Structural, Scalar, Scalar];
            assert_eq!(
                different.machines[0].runtime_parameter_formals().unwrap(),
                [1, 2, 0]
            );
            assert_ne!(
                semantic_fingerprint(&original).unwrap(),
                semantic_fingerprint(&different).unwrap()
            );
            assert_eq!(
                decode_module(&encode_module(&different).unwrap()).unwrap(),
                different
            );
        }

        #[test]
        fn machine_parameter_order_rejects_missing_extra_and_wrong_lane() {
            for order in [
                vec![],
                vec![Scalar, Structural],
                vec![Scalar; 3],
                vec![Scalar, Structural, Structural],
                vec![Scalar, Structural, Scalar, Scalar],
            ] {
                let mut invalid = module();
                invalid.machines[0].parameter_order = order;
                assert!(invalid.machines[0].validate_parameter_order().is_err());
                assert!(decode_module(&encode_raw(&invalid).unwrap()).is_err());
            }
        }

        #[test]
        fn machine_parameter_order_has_no_erased_formal_lane() {
            let original = module();
            assert_eq!(
                original.machines[0]
                    .declaration
                    .as_ref()
                    .unwrap()
                    .parameter_types
                    .len(),
                4
            );
            assert_eq!(original.machines[0].parameter_order.len(), 3);
            assert_eq!(
                original.machines[0].runtime_parameter_formals().unwrap(),
                [0, 2, 1]
            );
        }

        fn rank_tuple_fixture() -> terminal_psi::TerminalRankedScc {
            use terminal_psi::{
                TerminalBlockNaturalRank, TerminalIntegerRankValue, TerminalNaturalCycle,
                TerminalNaturalRankComparison, TerminalNaturalRankEdge, TerminalRankedScc,
            };
            let narrow = IntegerType::new(IntegerSign::Signed, 8).unwrap();
            let wide = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
            let value = |integer_type, identity| TerminalIntegerRankValue {
                integer_type,
                value: ValueId::new(identity).unwrap(),
            };
            let mut observations = arena::Arena::new();
            observations.insert(value(wide, 999));
            let header = observations.insert_many([value(narrow, 1), value(wide, 2)]);
            let arrival = observations.insert_many([value(narrow, 3), value(wide, 4)]);
            let block = semantic_vocabulary::BlockId::new(1).unwrap();
            TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
                observations,
                ranks: vec![TerminalBlockNaturalRank {
                    block,
                    values: header,
                }],
                edges: vec![TerminalNaturalRankEdge {
                    edge: semantic_vocabulary::EdgeId::new(1).unwrap(),
                    source: block,
                    target: block,
                    successor_ranks: arrival,
                    comparison: TerminalNaturalRankComparison::Strict,
                }],
            }])
        }

        #[test]
        fn rank_tuple_wire_retains_order_types_and_ssa_values_without_arena_coordinates() {
            let original = rank_tuple_fixture();
            let mut writer = super::Writer::default();
            super::encode_ranked_scc(&mut writer, Some(&original)).unwrap();
            let bytes = writer.finish();
            assert_eq!(bytes[0], 3);
            assert_eq!(
                super::decode_ranked_scc(&mut super::Reader::new(&bytes)).unwrap(),
                Some(original)
            );
            assert_eq!(
                super::decode_ranked_scc(&mut super::Reader::new(&[2])),
                Err(crate::CodecError::InvalidTag("TerminalRankedScc", 2))
            );
        }

        #[test]
        fn rank_tuple_wire_rejects_a_dropped_successor_suffix() {
            let mut original = rank_tuple_fixture();
            let terminal_psi::TerminalRankedScc::Natural(components) = &mut original;
            let component = &mut components[0];
            let first = component
                .rank_values(component.edges[0].successor_ranks)
                .unwrap()[0];
            component.edges[0].successor_ranks = component.observations.insert_many([first]);
            let mut writer = super::Writer::default();
            assert_eq!(
                super::encode_ranked_scc(&mut writer, Some(&original)),
                Err(crate::CodecError::MalformedRanking(
                    "incomplete or differently typed rank tuple"
                ))
            );

            let mut writer = super::Writer::default();
            writer.u8(3);
            writer.len("components", 1).unwrap();
            writer.len("blocks", 1).unwrap();
            writer.id(semantic_vocabulary::BlockId::new(1).unwrap());
            writer.len("rank", 2).unwrap();
            for identity in [1, 2] {
                super::encode_integer_type(
                    &mut writer,
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                );
                writer.id(ValueId::new(identity).unwrap());
            }
            writer.len("edges", 1).unwrap();
            writer.id(semantic_vocabulary::EdgeId::new(1).unwrap());
            writer.id(semantic_vocabulary::BlockId::new(1).unwrap());
            writer.id(semantic_vocabulary::BlockId::new(1).unwrap());
            writer.len("successor", 1).unwrap();
            super::encode_integer_type(
                &mut writer,
                IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            );
            writer.id(ValueId::new(3).unwrap());
            writer.u8(2);
            let bytes = writer.finish();
            assert_eq!(
                super::decode_ranked_scc(&mut super::Reader::new(&bytes)),
                Err(crate::CodecError::MalformedRanking(
                    "incomplete or differently typed rank tuple"
                ))
            );
        }

        #[test]
        fn rank_tuple_wire_rejects_reordered_differently_typed_components() {
            let mut original = rank_tuple_fixture();
            let terminal_psi::TerminalRankedScc::Natural(components) = &mut original;
            let component = &mut components[0];
            let mut values = component
                .rank_values(component.edges[0].successor_ranks)
                .unwrap()
                .to_vec();
            values.swap(0, 1);
            component.edges[0].successor_ranks = component.observations.insert_many(values);
            let mut writer = super::Writer::default();
            assert_eq!(
                super::encode_ranked_scc(&mut writer, Some(&original)),
                Err(crate::CodecError::MalformedRanking(
                    "incomplete or differently typed rank tuple"
                ))
            );
        }
    }

    use terminal_psi::{
        BoundaryParameterKind, EntryClaim, LoanCompletion, RegisterTargetApplicability,
        StructuralPlaceDeclaration, TerminalBlockNaturalRank, TerminalIntegerRankValue,
        TerminalMachine, TerminalMachineResult, TerminalNaturalCycle,
        TerminalNaturalRankComparison, TerminalNaturalRankEdge, TerminalRankedScc,
        ValueDeclaration,
    };

    use super::CodecError;
    use super::scalar_wire::{
        decode_integer_type, decode_scalar_type, encode_integer_type, encode_scalar_type,
    };
    use super::structural_place_wire::{
        decode_structural_place_kind, encode_structural_place_kind,
    };
    use super::structural_result_wire::{decode_function_result, encode_function_result};
    use super::wire::{Reader, Writer};
    use super::wire_fields::{
        ClaimBinding, Field, Id, List, Multiplicity, OptionalId, Path, QualificationSet, Scalar,
        delegated_fields, wire_enum, wire_record,
    };
    use crate::sections::semantic_module::wire::decode_counted;

    wire_record! {
        Machine: TerminalMachine {
            id: Id,
            assembly_target_requirement: AssemblyTarget,
            attachment: OptionalId,
            nominal_cleanup_subject: OptionalId,
            ranked_scc: RankedScc,
            structural_places: List<Place>,
            block_entry_claims: List<Claim>,
            loan_completions: List<LoanCompletionRow>,
            content_identity_reshuffles: List<super::content_wire::Reshuffle>,
            content_partition_compositions: List<super::content_wire::PartitionComposition>,
            entry: Id,
            blocks: List<BlockRow>,
            contract: super::contract_wire::Contract,
        }
        Declaration: ValueDeclaration { id: Id, scalar_type: Scalar, qualifications: QualificationSet }
        Place: StructuralPlaceDeclaration { id: Id, kind: PlaceKind }
        Claim: EntryClaim { claim: Id, input: Id, path: Path }
        LoanCompletionRow: LoanCompletion {
            operation: Id,
            place: Id,
            path: Path,
            multiplicity: Multiplicity,
            claims: List<ClaimBinding>,
        }
    }

    wire_enum! {
        ParameterKind: BoundaryParameterKind, "machine parameter kind" { 0 => Scalar, 1 => Structural }
        MachineResult: TerminalMachineResult, "TerminalMachineResult" {
            0 => Unit,
            1 => Scalar(declaration: Declaration),
            2 => Structural(result: FunctionResult),
        }
    }

    delegated_fields! {
        FunctionResult: terminal_psi::StructuralResultDeclaration
            => encode_function_result, decode_function_result;
        BlockRow: terminal_psi::Block => super::block_wire::encode_block, super::block_wire::decode_block;
    }

    struct PlaceKind;
    impl Field<semantic_vocabulary::StructuralPlaceKind> for PlaceKind {
        fn write(
            writer: &mut Writer,
            value: &semantic_vocabulary::StructuralPlaceKind,
        ) -> Result<(), CodecError> {
            encode_structural_place_kind(writer, *value);
            Ok(())
        }
        fn read(
            reader: &mut Reader<'_>,
        ) -> Result<semantic_vocabulary::StructuralPlaceKind, CodecError> {
            decode_structural_place_kind(reader)
        }
    }

    struct AssemblyTarget;
    impl Field<RegisterTargetApplicability> for AssemblyTarget {
        fn write(
            writer: &mut Writer,
            value: &RegisterTargetApplicability,
        ) -> Result<(), CodecError> {
            writer.u8(match value {
                RegisterTargetApplicability::Any => 0,
                RegisterTargetApplicability::X86_64 => 1,
                RegisterTargetApplicability::Aarch64 => 2,
            });
            Ok(())
        }
        fn read(reader: &mut Reader<'_>) -> Result<RegisterTargetApplicability, CodecError> {
            Ok(match reader.u8()? {
                0 => RegisterTargetApplicability::Any,
                1 => RegisterTargetApplicability::X86_64,
                2 => RegisterTargetApplicability::Aarch64,
                _ => {
                    return crate::codec_error::malformed(
                        "unknown machine assembly target requirement",
                    );
                }
            })
        }
    }

    pub(crate) struct ReachApplication;
    impl Field<Option<terminal_psi::ClosedReachApplication>> for ReachApplication {
        fn write(
            writer: &mut Writer,
            value: &Option<terminal_psi::ClosedReachApplication>,
        ) -> Result<(), CodecError> {
            super::reach_application_wire::encode(writer, value.as_ref())
        }
        fn read(
            reader: &mut Reader<'_>,
        ) -> Result<Option<terminal_psi::ClosedReachApplication>, CodecError> {
            super::reach_application_wire::decode(reader)
        }
    }

    struct RankedScc;
    impl Field<Option<TerminalRankedScc>> for RankedScc {
        fn write(writer: &mut Writer, value: &Option<TerminalRankedScc>) -> Result<(), CodecError> {
            encode_ranked_scc(writer, value.as_ref())
        }
        fn read(reader: &mut Reader<'_>) -> Result<Option<TerminalRankedScc>, CodecError> {
            decode_ranked_scc(reader)
        }
    }

    pub(crate) fn encode_machine(
        writer: &mut Writer,
        machine: &TerminalMachine,
    ) -> Result<(), CodecError> {
        Machine::write(writer, machine)
    }

    pub(crate) fn encode_declarations(
        writer: &mut Writer,
        label: &'static str,
        declarations: &[ValueDeclaration],
    ) -> Result<(), CodecError> {
        writer.len(label, declarations.len())?;
        for declaration in declarations {
            encode_declaration(writer, *declaration);
        }
        Ok(())
    }

    pub(crate) fn encode_declaration(writer: &mut Writer, declaration: ValueDeclaration) {
        writer.id(declaration.id);
        encode_scalar_type(writer, declaration.scalar_type);
        writer.u64(declaration.qualifications.get());
    }

    pub(crate) fn decode_machine(reader: &mut Reader<'_>) -> Result<TerminalMachine, CodecError> {
        Machine::read(reader)
    }

    fn encode_ranked_scc(
        writer: &mut Writer,
        ranked_scc: Option<&TerminalRankedScc>,
    ) -> Result<(), CodecError> {
        let Some(ranking) = ranked_scc else {
            writer.u8(0);
            return Ok(());
        };
        match ranking {
            TerminalRankedScc::Natural(components) => {
                if components.is_empty() {
                    return Err(CodecError::MalformedRanking(
                        "empty ranked component roster",
                    ));
                }
                writer.u8(3);
                writer.len("integer cycle components", components.len())?;
                for component in components {
                    if component.ranks.is_empty() {
                        return Err(CodecError::MalformedRanking("empty cyclic rank roster"));
                    }
                    writer.len("integer block ranks", component.ranks.len())?;
                    for rank in &component.ranks {
                        writer.id(rank.block);
                        encode_rank_values(writer, component, rank.values)?;
                    }
                    writer.len("integer rank edges", component.edges.len())?;
                    for edge in &component.edges {
                        writer.id(edge.edge);
                        writer.id(edge.source);
                        writer.id(edge.target);
                        encode_rank_values(writer, component, edge.successor_ranks)?;
                        writer.u8(match edge.comparison {
                            TerminalNaturalRankComparison::Preserving => 1,
                            TerminalNaturalRankComparison::Strict => 2,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn decode_ranked_scc(reader: &mut Reader<'_>) -> Result<Option<TerminalRankedScc>, CodecError> {
        match reader.u8()? {
            0 => Ok(None),
            3 => {
                let components = decode_counted(reader, |reader| {
                    let mut observations = arena::Arena::new();
                    let ranks = decode_counted(reader, |reader| {
                        let block = reader.id("BlockId")?;
                        let values = decode_rank_values(reader, &mut observations)?;
                        Ok(TerminalBlockNaturalRank { block, values })
                    })?;
                    let edges = decode_counted(reader, |reader| {
                        let edge = reader.id("EdgeId")?;
                        let source = reader.id("BlockId")?;
                        let target = reader.id("BlockId")?;
                        let successor_ranks = decode_rank_values(reader, &mut observations)?;
                        let comparison = match reader.u8()? {
                            1 => TerminalNaturalRankComparison::Preserving,
                            2 => TerminalNaturalRankComparison::Strict,
                            tag => {
                                return Err(CodecError::InvalidTag(
                                    "TerminalNaturalRankComparison",
                                    tag,
                                ));
                            }
                        };
                        Ok(TerminalNaturalRankEdge {
                            edge,
                            source,
                            target,
                            successor_ranks,
                            comparison,
                        })
                    })?;
                    let component = TerminalNaturalCycle {
                        observations,
                        ranks,
                        edges,
                    };
                    if component.ranks.is_empty()
                        || component
                            .ranks
                            .iter()
                            .any(|rank| component.rank_values(rank.values).is_none())
                        || component
                            .edges
                            .iter()
                            .any(|edge| component.rank_values(edge.successor_ranks).is_none())
                    {
                        return Err(CodecError::MalformedRanking(
                            "incomplete or differently typed rank tuple",
                        ));
                    }
                    Ok(component)
                })?;
                if components.is_empty() {
                    return Err(CodecError::MalformedRanking(
                        "empty ranked component roster",
                    ));
                }
                Ok(Some(TerminalRankedScc::Natural(components)))
            }
            tag => Err(CodecError::InvalidTag("TerminalRankedScc", tag)),
        }
    }

    fn encode_rank_values(
        writer: &mut Writer,
        component: &TerminalNaturalCycle,
        span: arena::HandleSpan<TerminalIntegerRankValue>,
    ) -> Result<(), CodecError> {
        let values = component
            .rank_values(span)
            .ok_or(CodecError::MalformedRanking(
                "incomplete or differently typed rank tuple",
            ))?;
        writer.len("integer rank tuple", values.len())?;
        for value in values {
            encode_integer_type(writer, value.integer_type);
            writer.id(value.value);
        }
        Ok(())
    }

    fn decode_rank_values(
        reader: &mut Reader<'_>,
        observations: &mut arena::Arena<TerminalIntegerRankValue>,
    ) -> Result<arena::HandleSpan<TerminalIntegerRankValue>, CodecError> {
        let values = decode_counted(reader, |reader| {
            Ok(TerminalIntegerRankValue {
                integer_type: decode_integer_type(reader)?,
                value: reader.id("ValueId")?,
            })
        })?;
        if values.is_empty() {
            return Err(CodecError::MalformedRanking("empty rank tuple"));
        }
        Ok(observations.insert_many(values))
    }

    pub(crate) fn decode_declarations(
        reader: &mut Reader<'_>,
    ) -> Result<Vec<ValueDeclaration>, CodecError> {
        decode_counted(reader, decode_declaration)
    }

    pub(crate) fn decode_declaration(
        reader: &mut Reader<'_>,
    ) -> Result<ValueDeclaration, CodecError> {
        Ok(ValueDeclaration {
            id: reader.id("ValueId")?,
            scalar_type: decode_scalar_type(reader)?,
            qualifications: semantic_vocabulary::ScalarQualificationSetId::new(reader.u64()?),
        })
    }
}
pub(crate) mod provider_candidate_wire {

    use terminal_psi::{
        ProviderCandidateConformance, ProviderInstanceRequirements, ProviderParameterRefinement,
        ProviderRefinement, ProviderSignature, ProviderSignatureParameter,
        RequiredProviderInstance,
    };

    use super::CodecError;
    use super::structural_signature_wire::DomainRequirement;
    use super::wire::{Reader, Writer};
    use super::wire_fields::{
        Access, Field, Id, Ids, Index, List, Multiplicity, Nominal, Optional, PathQualification,
        ProviderPackage, U32, wire_record,
    };

    wire_record! {
        InstanceRequirements: ProviderInstanceRequirements {
            template: Index,
            package: ProviderPackage,
            selecting_declaration: Optional<Nominal>,
            source_candidate_identity: Index,
            required_instances: List<RequiredInstance>,
        }
        RequiredInstance: RequiredProviderInstance {
            requirement_identity: Index,
            candidate_identity: Index,
            declaration_application: Index,
            receiving_application: Index,
            receiving_region_arguments: List<Index>,
        }
        Candidate: ProviderCandidateConformance {
            boundary: Id,
            requirement_identity: Index,
            source_requirement_identity: Index,
            declaration_application: Index,
            receiving_application: Index,
            receiving_region_arguments: List<Index>,
            provider_identity: Index,
            provider_declaration: Nominal,
            candidate_identity: Index,
            source_candidate_identity: Index,
            source_candidate_package_identity: ProviderPackage,
            candidate: Id,
            signature: Signature,
            refinement: Refinement,
        }
        Signature: ProviderSignature { parameters: List<SignatureParameter> }
        SignatureParameter: ProviderSignatureParameter {
            position: U32,
            structural_type: Id,
            multiplicity: Multiplicity,
            access: Access,
            qualifications: Ids,
            projected_qualifications: List<PathQualification>,
        }
        Refinement: ProviderRefinement {
            positional_parameters: List<ParameterRefinement>,
            required_domains: List<DomainRequirement>,
            realized_service_ceiling: Ids,
        }
        ParameterRefinement: ProviderParameterRefinement { boundary_index: U32, candidate_index: U32 }
    }

    pub(crate) fn encode_provider_candidate(
        writer: &mut Writer,
        candidate: &ProviderCandidateConformance,
    ) -> Result<(), CodecError> {
        Candidate::write(writer, candidate)
    }

    pub(crate) fn decode_provider_candidate(
        reader: &mut Reader<'_>,
    ) -> Result<ProviderCandidateConformance, CodecError> {
        Candidate::read(reader)
    }

    pub fn encode_provider_candidate_record(
        candidate: &ProviderCandidateConformance,
    ) -> Result<Vec<u8>, CodecError> {
        let mut writer = Writer::default();
        writer.u16(super::FORMAT_MARKER);
        writer.u16(terminal_psi::VocabularyMarker::CURRENT.get());
        encode_provider_candidate(&mut writer, candidate)?;
        Ok(writer.finish())
    }

    pub fn decode_provider_candidate_record(
        bytes: &[u8],
    ) -> Result<ProviderCandidateConformance, CodecError> {
        let mut reader = Reader::new(bytes);
        let format = reader.u16()?;
        if format != super::FORMAT_MARKER {
            return Err(CodecError::UnsupportedFormatMarker(format));
        }
        let marker = reader.u16()?;
        if marker != terminal_psi::VocabularyMarker::CURRENT.get() {
            return Err(CodecError::UnsupportedVocabularyMarker(marker));
        }
        let candidate = decode_provider_candidate(&mut reader)?;
        if reader.remaining() != 0 {
            return Err(CodecError::TrailingBytes(reader.remaining()));
        }
        Ok(candidate)
    }
}
pub(crate) mod reach_application_wire {

    use terminal_psi::{
        ClosedReachApplication, ClosedReachArgument, ClosedReachCall, ClosedReachCallApplication,
        ClosedReachCapture, ClosedReachMachineBinding, ClosedReachParameter, ClosedReachSchema,
    };

    use super::CodecError;
    use super::structural_signature_wire::encode_service_ceiling;
    use super::wire::{Reader, Writer};
    use crate::sections::semantic_module::wire::{
        decode_counted, decode_ids, decode_optional_id, encode_optional_id,
    };
    use terminal_psi::TerminalIdentityIndex;

    pub(crate) fn encode(
        writer: &mut Writer,
        application: Option<&ClosedReachApplication>,
    ) -> Result<(), CodecError> {
        let Some(application) = application else {
            writer.boolean(false);
            return Ok(());
        };
        writer.boolean(true);
        writer.u32(application.template_identity.0);
        writer.bytes(&application.template_commitment);
        encode_operational(
            writer,
            application.template_base_commitment,
            &application.suspension_dependency,
            &application.blocking_dependency,
            &application.conditional_carry,
        )?;
        writer.bytes(&application.specialization_commitment);
        encode_kinds(writer, &application.parameter_kinds)?;
        encode_telescope(writer, &application.telescope)?;
        writer.boolean(application.capture.is_some());
        if let Some(capture) = &application.capture {
            writer.u32(capture.template_identity.0);
            writer.bytes(&capture.template_commitment);
            encode_operational(
                writer,
                capture.template_base_commitment,
                &capture.suspension_dependency,
                &capture.blocking_dependency,
                &capture.conditional_carry,
            )?;
            writer.bytes(&capture.specialization_commitment);
            encode_kinds(writer, &capture.parameter_kinds)?;
            encode_telescope(writer, &capture.telescope)?;
        }
        encode_service_ceiling(writer, &application.fixed)?;
        writer.len("reach dependencies", application.dependencies.len())?;
        for binder in &application.dependencies {
            writer.u32(*binder);
        }
        writer.len("reach calls", application.calls.len())?;
        for call in &application.calls {
            super::wire::encode_invocation_site(writer, call.site);
            writer.u32(call.binder);
            writer.boolean(call.application.is_some());
            if let Some(application) = &call.application {
                writer.id(application.callee);
                writer.bytes(&application.specialization_commitment);
                writer.len("reach call arguments", application.arguments.len())?;
                for argument in &application.arguments {
                    encode_argument(writer, argument);
                }
            }
        }
        Ok(())
    }

    fn encode_kinds(writer: &mut Writer, kinds: &[u8]) -> Result<(), CodecError> {
        writer.len("reach parameter kinds", kinds.len())?;
        writer.bytes(kinds);
        Ok(())
    }

    fn decode_kinds(reader: &mut Reader<'_>) -> Result<Vec<u8>, CodecError> {
        decode_counted(reader, |reader| {
            let kind = reader.u8()?;
            if kind > 5 {
                return Err(CodecError::InvalidTag("ReachParameterKind", kind));
            }
            Ok(kind)
        })
    }

    fn encode_telescope(
        writer: &mut Writer,
        telescope: &[ClosedReachParameter],
    ) -> Result<(), CodecError> {
        writer.len("reach telescope", telescope.len())?;
        for parameter in telescope {
            match parameter {
                ClosedReachParameter::Type { argument } => {
                    writer.u8(0);
                    writer.string("reach type argument", argument)?;
                }
                ClosedReachParameter::Const { argument } => {
                    writer.u8(1);
                    writer.string("reach const argument", argument)?;
                }
                ClosedReachParameter::Proposition { argument } => {
                    writer.u8(2);
                    writer.string("reach proposition argument", argument)?;
                }
                ClosedReachParameter::Machine(binding) => {
                    writer.u8(3);
                    writer.boolean(binding.nominal_requirement.is_some());
                    if let Some(requirement) = &binding.nominal_requirement {
                        writer.string("reach requirement", requirement)?;
                    }
                    encode_kinds(writer, &binding.requirement_parameter_kinds)?;
                    writer.len(
                        "reach requirement arguments",
                        binding.requirement_arguments.len(),
                    )?;
                    for argument in &binding.requirement_arguments {
                        writer.u32(argument.0);
                    }
                    encode_service_ceiling(writer, &binding.upper_bound)?;
                    writer.boolean(binding.upper_bound_may_suspend);
                    writer.boolean(binding.upper_bound_may_block);
                    writer.boolean(binding.selected_may_suspend);
                    writer.boolean(binding.selected_may_block);
                    writer.u32(binding.selected_identity.0);
                    writer.bytes(&binding.selected_contract_commitment);
                    encode_service_ceiling(writer, &binding.selected_reach)?;
                    encode_optional_id(writer, binding.callee);
                    writer.boolean(binding.schema.is_some());
                    if let Some(schema) = &binding.schema {
                        writer.u32(schema.template_identity.0);
                        writer.bytes(&schema.template_commitment);
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn decode(
        reader: &mut Reader<'_>,
    ) -> Result<Option<ClosedReachApplication>, CodecError> {
        if !reader.boolean()? {
            return Ok(None);
        }
        let template_identity = TerminalIdentityIndex(reader.u32()?);
        let template_commitment = reader.array()?;
        let (
            template_base_commitment,
            suspension_dependency,
            blocking_dependency,
            conditional_carry,
        ) = decode_operational(reader)?;
        let specialization_commitment = reader.array()?;
        let parameter_kinds = decode_kinds(reader)?;
        let telescope = decode_telescope(reader)?;
        let capture = if reader.boolean()? {
            let template_identity = TerminalIdentityIndex(reader.u32()?);
            let template_commitment = reader.array()?;
            let (
                template_base_commitment,
                suspension_dependency,
                blocking_dependency,
                conditional_carry,
            ) = decode_operational(reader)?;
            Some(ClosedReachCapture {
                template_identity,
                template_commitment,
                template_base_commitment,
                suspension_dependency,
                blocking_dependency,
                conditional_carry,
                specialization_commitment: reader.array()?,
                parameter_kinds: decode_kinds(reader)?,
                telescope: decode_telescope(reader)?,
            })
        } else {
            None
        };
        Ok(Some(ClosedReachApplication {
            template_identity,
            template_commitment,
            template_base_commitment,
            suspension_dependency,
            blocking_dependency,
            conditional_carry,
            specialization_commitment,
            parameter_kinds,
            telescope,
            capture,
            fixed: decode_ids(reader, "ServiceId")?,
            dependencies: decode_counted(reader, |reader| reader.u32())?,
            calls: decode_counted(reader, |reader| {
                Ok(ClosedReachCall {
                    site: super::wire::decode_invocation_site(reader)?,
                    binder: reader.u32()?,
                    application: if reader.boolean()? {
                        Some(ClosedReachCallApplication {
                            callee: reader.id("MachineId")?,
                            specialization_commitment: reader.array()?,
                            arguments: decode_counted(reader, decode_argument)?,
                        })
                    } else {
                        None
                    },
                })
            })?,
        }))
    }

    fn decode_telescope(reader: &mut Reader<'_>) -> Result<Vec<ClosedReachParameter>, CodecError> {
        decode_counted(reader, |reader| {
            Ok(match reader.u8()? {
                0 => ClosedReachParameter::Type {
                    argument: reader.string("reach const argument")?,
                },
                1 => ClosedReachParameter::Const {
                    argument: reader.string("reach proposition argument")?,
                },
                2 => ClosedReachParameter::Proposition {
                    argument: reader.string("reach requirement")?,
                },
                3 => {
                    let nominal_requirement = if reader.boolean()? {
                        Some(reader.string("reach selected identity")?)
                    } else {
                        None
                    };
                    ClosedReachParameter::Machine(ClosedReachMachineBinding {
                        nominal_requirement,
                        requirement_parameter_kinds: decode_kinds(reader)?,
                        requirement_arguments: decode_counted(reader, |reader| {
                            Ok(TerminalIdentityIndex(reader.u32()?))
                        })?,
                        upper_bound: decode_ids(reader, "ServiceId")?,
                        upper_bound_may_suspend: reader.boolean()?,
                        upper_bound_may_block: reader.boolean()?,
                        selected_may_suspend: reader.boolean()?,
                        selected_may_block: reader.boolean()?,
                        selected_identity: TerminalIdentityIndex(reader.u32()?),
                        selected_contract_commitment: reader.array()?,
                        selected_reach: decode_ids(reader, "ServiceId")?,
                        callee: decode_optional_id(reader, "MachineId")?,
                        schema: if reader.boolean()? {
                            Some(ClosedReachSchema {
                                template_identity: TerminalIdentityIndex(reader.u32()?),
                                template_commitment: reader.array()?,
                            })
                        } else {
                            None
                        },
                    })
                }
                tag => return Err(CodecError::InvalidTag("ClosedReachParameter", tag)),
            })
        })
    }

    type Dependency = terminal_psi::ContractDependency<bool, Vec<u32>>;
    type Carry = terminal_psi::ConditionalCarryRequirement;

    fn encode_term(writer: &mut Writer, term: &Dependency) -> Result<(), CodecError> {
        writer.boolean(term.concrete);
        writer.len("callback dependency", term.parameters.len())?;
        for parameter in &term.parameters {
            writer.u32(*parameter);
        }
        Ok(())
    }

    fn decode_term(reader: &mut Reader<'_>) -> Result<Dependency, CodecError> {
        Ok(Dependency {
            concrete: reader.boolean()?,
            parameters: decode_counted(reader, |reader| reader.u32())?,
        })
    }

    fn encode_coordinate(writer: &mut Writer, coordinate: usize) -> Result<(), CodecError> {
        writer.u64(
            u64::try_from(coordinate)
                .map_err(|_| CodecError::CollectionTooLong("carry coordinate"))?,
        );
        Ok(())
    }

    fn decode_coordinate(reader: &mut Reader<'_>) -> Result<usize, CodecError> {
        usize::try_from(reader.u64()?)
            .map_err(|_| CodecError::CollectionTooLong("carry coordinate"))
    }

    fn encode_acquisition(
        writer: &mut Writer,
        machine: &str,
        state: &str,
        source: &language_semantics::ScopedPermissionEventSource<String>,
    ) -> Result<(), CodecError> {
        use language_semantics::ScopedPermissionEventSource as Source;
        writer.string("carry acquisition machine", machine)?;
        writer.string("carry acquisition state", state)?;
        match source {
            Source::StateEntry => writer.u8(0),
            Source::Statement { statement_index } => {
                writer.u8(1);
                encode_coordinate(writer, *statement_index)?;
            }
            Source::Call {
                statement_index,
                call_ordinal,
                target_symbol,
            } => {
                writer.u8(2);
                encode_coordinate(writer, *statement_index)?;
                encode_coordinate(writer, *call_ordinal)?;
                writer.string("carry acquisition target", target_symbol)?;
            }
            Source::InstructionDisposition { statement_index } => {
                writer.u8(3);
                encode_coordinate(writer, *statement_index)?;
            }
            Source::InstructionExitDisposition { statement_index } => {
                writer.u8(4);
                encode_coordinate(writer, *statement_index)?;
            }
            Source::StateExit => writer.u8(5),
        }
        Ok(())
    }
    fn decode_acquisition(
        reader: &mut Reader<'_>,
    ) -> Result<
        (
            String,
            String,
            language_semantics::ScopedPermissionEventSource<String>,
        ),
        CodecError,
    > {
        use language_semantics::ScopedPermissionEventSource as Source;
        let machine = reader.string("carry acquisition machine")?;
        let state = reader.string("carry acquisition state")?;
        let source = match reader.u8()? {
            0 => Source::StateEntry,
            1 => Source::Statement {
                statement_index: decode_coordinate(reader)?,
            },
            2 => Source::Call {
                statement_index: decode_coordinate(reader)?,
                call_ordinal: decode_coordinate(reader)?,
                target_symbol: reader.string("carry acquisition target")?,
            },
            3 => Source::InstructionDisposition {
                statement_index: decode_coordinate(reader)?,
            },
            4 => Source::InstructionExitDisposition {
                statement_index: decode_coordinate(reader)?,
            },
            5 => Source::StateExit,
            tag => return Err(CodecError::InvalidTag("carry acquisition", tag)),
        };
        Ok((machine, state, source))
    }

    fn encode_operational(
        writer: &mut Writer,
        base: [u8; 32],
        suspension: &Dependency,
        blocking: &Dependency,
        carry: &[Carry],
    ) -> Result<(), CodecError> {
        use language_semantics::SuspensionValueOrigin;
        writer.bytes(&base);
        encode_term(writer, suspension)?;
        encode_term(writer, blocking)?;
        writer.len("conditional carry", carry.len())?;
        for row in carry {
            writer.u32(row.state);
            writer.u64(row.statement_index);
            writer.u64(row.call_ordinal);
            match &row.subject {
                SuspensionValueOrigin::Parameter { symbol, position } => {
                    writer.u8(0);
                    writer.string("carry subject", symbol)?;
                    encode_coordinate(writer, *position)?;
                }
                SuspensionValueOrigin::Local {
                    symbol,
                    statement_index,
                    environment_position,
                } => {
                    writer.u8(1);
                    writer.string("carry subject", symbol)?;
                    encode_coordinate(writer, *statement_index)?;
                    encode_coordinate(writer, *environment_position)?;
                }
                SuspensionValueOrigin::CallArgument { position } => {
                    writer.u8(2);
                    encode_coordinate(writer, *position)?;
                }
            }
            writer.string("carry type", &row.value_type)?;
            writer.len("carry acquisitions", row.claims.len())?;
            for arrival in &row.claims {
                let language_semantics::ScopedPermissionClaimIdentity::Established {
                    machine_symbol,
                    state_symbol,
                    source,
                    ordinal,
                } = &arrival.claim_identity
                else {
                    return Err(CodecError::MalformedStructuralFoundation(
                        "conditional carry receiving claim is unknown",
                    ));
                };
                encode_acquisition(writer, machine_symbol, state_symbol, source)?;
                writer.u32(*ordinal);
                match &arrival.provenance {
                    language_semantics::ScopedPermissionProvenance::Unknown => {
                        return Err(CodecError::MalformedStructuralFoundation(
                            "conditional carry acquisition join is unknown",
                        ));
                    }
                    language_semantics::ScopedPermissionProvenance::Established {
                        machine_symbol,
                        state_symbol,
                        source,
                    } => {
                        writer.u8(1);
                        encode_acquisition(writer, machine_symbol, state_symbol, source)?;
                    }
                    language_semantics::ScopedPermissionProvenance::Joined {
                        machine_symbol,
                        state_symbol,
                        source,
                        ordinal,
                    } => {
                        writer.u8(2);
                        encode_acquisition(writer, machine_symbol, state_symbol, source)?;
                        writer.u32(*ordinal);
                    }
                }
            }
            encode_term(writer, &row.condition)?;
        }
        Ok(())
    }

    fn decode_operational(
        reader: &mut Reader<'_>,
    ) -> Result<([u8; 32], Dependency, Dependency, Vec<Carry>), CodecError> {
        use language_semantics::SuspensionValueOrigin;
        let base = reader.array()?;
        let suspension = decode_term(reader)?;
        let blocking = decode_term(reader)?;
        let carry = decode_counted(reader, |reader| {
            let state = reader.u32()?;
            let statement_index = reader.u64()?;
            let call_ordinal = reader.u64()?;
            let subject = match reader.u8()? {
                0 => SuspensionValueOrigin::Parameter {
                    symbol: reader.string("carry subject")?,
                    position: decode_coordinate(reader)?,
                },
                1 => SuspensionValueOrigin::Local {
                    symbol: reader.string("carry subject")?,
                    statement_index: decode_coordinate(reader)?,
                    environment_position: decode_coordinate(reader)?,
                },
                2 => SuspensionValueOrigin::CallArgument {
                    position: decode_coordinate(reader)?,
                },
                tag => return Err(CodecError::InvalidTag("carry subject", tag)),
            };
            let value_type = reader.string("carry type")?;
            let claims = decode_counted(reader, |reader| {
                let (machine_symbol, state_symbol, source) = decode_acquisition(reader)?;
                let claim_identity =
                    language_semantics::ScopedPermissionClaimIdentity::Established {
                        machine_symbol,
                        state_symbol,
                        source,
                        ordinal: reader.u32()?,
                    };
                let tag = reader.u8()?;
                let (machine_symbol, state_symbol, source) = decode_acquisition(reader)?;
                let provenance = match tag {
                    1 => language_semantics::ScopedPermissionProvenance::Established {
                        machine_symbol,
                        state_symbol,
                        source,
                    },
                    2 => language_semantics::ScopedPermissionProvenance::Joined {
                        machine_symbol,
                        state_symbol,
                        source,
                        ordinal: reader.u32()?,
                    },
                    tag => return Err(CodecError::InvalidTag("carry acquisition join", tag)),
                };
                Ok(language_semantics::PermissionArrival {
                    claim_identity,
                    provenance,
                })
            })?;
            Ok(Carry {
                state,
                statement_index,
                call_ordinal,
                subject,
                value_type,
                claims,
                condition: decode_term(reader)?,
            })
        })?;
        Ok((base, suspension, blocking, carry))
    }

    pub(crate) fn encode_argument(writer: &mut Writer, argument: &ClosedReachArgument) {
        match argument {
            ClosedReachArgument::Type(identity) => {
                writer.u8(0);
                writer.u32(identity.0);
            }
            ClosedReachArgument::Const(identity) => {
                writer.u8(1);
                writer.u32(identity.0);
            }
            ClosedReachArgument::Proposition(identity) => {
                writer.u8(2);
                writer.u32(identity.0);
            }
            ClosedReachArgument::Machine {
                identity,
                contract_commitment,
            } => {
                writer.u8(3);
                writer.u32(identity.0);
                writer.bytes(contract_commitment);
            }
        }
    }
    pub(crate) fn decode_argument(
        reader: &mut Reader<'_>,
    ) -> Result<ClosedReachArgument, CodecError> {
        Ok(match reader.u8()? {
            0 => ClosedReachArgument::Type(TerminalIdentityIndex(reader.u32()?)),
            1 => ClosedReachArgument::Const(TerminalIdentityIndex(reader.u32()?)),
            2 => ClosedReachArgument::Proposition(TerminalIdentityIndex(reader.u32()?)),
            3 => ClosedReachArgument::Machine {
                identity: TerminalIdentityIndex(reader.u32()?),
                contract_commitment: reader.array()?,
            },
            tag => return Err(CodecError::InvalidTag("ClosedReachArgument", tag)),
        })
    }
}

#[cfg(test)]
mod counted_collection_bound_tests {

    use semantic_vocabulary::Proposition;
    use terminal_psi::{CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard};

    use super::wire::{Reader, bounded_count, decode_bytes};
    use super::{CodecError, decode_module, encode_module};
    use crate::{decode_crash_route_buckets, encode_crash_route_buckets};

    use super::target_family_tests::unit_machine;

    #[test]
    fn bounded_count_refuses_a_count_the_remaining_input_cannot_hold() {
        let mut reader = Reader::new(&[2, 0, 0, 0, 0x7F]);
        assert_eq!(
            bounded_count(&mut reader, 1),
            Err(CodecError::UnexpectedEnd)
        );

        let mut reader = Reader::new(&[2, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(bounded_count(&mut reader, 4), Ok(2));
        let mut reader = Reader::new(&[3, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(
            bounded_count(&mut reader, 4),
            Err(CodecError::UnexpectedEnd)
        );

        let mut reader = Reader::new(&[0, 0, 0, 0]);
        assert_eq!(bounded_count(&mut reader, 1), Ok(0));
    }

    #[test]
    fn over_declared_crash_route_counts_are_refused_before_their_elements() {
        assert_eq!(
            decode_crash_route_buckets(&[2, 0, 0, 0, 0x09]),
            Err(CodecError::UnexpectedEnd)
        );
        assert_eq!(
            decode_crash_route_buckets(&[0xFF, 0xFF, 0xFF, 0xFF, 0x09]),
            Err(CodecError::UnexpectedEnd)
        );
    }

    #[test]
    fn valid_crash_route_rosters_round_trip() {
        let routes = vec![
            CrashRouteBucket {
                cause: CrashCause::Trap,
                alternatives: vec![CrashRouteGuard::Truth],
            },
            CrashRouteBucket {
                cause: CrashCause::Abort,
                alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
                    Proposition::Truth,
                ))],
            },
        ];
        let bytes = encode_crash_route_buckets(&routes).unwrap();
        assert_eq!(decode_crash_route_buckets(&bytes).unwrap(), routes);
    }

    #[test]
    fn over_declared_string_rosters_and_byte_payloads_are_refused() {
        let mut reader = Reader::new(&[3, 0, 0, 0, 0x2A]);
        assert_eq!(
            reader.strings("test roster"),
            Err(CodecError::UnexpectedEnd)
        );

        let mut reader = Reader::new(&[9, 0, 0, 0, 1, 2]);
        assert_eq!(decode_bytes(&mut reader), Err(CodecError::UnexpectedEnd));
        let mut reader = Reader::new(&[2, 0, 0, 0, 1, 2]);
        assert_eq!(decode_bytes(&mut reader).unwrap(), vec![1, 2]);
    }

    #[test]
    fn every_proper_prefix_of_a_canonical_module_is_refused() {
        let machine = semantic_vocabulary::MachineId::new(1).unwrap();
        let mut module = terminal_psi::TerminalModule::for_entry(machine);
        module.machines.push(unit_machine(machine));
        let bytes = encode_module(&module).unwrap();
        assert_eq!(decode_module(&bytes).unwrap(), module);
        for length in 0..bytes.len() {
            assert!(
                decode_module(&bytes[..length]).is_err(),
                "a module prefix of {length} bytes must not decode"
            );
        }
    }
}
#[cfg(test)]
mod external_provider_tests {
    use super::{
        module_wire::encode_raw, numeric_intrinsic_tests, target_family_tests::identity_table,
    };
    use crate::{decode_module, encode_module, semantic_fingerprint};
    use language_semantics::{ExternalBindingIdentity, numeric_intrinsics::NumericIntrinsic};
    use semantic_vocabulary::PackageKeyIdentity;
    use terminal_psi::*;

    fn module() -> TerminalModule {
        let mut module = numeric_intrinsic_tests::module(NumericIntrinsic::Negation(
            numerics::literals::FloatFormat::F32,
        ));
        module.identities = identity_table(8);
        module.external_providers = vec![ExternalProviderConformance {
            requirement: NominalDeclarationIdentity::new(TerminalIdentityIndex(3)),
            requirement_identity: TerminalIdentityIndex(3),
            requirement_package: None,
            numeric_intrinsic: module.boundary_machines[0].numeric_intrinsic,
            requirement_schema: Some(ExternalRequirementSchema {
                identity: TerminalIdentityIndex(3),
                method: TerminalIdentityIndex(4),
                owner: TerminalIdentityIndex(3),
            }),
            target: Some("macos_arm64".into()),
            realization: CheckedMachineDeclaration {
                machine: NominalDeclarationIdentity::new(TerminalIdentityIndex(4)),
                state: NominalDeclarationIdentity::new(TerminalIdentityIndex(5)),
                semantic_declaration: TerminalIdentityIndex(6),
                parameter_types: vec![TerminalIdentityIndex(7)],
                callable: TerminalIdentityIndex(8),
                result_type: TerminalIdentityIndex(7),
                contract: [17; 32],
                published_may_suspend: false,
                published_may_block: false,
                specialization: None,
            },
            realization_package: PackageKeyIdentity::from_digest([1; 32]),
            provider: Some(ExternalProviderOwner {
                declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(6)),
                identity: TerminalIdentityIndex(6),
                package: PackageKeyIdentity::from_digest([2; 32]),
            }),
            binding: ExternalBindingIdentity::CompilerIntrinsic,
        }];
        module
    }

    #[test]
    fn external_provider_satisfaction_round_trips_all_mechanisms_without_source() {
        for binding in [
            ExternalBindingIdentity::CompilerIntrinsic,
            ExternalBindingIdentity::Syscall { number: 14 },
            ExternalBindingIdentity::VtableSlot { index: 9 },
            ExternalBindingIdentity::VtableField {
                field: "read".into(),
            },
            ExternalBindingIdentity::TableFunction {
                field: "allocate".into(),
            },
        ] {
            let mut module = module();
            module.external_providers[0].binding = binding;
            let bytes = encode_module(&module).unwrap();
            let decoded = decode_module(&bytes).unwrap();
            assert_eq!(decoded, module);
            assert_eq!(encode_module(&decoded).unwrap(), bytes);
            let mut missing = module.clone();
            missing.external_providers.clear();
            assert_ne!(
                semantic_fingerprint(&module).unwrap(),
                semantic_fingerprint(&missing).unwrap()
            );
        }
    }

    #[test]
    fn external_provider_satisfaction_survives_without_an_executable_boundary_call() {
        let mut module = module();
        module.boundary_machines.clear();
        let bytes = encode_module(&module).unwrap();
        let decoded = decode_module(&bytes).unwrap();
        assert_eq!(decoded.external_providers, module.external_providers);
        assert!(decoded.boundary_machines.is_empty());
        assert_eq!(encode_module(&decoded).unwrap(), bytes);
    }

    #[test]
    fn external_provider_satisfaction_rejects_missing_coordinates_and_duplicate_edges() {
        for mutation in 0..6 {
            let mut module = module();
            let candidate = &mut module.external_providers[0];
            match mutation {
                0 => candidate.realization.callable = TerminalIdentityIndex(99),
                1 => candidate
                    .realization
                    .parameter_types
                    .push(TerminalIdentityIndex(99)),
                2 => candidate.provider.as_mut().unwrap().identity = TerminalIdentityIndex(99),
                3 => {
                    candidate.binding = ExternalBindingIdentity::TableFunction {
                        field: String::new(),
                    }
                }
                4 => {
                    let duplicate = candidate.clone();
                    module.external_providers.push(duplicate);
                }
                5 => candidate.target = Some(String::new()),
                _ => unreachable!(),
            }
            let bytes = encode_raw(&module).unwrap();
            let error = decode_module(&bytes).unwrap_err().to_string();
            assert!(
                error.contains("external provider") || error.contains("numeric intrinsic"),
                "{error}"
            );
        }
    }

    #[test]
    fn external_provider_satisfaction_binds_declaration_package_owner_and_mechanism() {
        let original = module();
        let fingerprint = semantic_fingerprint(&original).unwrap();
        for mutation in 0..5 {
            let mut changed = original.clone();
            let candidate = &mut changed.external_providers[0];
            match mutation {
                0 => candidate.realization.contract = [18; 32],
                1 => candidate.realization_package = PackageKeyIdentity::from_digest([3; 32]),
                2 => {
                    candidate.provider.as_mut().unwrap().package =
                        PackageKeyIdentity::from_digest([4; 32])
                }
                3 => candidate.provider = None,
                4 => candidate.binding = ExternalBindingIdentity::Syscall { number: 14 },
                _ => unreachable!(),
            }
            assert_ne!(semantic_fingerprint(&changed).unwrap(), fingerprint);
        }
    }
}
#[cfg(test)]
mod numeric_intrinsic_tests {
    use super::module_wire::encode_raw;
    use super::target_family_tests::{identity_table, unit_machine};
    use crate::{decode_module, encode_module, semantic_fingerprint};
    use language_semantics::numeric_intrinsics::{NumericIntrinsic, NumericResult, NumericType};
    use semantic_vocabulary::{
        BoundaryMachineId, MachineId, QualifiedScalarType, ScalarQualificationSetId, ScalarType,
    };
    use terminal_psi::*;

    pub(crate) fn module(meaning: NumericIntrinsic) -> TerminalModule {
        let machine = MachineId::new(1).unwrap();
        let mut module = TerminalModule::for_entry(machine);
        module.machines.push(unit_machine(machine));
        module.identities = identity_table(3);
        let signature = meaning.signature().unwrap();
        let result = match signature.result {
            NumericResult::Primitive(primitive) => primitive.scalar_type().unwrap(),
            NumericResult::Boolean => ScalarType::Boolean,
            NumericResult::FloatClass => panic!("this helper creates scalar boundaries"),
        };
        module.boundary_machines.push(BoundaryMachineDeclaration {
            id: BoundaryMachineId::new(1).unwrap(),
            numeric_intrinsic: Some(meaning),
            declaration: None,
            role: BoundaryDeclarationRole::Callable,
            kind: BoundaryRequirementKind::Machine,
            completion: BoundaryCompletion::NormalReturn,
            unconditional_termination: false,
            identity: TerminalIdentityIndex(3),
            owner: NominalDeclarationIdentity::new(TerminalIdentityIndex(3)),
            attachment: None,
            parameter_order: vec![BoundaryParameterKind::Scalar; signature.operand_count],
            scalar_parameters: vec![
                signature.operand.scalar_type().unwrap();
                signature.operand_count
            ],
            structural_parameters: Vec::new(),
            result: BoundaryMachineResult::Scalar(QualifiedScalarType {
                scalar_type: result,
                qualifications: ScalarQualificationSetId::ZERO,
            }),
            normal_qualifications: Vec::new(),
            scalar_domain_requirements: Vec::new(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            scalar_requires: Vec::new(),
            scalar_ensures: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            result_reference_regions: Vec::new(),
            reference_outputs: Vec::new(),
            fixed_service_reach: Vec::new(),
            owning_service: None,
            published_synchronous_entries: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
        module
    }

    #[test]
    fn numeric_meaning_survives_portable_round_trip_and_changes_program_identity() {
        let meaning = language_semantics::numeric_intrinsics::NamedNumericIntrinsic::resolve(
            "F64",
            "is_nan",
            numerics::arithmetic::ArithmeticDomain::Exact,
        )
        .unwrap()
        .execution;
        let module = module(meaning);
        let bytes = encode_module(&module).unwrap();
        let decoded = decode_module(&bytes).unwrap();
        assert_eq!(decoded, module);
        assert_eq!(
            decoded.boundary_machines[0].numeric_intrinsic,
            Some(meaning)
        );
        assert_eq!(encode_module(&decoded).unwrap(), bytes);
        let mut changed = module.clone();
        changed.boundary_machines[0].numeric_intrinsic = None;
        assert_ne!(
            semantic_fingerprint(&module).unwrap(),
            semantic_fingerprint(&changed).unwrap()
        );
    }

    #[test]
    fn numeric_boundary_refuses_substituted_parameters_results_and_evidence_roles() {
        let meaning = NumericIntrinsic::Negation(numerics::literals::FloatFormat::F32);
        for mutation in 0..4 {
            let mut module = module(meaning);
            let boundary = &mut module.boundary_machines[0];
            match mutation {
                0 => boundary.scalar_parameters[0] = NumericType::F64.scalar_type().unwrap(),
                1 => boundary.result = BoundaryMachineResult::Unit,
                2 => boundary.role = BoundaryDeclarationRole::ContractEvidence,
                3 => {
                    boundary
                        .scalar_parameters
                        .push(NumericType::F32.scalar_type().unwrap());
                    boundary.parameter_order.push(BoundaryParameterKind::Scalar);
                }
                _ => unreachable!(),
            }
            let error = terminal_psi::validate_numeric_intrinsic_signature(
                &module.boundary_machines[0],
                &module.structural_types,
            )
            .unwrap_err();
            assert!(error.contains("numeric intrinsic"), "{error}");
            let bytes = encode_raw(&module).unwrap();
            let error = decode_module(&bytes).unwrap_err().to_string();
            assert!(error.contains("numeric intrinsic"), "{error}");
        }
    }

    #[test]
    fn integer_width_numeric_meanings_round_trip_without_carrier_substitution() {
        use language_semantics::numeric_intrinsics::PrimitiveIntegerComparisonOperation;
        use numerics::{arithmetic::ArithmeticDomain, integer_carrier::FixedInteger};
        for meaning in [
            NumericIntrinsic::IntegerComparison {
                operation: PrimitiveIntegerComparisonOperation::Less,
                integer_type: NumericType::Integer(FixedInteger::unsigned(13)),
            },
            NumericIntrinsic::Conversion {
                source: NumericType::Integer(FixedInteger::signed(13)),
                target: NumericType::F32,
                domain: ArithmeticDomain::Exact,
            },
        ] {
            let original = module(meaning);
            let bytes = encode_module(&original).unwrap();
            assert_eq!(decode_module(&bytes).unwrap(), original);
            let mut substituted = original.clone();
            substituted.boundary_machines[0].scalar_parameters[0] =
                NumericType::Integer(FixedInteger::unsigned(16))
                    .scalar_type()
                    .unwrap();
            assert!(decode_module(&encode_raw(&substituted).unwrap()).is_err());
        }
    }

    #[test]
    fn numeric_meaning_wire_rejects_noncanonical_padding_unknown_rows_and_truncation() {
        let meaning = NumericIntrinsic::Negation(numerics::literals::FloatFormat::F32);
        let boundary = &module(meaning).boundary_machines[0];
        let mut writer = super::wire::Writer::default();
        <super::structural_signature_wire::BoundaryMachine as super::wire_fields::Field<_>>::write(
            &mut writer,
            boundary,
        )
        .unwrap();
        let bytes = writer.finish();
        for length in 0..bytes.len() {
            assert!(
                <super::structural_signature_wire::BoundaryMachine as super::wire_fields::Field<
                    _,
                >>::read(&mut super::wire::Reader::new(&bytes[..length]))
                .is_err()
            );
        }
        for header in [
            [0, 255, 0, 0],
            [0, 0, 2, 0],
            [1, 255, 0, 0],
            [2, 0, 8, 0],
            [3, 0, 0, 1],
            [4, 8, 0, 1],
            [255, 0, 0, 0],
        ] {
            let mut bytes = [0; NumericIntrinsic::ENCODED_SIZE];
            bytes[..4].copy_from_slice(&header);
            assert!(
                NumericIntrinsic::from_canonical_encoding(bytes).is_none(),
                "{bytes:?}"
            );
        }
    }
}
#[cfg(test)]
mod reference_transport_tests {

    use super::target_family_tests::identity_table;
    use crate::{CodecError, decode_module, encode_module};
    use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        BindingRelevance, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
        StructuralRecordStorage, StructuralReferentIdentity, StructuralTypeDeclaration,
        StructuralTypeShape, TerminalIdentityIndex, TerminalModule,
    };

    fn reference_module(foreign_descriptor: bool) -> TerminalModule {
        let mut module = TerminalModule::declarations();
        module.identities = identity_table(4);
        module
            .structural_referents
            .push(StructuralReferentIdentity {
                id: StructuralTypeId::new(1).unwrap(),
                identity: TerminalIdentityIndex(1),
                foreign_descriptor,
                abstract_opaque: false,
            });
        module.structural_types.push(StructuralTypeDeclaration {
            requires: Vec::new(),
            id: StructuralTypeId::new(2).unwrap(),
            identity: TerminalIdentityIndex(2),
            quotient_carrier: None,
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: false,
            descriptor_cleanup: true,
            scalar_restriction: None,
            shape: StructuralTypeShape::Reference {
                referent: StructuralTypeId::new(1).unwrap(),
                access: StructuralAccess::SharedBorrow,
            },
        });
        module.task_activation_demands.structural_types = module.structural_types.clone();
        module.task_activation_demands.structural_referents = module.structural_referents.clone();
        module
    }

    #[test]
    fn reference_transport_round_trip_preserves_both_roles_without_storage() {
        for descriptor in [false, true] {
            let module = reference_module(descriptor);
            let bytes = encode_module(&module).unwrap();
            let decoded = decode_module(&bytes).unwrap();
            assert_eq!(decoded, module);
            assert_eq!(encode_module(&decoded).unwrap(), bytes);
            let referent = StructuralTypeId::new(1).unwrap();
            assert_eq!(
                decoded.foreign_reference_descriptor(referent),
                Some(descriptor)
            );
            assert!(decoded.structural_type(referent).is_none());
        }
    }

    #[test]
    fn reference_transport_cannot_supply_owned_field_storage() {
        for descriptor in [false, true] {
            let mut module = reference_module(descriptor);
            module.native_layouts.insert(
                StructuralTypeId::new(3).unwrap(),
                TerminalIdentityIndex(3),
                TerminalIdentityIndex::ABSENT,
                TerminalIdentityIndex::ABSENT,
                [terminal_psi::NativeLayoutFieldIdentity {
                    field: StructuralFieldId::new(1).unwrap(),
                    declaration_identity: TerminalIdentityIndex(4),
                }],
                [],
            );
            module.structural_types.push(StructuralTypeDeclaration {
                requires: Vec::new(),
                id: StructuralTypeId::new(3).unwrap(),
                identity: TerminalIdentityIndex(3),
                quotient_carrier: None,
                owns_linear_claim: false,
                copyable: false,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                shape: StructuralTypeShape::Record {
                    storage: StructuralRecordStorage::Packed,
                    fields: vec![StructuralFieldDeclaration {
                        inline_carrier: None,
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: TerminalIdentityIndex(4),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(
                            StructuralTypeId::new(1).unwrap(),
                        ),
                        storage: None,
                    }],
                },
            });
            let bytes = encode_module(&module).unwrap();
            let error = decode_module(&bytes).expect_err("transport supplied owned storage");
            assert!(
                error.to_string().contains("unknown structural type"),
                "{error}"
            );
        }
    }

    #[test]
    fn reference_transport_rejects_the_pre_role_wire_format() {
        let mut bytes = encode_module(&reference_module(false)).unwrap();
        bytes[8..10].copy_from_slice(&223_u16.to_le_bytes());
        assert!(matches!(
            decode_module(&bytes),
            Err(CodecError::UnsupportedFormatMarker(223))
        ));
    }

    fn inline_carrier_module(byte_field: bool) -> TerminalModule {
        let mut module = TerminalModule::declarations();
        module.identities = identity_table(4);
        let (shape, field_type) = if byte_field {
            let carrier = terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 6 };
            (
                StructuralTypeShape::ByteSequence(carrier),
                StructuralFieldType::ByteSequence(carrier),
            )
        } else {
            (
                StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Boolean),
                StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Boolean),
            )
        };
        let carrier = StructuralTypeDeclaration {
            requires: Vec::new(),
            id: StructuralTypeId::new(1).unwrap(),
            identity: TerminalIdentityIndex(1),
            quotient_carrier: None,
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: true,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape,
        };
        let mut duplicate = carrier.clone();
        duplicate.id = StructuralTypeId::new(2).unwrap();
        duplicate.identity = TerminalIdentityIndex(2);
        let mut record = carrier.clone();
        record.id = StructuralTypeId::new(3).unwrap();
        record.identity = TerminalIdentityIndex(3);
        record.shape = StructuralTypeShape::Record {
            storage: StructuralRecordStorage::Packed,
            fields: vec![StructuralFieldDeclaration {
                id: StructuralFieldId::new(1).unwrap(),
                identity: TerminalIdentityIndex(4),
                relevance: BindingRelevance::Relevant,
                storage: None,
                inline_carrier: Some(carrier.id),
                field_type,
            }],
        };
        let StructuralTypeShape::Record { fields, .. } = &record.shape else {
            panic!("record")
        };
        module.native_layouts.insert(
            record.id,
            record.identity,
            TerminalIdentityIndex::ABSENT,
            TerminalIdentityIndex::ABSENT,
            fields
                .iter()
                .map(|field| terminal_psi::NativeLayoutFieldIdentity {
                    field: field.id,
                    declaration_identity: field.identity,
                }),
            [],
        );
        module.structural_types = vec![carrier, duplicate, record];
        module
    }

    #[test]
    fn inline_carrier_round_trip_retains_exact_scalar_and_byte_identities() {
        for byte_field in [false, true] {
            let module = inline_carrier_module(byte_field);
            let bytes = encode_module(&module).unwrap();
            let decoded = decode_module(&bytes).unwrap();
            assert_eq!(decoded, module);
            assert_eq!(encode_module(&decoded).unwrap(), bytes);
        }
    }

    #[test]
    fn inline_carrier_ingress_rejects_missing_incompatible_and_identity_only_types() {
        for byte_field in [false, true] {
            for identity in [3, 4, 99] {
                let mut module = inline_carrier_module(byte_field);
                let StructuralTypeShape::Record { fields, .. } =
                    &mut module.structural_types[2].shape
                else {
                    panic!("record")
                };
                fields[0].inline_carrier = Some(StructuralTypeId::new(identity).unwrap());
                if identity == 4 {
                    module.identities = identity_table(5);
                    module
                        .structural_referents
                        .push(StructuralReferentIdentity {
                            id: StructuralTypeId::new(4).unwrap(),
                            identity: TerminalIdentityIndex(4),
                            foreign_descriptor: true,
                            abstract_opaque: false,
                        });
                    let mut reference = module.structural_types[0].clone();
                    reference.id = StructuralTypeId::new(5).unwrap();
                    reference.identity = TerminalIdentityIndex(5);
                    reference.no_code_affine_cleanup = false;
                    reference.descriptor_cleanup = true;
                    reference.shape = StructuralTypeShape::Reference {
                        referent: StructuralTypeId::new(4).unwrap(),
                        access: StructuralAccess::SharedBorrow,
                    };
                    module.structural_types.push(reference);
                }
                let error =
                    decode_module(&super::module_wire::encode_raw(&module).unwrap()).unwrap_err();
                assert!(error.to_string().contains("inline field"), "{error}");
            }
        }
    }

    #[test]
    fn opaque_storage_roles_round_trip_and_reject_redirected_carriers() {
        use language_semantics::opaque_representations::{
            OpaqueRepresentationApplicationOrigin, OpaqueRepresentationCopyDisposition,
            OpaqueRepresentationLifecycleDisposition, SelectedOpaqueApplication,
            selected_application_commitment,
        };
        let mut module = inline_carrier_module(false);
        module.identities = identity_table(5);
        let mut carrier = module.structural_types[2].clone();
        carrier.id = StructuralTypeId::new(4).unwrap();
        carrier.identity = TerminalIdentityIndex(5);
        let StructuralTypeShape::Record { fields, .. } = &mut carrier.shape else {
            panic!("record")
        };
        fields[0].id = StructuralFieldId::new(2).unwrap();
        module.native_layouts.insert(
            carrier.id,
            TerminalIdentityIndex(3),
            TerminalIdentityIndex::ABSENT,
            TerminalIdentityIndex::ABSENT,
            fields
                .iter()
                .map(|field| terminal_psi::NativeLayoutFieldIdentity {
                    field: field.id,
                    declaration_identity: field.identity,
                }),
            [],
        );
        module.structural_types.push(carrier);
        let identity_encoding = b"opaque storage correspondence codec control".to_vec();
        let (application_report_fingerprint, conformance_application_commitment) =
            language_semantics::conformance_identity::source_identity_commitments(
                &identity_encoding,
            );
        let lifecycle = OpaqueRepresentationLifecycleDisposition::Inert;
        let copy_disposition = OpaqueRepresentationCopyDisposition::PlacementOnly;
        let origin = OpaqueRepresentationApplicationOrigin::NamedConformance;
        module
            .opaque_representations
            .push(terminal_psi::TerminalOpaqueRepresentation {
                opaque: StructuralTypeId::new(3).unwrap(),
                carrier: StructuralTypeId::new(4).unwrap(),
                opaque_storage_identity: TerminalIdentityIndex(3),
                carrier_storage_identity: TerminalIdentityIndex(5),
                application: SelectedOpaqueApplication {
                    opaque_identity: "package::Opaque".into(),
                    carrier_identity: "package::Carrier".into(),
                    conformance_identity: "package::Representation".into(),
                    application_report_fingerprint,
                    conformance_application_commitment,
                    identity_encoding,
                    schema_version: 1,
                    lifecycle,
                    copy_disposition,
                    origin,
                    selected_application_commitment: selected_application_commitment(
                        conformance_application_commitment,
                        lifecycle,
                        copy_disposition,
                        origin,
                    ),
                },
            });
        let bytes = encode_module(&module).unwrap();
        assert_eq!(decode_module(&bytes).unwrap(), module);
        let mut redirected = module.clone();
        redirected.opaque_representations[0].carrier = StructuralTypeId::new(2).unwrap();
        let error =
            decode_module(&super::module_wire::encode_raw(&redirected).unwrap()).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("substituted its exact storage identity"),
            "{error}"
        );
        redirected.opaque_representations[0].carrier_storage_identity = TerminalIdentityIndex(2);
        let error =
            decode_module(&super::module_wire::encode_raw(&redirected).unwrap()).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("selected carrier storage contract"),
            "{error}"
        );
        let mut changed_native = module;
        changed_native.native_layouts = terminal_psi::NativeLayoutCatalog::default();
        for declaration in changed_native
            .structural_types
            .iter()
            .filter(|row| row.is_record_shape())
        {
            changed_native.native_layouts.insert(
                declaration.id,
                declaration.identity,
                TerminalIdentityIndex::ABSENT,
                TerminalIdentityIndex::ABSENT,
                declaration
                    .shape
                    .record_fields()
                    .unwrap()
                    .iter()
                    .map(|field| terminal_psi::NativeLayoutFieldIdentity {
                        field: field.id,
                        declaration_identity: field.identity,
                    }),
                [],
            );
        }
        let error =
            decode_module(&super::module_wire::encode_raw(&changed_native).unwrap()).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("selected native layout provenance"),
            "{error}"
        );
    }
}
#[cfg(test)]
mod routed_scalar_tests {
    use super::module_wire::encode_raw;
    use super::target_family_tests::{identity_table, unit_machine};
    use crate::{decode_module, encode_module, semantic_fingerprint};
    use semantic_vocabulary::{
        DomainSemanticId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
        Proposition, ScalarDomainId, ScalarQualificationSetId, ScalarTerm, ScalarType, ValueId,
    };
    use terminal_psi::{
        BoundaryCompletion, BoundaryDeclarationRole, BoundaryParameterKind,
        BoundaryRequirementKind, CheckedMachineSpecialization, ClosedReachApplication,
        ContractClause, NominalDeclarationIdentity, ScalarDomainDeclaration,
        ScalarDomainEstablishmentRoute, ScalarDomainPredicate, ScalarQualificationSet,
        ScalarReturnOrigin, ScalarReturnQualification, ScalarReturnSite, TerminalIdentityIndex,
        TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
    };

    fn module() -> TerminalModule {
        let machine_id = MachineId::new(1).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
        let subject = ValueId::new(1).unwrap();
        let result = ValueId::new(2).unwrap();
        let domain = ScalarDomainId::new(1).unwrap();
        let qualifications = ScalarQualificationSetId::new(1);
        let predicate = Proposition::LessOrEqual(
            ScalarTerm::Integer {
                scalar_type: scalar_type.as_integer().unwrap(),
                value: IntegerValue::Signed(0),
            },
            ScalarTerm::Value {
                id: subject,
                scalar_type,
            },
        );
        let mut guarantee = predicate.clone();
        guarantee.remap_value_ids(|_| result);
        let mut machine = unit_machine(machine_id);
        machine.declaration.as_mut().unwrap().callable = TerminalIdentityIndex(3);
        machine.declaration.as_mut().unwrap().semantic_declaration = TerminalIdentityIndex(1);
        machine.parameter_order.push(BoundaryParameterKind::Scalar);
        machine.parameters.push(ValueDeclaration {
            id: subject,
            scalar_type,
            qualifications: ScalarQualificationSetId::ZERO,
        });
        machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type,
            qualifications,
        });
        machine.blocks[0].terminator = Terminator::Return {
            edge: EdgeId::new(1).unwrap(),
            value: subject,
            cleanup_actions: Vec::new(),
        };
        machine.contract.ensures.push(ContractClause {
            obligation: ObligationId::new(1).unwrap(),
            proposition: guarantee,
        });
        let mut module = TerminalModule::for_entry(machine_id);
        module.identities = identity_table(4);
        module.machines.push(machine);
        module
            .scalar_qualifications
            .domains
            .push(ScalarDomainDeclaration {
                required_memberships: Some(Vec::new()),
                id: domain,
                semantic_domain: DomainSemanticId::new(1).unwrap(),
                identity: TerminalIdentityIndex(4),
                carrier: scalar_type,
                predicate: Some(ScalarDomainPredicate {
                    subject,
                    proposition: predicate,
                }),
                establishment_routes: Some(vec![ScalarDomainEstablishmentRoute::ExactMachine {
                    declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
                    machine_identity: TerminalIdentityIndex(3),
                }]),
            });
        module
            .scalar_qualifications
            .sets
            .push(ScalarQualificationSet {
                id: qualifications,
                domains: vec![domain],
            });
        module
            .scalar_qualifications
            .returns
            .push(ScalarReturnQualification {
                site: ScalarReturnSite::NormalExit,
                machine: machine_id,
                edge: EdgeId::new(1).unwrap(),
                source: subject,
                result,
                domain,
                origins: vec![ScalarReturnOrigin::Established { route: 0 }],
            });
        module
    }

    #[test]
    fn routed_scalar_result_round_trips_and_commits_predicate_and_origin() {
        let module = module();
        let bytes = encode_module(&module).unwrap();
        assert_eq!(decode_module(&bytes).unwrap(), module);
        let original = semantic_fingerprint(&module).unwrap();
        let mut changed = module.clone();
        changed.scalar_qualifications.domains[0]
            .predicate
            .as_mut()
            .unwrap()
            .proposition = Proposition::Truth;
        assert_ne!(semantic_fingerprint(&changed).unwrap(), original);
        let mut changed = module.clone();
        changed.scalar_qualifications.returns[0].origins = vec![ScalarReturnOrigin::Forwarded];
        assert_ne!(semantic_fingerprint(&changed).unwrap(), original);
    }

    #[test]
    fn scalar_boundary_result_retains_membership_and_rejects_detached_types() {
        use terminal_psi::{
            BoundaryMachineDeclaration, BoundaryMachineResult, Operation, OperationKind,
            OperationResult,
        };
        let mut module = module();
        let result = module.machines[0].result.scalar().unwrap().value_type();
        let boundary = semantic_vocabulary::BoundaryMachineId::new(1).unwrap();
        module.boundary_machines.push(BoundaryMachineDeclaration {
            numeric_intrinsic: None,
            normal_qualifications: Vec::new(),
            scalar_domain_requirements: Vec::new(),
            declaration: None,
            role: BoundaryDeclarationRole::Callable,
            id: boundary,
            kind: BoundaryRequirementKind::Machine,
            completion: BoundaryCompletion::NormalReturn,
            unconditional_termination: false,
            identity: TerminalIdentityIndex(1),
            attachment: None,
            owner: NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
            parameter_order: Vec::new(),
            scalar_parameters: Vec::new(),
            crash_routes: Vec::new(),
            structural_parameters: Vec::new(),
            result: BoundaryMachineResult::Scalar(result),
            requires: Vec::new(),
            scalar_requires: Vec::new(),
            scalar_ensures: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            result_reference_regions: Vec::new(),
            reference_outputs: Vec::new(),
            fixed_service_reach: Vec::new(),
            owning_service: None,
            published_synchronous_entries: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
        module.machines[0].blocks[0].operations.push(Operation {
            id: semantic_vocabulary::OperationId::new(1).unwrap(),
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(3).unwrap(),
                scalar_type: result.scalar_type,
                qualifications: result.qualifications,
            }),
            kind: OperationKind::BoundaryCall {
                boundary,
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
                requirement_obligations: Vec::new(),
            },
        });
        assert_eq!(
            decode_module(&encode_module(&module).unwrap()).unwrap(),
            module
        );
        for mutation in 0..4 {
            let mut malformed = module.clone();
            let BoundaryMachineResult::Scalar(declared) =
                &mut malformed.boundary_machines[0].result
            else {
                panic!("scalar declaration");
            };
            match mutation {
                0 => declared.qualifications = ScalarQualificationSetId::new(99),
                1 => declared.scalar_type = ScalarType::Boolean,
                2 => declared.qualifications = ScalarQualificationSetId::ZERO,
                _ => {
                    let OperationResult::Scalar(actual) =
                        &mut malformed.machines[0].blocks[0].operations[0].result
                    else {
                        panic!("scalar operation");
                    };
                    actual.qualifications = ScalarQualificationSetId::ZERO;
                }
            }
            assert!(
                decode_module(&encode_raw(&malformed).unwrap()).is_err(),
                "mutation {mutation}"
            );
        }
    }

    #[test]
    fn routed_scalar_completion_retains_the_exact_formation_edge() {
        use terminal_psi::ScalarReturnSite;
        let mut module = module();
        let machine = &mut module.machines[0];
        let mut completion = machine.blocks[0].clone();
        completion.id = semantic_vocabulary::BlockId::new(2).unwrap();
        let destination = ValueId::new(3).unwrap();
        completion.parameters.push(ValueDeclaration {
            id: destination,
            ..machine.result.scalar().unwrap()
        });
        completion.terminator = Terminator::Return {
            edge: EdgeId::new(2).unwrap(),
            value: destination,
            cleanup_actions: Vec::new(),
        };
        machine.blocks[0].terminator = Terminator::Jump {
            edge: EdgeId::new(1).unwrap(),
            target: completion.id,
            arguments: vec![machine.parameters[0].id],
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            cleanup_actions: Vec::new(),
        };
        machine.blocks.push(completion);
        let formation = &mut module.scalar_qualifications.returns[0];
        formation.site = ScalarReturnSite::Completion {
            argument_ordinal: 0,
        };
        formation.result = destination;
        let mut returned = formation.clone();
        returned.site = ScalarReturnSite::NormalExit;
        returned.edge = EdgeId::new(2).unwrap();
        returned.source = destination;
        returned.result = machine.result.scalar().unwrap().id;
        returned.origins = vec![ScalarReturnOrigin::Forwarded];
        module.scalar_qualifications.returns.push(returned);
        assert_eq!(
            decode_module(&encode_module(&module).unwrap()).unwrap(),
            module
        );
        for mutation in 0..5 {
            let mut malformed = module.clone();
            match mutation {
                0 => {
                    malformed.scalar_qualifications.returns.remove(0);
                }
                1 => {
                    malformed.scalar_qualifications.returns[0].edge = EdgeId::new(2).unwrap();
                }
                2 => {
                    malformed.scalar_qualifications.returns[0].site =
                        ScalarReturnSite::Completion {
                            argument_ordinal: 1,
                        };
                }
                3 => {
                    malformed.scalar_qualifications.returns[0].origins =
                        vec![ScalarReturnOrigin::Forwarded];
                }
                _ => {
                    malformed.scalar_qualifications.returns[0].origins =
                        vec![ScalarReturnOrigin::Vacuous];
                }
            }
            assert!(
                decode_module(&encode_raw(&malformed).unwrap()).is_err(),
                "mutation {mutation}"
            );
        }
        module.scalar_qualifications.domains[0].establishment_routes = None;
        module.scalar_qualifications.domains[0].predicate = None;
        module.scalar_qualifications.returns[0].origins = vec![ScalarReturnOrigin::Vacuous];
        assert_eq!(
            decode_module(&encode_module(&module).unwrap()).unwrap(),
            module
        );
    }

    #[test]
    fn routed_scalar_result_rejects_detached_or_forged_transport() {
        type Mutation = fn(&mut TerminalModule);
        for (label, mutate) in [
            (
                "missing return row",
                (|module| module.scalar_qualifications.returns.clear()) as Mutation,
            ),
            ("wrong edge", |module| {
                module.scalar_qualifications.returns[0].edge = EdgeId::new(9).unwrap()
            }),
            ("wrong source", |module| {
                module.scalar_qualifications.returns[0].source = ValueId::new(9).unwrap()
            }),
            ("wrong result", |module| {
                module.scalar_qualifications.returns[0].result = ValueId::new(9).unwrap()
            }),
            ("wrong issuer", |module| {
                module.scalar_qualifications.domains[0]
                    .establishment_routes
                    .as_mut()
                    .unwrap()[0] = ScalarDomainEstablishmentRoute::ExactMachine {
                    declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
                    machine_identity: TerminalIdentityIndex(2),
                }
            }),
            ("missing predicate", |module| {
                module.scalar_qualifications.domains[0].predicate = None
            }),
            ("missing result predicate", |module| {
                module.machines[0].contract.ensures.clear()
            }),
            ("substituted predicate", |module| {
                module.machines[0].contract.ensures[0].proposition = Proposition::Truth
            }),
            ("wrong subject carrier", |module| {
                module.scalar_qualifications.domains[0]
                    .predicate
                    .as_mut()
                    .unwrap()
                    .proposition = Proposition::Equal(
                    ScalarTerm::Value {
                        id: ValueId::new(1).unwrap(),
                        scalar_type: ScalarType::Boolean,
                    },
                    ScalarTerm::Boolean(true),
                );
            }),
            ("false forwarding", |module| {
                module.scalar_qualifications.returns[0].origins =
                    vec![ScalarReturnOrigin::Forwarded]
            }),
            ("missing origin", |module| {
                module.scalar_qualifications.returns[0].origins.clear()
            }),
            ("wrong route", |module| {
                module.scalar_qualifications.returns[0].origins =
                    vec![ScalarReturnOrigin::Established { route: 1 }]
            }),
        ] {
            let mut candidate = module();
            mutate(&mut candidate);
            assert!(
                decode_module(&encode_raw(&candidate).unwrap()).is_err(),
                "{label}"
            );
        }
    }

    #[test]
    fn routed_scalar_forwarding_preserves_membership_without_issuer_status() {
        let mut module = module();
        module.machines[0].parameters[0].qualifications = ScalarQualificationSetId::new(1);
        module.machines[0].declaration.as_mut().unwrap().callable = TerminalIdentityIndex(2);
        module.scalar_qualifications.returns[0].origins = vec![ScalarReturnOrigin::Forwarded];
        assert_eq!(
            decode_module(&encode_module(&module).unwrap()).unwrap(),
            module
        );
    }

    // An explicit empty issuer set can transport an existing qualification,
    // but it cannot make a fresh result. The wire and semantic identity must
    // retain that distinction even when both choices contain zero routes.
    #[test]
    fn empty_issuer_set_round_trips_without_authorizing_fresh_membership() {
        let mut denied = module();
        denied.scalar_qualifications.domains[0].establishment_routes = Some(Vec::new());
        // Encoding trusts its stage producer. Receiving formation validation,
        // rather than serialization, must refuse this unchanged fresh result.
        let encoded = encode_module(&denied).unwrap();
        let error = decode_module(&encoded)
            .expect_err("an empty issuer set cannot establish fresh membership");
        assert!(
            matches!(error, crate::CodecError::MalformedScalarReturnFoundation { ref reason, .. }
            if reason.contains("origin authorized=false") && reason.contains("issuer=None")),
            "{error:?}"
        );

        denied.machines[0].parameters[0].qualifications = ScalarQualificationSetId::new(1);
        denied.machines[0].declaration.as_mut().unwrap().callable = TerminalIdentityIndex(2);
        denied.scalar_qualifications.returns[0].origins = vec![ScalarReturnOrigin::Forwarded];
        assert_eq!(
            decode_module(&encode_module(&denied).unwrap()).unwrap(),
            denied
        );

        let mut unrestricted = denied.clone();
        unrestricted.scalar_qualifications.domains[0].establishment_routes = None;
        assert_eq!(
            decode_module(&encode_module(&unrestricted).unwrap()).unwrap(),
            unrestricted
        );
        assert_ne!(
            semantic_fingerprint(&denied).unwrap(),
            semantic_fingerprint(&unrestricted).unwrap()
        );

        let mut detached = denied;
        detached.machines[0].parameters[0].qualifications = ScalarQualificationSetId::ZERO;
        let error = decode_module(&encode_raw(&detached).unwrap())
            .expect_err("forwarding needs actual existing membership on its unchanged source");
        assert!(
            matches!(error, crate::CodecError::MalformedScalarReturnFoundation { ref reason, .. }
            if reason.contains("origin authorized=false")),
            "{error:?}"
        );
    }

    #[test]
    fn declaration_contract_roundtrip_keeps_public_promises_separate_from_body() {
        let mut module = module();
        let required = semantic_vocabulary::ContractId::new(2).unwrap();
        let mut public = module.machines[0].contract.clone();
        public.id = required;
        public.declaration.as_mut().unwrap().parameter_types = vec![TerminalIdentityIndex(3)];
        public.declaration.as_mut().unwrap().result_type = TerminalIdentityIndex(3);
        public.requires = vec![Proposition::Truth];
        public.ensures.clear();
        public.termination = semantic_vocabulary::Termination::Terminates;
        module.declaration_contracts.push(public);
        let decoded = decode_module(&encode_module(&module).unwrap()).unwrap();
        assert_eq!(decoded, module);
        assert_eq!(
            decoded.declaration_contract(required).unwrap().requires,
            [Proposition::Truth]
        );
        assert!(
            decoded
                .declaration_contract(required)
                .unwrap()
                .ensures
                .is_empty()
        );
        assert_eq!(decoded.machines[0].contract.ensures.len(), 1);
        assert_eq!(
            decoded.declaration_contract(required).unwrap().termination,
            semantic_vocabulary::Termination::Terminates
        );

        let mut executable_requirement = module.clone();
        let body = &mut executable_requirement.machines[0];
        let operation = body
            .operations()
            .map(|operation| operation.id.get())
            .max()
            .unwrap_or(0)
            + 1;
        body.blocks[0]
            .operations
            .push(terminal_psi::Operation::unsuspended(
                semantic_vocabulary::OperationId::new(operation).unwrap(),
                terminal_psi::OperationResult::Unit,
                terminal_psi::OperationKind::CallUnit {
                    callee: semantic_vocabulary::MachineId::new(required.get()).unwrap(),
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            ));
        assert!(
            decode_module(&encode_raw(&executable_requirement).unwrap()).is_err(),
            "a declaration-only contract cannot supply an executable machine"
        );

        // Fixed model commitments isolate artifact reference integrity. They
        // establish no source application, selection or refinement proof.
        let mut captured = module.clone();
        captured.declaration_contracts[0]
            .declaration
            .as_mut()
            .unwrap()
            .specialization = Some(CheckedMachineSpecialization {
            template: NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
            template_callable: TerminalIdentityIndex(3),
            template_semantic_declaration: TerminalIdentityIndex(1),
            commitment: [9; 32],
            capture: Some(terminal_psi::CheckedMachineCapture {
                template_identity: TerminalIdentityIndex(1),
                template_commitment: [7; 32],
                specialization_commitment: [8; 32],
            }),
        });
        assert_eq!(
            decode_module(&encode_raw(&captured).unwrap()).unwrap(),
            captured
        );
        captured.declaration_contracts[0]
            .declaration
            .as_mut()
            .unwrap()
            .specialization
            .as_mut()
            .unwrap()
            .capture
            .as_mut()
            .unwrap()
            .template_identity = TerminalIdentityIndex(99);
        assert!(
            decode_module(&encode_raw(&captured).unwrap()).is_err(),
            "a nonempty capture identity still needs its actual declaration reference"
        );

        for mutation in 0..4 {
            let mut malformed = module.clone();
            let contract = &mut malformed.declaration_contracts[0];
            match mutation {
                0 => contract.id = malformed.machines[0].contract.id,
                1 => contract
                    .declaration
                    .as_mut()
                    .unwrap()
                    .parameter_types
                    .clear(),
                2 => contract.parameter_order.clear(),
                _ => {
                    contract.requires = vec![Proposition::Equal(
                        ScalarTerm::Value {
                            id: ValueId::new(99).unwrap(),
                            scalar_type: contract.parameters[0].scalar_type,
                        },
                        ScalarTerm::Value {
                            id: ValueId::new(99).unwrap(),
                            scalar_type: contract.parameters[0].scalar_type,
                        },
                    )]
                }
            }
            assert!(
                decode_module(&encode_raw(&malformed).unwrap()).is_err(),
                "mutation {mutation}"
            );
        }
    }

    #[test]
    fn previous_issuer_choice_format_refuses_before_decoding() {
        let mut bytes = b"PSITERM\0".to_vec();
        bytes.extend_from_slice(&257_u16.to_le_bytes());
        assert_eq!(
            decode_module(&bytes),
            Err(crate::CodecError::UnsupportedFormatMarker(257))
        );
    }

    #[test]
    fn previous_body_owned_signature_format_refuses_before_decoding() {
        let mut bytes = b"PSITERM\0".to_vec();
        bytes.extend_from_slice(&256_u16.to_le_bytes());
        assert_eq!(
            decode_module(&bytes),
            Err(crate::CodecError::UnsupportedFormatMarker(256))
        );
    }

    #[test]
    fn previous_callback_contract_format_refuses_before_decoding() {
        let mut bytes = b"PSITERM\0".to_vec();
        bytes.extend_from_slice(&253_u16.to_le_bytes());
        assert!(matches!(
            decode_module(&bytes),
            Err(crate::CodecError::UnsupportedFormatMarker(253))
        ));
    }

    #[test]
    fn routed_specialization_retains_guarded_acquisitions_and_exact_issuer() {
        let mut module = module();
        let declaration = module.machines[0].declaration.as_mut().unwrap();
        declaration.callable = TerminalIdentityIndex(2);
        declaration.specialization = Some(CheckedMachineSpecialization {
            capture: None,
            template: NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
            template_callable: TerminalIdentityIndex(3),
            template_semantic_declaration: TerminalIdentityIndex(1),
            commitment: [9; 32],
        });
        module.machines[0].closed_reach_application = Some(ClosedReachApplication {
            template_identity: TerminalIdentityIndex(1),
            template_base_commitment: [7; 32],
            template_commitment: terminal_psi::template_contract_commitment(
                [7; 32],
                0,
                &false.into(),
                &false.into(),
                &[],
            )
            .unwrap(),
            suspension_dependency: false.into(),
            blocking_dependency: false.into(),
            conditional_carry: Vec::new(),
            specialization_commitment: [9; 32],
            parameter_kinds: Vec::new(),
            telescope: Vec::new(),
            capture: None,
            fixed: Vec::new(),
            dependencies: Vec::new(),
            calls: Vec::new(),
        });
        assert_eq!(
            decode_module(&encode_module(&module).unwrap()).unwrap(),
            module
        );
        {
            use language_semantics::{
                ScopedPermissionClaimIdentity, ScopedPermissionEventSource, SuspensionValueOrigin,
            };
            let application = module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap();
            application.parameter_kinds = vec![3];
            application.telescope = vec![terminal_psi::ClosedReachParameter::Machine(
                terminal_psi::ClosedReachMachineBinding {
                    nominal_requirement: None,
                    requirement_parameter_kinds: Vec::new(),
                    requirement_arguments: Vec::new(),
                    upper_bound: Vec::new(),
                    upper_bound_may_suspend: true,
                    upper_bound_may_block: false,
                    selected_may_suspend: false,
                    selected_may_block: false,
                    selected_identity: TerminalIdentityIndex(2),
                    selected_contract_commitment: [3; 32],
                    selected_reach: Vec::new(),
                    callee: None,
                    schema: None,
                },
            )];
            application.suspension_dependency = terminal_psi::ContractDependency {
                concrete: false,
                parameters: vec![0],
            };
            application.conditional_carry = vec![terminal_psi::ConditionalCarryRequirement {
                state: 0,
                statement_index: 2,
                call_ordinal: 1,
                subject: SuspensionValueOrigin::Local {
                    symbol: "helper::kept".into(),
                    statement_index: 0,
                    environment_position: 0,
                },
                value_type: "Kept".into(),
                claims: vec![language_semantics::PermissionArrival {
                    claim_identity: ScopedPermissionClaimIdentity::Established {
                        machine_symbol: "helper".into(),
                        state_symbol: "helper::entry".into(),
                        source: ScopedPermissionEventSource::Statement { statement_index: 0 },
                        ordinal: 0,
                    },
                    provenance: language_semantics::ScopedPermissionProvenance::Joined {
                        machine_symbol: "helper".into(),
                        state_symbol: "helper::entry".into(),
                        source: ScopedPermissionEventSource::Call {
                            statement_index: 0,
                            call_ordinal: 0,
                            target_symbol: "acquire".into(),
                        },
                        ordinal: 0,
                    },
                }],
                condition: application.suspension_dependency.clone(),
            }];
            application.template_commitment = terminal_psi::template_contract_commitment(
                application.template_base_commitment,
                1,
                &application.suspension_dependency,
                &application.blocking_dependency,
                &application.conditional_carry,
            )
            .unwrap();
        }
        // A captured helper still uses the callback supplied to its method.
        // The guard names that method slot after the capture's type slot.
        let capture_commitment = {
            let application = module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap();
            application.suspension_dependency.parameters = vec![1];
            application.conditional_carry[0].condition.parameters = vec![1];
            let capture_commitment = terminal_psi::template_contract_commitment(
                [8; 32],
                2,
                &application.suspension_dependency,
                &application.blocking_dependency,
                &application.conditional_carry,
            )
            .unwrap();
            application.capture = Some(terminal_psi::ClosedReachCapture {
                template_identity: TerminalIdentityIndex(1),
                template_base_commitment: [8; 32],
                template_commitment: capture_commitment,
                suspension_dependency: application.suspension_dependency.clone(),
                blocking_dependency: application.blocking_dependency.clone(),
                conditional_carry: application.conditional_carry.clone(),
                specialization_commitment: [6; 32],
                parameter_kinds: vec![0],
                telescope: vec![terminal_psi::ClosedReachParameter::Type {
                    argument: "u32".into(),
                }],
            });
            application.template_commitment = terminal_psi::template_contract_commitment(
                application.template_base_commitment,
                2,
                &application.suspension_dependency,
                &application.blocking_dependency,
                &application.conditional_carry,
            )
            .unwrap();
            capture_commitment
        };
        module.machines[0]
            .declaration
            .as_mut()
            .unwrap()
            .specialization
            .as_mut()
            .unwrap()
            .capture = Some(terminal_psi::CheckedMachineCapture {
            template_identity: TerminalIdentityIndex(1),
            template_commitment: capture_commitment,
            specialization_commitment: [6; 32],
        });
        let preserved = module.clone();
        module.machines[0]
            .closed_reach_application
            .as_mut()
            .unwrap()
            .conditional_carry
            .clear();
        assert!(decode_module(&encode_raw(&module).unwrap()).is_err());
        module = preserved.clone();
        module.machines[0]
            .closed_reach_application
            .as_mut()
            .unwrap()
            .conditional_carry[0]
            .condition
            .parameters = vec![2];
        assert!(decode_module(&encode_raw(&module).unwrap()).is_err());
        module = preserved.clone();
        module.machines[0]
            .closed_reach_application
            .as_mut()
            .unwrap()
            .conditional_carry[0]
            .subject = language_semantics::SuspensionValueOrigin::CallArgument { position: 0 };
        assert!(decode_module(&encode_raw(&module).unwrap()).is_err());
        module = preserved.clone();
        if let terminal_psi::ClosedReachParameter::Machine(binding) = &mut module.machines[0]
            .closed_reach_application
            .as_mut()
            .unwrap()
            .telescope[0]
        {
            binding.selected_may_suspend = true;
        }
        assert!(decode_module(&encode_raw(&module).unwrap()).is_err());
        module = preserved;
        assert_eq!(
            decode_module(&encode_module(&module).unwrap()).unwrap(),
            module
        );
        module.machines[0]
            .declaration
            .as_mut()
            .unwrap()
            .specialization
            .as_mut()
            .unwrap()
            .template_callable = TerminalIdentityIndex(4);
        assert!(decode_module(&encode_raw(&module).unwrap()).is_err());
    }
}
#[cfg(test)]
pub(crate) mod target_family_tests {

    use crate::sections::semantic_module::module_wire::encode_raw;
    use crate::{CodecError, decode_module, encode_module};
    use semantic_vocabulary::{BlockId, BoundaryMachineId, ContractId, EdgeId, MachineId};
    use terminal_psi::{
        Block, BoundaryCompletion, BoundaryMachineDeclaration, BoundaryMachineResult,
        BoundaryRequirementKind, TargetMachineCallable, TargetMachineFamily,
        TargetMachineFamilyMember, TargetMachineSupply, TerminalIdentityIndex, TerminalMachine,
        TerminalMachineResult, TerminalModule, Terminator,
    };

    #[test]
    fn pre_installation_module_header_rejects_before_reading_tables() {
        let mut bytes = b"PSITERM\0".to_vec();
        bytes.extend_from_slice(&246_u16.to_le_bytes());
        assert!(matches!(
            decode_module(&bytes),
            Err(CodecError::UnsupportedFormatMarker(246))
        ));
    }

    #[test]
    fn declaration_catalog_round_trip_preserves_absent_default_entry() {
        let mut module = TerminalModule::declarations();
        for retain_body in [false, true] {
            if retain_body {
                module
                    .machines
                    .push(unit_machine(MachineId::new(1).unwrap()));
            }
            let bytes = encode_module(&module).unwrap();
            let decoded = decode_module(&bytes).unwrap();
            assert_eq!(decoded, module);
            assert!(decoded.entry.is_none());
            assert_eq!(encode_module(&decoded).unwrap(), bytes);
        }
    }

    #[test]
    fn target_family_round_trip_preserves_exact_tags_callables_and_supplies() {
        let machine = MachineId::new(1).unwrap();
        let mut module = TerminalModule::for_entry(machine);
        module.machines.push(unit_machine(machine));
        module.identities = identity_table(10);
        let mut members = vec![TargetMachineFamilyMember {
            target: "freestanding_custom".into(),
            callable: TargetMachineCallable::Machine(machine),
            supply: TargetMachineSupply::CheckedBody,
        }];
        for (boundary_position, (target, supply)) in [
            ("linux_x86_64", TargetMachineSupply::Requirement),
            ("macos_arm64", TargetMachineSupply::TopLevelRequirement),
            ("opaque:custom", TargetMachineSupply::Boundary),
            ("target.with.dots", TargetMachineSupply::AdmissionClaim),
            ("windows_x86_64", TargetMachineSupply::ExternalRealization),
        ]
        .into_iter()
        .enumerate()
        {
            let boundary = BoundaryMachineId::new(boundary_position as u64 + 1).unwrap();
            module.boundary_machines.push(BoundaryMachineDeclaration {
                numeric_intrinsic: None,
                normal_qualifications: Vec::new(),
                scalar_domain_requirements: Vec::new(),
                declaration: None,
                role: terminal_psi::BoundaryDeclarationRole::Callable,
                id: boundary,
                kind: BoundaryRequirementKind::Machine,
                completion: BoundaryCompletion::NormalReturn,
                unconditional_termination: false,
                identity: TerminalIdentityIndex(
                    u32::try_from(boundary.get()).expect("boundary identity"),
                ),
                attachment: None,
                owner: terminal_psi::NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
                parameter_order: Vec::new(),
                scalar_parameters: Vec::new(),
                crash_routes: Vec::new(),
                structural_parameters: Vec::new(),
                result: BoundaryMachineResult::Unit,
                requires: Vec::new(),
                scalar_requires: Vec::new(),
                scalar_ensures: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                result_reference_regions: Vec::new(),
                reference_outputs: Vec::new(),
                fixed_service_reach: Vec::new(),
                owning_service: None,
                published_synchronous_entries: Vec::new(),
                published_service_ceiling: Vec::new(),
            });
            members.push(TargetMachineFamilyMember {
                target: target.into(),
                callable: TargetMachineCallable::Boundary(boundary),
                supply,
            });
        }
        module.target_machine_families.push(TargetMachineFamily {
            canonical: members[0].callable,
            head_contract: [1; 32],
            members,
        });
        let owner = semantic_vocabulary::ServiceId::new(1).unwrap();
        module.services.push(terminal_psi::ServiceDeclaration {
            id: owner,
            identity: TerminalIdentityIndex(10),
            parents: Vec::new(),
        });
        module.boundary_machines[0].kind = BoundaryRequirementKind::Trait;
        module.boundary_machines[0].owning_service = Some(owner);
        for canonical in [
            TargetMachineCallable::Machine(machine),
            TargetMachineCallable::Boundary(BoundaryMachineId::new(1).unwrap()),
        ] {
            module.target_machine_families[0].canonical = canonical;
            let encoded_bytes = encode_module(&module).unwrap();
            let decoded_module = decode_module(&encoded_bytes).unwrap();
            assert_eq!(decoded_module, module);
            assert_eq!(encode_module(&decoded_module).unwrap(), encoded_bytes);
        }
    }

    #[test]
    fn target_family_wire_rejects_duplicate_tags_and_missing_callables() {
        let machine = TargetMachineCallable::Machine(MachineId::new(1).unwrap());
        let boundary = TargetMachineCallable::Boundary(BoundaryMachineId::new(1).unwrap());
        for (case, members, expected_error) in [
            (
                "duplicate deployment tag",
                vec![
                    TargetMachineFamilyMember {
                        target: "custom_target".into(),
                        callable: machine,
                        supply: TargetMachineSupply::CheckedBody,
                    },
                    TargetMachineFamilyMember {
                        target: "custom_target".into(),
                        callable: boundary,
                        supply: TargetMachineSupply::Requirement,
                    },
                ],
                CodecError::NonCanonicalOrder("target-family members by deployment tag"),
            ),
            (
                "missing machine",
                vec![TargetMachineFamilyMember {
                    target: "custom_target".into(),
                    callable: machine,
                    supply: TargetMachineSupply::CheckedBody,
                }],
                CodecError::MalformedStructuralFoundation(
                    "target-family checked member has no matching body supply",
                ),
            ),
            (
                "missing boundary",
                vec![TargetMachineFamilyMember {
                    target: "custom_target".into(),
                    callable: boundary,
                    supply: TargetMachineSupply::Requirement,
                }],
                CodecError::MalformedStructuralFoundation(
                    "target-family opaque member has no matching declaration supply",
                ),
            ),
        ] {
            let entry = MachineId::new(2).unwrap();
            let mut module = TerminalModule::for_entry(entry);
            module.machines.push(unit_machine(entry));
            module.identities = identity_table(10);
            module.target_machine_families.push(TargetMachineFamily {
                canonical: members[0].callable,
                head_contract: [1; 32],
                members,
            });
            let encoded_bytes = encode_raw(&module).unwrap();
            if matches!(expected_error, CodecError::NonCanonicalOrder(_)) {
                assert_eq!(
                    encode_module(&module),
                    Err(expected_error.clone()),
                    "{case}"
                );
            } else {
                assert_eq!(encode_module(&module).unwrap(), encoded_bytes, "{case}");
                assert_eq!(
                    crate::semantic_fingerprint(&module).unwrap(),
                    super::fingerprint_bytes(&encoded_bytes),
                    "{case}"
                );
            }
            assert_eq!(decode_module(&encoded_bytes), Err(expected_error), "{case}");
        }
    }

    pub(crate) fn unit_machine(machine: MachineId) -> TerminalMachine {
        let block = BlockId::new(1).unwrap();
        TerminalMachine {
            assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
            id: machine,
            attachment: None,
            nominal_cleanup_subject: None,
            ranked_scc: None,
            structural_places: Vec::new(),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block,
            blocks: vec![Block {
                id: block,
                parameters: Vec::new(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(1).unwrap(),
                    cleanup_actions: Vec::new(),
                },
            }],
            contract: terminal_psi::MachineContract {
                declaration: Some(terminal_psi::CheckedMachineDeclaration {
                    semantic_declaration: Default::default(),
                    parameter_types: Vec::new(),
                    callable: Default::default(),
                    result_type: Default::default(),
                    machine: terminal_psi::NominalDeclarationIdentity::new(TerminalIdentityIndex(
                        1,
                    )),
                    state: terminal_psi::NominalDeclarationIdentity::new(TerminalIdentityIndex(2)),
                    contract: [7; 32],
                    published_may_suspend: false,
                    published_may_block: false,
                    specialization: None,
                }),
                parameters: Vec::new(),
                parameter_order: Vec::new(),
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Unit,
                entry_claims: Vec::new(),
                declared_service_reach: None,
                closed_reach_application: None,
                published_synchronous_entries: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                ..terminal_psi::MachineContract::empty(ContractId::new(1).unwrap())
            },
        }
    }

    pub(crate) fn identity_table(count: u32) -> terminal_psi::TerminalIdentityTable {
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        for position in 1..=count {
            identities.intern(&format!("test::identity{position}"));
        }
        identities.build()
    }

    #[test]
    fn assembly_catalog_target_demand_survives_wire_and_changes_identity() {
        let machine = MachineId::new(1).unwrap();
        let mut module = TerminalModule::for_entry(machine);
        module.machines.push(unit_machine(machine));
        let portable = encode_module(&module).unwrap();
        module.machines[0].assembly_target_requirement =
            terminal_psi::RegisterTargetApplicability::X86_64;
        let x86 = encode_module(&module).unwrap();
        assert_ne!(portable, x86);
        assert_eq!(decode_module(&x86).unwrap(), module);
        module.machines[0].assembly_target_requirement =
            terminal_psi::RegisterTargetApplicability::Aarch64;
        let arm = encode_module(&module).unwrap();
        assert_ne!(x86, arm);
        assert_eq!(decode_module(&arm).unwrap(), module);
        let mut old = portable;
        old[8..10].copy_from_slice(&164_u16.to_le_bytes());
        assert!(matches!(
            decode_module(&old),
            Err(CodecError::UnsupportedFormatMarker(164))
        ));
    }
}

pub(crate) use crate::codec_error::CodecError;
pub(crate) use canonical_order::{
    canonical_proposition_order_key, crash_routes_are_canonical, validate_canonical_order,
    validate_crash_route_predicates,
};
pub(crate) use module_foundation_validation::validate_structural_foundation;
pub(crate) use module_wire::{decode_module_body, encode_raw};
pub(crate) use wire_fields::{CrashRoutes, Field};

#[cfg(test)]
mod qualified_cleanup_receiving_tests {

    use super::module_wire::encode_raw;
    use crate::{CanonicalTerminalArtifact, CodecError, decode_module};
    use terminal_psi::{StructuralAccess, TerminalAffineCleanupAction, Terminator};

    #[test]
    #[ignore = "explicit qualification supplies OMEGA_QUALIFIED_CLEANUP_CANONICAL_ARTIFACT"]
    fn qualified_owned_cleanup_rejects_overlapping_whole_discard() {
        let path = std::env::var_os("OMEGA_QUALIFIED_CLEANUP_CANONICAL_ARTIFACT")
            .expect("supply actual public CLI canonical output");
        let bytes = std::fs::read(path).expect("read canonical artifact");
        let artifact = CanonicalTerminalArtifact::from_bytes(&bytes)
            .expect("intact qualified cleanup must cross canonical receiving");
        let mut module = artifact.semantic_module().clone();
        let mut mutated = false;
        for machine in &mut module.machines {
            let qualified_roots = machine
                .structural_parameters
                .iter()
                .chain(
                    machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.structural_parameters),
                )
                .filter(|parameter| {
                    parameter.access == StructuralAccess::Owned
                        && (!parameter.qualifications.is_empty()
                            || !parameter.projected_qualifications.is_empty())
                })
                .map(|parameter| parameter.place)
                .chain(
                    machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .filter_map(|operation| {
                            operation
                                .result
                                .as_structural()
                                .filter(|result| {
                                    !result.qualifications.is_empty()
                                        || !result.projected_qualifications.is_empty()
                                })
                                .map(|result| result.place)
                        }),
                )
                .collect::<Vec<_>>();
            for block in &mut machine.blocks {
                let rosters: Vec<_> = match &mut block.terminator {
                    Terminator::Jump {
                        cleanup_actions, ..
                    } => vec![cleanup_actions],
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        vec![
                            &mut when_true.cleanup_actions,
                            &mut when_false.cleanup_actions,
                        ]
                    }
                    Terminator::StructuralCase { cases, .. } => cases
                        .iter_mut()
                        .map(|case| &mut case.cleanup_actions)
                        .collect(),
                    _ => Vec::new(),
                };
                for actions in rosters {
                    let root = actions.iter().find_map(|action| match action {
                        TerminalAffineCleanupAction::DiscardResidual(discard)
                            if qualified_roots.contains(&discard.place) =>
                        {
                            Some(discard.place)
                        }
                        _ => None,
                    });
                    if let Some(root) = root {
                        actions.push(TerminalAffineCleanupAction::DiscardRoot(root));
                        mutated = true;
                        break;
                    }
                }
                if mutated {
                    break;
                }
            }
            if mutated {
                break;
            }
        }
        assert!(
            mutated,
            "fixture must exercise a qualified owned residual root"
        );
        let wire = encode_raw(&module).expect("encode intentional malformed wire");
        assert_eq!(
            decode_module(&wire).unwrap_err(),
            CodecError::MalformedStructuralFoundation(
                "affine region cleanup overlaps a whole-root discard"
            )
        );
    }
}
pub(crate) use proposition_wire::{decode_proposition, encode_proposition};
pub(crate) use scalar_term_wire::{decode_scalar_term, encode_scalar_term};
pub(crate) use terminal_psi::{SemanticFingerprint, TerminalModule, TerminalPsiIdentity};
pub(crate) use wire::{Reader, Writer, decode_canonical, decode_counted};

pub(crate) const MAGIC: &[u8; 8] = b"PSITERM\0";

// Marker 262 distinguishes an opaque supplied value from an empty record.
// Earlier decoders cannot interpret that shape. The complete write frames,
// numeric meanings and finite families published in 261/260/259 remain intact.
pub(crate) const FORMAT_MARKER: u16 = 262;

pub(crate) const FINGERPRINT_DOMAIN: &[u8] = b"psi-terminal-semantic-fingerprint\0";

pub(crate) const MAX_CONTENT_IDENTITY_BYTES: usize = 1 << 20;

pub fn encode_module(module: &TerminalModule) -> Result<Vec<u8>, CodecError> {
    if module.identities.invocation_scope().is_some() {
        return Err(CodecError::MalformedStructuralFoundation(
            "invocation code cannot enter a portable Terminal artifact",
        ));
    }
    encode_execution_module(module)
}

pub(crate) fn encode_execution_module(module: &TerminalModule) -> Result<Vec<u8>, CodecError> {
    scalar_qualification_wire::validate(&module.scalar_qualifications)?;
    validate_canonical_order(module)?;
    encode_raw(module)
}

pub fn encode_structural_type_declaration(
    declaration: &terminal_psi::StructuralTypeDeclaration,
) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    structural_type_wire::encode_structural_type(&mut writer, declaration)?;
    Ok(writer.finish())
}

pub(crate) static MODULE_DECODES: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

pub fn module_decode_count() -> u64 {
    MODULE_DECODES.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn decode_module(bytes: &[u8]) -> Result<TerminalModule, CodecError> {
    MODULE_DECODES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut reader = Reader::new(bytes);
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(CodecError::InvalidMagic);
    }
    let format_marker = reader.u16()?;
    if format_marker != FORMAT_MARKER {
        return Err(CodecError::UnsupportedFormatMarker(format_marker));
    }
    let module = decode_module_body(&mut reader)?;
    if reader.remaining() != 0 {
        return Err(CodecError::TrailingBytes(reader.remaining()));
    }
    scalar_qualification_wire::validate(&module.scalar_qualifications)?;
    validate_canonical_order(&module)?;
    validate_structural_foundation(&module)?;
    crate::identity_formations::validate_identity_formations(
        &module,
        &mut proof_admission::Budget::default(),
    )?;
    dynamic_result_admission::validate_dynamic_descriptor_results(&module)?;
    let canonical = encode_raw(&module)?;
    if canonical != bytes {
        return Err(CodecError::NonCanonicalEncoding);
    }
    Ok(module)
}

pub fn encode_crash_route_buckets(
    routes: &[terminal_psi::CrashRouteBucket],
) -> Result<Vec<u8>, CodecError> {
    if !crash_routes_are_canonical(routes) {
        return Err(CodecError::NonCanonicalOrder(
            "crash routes by cause and guard",
        ));
    }
    validate_crash_route_predicates(routes)?;
    let mut writer = Writer::default();
    writer.counted("crash route buckets", routes, contract_wire::Bucket::write)?;
    Ok(writer.finish())
}

pub fn decode_crash_route_buckets(
    bytes: &[u8],
) -> Result<Vec<terminal_psi::CrashRouteBucket>, CodecError> {
    decode_canonical(bytes, CrashRoutes::read, |routes| {
        encode_crash_route_buckets(routes)
    })
}

pub fn decode_canonical_proposition(
    bytes: &[u8],
) -> Result<semantic_vocabulary::Proposition, CodecError> {
    decode_canonical(bytes, decode_proposition, canonical_proposition_order_key)
}

pub fn semantic_fingerprint(module: &TerminalModule) -> Result<SemanticFingerprint, CodecError> {
    Ok(encoded_fingerprint(
        module,
        &encode_execution_module(module)?,
    ))
}

pub(crate) fn encoded_fingerprint(module: &TerminalModule, bytes: &[u8]) -> SemanticFingerprint {
    let local = module.identities.invocation_local_entries();
    if local.is_empty() {
        return fingerprint_bytes(bytes);
    }
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(b"psi-invocation-code-fingerprint\0");
    digest.raw(&(bytes.len() as u64).to_le_bytes());
    digest.raw(bytes);
    digest.raw(&(local.len() as u64).to_le_bytes());
    for identity in local {
        digest.raw(&identity.0.to_le_bytes());
    }
    SemanticFingerprint::from_bytes(digest.finish())
}

pub fn terminal_psi_identity(module: &TerminalModule) -> Result<TerminalPsiIdentity, CodecError> {
    Ok(TerminalPsiIdentity {
        vocabulary_marker: module.vocabulary_marker,
        program_fingerprint: semantic_fingerprint(module)?,
    })
}

pub(crate) fn fingerprint_bytes(bytes: &[u8]) -> SemanticFingerprint {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(FINGERPRINT_DOMAIN);
    let byte_len =
        u64::try_from(bytes.len()).expect("terminal-Psi bytes fit the u64 digest domain");
    digest.raw(&byte_len.to_le_bytes());
    digest.raw(bytes);
    SemanticFingerprint::from_bytes(digest.finish())
}

#[cfg(test)]
mod normal_qualification_tests {

    use super::module_wire::encode_raw;
    use super::target_family_tests::{identity_table, unit_machine};
    use crate::{CodecError, decode_module, encode_module, semantic_fingerprint};
    use semantic_vocabulary::{
        BoundaryMachineId, DomainSemanticId, MachineId, PlaceId, StructuralDomainId,
        StructuralFieldId, StructuralTypeId,
    };
    use terminal_psi::{
        BindingRelevance, BoundaryCompletion, BoundaryLifetimeDependency,
        BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind,
        BoundaryReferenceDestination, BoundaryReferenceOutput, BoundaryRequirementKind,
        NominalDeclarationIdentity, StructuralAccess, StructuralDomainDeclaration,
        StructuralDomainRequirement, StructuralFieldDeclaration, StructuralFieldType,
        StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
        StructuralRecordStorage, StructuralTypeDeclaration, StructuralTypeShape,
        TerminalIdentityIndex, TerminalModule,
    };

    fn type_id(value: u64) -> StructuralTypeId {
        StructuralTypeId::new(value).unwrap()
    }

    fn field(value: u32) -> StructuralPathSegment {
        StructuralPathSegment::Field(TerminalIdentityIndex(value))
    }

    fn module() -> TerminalModule {
        let machine = MachineId::new(1).unwrap();
        let mut module = TerminalModule::for_entry(machine);
        module.identities = identity_table(12);
        module.machines.push(unit_machine(machine));
        let shapes = [
            StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Packed,
                fields: Vec::new(),
            },
            StructuralTypeShape::FixedArray {
                element: type_id(1),
                length: terminal_psi::ArrayExtent::Literal(3),
            },
            StructuralTypeShape::Reference {
                referent: type_id(2),
                access: StructuralAccess::MutableBorrow,
            },
            StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Packed,
                fields: [8, 9]
                    .into_iter()
                    .enumerate()
                    .map(|(position, identity)| StructuralFieldDeclaration {
                        inline_carrier: None,
                        id: StructuralFieldId::new(position as u64 + 1).unwrap(),
                        identity: TerminalIdentityIndex(identity),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(type_id(1)),
                        storage: None,
                    })
                    .collect(),
            },
        ];
        for (position, shape) in shapes.into_iter().enumerate() {
            if let StructuralTypeShape::Record { fields, .. } = &shape {
                module.native_layouts.insert(
                    type_id(position as u64 + 1),
                    TerminalIdentityIndex(position as u32 + 4),
                    TerminalIdentityIndex::ABSENT,
                    TerminalIdentityIndex::ABSENT,
                    fields
                        .iter()
                        .map(|field| terminal_psi::NativeLayoutFieldIdentity {
                            field: field.id,
                            declaration_identity: field.identity,
                        }),
                    [],
                );
            }
            module.structural_types.push(StructuralTypeDeclaration {
                requires: Vec::new(),
                id: type_id(position as u64 + 1),
                identity: TerminalIdentityIndex(position as u32 + 4),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: position != 2,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                shape,
            });
        }
        let domain = StructuralDomainId::new(1).unwrap();
        module.structural_domains.push(StructuralDomainDeclaration {
            id: domain,
            semantic_domain: DomainSemanticId::new(1).unwrap(),
            identity: TerminalIdentityIndex(10),
            carrier: type_id(1),
            required_memberships: Some(Vec::new()),
            complete_predicate_theory: false,
            predicate: None,
            content_projection: None,
            byte_predicate: None,
            establishment_routes: None,
        });
        module.boundary_machines.push(BoundaryMachineDeclaration {
            id: BoundaryMachineId::new(1).unwrap(),
            declaration: None,
            role: terminal_psi::BoundaryDeclarationRole::Callable,
            kind: BoundaryRequirementKind::Machine,
            completion: BoundaryCompletion::NormalReturn,
            numeric_intrinsic: None,
            unconditional_termination: false,
            identity: TerminalIdentityIndex(11),
            owner: NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
            attachment: None,
            scalar_parameters: Vec::new(),
            parameter_order: vec![BoundaryParameterKind::Structural; 3],
            crash_routes: Vec::new(),
            structural_parameters: [1, 4, 3]
                .into_iter()
                .enumerate()
                .map(|(position, carrier)| StructuralParameterDeclaration {
                    place: PlaceId::new(position as u64 + 1).unwrap(),
                    position: position as u32,
                    structural_type: type_id(carrier),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::MutableBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    additional_input_qualifications: Vec::new(),
                })
                .collect(),
            result: BoundaryMachineResult::Unit,
            requires: vec![StructuralDomainRequirement {
                argument_index: 1,
                path: vec![field(8)],
                domain,
            }],
            normal_qualifications: [
                (0, Vec::new()),
                (1, vec![field(8)]),
                (
                    2,
                    vec![
                        StructuralPathSegment::Referent,
                        StructuralPathSegment::AllElements,
                    ],
                ),
            ]
            .into_iter()
            .map(|(argument_index, path)| StructuralDomainRequirement {
                argument_index,
                path,
                domain,
            })
            .collect(),
            scalar_domain_requirements: Vec::new(),
            scalar_requires: Vec::new(),
            scalar_ensures: Vec::new(),
            result_reference_regions: Vec::new(),
            reference_outputs: vec![BoundaryReferenceOutput {
                destination: BoundaryReferenceDestination::Parameter(2),
                path: Vec::new(),
                access: StructuralAccess::MutableBorrow,
                dependencies: vec![BoundaryLifetimeDependency::Stored {
                    argument_index: 2,
                    path: Vec::new(),
                    access: StructuralAccess::MutableBorrow,
                }],
            }],
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            fixed_service_reach: Vec::new(),
            owning_service: None,
            published_synchronous_entries: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
        module
    }

    // A dormant structural declaration still commits its issuer choice. No
    // boundary in this module may establish membership merely to test its wire.
    #[test]
    fn structural_wire_preserves_an_explicit_empty_issuer_set() {
        let mut unrestricted = module();
        unrestricted.boundary_machines.clear();
        let unrestricted_bytes = encode_module(&unrestricted).unwrap();
        assert_eq!(decode_module(&unrestricted_bytes).unwrap(), unrestricted);

        let mut denied = unrestricted.clone();
        denied.structural_domains[0].establishment_routes = Some(Vec::new());
        let denied_bytes = encode_module(&denied).unwrap();
        let decoded = decode_module(&denied_bytes).unwrap();
        assert_eq!(decoded, denied);
        assert_eq!(
            decoded.structural_domains[0].establishment_routes,
            Some(Vec::new())
        );
        assert_ne!(denied_bytes, unrestricted_bytes);
        assert_ne!(
            semantic_fingerprint(&denied).unwrap(),
            semantic_fingerprint(&unrestricted).unwrap()
        );
    }

    #[test]
    fn normal_qualification_round_trip_retains_root_field_and_all_referent_elements() {
        let module = module();
        let bytes = encode_module(&module).unwrap();
        let decoded = decode_module(&bytes).unwrap();
        assert_eq!(decoded, module);
        assert_eq!(encode_module(&decoded).unwrap(), bytes);
    }

    #[test]
    fn input_fact_lifetimes_survive_encoding_and_change_the_semantic_identity() {
        let mut module = module();
        let domain = module.structural_domains[0].id;
        let parameters = &mut module.boundary_machines[0].structural_parameters;
        parameters[0].qualifications.push(domain);
        parameters[0].additional_input_qualifications.push(
            terminal_psi::StructuralPathQualification {
                path: Vec::new(),
                domain,
            },
        );
        let projected = terminal_psi::StructuralPathQualification {
            path: vec![field(8)],
            domain,
        };
        parameters[1]
            .projected_qualifications
            .push(projected.clone());
        parameters[1]
            .additional_input_qualifications
            .push(projected);
        let bytes = encode_module(&module).unwrap();
        assert_eq!(decode_module(&bytes).unwrap(), module);
        let identity = semantic_fingerprint(&module).unwrap();
        for parameter in &mut module.boundary_machines[0].structural_parameters {
            parameter.additional_input_qualifications.clear();
        }
        assert_ne!(semantic_fingerprint(&module).unwrap(), identity);
    }

    #[test]
    fn an_input_fact_cannot_replace_missing_membership_evidence() {
        let mut module = module();
        let domain = module.structural_domains[0].id;
        module.boundary_machines[0].structural_parameters[0]
            .additional_input_qualifications
            .push(terminal_psi::StructuralPathQualification {
                path: Vec::new(),
                domain,
            });
        let wire = encode_raw(&module).unwrap();
        assert_eq!(
            decode_module(&wire).unwrap_err(),
            CodecError::NonCanonicalOrder("input observations by exact qualification subject",)
        );
    }

    #[test]
    fn normal_qualification_and_required_paths_change_whole_semantic_fingerprint() {
        let module = module();
        let original = semantic_fingerprint(&module).unwrap();
        let mut changed = module.clone();
        changed.boundary_machines[0].normal_qualifications.clear();
        assert_ne!(semantic_fingerprint(&changed).unwrap(), original);
        let mut changed = module.clone();
        changed.boundary_machines[0].normal_qualifications[2].path[1] =
            StructuralPathSegment::FixedIndex(0);
        assert_eq!(
            decode_module(&encode_module(&changed).unwrap()).unwrap(),
            changed
        );
        assert_ne!(semantic_fingerprint(&changed).unwrap(), original);
        let mut changed = module.clone();
        changed.boundary_machines[0].requires[0].path = vec![field(9)];
        assert_eq!(
            decode_module(&encode_module(&changed).unwrap()).unwrap(),
            changed
        );
        assert_ne!(semantic_fingerprint(&changed).unwrap(), original);
    }

    #[test]
    fn normal_qualification_public_decoder_rejects_wrong_carrier_and_missing_path() {
        let original = module();
        assert_eq!(
            decode_module(&encode_module(&original).unwrap()).unwrap(),
            original
        );
        for path in [
            vec![StructuralPathSegment::Referent],
            vec![StructuralPathSegment::FixedIndex(0)],
        ] {
            let mut malformed = original.clone();
            malformed.boundary_machines[0].normal_qualifications[2].path = path;
            assert_eq!(
                decode_module(&encode_raw(&malformed).unwrap()),
                Err(CodecError::MalformedStructuralFoundation(
                    "boundary normal guarantee has the wrong mutable path carrier"
                )),
            );
        }
        let mut malformed = original;
        malformed.boundary_machines[0].requires[0].path = vec![field(12)];
        assert_eq!(
            decode_module(&encode_raw(&malformed).unwrap()),
            Err(CodecError::MalformedStructuralFoundation(
                "boundary requirement domain has the wrong path carrier"
            )),
        );
    }
}

#[cfg(test)]
mod global_descriptor_table_tests {
    use super::target_family_tests::{identity_table, unit_machine};
    use crate::{decode_module, encode_module};
    use semantic_vocabulary::{
        MachineId, OperationId, PlaceId, ServiceId, StructuralPlaceKind, StructuralTypeId,
    };
    use terminal_psi::*;

    fn descriptor_module(operation: GlobalDescriptorTableOperation) -> TerminalModule {
        let machine_id = MachineId::new(1).unwrap();
        let mut module = TerminalModule::for_entry(machine_id);
        module.identities = identity_table(4);
        let service = ServiceId::new(1).unwrap();
        module.services.push(ServiceDeclaration {
            id: service,
            identity: TerminalIdentityIndex(4),
            parents: vec![],
        });
        let carrier = StructuralTypeId::new(1).unwrap();
        module.structural_types.push(StructuralTypeDeclaration {
            requires: Vec::new(),
            id: carrier,
            identity: TerminalIdentityIndex(3),
            quotient_carrier: None,
            owns_linear_claim: false,
            copyable: false,
            no_code_affine_cleanup: true,
            descriptor_cleanup: true,
            scalar_restriction: None,
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                capacity: 10,
                access: Some(StructuralAccess::MutableBorrow),
            }),
        });
        let place = PlaceId::new(1).unwrap();
        let mut machine = unit_machine(machine_id);
        machine.declaration.as_mut().unwrap().parameter_types = vec![TerminalIdentityIndex(3)];
        machine
            .parameter_order
            .push(BoundaryParameterKind::Structural);
        machine
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place,
                position: 0,
                structural_type: carrier,
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::MutableBorrow,
                qualifications: vec![],
                projected_qualifications: vec![],
                additional_input_qualifications: Vec::new(),
            });
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::Parameter { position: 0 },
        });
        machine
            .declared_service_reach
            .get_or_insert_with(Vec::new)
            .push(service);
        machine.blocks[0].operations.push(Operation::unsuspended(
            OperationId::new(1).unwrap(),
            OperationResult::Unit,
            OperationKind::GlobalDescriptorTable {
                service,
                operation,
                source: StructuralArgument::whole(place, StructuralAccess::MutableBorrow),
            },
        ));
        module.machines.push(machine);
        module
    }

    fn refusal(module: &TerminalModule) -> String {
        let encoded = encode_module(module).expect("encode deliberately forged retained input");
        format!(
            "{:?}",
            decode_module(&encoded).expect_err("receiving validation must refuse")
        )
    }

    #[test]
    fn descriptor_table_directions_round_trip_with_exact_borrowed_bytes() {
        for operation in [
            GlobalDescriptorTableOperation::Store,
            GlobalDescriptorTableOperation::Load,
        ] {
            let module = descriptor_module(operation);
            let bytes = encode_module(&module).unwrap();
            assert_eq!(decode_module(&bytes).unwrap(), module);
        }
    }

    #[test]
    fn descriptor_table_receiving_rejects_wrong_extent_and_shared_backing() {
        let mut short = descriptor_module(GlobalDescriptorTableOperation::Store);
        short.structural_types[0].shape =
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                capacity: 9,
                access: Some(StructuralAccess::MutableBorrow),
            });
        assert!(refusal(&short).contains("exact unqualified ten-byte array"));

        let mut shared = descriptor_module(GlobalDescriptorTableOperation::Store);
        shared.structural_types[0].shape =
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                capacity: 10,
                access: Some(StructuralAccess::SharedBorrow),
            });
        assert!(refusal(&shared).contains("byte borrow exceeds its source access"));

        let mut unreadable = descriptor_module(GlobalDescriptorTableOperation::Load);
        if let OperationKind::GlobalDescriptorTable { source, .. } =
            &mut unreadable.machines[0].blocks[0].operations[0].kind
        {
            source.access = StructuralAccess::WriteOnlyBorrow;
        }
        assert!(refusal(&unreadable).contains("declared byte borrow"));
    }
}

#[cfg(test)]
mod descriptor_operation_tests {
    use super::target_family_tests::{identity_table, unit_machine};
    use crate::{decode_module, encode_module};
    use semantic_vocabulary::{
        IntegerSign, IntegerType, MachineId, OperationId, PlaceId, ScalarType, ServiceId,
        StructuralPlaceKind, StructuralTypeId, ValueId,
    };
    use terminal_psi::*;

    fn unsigned(bits: u16) -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap())
    }

    fn task_module() -> TerminalModule {
        let machine_id = MachineId::new(1).unwrap();
        let mut module = TerminalModule::for_entry(machine_id);
        module.identities = identity_table(8);
        let service = ServiceId::new(1).unwrap();
        module.services.push(ServiceDeclaration {
            id: service,
            identity: TerminalIdentityIndex(8),
            parents: vec![],
        });
        let shapes = [
            StructuralTypeShape::FixedArray {
                element: StructuralTypeId::new(4).unwrap(),
                length: ArrayExtent::Literal(5),
            },
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                capacity: 104,
                access: Some(StructuralAccess::SharedBorrow),
            }),
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                capacity: 10,
                access: Some(StructuralAccess::MutableBorrow),
            }),
            StructuralTypeShape::PrimitiveScalar(unsigned(64)),
        ];
        for (index, shape) in shapes.into_iter().enumerate() {
            module.structural_types.push(StructuralTypeDeclaration {
                requires: Vec::new(),
                id: StructuralTypeId::new(index as u64 + 1).unwrap(),
                identity: TerminalIdentityIndex(index as u32 + 3),
                quotient_carrier: None,
                owns_linear_claim: false,
                copyable: false,
                no_code_affine_cleanup: true,
                descriptor_cleanup: true,
                scalar_restriction: None,
                shape,
            });
        }
        let mut machine = unit_machine(machine_id);
        machine.declaration.as_mut().unwrap().parameter_types =
            (3..7).map(TerminalIdentityIndex).collect();
        for (position, access) in [
            StructuralAccess::MutableBorrow,
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow,
        ]
        .into_iter()
        .enumerate()
        {
            let place = PlaceId::new(position as u64 + 1).unwrap();
            machine
                .parameter_order
                .push(BoundaryParameterKind::Structural);
            machine
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place,
                    position: position as u32,
                    structural_type: StructuralTypeId::new(position as u64 + 1).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access,
                    qualifications: vec![],
                    projected_qualifications: vec![],
                    additional_input_qualifications: Vec::new(),
                });
            machine.structural_places.push(StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::Parameter {
                    position: position as u32,
                },
            });
        }
        let selector = ValueId::new(1).unwrap();
        machine.parameters.push(ValueDeclaration {
            id: selector,
            scalar_type: unsigned(16),
            qualifications: Default::default(),
        });
        machine.parameter_order.push(BoundaryParameterKind::Scalar);
        machine
            .declared_service_reach
            .get_or_insert_with(Vec::new)
            .push(service);
        machine.blocks[0].operations.push(Operation::unsuspended(
            OperationId::new(1).unwrap(),
            OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(2).unwrap(),
                scalar_type: unsigned(8),
                qualifications: Default::default(),
            }),
            OperationKind::ActivateTaskRegister {
                service,
                entries: 5,
                selector,
                gdt: StructuralArgument::whole(
                    PlaceId::new(1).unwrap(),
                    StructuralAccess::MutableBorrow,
                ),
                task_state: StructuralArgument::whole(
                    PlaceId::new(2).unwrap(),
                    StructuralAccess::SharedBorrow,
                ),
                scratch: StructuralArgument::whole(
                    PlaceId::new(3).unwrap(),
                    StructuralAccess::MutableBorrow,
                ),
            },
        ));
        machine.blocks[0].operations.push(Operation::unsuspended(
            OperationId::new(2).unwrap(),
            OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(3).unwrap(),
                scalar_type: unsigned(16),
                qualifications: Default::default(),
            }),
            OperationKind::ReadTaskRegister { service },
        ));
        module.machines.push(machine);
        module
    }

    fn refusal(module: &TerminalModule) -> String {
        let bytes = encode_module(module).expect("encode deliberately malformed retained input");
        format!(
            "{:?}",
            decode_module(&bytes).expect_err("receiving must refuse the changed obligation")
        )
    }

    #[test]
    fn task_register_operations_round_trip_with_exact_loans() {
        let module = task_module();
        let bytes = encode_module(&module).unwrap();
        assert_eq!(decode_module(&bytes).unwrap(), module);
    }

    #[test]
    fn task_register_receiving_rejects_forged_operands_and_result() {
        let mut shared = task_module();
        if let OperationKind::ActivateTaskRegister { gdt, .. } =
            &mut shared.machines[0].blocks[0].operations[0].kind
        {
            gdt.access = StructuralAccess::SharedBorrow;
        }
        assert!(refusal(&shared).contains("mutable GDT"));
        let mut short = task_module();
        short.structural_types[1].shape =
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                capacity: 103,
                access: Some(StructuralAccess::SharedBorrow),
            });
        assert!(refusal(&short).contains("exact unqualified"));
        let mut extent = task_module();
        if let OperationKind::ActivateTaskRegister { entries, .. } =
            &mut extent.machines[0].blocks[0].operations[0].kind
        {
            *entries = 6;
        }
        assert!(refusal(&extent).contains("exact unqualified"));
        let mut narrow = task_module();
        narrow.structural_types[3].shape = StructuralTypeShape::PrimitiveScalar(unsigned(32));
        assert!(refusal(&narrow).contains("declared element width"));
        let mut result = task_module();
        if let OperationResult::Scalar(result) =
            &mut result.machines[0].blocks[0].operations[1].result
        {
            result.scalar_type = unsigned(8);
        }
        assert!(refusal(&result).contains("exact unsigned result"));
        let mut selector = task_module();
        selector.machines[0].parameters[0].scalar_type = unsigned(32);
        assert!(refusal(&selector).contains("u16 selector"));
    }

    fn segment_module(kind: SegmentReload) -> TerminalModule {
        let mut module = task_module();
        let operations = &mut module.machines[0].blocks[0].operations;
        let OperationKind::ActivateTaskRegister {
            service,
            entries,
            gdt,
            selector,
            scratch,
            ..
        } = operations[0].kind.clone()
        else {
            unreachable!()
        };
        operations[0].kind = OperationKind::ReloadSegmentDescriptors {
            service,
            entries,
            gdt,
            selector,
            scratch,
            kind,
        };
        operations[1].kind = OperationKind::ReadSegmentSelector {
            service,
            register: SegmentRegister::Code,
        };
        module
    }

    #[test]
    fn segment_operations_round_trip_with_exact_loans_and_selectors() {
        for kind in [SegmentReload::Code, SegmentReload::Data] {
            for register in [
                SegmentRegister::Code,
                SegmentRegister::Data,
                SegmentRegister::Extra,
                SegmentRegister::Stack,
                SegmentRegister::Fs,
                SegmentRegister::Gs,
            ] {
                let mut module = segment_module(kind);
                if let OperationKind::ReadSegmentSelector {
                    register: selected, ..
                } = &mut module.machines[0].blocks[0].operations[1].kind
                {
                    *selected = register;
                }
                assert_eq!(
                    decode_module(&encode_module(&module).unwrap()).unwrap(),
                    module
                );
            }
        }
    }

    #[test]
    fn segment_receiving_rejects_lost_table_scratch_selector_and_result_obligations() {
        for kind in [SegmentReload::Code, SegmentReload::Data] {
            let mut shared = segment_module(kind);
            if let OperationKind::ReloadSegmentDescriptors { gdt, .. } =
                &mut shared.machines[0].blocks[0].operations[0].kind
            {
                gdt.access = StructuralAccess::SharedBorrow;
            }
            assert!(refusal(&shared).contains("mutable GDT"));
            let mut extent = segment_module(kind);
            if let OperationKind::ReloadSegmentDescriptors { entries, .. } =
                &mut extent.machines[0].blocks[0].operations[0].kind
            {
                *entries = 6;
            }
            assert!(refusal(&extent).contains("exact unqualified"));
            let mut scratch = segment_module(kind);
            scratch.structural_types[2].shape =
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                    capacity: 9,
                    access: Some(StructuralAccess::MutableBorrow),
                });
            assert!(refusal(&scratch).contains("exact unqualified"));
            let mut selector = segment_module(kind);
            selector.machines[0].parameters[0].scalar_type = unsigned(32);
            assert!(refusal(&selector).contains("u16 selector"));
            let mut result = segment_module(kind);
            if let OperationResult::Scalar(result) =
                &mut result.machines[0].blocks[0].operations[1].result
            {
                result.scalar_type = unsigned(8);
            }
            assert!(refusal(&result).contains("exact unsigned result"));
        }
    }
}

#[cfg(test)]
mod opaque_value_tests {
    use super::target_family_tests::identity_table;
    use crate::{decode_module, encode_module};
    use semantic_vocabulary::{StructuralCaseId, StructuralTypeId};
    use terminal_psi::{
        StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
        TerminalIdentityIndex, TerminalModule,
    };

    #[test]
    fn opaque_values_round_trip_without_granting_construction_or_contents() {
        // A supplied opaque value can cross the artifact boundary as a whole.
        // Its presence grants neither zero construction nor readable children.
        let mut module = TerminalModule::declarations();
        let identity = StructuralTypeId::new(1).unwrap();
        module.identities = identity_table(1);
        module.structural_types.push(StructuralTypeDeclaration {
            requires: Vec::new(),
            id: identity,
            identity: TerminalIdentityIndex(1),
            quotient_carrier: None,
            owns_linear_claim: false,
            copyable: false,
            no_code_affine_cleanup: true,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape: StructuralTypeShape::Opaque,
        });
        let bytes = encode_module(&module).unwrap();
        let decoded = decode_module(&bytes).unwrap();
        assert_eq!(decoded, module);
        assert_eq!(encode_module(&decoded).unwrap(), bytes);
        assert!(!terminal_semantics::zeroed_structural_shape(
            decoded.structural_types.iter(),
            identity,
        ));
        assert_eq!(
            crate::foundation_validation::validate_structural_path(&decoded, identity, &[])
                .unwrap(),
            identity,
        );
        for child in [
            StructuralPathSegment::Field(TerminalIdentityIndex(1)),
            StructuralPathSegment::FixedIndex(0),
            StructuralPathSegment::Case(StructuralCaseId::new(1).unwrap()),
            StructuralPathSegment::Referent,
            StructuralPathSegment::AllElements,
            StructuralPathSegment::RepresentationView {
                structural_type: identity,
                stored_scalar: None,
            },
        ] {
            assert!(
                crate::foundation_validation::validate_structural_path(
                    &decoded,
                    identity,
                    &[child],
                )
                .is_err()
            );
        }
        module.structural_types[0].quotient_carrier = Some(identity);
        assert!(
            encode_module(&module)
                .unwrap_err()
                .to_string()
                .contains("opaque data cannot supply a visible carrier")
        );
    }
}

#[cfg(test)]
mod format_marker_tests {
    use super::*;

    #[test]
    fn legacy_semantic_markers_are_refused_before_payload_interpretation() {
        // Independent Id and interrupt layouts both used 252 (and earlier
        // independent Id and relocation layouts both used 251).
        // Neither body may be interpreted as the combined layout, regardless
        // of which payload follows their shared legacy marker.
        for legacy in [250u16, 251, 252, 256, 261] {
            for payload in [&[][..], &[0xff, 0xff, 0xff][..]] {
                let mut bytes = MAGIC.to_vec();
                bytes.extend_from_slice(&legacy.to_le_bytes());
                bytes.extend_from_slice(payload);
                assert!(matches!(
                    decode_module(&bytes),
                    Err(CodecError::UnsupportedFormatMarker(marker)) if marker == legacy
                ));
            }
        }
    }
}
