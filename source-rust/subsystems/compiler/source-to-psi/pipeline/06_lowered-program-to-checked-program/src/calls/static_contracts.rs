//! Static declarations are formed without runtime facsimiles. Each selected
//! public implementation proves the required header in its positional scope.

use arena::IdTable;
use semantic_vocabulary::CallableCrashPolicy;
use semantic_vocabulary::{
    MachineId, ObligationId, PlaceId, PropositionContext, ServiceId, StructuralPlaceKind, ValueId,
};
use terminal_check_tables::contracts::{
    CallableHeader, StaticCallableEnvelope, StaticImplementation, StaticRequirementSchema,
    StaticServiceCeiling, StaticSignature as Signature, StaticValueType as ValueType,
};
use terminal_check_tables::program::Elaboration;
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason, RefusalSite};
use terminal_semantics::callable_envelopes::{
    CallableOperationalView, callable_operational_refines,
};
use terminal_semantics::calls::callable_refinement::{
    CallableContractView, CallableRefinementGoalKind, contract_refinement_context,
    contract_refinement_goals,
};

fn fault(site: impl Into<RefusalSite>, detail: &'static str) -> Refusal {
    Refusal::at(RefusalReason::ForeignIdentity, site.into(), detail)
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

pub(crate) fn check_envelope(
    module: &terminal_psi::TerminalModule,
    site: impl Into<RefusalSite>,
    envelope: &StaticCallableEnvelope,
    signature: Option<&Signature>,
) -> Checked<()> {
    let site = site.into();
    if let StaticServiceCeiling::Closed(row) = &envelope.service_reach {
        crate::ceilings::well_formed_reach(module, site, row)?;
    }
    if envelope
        .invocations
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(fault(
            site,
            "static invocation ceiling is repeated or unordered",
        ));
    }
    // Invocation ordinals name relevant runtime formals. The static signature
    // also keeps erased source formals; consume its runtime projection once.
    let mut formals = signature.map(|signature| {
        signature
            .parameters
            .iter()
            .enumerate()
            .filter(move |(position, _)| !signature.is_erased(*position))
    });
    let mut next_runtime = 0;
    for invocation in &envelope.invocations {
        match invocation {
            terminal_psi::TerminalCallableInvocation::Service(service)
                if module
                    .interfaces
                    .services
                    .iter()
                    .all(|declaration| declaration.id != *service) =>
            {
                return Err(fault(
                    site,
                    "static invocation ceiling names no declared service",
                ));
            }
            terminal_psi::TerminalCallableInvocation::Parameter(position) => {
                if let Some(signature) = signature {
                    let position = usize::try_from(*position).map_err(|_| {
                        fault(
                            site,
                            "static invocation ordinal exceeds its formal namespace",
                        )
                    })?;
                    let (source, value_type) = loop {
                        let formal = formals.as_mut().and_then(Iterator::next)
                            .ok_or_else(|| fault(site, "static invocation ceiling escapes its runtime formal namespace"))?;
                        let requested = next_runtime == position;
                        next_runtime += 1;
                        if requested {
                            break formal;
                        }
                    };
                    if !(signature.is_place(source)
                        || matches!(
                            value_type,
                            ValueType::Scalar(semantic_vocabulary::ScalarType::Callable(_))
                        ))
                    {
                        return Err(fault(
                            site,
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
        return Err(Refusal::at(
            RefusalReason::Unsupported,
            site,
            "static progress premises need retained declaration subjects",
        ));
    }
    Ok(())
}

/// Rejoin a prepared bodyless machine to the complete source contract that
/// introduced it. Applied headers sharing a symbol still have distinct IDs.
pub(crate) fn requirement_declarations(
    elaboration: &Elaboration,
) -> Checked<IdTable<MachineId, Option<&StaticRequirementSchema>>> {
    let module = &elaboration.module;
    let mut declarations = elaboration.requirement_ceilings.map(|_, _| None);
    for schema in &elaboration.static_schemas {
        for declaration in &schema.requirements {
            let Some(instance) = &declaration.instance else {
                continue;
            };
            let id = instance.machine;
            let target = declarations.get_mut(id).ok_or_else(|| {
                fault(
                    id,
                    "abstract declaration has no exact requirement coordinate",
                )
            })?;
            if target.replace(declaration).is_some() {
                return Err(fault(
                    id,
                    "abstract requirement repeats its source declaration",
                ));
            }
            let machine = module
                .machine(id)
                .ok_or_else(|| fault(id, "abstract declaration names no exact machine"))?;
            let header = &instance.header;
            if module
                .identities
                .text(instance.member)
                .is_none_or(str::is_empty)
            {
                return Err(fault(
                    id,
                    "abstract requirement lost its exact member identity",
                ));
            }
            let signature = declaration.signature.as_ref().ok_or_else(|| {
                fault(id, "abstract requirement instance has no source signature")
            })?;
            check_envelope(module, id, &declaration.envelope, Some(signature))?;
            let differs = [
                (
                    "abstract requirement has a different source declaration",
                    schema.declaration != declaration.declaration
                        || elaboration.machines.get(id) != Some(&declaration.declaration),
                ),
                (
                    "abstract requirement has a different source header coordinate",
                    header.proof.id != machine.contract.id,
                ),
                (
                    "abstract requirement has a different source parameter order",
                    header.parameter_order != machine.parameter_order,
                ),
                (
                    "abstract requirement has different source scalar formals",
                    header.scalar_parameters != machine.parameters,
                ),
                (
                    "abstract requirement has different source structural formals",
                    header.structural_parameters != machine.structural_parameters,
                ),
                (
                    "abstract requirement has a different source result",
                    header.result != machine.result,
                ),
                (
                    "abstract requirement has different source erased formals",
                    header.proof.erased_scalar_formals != machine.erased_scalar_formals,
                ),
                (
                    "abstract requirement has different source entry predicates",
                    header.proof.requires != machine.requires,
                ),
                (
                    "abstract requirement has different source result predicates",
                    !header
                        .proof
                        .ensures
                        .iter()
                        .map(|row| &row.proposition)
                        .eq(machine.ensures.iter().map(|row| &row.proposition)),
                ),
                (
                    "abstract requirement has different source result memberships",
                    header.proof.membership_ensures != machine.contract.membership_ensures,
                ),
                (
                    "abstract requirement has different source crash routes",
                    header.proof.declared_crash_routes != machine.declared_crash_routes,
                ),
                (
                    "abstract requirement has a different source service ceiling",
                    header.proof.declared_service_reach != machine.declared_service_reach,
                ),
                (
                    "abstract requirement has a different source entry ceiling",
                    header.proof.declared_synchronous_entries
                        != machine.declared_synchronous_entries,
                ),
                (
                    "abstract requirement has a different source suspension bound",
                    header.proof.declared_suspension != machine.declared_suspension,
                ),
                (
                    "abstract requirement has a different source blocking bound",
                    header.proof.declared_blocking != machine.declared_blocking,
                ),
                (
                    "abstract requirement has a different source progress guarantee",
                    header.proof.termination != machine.termination,
                ),
            ];
            if let Some((reason, _)) = differs.into_iter().find(|(_, different)| *different) {
                return Err(fault(id, reason));
            }
        }
    }
    Ok(declarations)
}

fn schema_context(
    module: &terminal_psi::TerminalModule,
    site: impl Into<RefusalSite>,
    signature: &Signature,
    abstract_types: u32,
) -> Checked<PropositionContext> {
    let site = site.into();
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
            site,
            "static signature has an invalid formal mode namespace",
        ));
    }
    let values = signature.formal_values();
    let mut scalars = Vec::new();
    let mut places = Vec::new();
    for (position, value_type) in signature.parameters.iter().enumerate() {
        if let ValueType::Rigid(rigid) = value_type {
            if rigid.0 >= abstract_types {
                return Err(fault(site, "abstract schema input has no declared type"));
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
                    fault(site, "static place namespace exceeds its coordinate width")
                })?;
            let position = u32::try_from(position)
                .map_err(|_| fault(site, "static formal ordinal exceeds u32"))?;
            places.push((place, StructuralPlaceKind::Parameter { position }));
        } else {
            // Predicates name the stored carrier. The complete signature keeps
            // the arithmetic policy which controls operations over that value.
            let scalar = value_type
                .scalar()
                .ok_or_else(|| fault(site, "static value formal has no exact scalar type"))?;
            module
                .execution
                .callables
                .validate_scalar_type(scalar)
                .map_err(|detail| fault(site, detail))?;
            scalars.push((
                values[position].ok_or_else(|| {
                    fault(site, "static scalar formal lost its exact value identity")
                })?,
                scalar,
            ));
        }
        if let ValueType::Record(record) = value_type
            && module
                .types
                .structural_types
                .iter()
                .all(|declared| declared.id != *record)
        {
            return Err(fault(
                site,
                "static signature names an undeclared structural carrier",
            ));
        }
    }
    match signature.result {
        Some(ValueType::Scalar(scalar)) => {
            module
                .execution
                .callables
                .validate_scalar_type(scalar)
                .map_err(|detail| fault(site, detail))?;
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
                    fault(site, "static result namespace exceeds its coordinate width")
                })?;
            scalars.push((result, scalar));
        }
        Some(ValueType::Record(record)) => {
            if module
                .types
                .structural_types
                .iter()
                .all(|declared| declared.id != record)
            {
                return Err(fault(site, "static result carrier is undeclared"));
            }
            let result = u64::try_from(places.len())
                .ok()
                .and_then(|n| n.checked_add(1))
                .and_then(PlaceId::new)
                .ok_or_else(|| {
                    fault(
                        site,
                        "static result place namespace exceeds its coordinate width",
                    )
                })?;
            places.push((result, StructuralPlaceKind::Result));
        }
        // An abstract result is opaque exactly like an abstract input above:
        // it supplies no observable scalar, so a predicate naming `result`
        // still fails scope validation below and the schema is refused.
        Some(ValueType::Rigid(rigid)) if rigid.0 >= abstract_types => {
            return Err(fault(site, "abstract schema result has no declared type"));
        }
        Some(ValueType::Rigid(_)) => {}
        None => {}
    }
    PropositionContext::from_value_types_and_places(scalars, places).map_err(|_| {
        fault(
            site,
            "static declaration has conflicting formal/result identities",
        )
    })
}

fn check_service_limit(
    site: RefusalSite,
    required: &StaticServiceCeiling,
    concrete: &[ServiceId],
    open: bool,
) -> Checked<()> {
    if let StaticServiceCeiling::Closed(ceiling) = required {
        if open {
            return Err(Refusal::at(
                RefusalReason::Conflict,
                site,
                "an open callback dependency cannot establish the static requirement's finite service ceiling",
            ));
        }
        if concrete
            .iter()
            .any(|service| ceiling.binary_search(service).is_err())
        {
            return Err(Refusal::at(
                RefusalReason::Conflict,
                site,
                "implementation exceeds the static requirement's authored service ceiling",
            ));
        }
    }
    Ok(())
}

/// The check form retains a generic body's fixed crash routes. A callback
/// with an open crash requirement can add routes when it is selected. Never
/// use that body's fixed rows alone to prove a closed receiving requirement.
fn check_deferred_callback_crashes(
    elaboration: &Elaboration,
    site: RefusalSite,
    application: &terminal_psi::static_reach::StaticReachArgument,
    required: &CallableHeader,
) -> Checked<()> {
    if required.crash_policy == CallableCrashPolicy::Open {
        return Ok(());
    }
    let covers = |cause| {
        required.proof.crash_routes.iter().any(|route| {
            route.cause == cause
                && route
                    .alternatives
                    .contains(&terminal_psi::CrashRouteGuard::Truth)
        })
    };
    let mut pending = vec![application];
    while let Some(application) = pending.pop() {
        let parameters = &elaboration.static_reach_parameters;
        let first = parameters.partition_point(|row| row.owner < application.selected);
        for parameter in parameters[first..]
            .iter()
            .take_while(|row| row.owner == application.selected)
        {
            let requirement = elaboration
                .module
                .machine(parameter.requirement)
                .ok_or_else(|| {
                    fault(
                        site,
                        "generic callback lost its originating crash requirement",
                    )
                })?;
            if requirement.contract.declared_crash_routes.is_some() {
                // Its authored routes already contribute through the body. Its
                // own refinement question checks the nested selection.
                continue;
            }
            let actual = application.arguments.iter().find(|actual| {
                actual.parameter == parameter.parameter
                    && actual.requirement == parameter.requirement
            });
            let Some(actual) = actual else {
                return Err(Refusal::at(
                    RefusalReason::Unsupported,
                    site,
                    "an open crash dependency has no supplied callback application",
                ));
            };
            let selected = elaboration
                .module
                .machine(actual.selected)
                .ok_or_else(|| fault(site, "nested callback lost its selected crash contract"))?;
            if matches!(elaboration.static_conformances.get(actual.conformance as usize).map(|question| question.implementation),
                Some(StaticImplementation::Parameter(_))) && selected.contract.declared_crash_routes.is_none() {
                return Err(Refusal::at(RefusalReason::Conflict, site,
                    "a forwarded open crash dependency cannot establish a finite crash requirement"));
            }
            if selected
                .contract
                .crash_routes
                .iter()
                .any(|route| !covers(route.cause))
            {
                return Err(Refusal::at(
                    RefusalReason::Unsupported,
                    site,
                    "the nested callback's crash routes need coverage at its invocation",
                ));
            }
            if selected.declaration.is_none() {
                pending.push(actual);
            }
        }
    }
    Ok(())
}

/// An executable body supplies its published contract. A generic checking
/// body supplies a contract only for its selected application, after the
/// call's own arguments have been substituted into its reach dependencies.
fn machine_operations<'a>(
    elaboration: &'a Elaboration,
    site: RefusalSite,
    body: &terminal_psi::TerminalMachine,
    argument: Option<&terminal_psi::static_reach::StaticReachArgument>,
    question: &terminal_check_tables::contracts::StaticConformanceQuestion,
) -> Checked<CallableOperationalView<'a>> {
    let limit = &question.envelope.service_reach;
    if body.declaration.is_some() {
        let runtime = elaboration
            .module
            .execution
            .callables
            .machine_envelope(body.id)
            .ok_or_else(|| {
                fault(
                    site,
                    "static implementation lost its published operational contract",
                )
            })?;
        check_service_limit(site, limit, &runtime.service_reach, false)?;
        return Ok(CallableOperationalView::from(runtime));
    }
    let generic = elaboration
        .generic_callable_declarations
        .binary_search_by_key(&body.id, |declaration| declaration.machine)
        .ok()
        .map(|index| &elaboration.generic_callable_declarations[index])
        .ok_or_else(|| fault(site, "static conformance names no genuine checked body"))?;
    if question.provider.is_some() {
        return Err(fault(
            site,
            "a generic checking body is not an executable provider",
        ));
    }
    let argument = argument
        .filter(|argument| argument.selected == body.id)
        .ok_or_else(|| fault(site, "generic callback lost its selecting application"))?;
    check_deferred_callback_crashes(elaboration, site, argument, &question.required)?;
    let summary = |machine| {
        elaboration
            .static_reach_summaries
            .binary_search_by_key(&machine, |row| row.machine)
            .ok()
            .map(|index| &elaboration.static_reach_summaries[index].term)
    };
    let reach =
        terminal_semantics::static_reach::substitute(&generic.reach, &argument.arguments, summary)
            .map_err(|reason| fault(site, reason))?;
    check_service_limit(site, limit, &reach.concrete, !reach.parameters.is_empty())?;
    Ok(operational(&generic.envelope))
}

pub(crate) fn admit(elaboration: &Elaboration) -> Checked<()> {
    if elaboration.static_schemas.is_empty() && elaboration.static_conformances.is_empty() {
        return Ok(());
    }
    let module = &elaboration.module;
    let mut bound = Vec::new();
    let mut instances = Vec::new();
    for schema in &elaboration.static_schemas {
        let site = RefusalSite::Declaration(schema.declaration);
        for binding in &schema.bindings {
            if !binding.parameter.is_valid()
                || !binding.owner.is_valid()
                || binding.application.is_empty()
                || bound.contains(&binding.parameter)
                || !elaboration.generic_binders.contains(&binding.parameter)
            {
                return Err(fault(
                    site,
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
                        site,
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
                            site,
                            "static record requirement instance changes its binder, arguments or logical callable",
                        ));
                    }
                    instances.push(instance.machine);
                    arguments.push(instance.arguments.clone());
                }
            }
        }
        for requirement in &schema.requirements {
            let site = RefusalSite::Declaration(requirement.declaration);
            // A template requirement's open signature is checked once in its
            // own generic check form; here it contributes only its envelope.
            let Some(signature) = &requirement.signature else {
                check_envelope(module, site, &requirement.envelope, None)?;
                continue;
            };
            let context = schema_context(module, site, signature, requirement.abstract_types)?;
            for proposition in signature.requires.iter().chain(&signature.ensures) {
                context.validate(proposition).map_err(|_| {
                    fault(
                        site,
                        "static declaration predicate escapes its exact typed formal/result scope",
                    )
                })?;
            }
            if let terminal_check_tables::contracts::CrashCeiling::Authored(routes) =
                &signature.crashes
            {
                for guard in routes.iter().flat_map(|route| &route.alternatives) {
                    if let terminal_psi::CrashRouteGuard::Predicate(predicate) = guard {
                        context.validate(predicate.proposition()).map_err(|_| {
                            fault(site, "static crash guard escapes its invocation scope")
                        })?;
                    }
                }
            }
            check_envelope(module, site, &requirement.envelope, Some(signature))?;
        }
    }
    // Retain the precise source application for generic callback questions.
    // Ordinals are shared with reach substitution; a question cannot acquire
    // two different tuples just because both select the same machine.
    let mut selections = vec![None; elaboration.static_conformances.len()];
    if !elaboration.generic_callable_declarations.is_empty() {
        let mut pending = elaboration
            .static_reach_applications
            .iter()
            .flat_map(|call| &call.arguments)
            .collect::<Vec<_>>();
        while let Some(argument) = pending.pop() {
            let slot = selections
                .get_mut(argument.conformance as usize)
                .ok_or_else(|| {
                    fault(
                        argument.selected,
                        "static callback lost its refinement question",
                    )
                })?;
            if slot
                .replace(argument)
                .is_some_and(|previous| previous != argument)
            {
                return Err(fault(
                    argument.selected,
                    "static callback refinement has conflicting applications",
                ));
            }
            pending.extend(&argument.arguments);
        }
    }
    let forwarded = elaboration
        .static_conformances
        .iter()
        .any(|question| matches!(question.implementation, StaticImplementation::Parameter(_)))
        .then(|| requirement_declarations(elaboration))
        .transpose()?;
    for (index, question) in elaboration.static_conformances.iter().enumerate() {
        let site = RefusalSite::Declaration(question.declaration);
        if question.application.is_empty() {
            return Err(fault(
                site,
                "static conformance lost its exact applied declaration identity",
            ));
        }
        check_envelope(module, site, &question.envelope, None)?;
        let (supplied, operations) = match question.implementation {
            StaticImplementation::Machine(id) => {
                let body = module
                    .machine(id)
                    .filter(|body| !body.blocks.is_empty())
                    .ok_or_else(|| {
                        fault(site, "static conformance names no genuine checked body")
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
                    machine_operations(elaboration, site, body, selections[index], question)?,
                )
            }
            StaticImplementation::Parameter(parameter) => {
                if question.provider.is_some() || elaboration.static_reach_parameters.binary_search(&parameter).is_err() {
                    return Err(fault(site, "forwarded callback has no originating parameter"));
                }
                let source = forwarded.as_ref().and_then(|rows| rows.get(parameter.requirement)).and_then(|row| *row)
                    .ok_or_else(|| fault(site, "forwarded callback lost its source requirement"))?;
                let instance = source.instance.as_ref()
                    .ok_or_else(|| fault(site, "forwarded callback lost its applied source header"))?;
                match &source.envelope.service_reach {
                    StaticServiceCeiling::Open => check_service_limit(site, &question.envelope.service_reach, &[], true)?,
                    StaticServiceCeiling::Closed(services) => check_service_limit(site, &question.envelope.service_reach, services, false)?,
                }
                (view(&instance.header), operational(&source.envelope))
            }
            StaticImplementation::Native { source, candidate } => {
                let candidate = elaboration
                    .external_provider_candidates
                    .get(candidate)
                    .filter(|candidate| {
                        candidate.source == source
                            && module
                                .interfaces
                                .external_providers
                                .contains(&candidate.conformance)
                    })
                    .ok_or_else(|| {
                        fault(
                            site,
                            "static conformance lost its checked native declaration",
                        )
                    })?;
                let envelope = &candidate.supplied_envelope;
                check_service_limit(
                    site,
                    &question.envelope.service_reach,
                    &envelope.service_reach,
                    false,
                )?;
                (
                    view(&candidate.supplied),
                    CallableOperationalView::from(envelope),
                )
            }
        };
        callable_operational_refines(operational(&question.envelope), operations)
            .map_err(|detail| Refusal::at(RefusalReason::Conflict, site, detail))?;
        let required = view(&question.required);
        let context =
            contract_refinement_context(required).map_err(|detail| fault(site, detail))?;
        let declarations = question
            .required
            .scalar_parameters
            .iter()
            .chain(&question.required.proof.erased_scalar_formals)
            .chain(question.required.result.scalar_ref())
            .map(|formal| (formal.id, formal.scalar_type))
            .collect::<Vec<_>>();
        let goals = contract_refinement_goals(module, required, supplied)
            .map_err(|detail| fault(site, detail))?;
        for (position, goal) in goals.iter().enumerate() {
            let obligation = u64::try_from(position)
                .ok()
                .and_then(|n| n.checked_add(1))
                .and_then(ObligationId::new)
                .ok_or_else(|| {
                    fault(
                        site,
                        "static refinement obligation namespace exceeds its coordinate width",
                    )
                })?;
            crate::derive_contract_obligation(
                site,
                &context,
                obligation,
                &goal.proposition,
                &declarations,
                &mut Default::default(),
            )
            .map_err(|mut refused| {
                if let StaticImplementation::Machine(id) = question.implementation
                    && let Some(names) = elaboration.names.get(id)
                {
                    let bound = match goal.kind {
                        CallableRefinementGoalKind::Input => "input precondition",
                        CallableRefinementGoalKind::NormalOutput => "normal return guarantee",
                        CallableRefinementGoalKind::GuardedOutput { .. } => {
                            "conditional return guarantee"
                        }
                        CallableRefinementGoalKind::Crash { .. } => "crash guarantee",
                    };
                    refused.detail = format!(
                        "cannot prove the {bound} of `{}`: {}",
                        names.machine, refused.detail
                    );
                    refused.source_span = names.declared;
                }
                refused
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
    if !module.interfaces.provider_candidates.is_empty() {
        return Err(fault(
            RefusalSite::Program,
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
            return Err(fault(
                RefusalSite::Declaration(question.declaration),
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
        let required_content =
            terminal_psi::BoundaryContentGuarantee::from_contract(&question.required.proof)
                .map_err(|reason| fault(id, reason))?;
        let conservation = boundary
            .content_guarantees
            .iter()
            .filter(|row| matches!(row, terminal_psi::BoundaryContentGuarantee::Conservation(_)));
        if conservation.ne(&required_content)
            || boundary.parameter_order != question.required.parameter_order
            || boundary.scalar_parameters
                != question
                    .required
                    .scalar_parameters
                    .iter()
                    .map(|formal| formal.scalar_type)
                    .collect::<Vec<_>>()
            || boundary.structural_parameters != question.required.structural_parameters
            || boundary.scalar_requires != question.required.proof.requires
            || boundary.scalar_ensures.iter().ne(question
                .required
                .proof
                .ensures
                .iter()
                .map(|clause| &clause.proposition)
                .filter(|row| {
                    !matches!(
                        row,
                        semantic_vocabulary::Proposition::ContentConservation(_)
                    )
                }))
            || boundary.membership_ensures != question.required.proof.membership_ensures
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
    for candidate in &candidates {
        terminal_semantics::provider_candidates::conservation::verify(module, candidate)
            .map_err(|reason| fault(candidate.candidate, reason))?;
    }
    module.interfaces.provider_candidates = candidates;
    external_completion_providers(module)
}

/// A proved provider candidate stands for its boundary requirement. One that
/// stands for a requirement ending the process must end it with the
/// arguments it received (`external_completions`). A provider that
/// passes another status breaks the language; a body that ends through a
/// form the judgment does not follow is only not served.
fn external_completion_providers(module: &terminal_psi::TerminalModule) -> Checked<()> {
    use terminal_semantics::external_completions::{ExternalRealizationFault, validate};
    let Err(error) = validate(module) else {
        return Ok(());
    };
    let (reason, concerns, detail) = match error.fault {
        ExternalRealizationFault::ChangedArguments { edge } => (
            RefusalReason::Violation,
            Construct::Edge(edge),
            "this machine provides a requirement that ends the process, and this call does not \
             pass on the arguments the machine received, unchanged and in order: a provider \
             cannot change what its caller asked for",
        ),
        ExternalRealizationFault::OtherKind { edge } => (
            RefusalReason::Violation,
            Construct::Edge(edge),
            "this machine provides a requirement that ends the process, and this call ends it \
             in a different way than the requirement declares",
        ),
        ExternalRealizationFault::Returns { block } => (
            RefusalReason::Violation,
            Construct::Block(block),
            "this machine provides a requirement that ends the process, and one of its paths \
             returns to the caller",
        ),
        ExternalRealizationFault::Unfollowed { block } => (
            RefusalReason::Unsupported,
            Construct::Block(block),
            "provider of a requirement that ends the process, whose body does not call the \
             ending requirement directly with scalar arguments",
        ),
    };
    Err(crate::refusal(reason, error.machine, concerns, detail))
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
