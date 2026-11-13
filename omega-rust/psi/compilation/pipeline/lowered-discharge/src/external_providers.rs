//! Native provider intent becomes a checked conformance only after the same
//! signature, permission and kernel refinement judgments as ordinary code.

use semantic_vocabulary::{MachineId, ObligationId};
use structure_elaboration::{Elaboration, ProviderContractHeader};
use terminal_check_tables::{Checked, Construct, RefusalReason};
use terminal_semantics::calls::callable_refinement::{
    CallableContractView, contract_refinement_context, contract_refinement_goals,
};

fn view(header: &ProviderContractHeader) -> CallableContractView<'_> {
    CallableContractView {
        parameter_order: &header.parameter_order,
        scalar_parameters: &header.scalar_parameters,
        structural_parameters: &header.structural_parameters,
        result: &header.result,
        proof: &header.proof,
        evidence_owner: None,
        crash_policy: header.crash_policy,
    }
}

fn header_matches_boundary(
    header: &ProviderContractHeader,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
) -> bool {
    header.parameter_order == boundary.parameter_order
        && header
            .scalar_parameters
            .iter()
            .map(|formal| formal.scalar_type)
            .eq(boundary.scalar_parameters.iter().copied())
        && header.structural_parameters == boundary.structural_parameters
        && boundary.result.matches_machine_result(&header.result)
        && header.proof.requires == boundary.scalar_requires
        && header
            .proof
            .ensures
            .iter()
            .map(|clause| &clause.proposition)
            .eq(boundary.scalar_ensures.iter())
        && header.proof.crash_routes == boundary.crash_routes
        && header.proof.source_checked_requires.is_empty()
        && header.proof.erased_scalar_formals.is_empty()
        && header.proof.erased_proof_formals.is_empty()
        && header.proof.outcome_specific_ensures.is_empty()
}

pub(crate) fn publish(elaboration: &mut Elaboration) -> Checked<()> {
    if elaboration.external_provider_candidates.is_empty()
        && elaboration.module.external_providers.is_empty()
    {
        return Ok(());
    }
    let context = elaboration
        .module
        .entry
        .and_then(|entry| match entry {
            terminal_psi::TargetMachineCallable::Machine(machine) => Some(machine),
            _ => None,
        })
        .or_else(|| {
            elaboration
                .module
                .machines
                .first()
                .map(|machine| machine.id)
        })
        .ok_or_else(|| {
            crate::refusal(
                RefusalReason::ForeignIdentity,
                MachineId::new(1).expect("one is nonzero"),
                Construct::Machine,
                "provider conformance has no real diagnostic context",
            )
        })?;
    let refuse = |detail| {
        crate::refusal(
            RefusalReason::ForeignIdentity,
            context,
            Construct::Machine,
            detail,
        )
    };
    if !elaboration.module.external_providers.is_empty() {
        return Err(refuse("checked native conformances were already published"));
    }
    let mut checked = Vec::with_capacity(elaboration.external_provider_candidates.len());
    for candidate in &elaboration.external_provider_candidates {
        let module = &elaboration.module;
        if candidate.required.crash_policy
            != terminal_semantics::calls::callable_refinement::CallableCrashPolicy::Closed
            || candidate.supplied.crash_policy
                != terminal_semantics::calls::callable_refinement::CallableCrashPolicy::Closed
        {
            return Err(refuse(
                "native conformance cannot publish an open runtime crash contract",
            ));
        }
        let required = module
            .boundary_machine(candidate.requirement)
            .ok_or_else(|| refuse("native conformance has no exact required boundary header"))?;
        let declaration = required
            .declaration
            .as_ref()
            .ok_or_else(|| refuse("native requirement has no exact source declaration"))?;
        if required.role != terminal_psi::BoundaryDeclarationRole::Callable
            || declaration.source.machine != candidate.conformance.requirement
            || required.identity != candidate.conformance.requirement_identity
            || required.completion != candidate.completion
            || declaration.source.parameter_types
                != candidate.conformance.realization.parameter_types
            || declaration.source.result_type != candidate.conformance.realization.result_type
            || !header_matches_boundary(&candidate.required, required)
            || candidate.required_envelope.service_reach != required.published_service_ceiling
            || candidate.required_envelope.suspends != declaration.source.published_may_suspend
            || candidate.required_envelope.blocks != declaration.source.published_may_block
        {
            return Err(refuse(
                "native conformance changed its required public header",
            ));
        }
        if !required.requires.is_empty()
            || !required.normal_qualifications.is_empty()
            || !required.scalar_domain_requirements.is_empty()
            || !required.reference_outputs.is_empty()
            || !required.content_guarantees.is_empty()
            || !required.result_reference_regions.is_empty()
            || !required.program_local_root_introductions.is_empty()
        {
            return Err(crate::refusal(
                RefusalReason::Unsupported,
                context,
                Construct::Machine,
                "native conformance needs the complete retained structural boundary contract",
            ));
        }
        for (header, envelope) in [
            (&candidate.required, &candidate.required_envelope),
            (&candidate.supplied, &candidate.supplied_envelope),
        ] {
            terminal_semantics::callable_envelopes::validate_callable_envelope(
                module,
                envelope,
                &header.parameter_order,
                &header.scalar_parameters,
                &header.structural_parameters,
            )
            .map_err(refuse)?;
        }
        terminal_semantics::callable_envelopes::callable_envelope_refines(
            &candidate.required_envelope,
            &candidate.supplied_envelope,
        )
        .map_err(|detail| {
            crate::refusal(RefusalReason::Conflict, context, Construct::Machine, detail)
        })?;
        if candidate.conformance.realization.published_may_suspend
            != candidate.supplied_envelope.suspends
            || candidate.conformance.realization.published_may_block
                != candidate.supplied_envelope.blocks
        {
            return Err(refuse(
                "native conformance changed its supplied operational declaration",
            ));
        }
        // The header reader currently publishes these as exact source shapes.
        // Reference origins are part of that shape, never inferred from a body.
        for (required, supplied) in candidate
            .required
            .structural_parameters
            .iter()
            .zip(&candidate.supplied.structural_parameters)
        {
            if required.qualifications != supplied.qualifications
                || required.projected_qualifications != supplied.projected_qualifications
                || required.additional_input_qualifications
                    != supplied.additional_input_qualifications
            {
                return Err(refuse(
                    "native conformance changes a structural formal qualification",
                ));
            }
        }
        if let (
            terminal_psi::TerminalMachineResult::Structural(required),
            terminal_psi::TerminalMachineResult::Structural(supplied),
        ) = (&candidate.required.result, &candidate.supplied.result)
            && (required.qualifications != supplied.qualifications
                || required.projected_qualifications != supplied.projected_qualifications
                || required.reference_sources != supplied.reference_sources)
        {
            return Err(refuse(
                "native conformance changes its result qualification or reference origin",
            ));
        }
        let required_view = view(&candidate.required);
        let goals = contract_refinement_goals(module, required_view, view(&candidate.supplied))
            .map_err(refuse)?;
        let proof_context = contract_refinement_context(required_view).map_err(refuse)?;
        let declarations = candidate
            .required
            .scalar_parameters
            .iter()
            .chain(&candidate.required.proof.erased_scalar_formals)
            .chain(candidate.required.result.scalar_ref())
            .map(|formal| (formal.id, formal.scalar_type))
            .collect::<Vec<_>>();
        for (position, goal) in goals.iter().enumerate() {
            let obligation = u64::try_from(position)
                .ok()
                .and_then(|position| position.checked_add(1))
                .and_then(ObligationId::new)
                .ok_or_else(|| {
                    refuse("native conformance goal namespace exceeds its coordinate width")
                })?;
            crate::derive_contract_obligation(
                &proof_context,
                obligation,
                &goal.proposition,
                &declarations,
                &mut Default::default(),
            )
            .map_err(|detail| {
                crate::refusal(
                    RefusalReason::Unproved(terminal_check_tables::UnprovedCause::NoDerivation),
                    context,
                    Construct::Machine,
                    detail,
                )
            })?;
        }
        let native_headers = module
            .boundary_machines
            .iter()
            .filter(|boundary| {
                boundary.declaration.as_ref().is_some_and(|declaration| {
                    declaration.source.machine == candidate.conformance.realization.machine
                })
            })
            .collect::<Vec<_>>();
        match (candidate.native_boundary, native_headers.as_slice()) {
            (None, []) => {}
            (Some(id), [boundary]) if id == boundary.id => {
                let actual = boundary
                    .declaration
                    .as_ref()
                    .ok_or_else(|| refuse("native call lost its exact source declaration"))?;
                if boundary.kind != semantic_vocabulary::BoundaryRequirementKind::Machine
                    || actual.source != candidate.conformance.realization
                    || boundary.completion != candidate.completion
                    || !header_matches_boundary(&candidate.supplied, boundary)
                    || boundary.published_service_ceiling
                        != candidate.supplied_envelope.service_reach
                {
                    return Err(refuse(
                        "native invocation header differs from its proven conformance",
                    ));
                }
            }
            _ => {
                return Err(refuse(
                    "native conformance lost its exact boundary header association",
                ));
            }
        }
        candidate
            .conformance
            .validate(&module.identities)
            .map_err(refuse)?;
        checked.push(candidate.conformance.clone());
    }
    checked.sort();
    if checked.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(refuse(
            "native conformance has duplicate exact source candidates",
        ));
    }
    elaboration.module.external_providers = checked;
    Ok(())
}
