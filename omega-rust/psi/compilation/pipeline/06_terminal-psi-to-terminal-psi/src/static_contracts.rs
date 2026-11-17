//! Static declarations are formed without runtime facsimiles. Each selected
//! public implementation proves the required header in its positional scope.

use semantic_vocabulary::CallableCrashPolicy;
use semantic_vocabulary::{
    MachineId, ObligationId, PlaceId, PropositionContext, StructuralPlaceKind, ValueId,
};
use terminal_check_tables::contracts::{
    CallableHeader, StaticCallableEnvelope, StaticImplementation, StaticServiceCeiling,
    StaticSignature as Signature, StaticValueType as ValueType,
};
use terminal_check_tables::program::Elaboration;
use terminal_check_tables::{Checked, Construct, RefusalReason};
use terminal_semantics::callable_envelopes::{
    CallableOperationalView, callable_operational_refines,
};
use terminal_semantics::calls::callable_refinement::{
    CallableContractView, contract_refinement_context, contract_refinement_goals,
};

fn fault(machine: MachineId, detail: &'static str) -> terminal_check_tables::Refusal {
    crate::refusal(
        RefusalReason::ForeignIdentity,
        machine,
        Construct::Machine,
        detail,
    )
}
fn operational(envelope: &StaticCallableEnvelope) -> CallableOperationalView<'_> {
    CallableOperationalView {
        invocations: &envelope.invocations,
        suspends: envelope.suspends,
        blocks: envelope.blocks,
        termination: &envelope.termination,
    }
}
fn view(header: &CallableHeader) -> CallableContractView<'_> {
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

fn check_envelope(
    module: &terminal_psi::TerminalModule,
    machine: MachineId,
    envelope: &StaticCallableEnvelope,
    signature: Option<&Signature>,
) -> Checked<()> {
    if let StaticServiceCeiling::Closed(row) = &envelope.service_reach {
        crate::ceilings::well_formed_reach(module, machine, row)?;
    }
    if envelope
        .invocations
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(fault(
            machine,
            "static invocation ceiling is repeated or unordered",
        ));
    }
    for invocation in &envelope.invocations {
        match invocation {
            terminal_psi::TerminalCallableInvocation::Service(service)
                if module
                    .services
                    .iter()
                    .all(|declaration| declaration.id != *service) =>
            {
                return Err(fault(
                    machine,
                    "static invocation ceiling names no declared service",
                ));
            }
            terminal_psi::TerminalCallableInvocation::Parameter(position) => {
                if let Some(signature) = signature {
                    let position = usize::try_from(*position).map_err(|_| {
                        fault(
                            machine,
                            "static invocation ordinal exceeds its formal namespace",
                        )
                    })?;
                    if signature.parameters.get(position).is_none()
                        || !(signature.is_place(position)
                            || matches!(
                                signature.parameters[position],
                                ValueType::Scalar(semantic_vocabulary::ScalarType::Callable(_))
                            ))
                    {
                        return Err(fault(
                            machine,
                            "static invocation ceiling names no callable or inbound formal",
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    if let terminal_psi::TerminalCallableTermination::Terminates { premises } =
        &envelope.termination
        && !premises.is_empty()
    {
        return Err(crate::refusal(
            RefusalReason::Unsupported,
            machine,
            Construct::Machine,
            "static progress premises need retained declaration subjects",
        ));
    }
    Ok(())
}

fn schema_context(
    module: &terminal_psi::TerminalModule,
    machine: MachineId,
    signature: &Signature,
    abstract_types: u32,
) -> Checked<PropositionContext> {
    if signature.erased.windows(2).any(|pair| pair[0] >= pair[1])
        || signature
            .erased
            .iter()
            .any(|position| *position >= signature.parameters.len())
        || signature
            .borrowed
            .windows(2)
            .any(|pair| pair[0].position >= pair[1].position)
    {
        return Err(fault(
            machine,
            "static signature has an invalid formal mode namespace",
        ));
    }
    let values = signature.formal_values();
    let mut scalars = Vec::new();
    let mut places = Vec::new();
    for (position, value_type) in signature.parameters.iter().enumerate() {
        if let ValueType::Rigid(rigid) = value_type {
            if rigid.0 >= abstract_types {
                return Err(fault(machine, "abstract schema input has no declared type"));
            }
            if signature.borrowed(position).is_none() {
                // An opaque logical input occupies a formal value ID, but
                // supplies neither an observable scalar nor borrowed storage.
                // Any predicate trying to observe it must still fail scope
                // validation; closed applications check their actual loans.
                continue;
            }
        }
        if signature.is_place(position) {
            let place = u64::try_from(places.len())
                .ok()
                .and_then(|n| n.checked_add(1))
                .and_then(PlaceId::new)
                .ok_or_else(|| {
                    fault(
                        machine,
                        "static place namespace exceeds its coordinate width",
                    )
                })?;
            let position = u32::try_from(position)
                .map_err(|_| fault(machine, "static formal ordinal exceeds u32"))?;
            places.push((place, StructuralPlaceKind::Parameter { position }));
        } else {
            // Predicates name the stored carrier. The complete signature keeps
            // the arithmetic policy which controls operations over that value.
            let scalar = value_type
                .scalar()
                .ok_or_else(|| fault(machine, "static value formal has no exact scalar type"))?;
            module
                .callables
                .validate_scalar_type(scalar)
                .map_err(|detail| fault(machine, detail))?;
            scalars.push((
                values[position].ok_or_else(|| {
                    fault(
                        machine,
                        "static scalar formal lost its exact value identity",
                    )
                })?,
                scalar,
            ));
        }
        if let ValueType::Record(record) = value_type
            && module
                .structural_types
                .iter()
                .all(|declared| declared.id != *record)
        {
            return Err(fault(
                machine,
                "static signature names an undeclared structural carrier",
            ));
        }
    }
    match signature.result {
        Some(ValueType::Scalar(scalar)) => {
            module
                .callables
                .validate_scalar_type(scalar)
                .map_err(|detail| fault(machine, detail))?;
            let relevant = signature
                .parameters
                .iter()
                .enumerate()
                .filter(|(position, value)| {
                    value.is_value()
                        && !signature.is_erased(*position)
                        && !signature.is_place(*position)
                })
                .count();
            let result = u64::try_from(relevant)
                .ok()
                .and_then(|n| n.checked_add(1))
                .and_then(ValueId::new)
                .ok_or_else(|| {
                    fault(
                        machine,
                        "static result namespace exceeds its coordinate width",
                    )
                })?;
            scalars.push((result, scalar));
        }
        Some(ValueType::Record(record)) => {
            if module
                .structural_types
                .iter()
                .all(|declared| declared.id != record)
            {
                return Err(fault(machine, "static result carrier is undeclared"));
            }
            let result = u64::try_from(places.len())
                .ok()
                .and_then(|n| n.checked_add(1))
                .and_then(PlaceId::new)
                .ok_or_else(|| {
                    fault(
                        machine,
                        "static result place namespace exceeds its coordinate width",
                    )
                })?;
            places.push((result, StructuralPlaceKind::Result));
        }
        Some(ValueType::Rigid(_)) => {
            return Err(crate::refusal(
                RefusalReason::Unsupported,
                machine,
                Construct::Machine,
                "abstract schema result needs a general logical value carrier",
            ));
        }
        None => {}
    }
    PropositionContext::from_value_types_and_places(scalars, places).map_err(|_| {
        fault(
            machine,
            "static declaration has conflicting formal/result identities",
        )
    })
}

pub(crate) fn admit(elaboration: &Elaboration) -> Checked<()> {
    if elaboration.static_schemas.is_empty() && elaboration.static_conformances.is_empty() {
        return Ok(());
    }
    let machine = elaboration
        .module
        .machines
        .first()
        .map(|machine| machine.id)
        .ok_or_else(|| {
            fault(
                MachineId::new(1).expect("one is nonzero"),
                "static questions have no real diagnostic context",
            )
        })?;
    let module = &elaboration.module;
    let mut bound = Vec::new();
    let mut instances = Vec::new();
    for schema in &elaboration.static_schemas {
        for binding in &schema.bindings {
            if !binding.parameter.is_valid()
                || !binding.owner.is_valid()
                || binding.application.is_empty()
                || bound.contains(&binding.parameter)
                || !elaboration.generic_binders.contains(&binding.parameter)
            {
                return Err(fault(
                    machine,
                    "static record has an invalid, repeated or unquantified binder account",
                ));
            }
            bound.push(binding.parameter);
            let mut members = Vec::new();
            for member in &binding.members {
                if member.application.is_empty()
                    || !member.signature.is_valid()
                    || members.contains(&member.requirement)
                    || !elaboration.static_schemas.iter().any(|schema| {
                        schema.declaration == member.declaring_trait
                            && schema
                                .requirements
                                .iter()
                                .any(|requirement| requirement.declaration == member.requirement)
                    })
                {
                    return Err(fault(
                        machine,
                        "static record member escapes its complete originating schema",
                    ));
                }
                members.push(member.requirement);
                let mut arguments = Vec::new();
                for instance in &member.instances {
                    if instances.contains(&instance.machine)
                        || arguments.contains(&instance.arguments)
                        || !elaboration.requirements.contains(&instance.machine)
                        || elaboration.machines.get(instance.machine) != Some(&member.signature)
                        || elaboration.value_arguments.get(instance.machine)
                            != Some(&instance.arguments)
                        || elaboration
                            .module
                            .machine(instance.machine)
                            .is_none_or(|machine| !machine.blocks.is_empty())
                    {
                        return Err(fault(
                            machine,
                            "static record requirement instance changes its binder, arguments or logical callable",
                        ));
                    }
                    instances.push(instance.machine);
                    arguments.push(instance.arguments.clone());
                }
            }
        }
        for requirement in &schema.requirements {
            let context = schema_context(
                module,
                machine,
                &requirement.signature,
                requirement.abstract_types,
            )?;
            for proposition in requirement
                .signature
                .requires
                .iter()
                .chain(&requirement.signature.ensures)
            {
                context.validate(proposition).map_err(|_| {
                    fault(
                        machine,
                        "static declaration predicate escapes its exact typed formal/result scope",
                    )
                })?;
            }
            if let terminal_check_tables::contracts::CrashCeiling::Authored(routes) =
                &requirement.signature.crashes
            {
                for guard in routes.iter().flat_map(|route| &route.alternatives) {
                    if let terminal_psi::CrashRouteGuard::Predicate(predicate) = guard {
                        context.validate(predicate.proposition()).map_err(|_| {
                            fault(machine, "static crash guard escapes its invocation scope")
                        })?;
                    }
                }
            }
            check_envelope(
                module,
                machine,
                &requirement.envelope,
                Some(&requirement.signature),
            )?;
        }
    }
    for question in &elaboration.static_conformances {
        if question.application.is_empty() {
            return Err(fault(
                machine,
                "static conformance lost its exact applied declaration identity",
            ));
        }
        check_envelope(module, machine, &question.envelope, None)?;
        let (supplied, envelope) = match question.implementation {
            StaticImplementation::Machine(id) => {
                let body = module
                    .machine(id)
                    .filter(|body| body.declaration.is_some() && !body.blocks.is_empty())
                    .ok_or_else(|| {
                        fault(machine, "static conformance names no genuine checked body")
                    })?;
                let envelope = module.callables.machine_envelope(id).ok_or_else(|| {
                    fault(
                        machine,
                        "static implementation lost its published operational contract",
                    )
                })?;
                (
                    CallableContractView {
                        parameter_order: &body.parameter_order,
                        scalar_parameters: &body.parameters,
                        structural_parameters: &body.structural_parameters,
                        result: &body.result,
                        proof: &body.contract,
                        evidence_owner: Some(terminal_psi::EvidenceCallableOwner::Machine(id)),
                        crash_policy: CallableCrashPolicy::Closed,
                    },
                    envelope,
                )
            }
            StaticImplementation::Native { source, candidate } => {
                let candidate = elaboration
                    .external_provider_candidates
                    .get(candidate)
                    .filter(|candidate| {
                        candidate.source == source
                            && module.external_providers.contains(&candidate.conformance)
                    })
                    .ok_or_else(|| {
                        fault(
                            machine,
                            "static conformance lost its checked native declaration",
                        )
                    })?;
                (view(&candidate.supplied), &candidate.supplied_envelope)
            }
        };
        if let StaticServiceCeiling::Closed(ceiling) = &question.envelope.service_reach
            && envelope
                .service_reach
                .iter()
                .any(|service| ceiling.binary_search(service).is_err())
        {
            return Err(crate::refusal(
                RefusalReason::Conflict,
                machine,
                Construct::Machine,
                "implementation exceeds the static requirement's authored service ceiling",
            ));
        }
        callable_operational_refines(
            operational(&question.envelope),
            CallableOperationalView::from(envelope),
        )
        .map_err(|detail| {
            crate::refusal(RefusalReason::Conflict, machine, Construct::Machine, detail)
        })?;
        let required = view(&question.required);
        let context =
            contract_refinement_context(required).map_err(|detail| fault(machine, detail))?;
        let declarations = question
            .required
            .scalar_parameters
            .iter()
            .chain(&question.required.proof.erased_scalar_formals)
            .chain(question.required.result.scalar_ref())
            .map(|formal| (formal.id, formal.scalar_type))
            .collect::<Vec<_>>();
        let goals = contract_refinement_goals(module, required, supplied)
            .map_err(|detail| fault(machine, detail))?;
        for (position, goal) in goals.iter().enumerate() {
            let obligation = u64::try_from(position)
                .ok()
                .and_then(|n| n.checked_add(1))
                .and_then(ObligationId::new)
                .ok_or_else(|| {
                    fault(
                        machine,
                        "static refinement obligation namespace exceeds its coordinate width",
                    )
                })?;
            crate::derive_contract_obligation(
                &context,
                obligation,
                &goal.proposition,
                &declarations,
                &mut Default::default(),
            )
            .map_err(|detail| {
                crate::refusal(
                    RefusalReason::Unproved(terminal_check_tables::UnprovedCause::NoDerivation),
                    machine,
                    Construct::Machine,
                    detail,
                )
            })?;
        }
    }
    Ok(())
}

/// Form the final catalog once, after the complete compilation discharged
/// supplied-body witnesses and this owner proved its static questions.
/// Candidate retention supplies metadata and creates no invocation edge.
pub(crate) fn publish(elaboration: &mut Elaboration) -> Checked<()> {
    admit(elaboration)?;
    let module = &mut elaboration.module;
    if !module.provider_candidates.is_empty() {
        let context = module.machines.first().map(|row| row.id).ok_or_else(|| {
            fault(
                MachineId::new(1).expect("one is nonzero"),
                "provider catalog has no diagnostic context",
            )
        })?;
        return Err(fault(
            context,
            "static provider candidates were already published",
        ));
    }
    let mut identities = std::mem::take(&mut module.identities).into_builder();
    let mut candidates = std::mem::take(&mut elaboration.provider_candidate_intents);
    for question in &elaboration.static_conformances {
        let Some(source) = &question.provider else {
            continue;
        };
        let StaticImplementation::Machine(id) = question.implementation else {
            let context = module.machines.first().map(|row| row.id).ok_or_else(|| {
                fault(
                    MachineId::new(1).expect("one is nonzero"),
                    "static provider has no diagnostic context",
                )
            })?;
            return Err(fault(
                context,
                "static provider candidate is not an executable declaration",
            ));
        };
        let body = module
            .machine(id)
            .ok_or_else(|| fault(id, "static provider body disappeared after admission"))?;
        let boundary = module.boundary_machine(source.boundary).ok_or_else(|| {
            fault(
                id,
                "static provider requirement disappeared after admission",
            )
        })?;
        if boundary.parameter_order != question.required.parameter_order
            || boundary.scalar_parameters
                != question
                    .required
                    .scalar_parameters
                    .iter()
                    .map(|formal| formal.scalar_type)
                    .collect::<Vec<_>>()
            || boundary.structural_parameters != question.required.structural_parameters
            || boundary.scalar_requires != question.required.proof.requires
            || boundary.scalar_ensures
                != question
                    .required
                    .proof
                    .ensures
                    .iter()
                    .map(|clause| clause.proposition.clone())
                    .collect::<Vec<_>>()
            || boundary.crash_routes != question.required.proof.crash_routes
            || question.required.crash_policy != CallableCrashPolicy::Closed
            || !match (&boundary.result, &question.required.result) {
                (
                    terminal_psi::BoundaryMachineResult::Unit,
                    terminal_psi::TerminalMachineResult::Unit,
                ) => true,
                (
                    terminal_psi::BoundaryMachineResult::Scalar(expected),
                    terminal_psi::TerminalMachineResult::Scalar(actual),
                ) => *expected == actual.value_type(),
                (
                    terminal_psi::BoundaryMachineResult::Structural(expected),
                    terminal_psi::TerminalMachineResult::Structural(actual),
                ) => {
                    expected.structural_type == actual.structural_type
                        && expected.multiplicity == actual.multiplicity
                        && expected.qualifications == actual.qualifications
                        && expected.projected_qualifications == actual.projected_qualifications
                        && actual.reference_sources.is_empty()
                }
                _ => false,
            }
        {
            return Err(fault(
                id,
                "static provider publication changes its proved required header",
            ));
        }
        let declared = body
            .declaration
            .as_ref()
            .ok_or_else(|| fault(id, "static provider body lost its checked declaration"))?;
        if identities
            .text(source.declaration.identity())
            .is_none_or(|identity| identity.is_empty())
            || source.source_requirement.is_empty()
            || source.source_candidate.is_empty()
            || source.provider_identity.is_empty()
            || question.application.is_empty()
        {
            return Err(fault(
                id,
                "static provider lost an exact source or application identity",
            ));
        }
        let declaration = source.declaration.identity();
        let application = identities.intern(&question.application);
        candidates.push(terminal_psi::ProviderCandidateConformance {
            boundary: source.boundary,
            requirement_identity: boundary.identity,
            source_requirement_identity: identities.intern(&source.source_requirement),
            declaration_application: application,
            receiving_application: application,
            receiving_region_arguments: Vec::new(),
            provider_identity: identities.intern(&source.provider_identity),
            provider_declaration: terminal_psi::NominalDeclarationIdentity::new(declaration),
            candidate_identity: declared.callable,
            source_candidate_identity: identities.intern(&source.source_candidate),
            source_candidate_package_identity: source.package,
            candidate: id,
            signature: terminal_psi::ProviderSignature {
                parameters: boundary
                    .structural_parameters
                    .iter()
                    .map(|parameter| terminal_psi::ProviderSignatureParameter {
                        position: parameter.position,
                        structural_type: parameter.structural_type,
                        multiplicity: parameter.multiplicity,
                        access: parameter.access,
                        qualifications: parameter.qualifications.clone(),
                        projected_qualifications: parameter.projected_qualifications.clone(),
                    })
                    .collect(),
            },
            refinement: terminal_psi::ProviderRefinement {
                positional_parameters: (0..boundary.structural_parameters.len())
                    .map(|index| {
                        let index = u32::try_from(index).map_err(|_| {
                            fault(id, "provider structural arity exceeds its coordinate width")
                        })?;
                        Ok(terminal_psi::ProviderParameterRefinement {
                            boundary_index: index,
                            candidate_index: index,
                        })
                    })
                    .collect::<Checked<Vec<_>>>()?,
                required_domains: boundary.requires.clone(),
                realized_service_ceiling: body.published_service_ceiling.clone(),
            },
        });
    }
    order_provider_candidates(&identities, &mut candidates)?;
    module.identities = identities.build();
    module.provider_candidates = candidates;
    Ok(())
}

fn order_provider_candidates(
    identities: &terminal_psi::TerminalIdentityTableBuilder,
    candidates: &mut [terminal_psi::ProviderCandidateConformance],
) -> Checked<()> {
    // Portable order names the identities, not their insertion coordinates.
    // Borrow the strings so sorting neither copies them nor builds a second
    // identity index. Validate first; a missing identity is never an empty key.
    let key = |row: &terminal_psi::ProviderCandidateConformance| {
        Some((
            row.boundary,
            identities
                .text(row.provider_identity)
                .filter(|text| !text.is_empty())?,
            identities
                .text(row.provider_declaration.identity())
                .filter(|text| !text.is_empty())?,
            identities
                .text(row.candidate_identity)
                .filter(|text| !text.is_empty())?,
            row.candidate,
        ))
    };
    for candidate in candidates.iter() {
        if key(candidate).is_none() {
            return Err(fault(
                candidate.candidate,
                "static provider lost its canonical conformance identity",
            ));
        }
    }
    candidates.sort_unstable_by_key(&key);
    if let Some(pair) = candidates
        .windows(2)
        .find(|pair| key(&pair[0]) == key(&pair[1]))
    {
        return Err(fault(
            pair[0].candidate,
            "static provider publication repeats an exact candidate",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod provider_order_tests {
    use super::order_provider_candidates;
    use terminal_psi::{
        NominalDeclarationIdentity, ProviderCandidateConformance, ProviderRefinement,
        ProviderSignature, TerminalIdentityIndex, TerminalIdentityTable,
    };

    fn candidate(
        provider: TerminalIdentityIndex,
        declaration: TerminalIdentityIndex,
        callable: TerminalIdentityIndex,
    ) -> ProviderCandidateConformance {
        ProviderCandidateConformance {
            boundary: semantic_vocabulary::BoundaryMachineId::new(1).unwrap(),
            requirement_identity: callable,
            source_requirement_identity: callable,
            declaration_application: declaration,
            receiving_application: declaration,
            receiving_region_arguments: Vec::new(),
            provider_identity: provider,
            provider_declaration: NominalDeclarationIdentity::new(declaration),
            candidate_identity: callable,
            source_candidate_identity: callable,
            source_candidate_package_identity: None,
            candidate: semantic_vocabulary::MachineId::new(1).unwrap(),
            signature: ProviderSignature::empty(),
            refinement: ProviderRefinement::empty(),
        }
    }

    #[test]
    fn provider_order_uses_text_at_each_identity_coordinate() {
        let mut identities = TerminalIdentityTable::default().into_builder();
        let z = identities.intern("z");
        let a = identities.intern("a");
        let rows = [
            candidate(a, z, a),
            candidate(z, a, a),
            candidate(a, a, z),
            candidate(a, a, a),
        ];
        let expected = [
            rows[3].clone(),
            rows[2].clone(),
            rows[0].clone(),
            rows[1].clone(),
        ];
        let mut rows = rows;
        order_provider_candidates(&identities, &mut rows).unwrap();
        assert_eq!(rows, expected);
        rows.reverse();
        order_provider_candidates(&identities, &mut rows).unwrap();
        assert_eq!(rows, expected);
    }

    #[test]
    fn duplicate_provider_keys_refuse_even_when_other_metadata_differs() {
        let mut identities = TerminalIdentityTable::default().into_builder();
        let identity = identities.intern("provider");
        let first = candidate(identity, identity, identity);
        for change_metadata in [false, true] {
            let mut second = first.clone();
            if change_metadata {
                second.source_requirement_identity = identities.intern("different source");
            }
            let refusal =
                order_provider_candidates(&identities, &mut [first.clone(), second]).unwrap_err();
            assert_eq!(
                refusal.detail,
                "static provider publication repeats an exact candidate"
            );
        }
    }

    #[test]
    fn missing_or_empty_provider_key_identities_refuse_before_sorting() {
        let mut identities = TerminalIdentityTable::default().into_builder();
        let identity = identities.intern("provider");
        for absent in [TerminalIdentityIndex(0), TerminalIdentityIndex(u32::MAX)] {
            for row in [
                candidate(absent, identity, identity),
                candidate(identity, absent, identity),
                candidate(identity, identity, absent),
            ] {
                let refusal = order_provider_candidates(&identities, &mut [row]).unwrap_err();
                assert_eq!(
                    refusal.detail,
                    "static provider lost its canonical conformance identity"
                );
            }
        }
    }
}
