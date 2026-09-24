//! Reconstruct scalar membership at every definition and transport boundary.
//!
//! Declarations carry locally defined, obligation-free tags. Ordinary values
//! cannot implicitly gain or lose them: only a catalogued successor binding
//! introduces or erases membership, creating a distinct SSA value without an opcode.
use crate::validation::{
    BTreeMap, BTreeSet, MachineId, ModuleError, OperationKind, TerminalMachine, TerminalModule,
    Terminator, ValueId,
};
use semantic_vocabulary::{
    Proposition, ScalarDomainId, ScalarQualificationSetId, ScalarTerm, ScalarType,
};
use terminal_psi::{
    ScalarDomainEstablishmentRoute, ScalarFloatRange, ScalarIntegerRange,
    ScalarQualificationCoercion, ValueDeclaration,
};

fn invalid(reason: &'static str) -> ModuleError {
    ModuleError::InvalidScalarQualification(reason)
}

fn invalid_float_range(
    machine: MachineId,
    parameter: ValueId,
    reason: &'static str,
) -> ModuleError {
    ModuleError::InvalidScalarFloatRange {
        machine,
        parameter,
        reason,
    }
}

fn invalid_integer_range(
    machine: MachineId,
    parameter: ValueId,
    reason: &'static str,
) -> ModuleError {
    ModuleError::InvalidScalarIntegerRange {
        machine,
        parameter,
        reason,
    }
}

pub(in crate::validation) fn declarations(
    machine: &TerminalMachine,
) -> impl Iterator<Item = &ValueDeclaration> {
    machine
        .parameters
        .iter()
        .chain(machine.result.scalar_ref())
        .chain(machine.blocks.iter().flat_map(|block| {
            block.parameters.iter().chain(
                block
                    .operations
                    .iter()
                    .filter_map(|operation| operation.result.scalar_ref()),
            )
        }))
}

pub(in crate::validation) fn validate(module: &TerminalModule) -> Result<(), ModuleError> {
    let catalog = &module.scalar_qualifications;
    // Retained authored floating ranges are closed delivery requirements on
    // direct scalar parameters: canonically ordered by `(machine, parameter)`,
    // attached to an existing machine and one of its own parameters, and
    // carrier-exact — the endpoints retain the parameter's declared IEEE
    // format and must IEEE-order. Fail closed on any malformed row.
    let mut float_ranges: BTreeMap<(MachineId, ValueId), &ScalarFloatRange> = BTreeMap::new();
    let mut previous_range = None;
    for range in &catalog.float_entry_ranges {
        let key = (range.machine, range.parameter);
        if previous_range.is_some_and(|previous| previous >= key) {
            return Err(invalid_float_range(
                range.machine,
                range.parameter,
                "noncanonical scalar float entry ranges",
            ));
        }
        previous_range = Some(key);
        let owner = module
            .machines
            .iter()
            .find(|machine| machine.id == range.machine)
            .ok_or_else(|| {
                invalid_float_range(
                    range.machine,
                    range.parameter,
                    "unknown range owner machine",
                )
            })?;
        let parameter = owner
            .parameters
            .iter()
            .find(|parameter| parameter.id == range.parameter)
            .ok_or_else(|| {
                invalid_float_range(
                    range.machine,
                    range.parameter,
                    "range is not attached to a direct scalar parameter",
                )
            })?;
        if !range.ordered() {
            return Err(invalid_float_range(
                range.machine,
                range.parameter,
                "endpoints do not share the declared format or are not IEEE ordered",
            ));
        }
        if parameter.scalar_type != ScalarType::IeeeFloat(range.format()) {
            return Err(invalid_float_range(
                range.machine,
                range.parameter,
                "range endpoints do not retain the parameter's declared IEEE format",
            ));
        }
        float_ranges.insert(key, range);
    }
    // Retained authored integer ranges are closed delivery requirements on
    // direct scalar parameters, ordered and owned exactly like the floating
    // rows. The carrier must be the parameter's declared fixed-width integer
    // type — an address carrier is not an entry-range carrier — and both
    // inclusive endpoints must be admitted and ordered under its signedness.
    // Unlike a floating row the same bounds also publish as `requires`
    // propositions on the owner contract: `LTE(minimum, parameter)` and
    // `LTE(parameter, maximum)`. A row whose owner does not publish both
    // conjuncts claims a bound no call edge replays, so it fails closed.
    let mut integer_ranges: BTreeMap<(MachineId, ValueId), &ScalarIntegerRange> = BTreeMap::new();
    let mut previous_integer = None;
    for range in &catalog.integer_entry_ranges {
        let key = (range.machine, range.parameter);
        if previous_integer.is_some_and(|previous| previous >= key) {
            return Err(invalid_integer_range(
                range.machine,
                range.parameter,
                "noncanonical scalar integer entry ranges",
            ));
        }
        previous_integer = Some(key);
        let owner = module
            .machines
            .iter()
            .find(|machine| machine.id == range.machine)
            .ok_or_else(|| {
                invalid_integer_range(
                    range.machine,
                    range.parameter,
                    "unknown range owner machine",
                )
            })?;
        let parameter = owner
            .parameters
            .iter()
            .find(|parameter| parameter.id == range.parameter)
            .ok_or_else(|| {
                invalid_integer_range(
                    range.machine,
                    range.parameter,
                    "range is not attached to a direct scalar parameter",
                )
            })?;
        if !range.ordered() {
            return Err(invalid_integer_range(
                range.machine,
                range.parameter,
                "range carrier is an address or the endpoints are not admitted and ordered",
            ));
        }
        if parameter.scalar_type != ScalarType::Integer(range.integer_type) {
            return Err(invalid_integer_range(
                range.machine,
                range.parameter,
                "range carrier does not retain the parameter's declared integer type",
            ));
        }
        let minimum = ScalarTerm::integer(range.integer_type, range.minimum).map_err(|_| {
            invalid_integer_range(
                range.machine,
                range.parameter,
                "range minimum is outside its declared carrier",
            )
        })?;
        let maximum = ScalarTerm::integer(range.integer_type, range.maximum).map_err(|_| {
            invalid_integer_range(
                range.machine,
                range.parameter,
                "range maximum is outside its declared carrier",
            )
        })?;
        let subject = ScalarTerm::value(parameter.id, parameter.scalar_type);
        let published = |expected: &Proposition| {
            let mut pending: Vec<&Proposition> = owner.contract.requires.iter().collect();
            while let Some(proposition) = pending.pop() {
                match proposition {
                    Proposition::Conjunction(terms) => pending.extend(terms.iter()),
                    proposition if proposition == expected => return true,
                    _ => {}
                }
            }
            false
        };
        if !published(&Proposition::LessOrEqual(minimum, subject.clone()))
            || !published(&Proposition::LessOrEqual(subject, maximum))
        {
            return Err(invalid_integer_range(
                range.machine,
                range.parameter,
                "range bounds are not published as owner requires propositions",
            ));
        }
        integer_ranges.insert(key, range);
    }
    // Boundary scalar signatures currently carry only payload types. A
    // provider installation cannot supply or forget membership — or a retained
    // floating or integer range — through that older interface, even when its
    // in-module body has a valid signature.
    for provider in &module.provider_candidates {
        if module
            .machines
            .iter()
            .find(|machine| machine.id == provider.candidate)
            .is_some_and(|machine| {
                machine
                    .parameters
                    .iter()
                    .chain(machine.result.scalar_ref())
                    .any(|value| !value.qualifications.is_empty())
                    || machine.parameters.iter().any(|parameter| {
                        float_ranges.contains_key(&(machine.id, parameter.id))
                            || integer_ranges.contains_key(&(machine.id, parameter.id))
                    })
            })
        {
            return Err(invalid("qualified scalar provider boundary is unsupported"));
        }
    }
    // Retained issuer routes resolve against the module's own issuer rows in
    // the same normalized identity vocabularies the producer emits: boundary
    // requirement routes join the boundary declarations and the provider
    // conformance rows that name those requirements; checked requirement
    // routes join the provider rows and the closed conformance and dynamic
    // dispatch identities; exact-machine routes join the callable identities
    // provider candidates, conformance realizations, dispatch rows, proof
    // output calls, and proof recursion members carry. A route that names no
    // retained issuer fails closed — it cannot borrow authority a private
    // issuer never published into this artifact.
    let mut boundary_issuers = BTreeSet::new();
    let mut requirement_issuers = BTreeSet::new();
    let mut machine_issuers = BTreeSet::new();
    // The same rows also bind every issuer identity to the dense machine
    // that supplies it. An establishment route authorizes a coercion only
    // inside the machine those rows name: a consumer that merely calls an
    // admitted requirement cannot mint membership inside its own body.
    let mut machine_boundaries: BTreeMap<MachineId, BTreeSet<&str>> = BTreeMap::new();
    let mut machine_requirements: BTreeMap<MachineId, BTreeSet<&str>> = BTreeMap::new();
    let mut machine_identities: BTreeMap<MachineId, BTreeSet<&str>> = BTreeMap::new();
    let boundary_identities: BTreeMap<_, &str> = module
        .boundary_machines
        .iter()
        .map(|declaration| (declaration.id, declaration.identity.as_str()))
        .collect();
    for declaration in &module.boundary_machines {
        boundary_issuers.insert(declaration.identity.as_str());
    }
    for provider in &module.provider_candidates {
        boundary_issuers.insert(provider.requirement_identity.as_str());
        requirement_issuers.insert(provider.requirement_identity.as_str());
        machine_issuers.insert(provider.candidate_identity.as_str());
        let boundaries = machine_boundaries.entry(provider.candidate).or_default();
        boundaries.insert(provider.requirement_identity.as_str());
        if let Some(identity) = boundary_identities.get(&provider.boundary) {
            boundaries.insert(identity);
        }
        machine_requirements
            .entry(provider.candidate)
            .or_default()
            .insert(provider.requirement_identity.as_str());
        machine_identities
            .entry(provider.candidate)
            .or_default()
            .insert(provider.candidate_identity.as_str());
    }
    for application in &module.closed_conformance_applications {
        for callable in &application.realization_callables {
            machine_issuers.insert(callable.source_callable_identity.as_str());
            machine_identities
                .entry(callable.machine)
                .or_default()
                .insert(callable.source_callable_identity.as_str());
        }
        for row in &application.rows {
            requirement_issuers.insert(row.public_requirement_identity.as_str());
            requirement_issuers.insert(row.requirement_identity.as_str());
            machine_issuers.insert(row.realization_identity.as_str());
            if let Some(callable) = &row.realization_callable_identity {
                machine_issuers.insert(callable.as_str());
                if let Some(machine) = application
                    .realization_callables
                    .iter()
                    .find(|entry| entry.source_callable_identity == *callable)
                    .map(|entry| entry.machine)
                {
                    let requirements = machine_requirements.entry(machine).or_default();
                    requirements.insert(row.public_requirement_identity.as_str());
                    requirements.insert(row.requirement_identity.as_str());
                    let identities = machine_identities.entry(machine).or_default();
                    identities.insert(row.realization_identity.as_str());
                    identities.insert(callable.as_str());
                }
            }
        }
    }
    for parameter in &module.dynamic_dispatch.parameters {
        for requirement in &parameter.requirements {
            requirement_issuers.insert(requirement.public_requirement_identity.as_str());
        }
    }
    for dispatch in module
        .dynamic_dispatch
        .direct_dispatches
        .iter()
        .map(|dispatch| {
            (
                dispatch.public_requirement_identity.as_str(),
                dispatch.requirement_identity.as_str(),
                dispatch.realization_identity.as_str(),
                dispatch.realization_callable_identity.as_str(),
                dispatch.realization,
            )
        })
        .chain(
            module
                .dynamic_dispatch
                .indirect_dispatches
                .iter()
                .map(|dispatch| {
                    (
                        dispatch.public_requirement_identity.as_str(),
                        dispatch.requirement_identity.as_str(),
                        dispatch.realization_identity.as_str(),
                        dispatch.realization_callable_identity.as_str(),
                        dispatch.realization,
                    )
                }),
        )
        .chain(
            module
                .dynamic_dispatch
                .stored_dispatches
                .iter()
                .map(|dispatch| {
                    (
                        dispatch.public_requirement_identity.as_str(),
                        dispatch.requirement_identity.as_str(),
                        dispatch.realization_identity.as_str(),
                        dispatch.realization_callable_identity.as_str(),
                        dispatch.realization,
                    )
                }),
        )
    {
        requirement_issuers.insert(dispatch.0);
        requirement_issuers.insert(dispatch.1);
        machine_issuers.insert(dispatch.2);
        machine_issuers.insert(dispatch.3);
        let requirements = machine_requirements.entry(dispatch.4).or_default();
        requirements.insert(dispatch.0);
        requirements.insert(dispatch.1);
        let identities = machine_identities.entry(dispatch.4).or_default();
        identities.insert(dispatch.2);
        identities.insert(dispatch.3);
    }
    for call in &module.proof_output_calls {
        machine_issuers.insert(call.target_machine_identity.as_str());
        if let Some(runtime_call) = &call.runtime_call {
            machine_identities
                .entry(runtime_call.callee)
                .or_default()
                .insert(call.target_machine_identity.as_str());
        }
        if let Some(dispatch) = &call.static_requirement_dispatch {
            let requirements = machine_requirements
                .entry(dispatch.realization)
                .or_default();
            requirements.insert(dispatch.public_requirement_identity.as_str());
            requirements.insert(dispatch.requirement_identity.as_str());
            let identities = machine_identities.entry(dispatch.realization).or_default();
            identities.insert(dispatch.realization_identity.as_str());
            identities.insert(dispatch.realization_callable_identity.as_str());
        }
    }
    for component in &module.proof_recursive_components {
        for member in &component.members {
            machine_issuers.insert(member.machine_identity.as_str());
            if let Some(machine) = module
                .machines
                .iter()
                .find(|machine| machine.contract.id == member.contract)
            {
                machine_identities
                    .entry(machine.id)
                    .or_default()
                    .insert(member.machine_identity.as_str());
            }
        }
    }
    let mut domains = BTreeMap::new();
    let mut domain_routes: BTreeMap<ScalarDomainId, &[ScalarDomainEstablishmentRoute]> =
        BTreeMap::new();
    let mut semantics = BTreeSet::new();
    let mut identities = BTreeSet::new();
    let mut previous = None;
    for domain in &catalog.domains {
        if previous.is_some_and(|previous| previous >= domain.id)
            || domain.identity.is_empty()
            || !semantics.insert(domain.semantic_domain)
            || !identities.insert(&domain.identity)
            || module.structural_domains.iter().any(|structural| {
                structural.semantic_domain == domain.semantic_domain
                    || structural.identity == domain.identity
            })
        {
            return Err(invalid(
                "noncanonical or conflicting scalar domain definition",
            ));
        }
        if domain
            .establishment_routes
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid("noncanonical scalar domain establishment routes"));
        }
        for route in &domain.establishment_routes {
            let resolved = match route {
                ScalarDomainEstablishmentRoute::CheckedRequirement {
                    requirement_identity,
                } => requirement_issuers.contains(requirement_identity.as_str()),
                ScalarDomainEstablishmentRoute::BoundaryRequirement {
                    requirement_identity,
                } => boundary_issuers.contains(requirement_identity.as_str()),
                ScalarDomainEstablishmentRoute::ExactMachine { machine_identity } => {
                    machine_issuers.contains(machine_identity.as_str())
                }
            };
            if route.identity().is_empty() || !resolved {
                return Err(invalid(
                    "scalar domain establishment route has no retained issuer",
                ));
            }
        }
        previous = Some(domain.id);
        domains.insert(domain.id, domain.carrier);
        domain_routes.insert(domain.id, domain.establishment_routes.as_slice());
    }
    let mut sets: BTreeMap<ScalarQualificationSetId, &[ScalarDomainId]> = BTreeMap::new();
    let mut memberships = BTreeSet::new();
    let mut previous = None;
    for set in &catalog.sets {
        if set.id.is_empty()
            || set.domains.is_empty()
            || previous.is_some_and(|previous| previous >= set.id)
            || set.domains.windows(2).any(|pair| pair[0] >= pair[1])
            || set
                .domains
                .iter()
                .any(|domain| !domains.contains_key(domain))
            || !memberships.insert(set.domains.as_slice())
        {
            return Err(invalid("noncanonical scalar qualification set"));
        }
        previous = Some(set.id);
        sets.insert(set.id, &set.domains);
    }
    let mut coercions = BTreeMap::new();
    let mut previous = None;
    for coercion in &catalog.coercions {
        let coordinate = (coercion.machine, coercion.edge, coercion.argument_ordinal);
        if previous.is_some_and(|previous| previous >= coordinate) {
            return Err(invalid("noncanonical scalar qualification coercions"));
        }
        previous = Some(coordinate);
        coercions.insert(coordinate, coercion);
    }
    for machine in &module.machines {
        let values = declarations(machine)
            .map(|value| (value.id, value))
            .collect::<BTreeMap<_, _>>();
        // The kind that produced each local value. Only an `IeeeFloatConstant`
        // producer lets a call site discharge a ranged parameter structurally;
        // every other producer must route through a caller parameter whose own
        // retained range is subsumed by the callee's.
        let producers = machine
            .blocks
            .iter()
            .flat_map(|block| block.operations.iter())
            .filter_map(|operation| {
                operation
                    .result
                    .scalar_ref()
                    .map(|result| (result.id, &operation.kind))
            })
            .collect::<BTreeMap<_, _>>();
        for value in values.values() {
            if !value.qualifications.is_empty() {
                let members = sets
                    .get(&value.qualifications)
                    .ok_or_else(|| invalid("unknown scalar qualification set"))?;
                if members
                    .iter()
                    .any(|domain| domains.get(domain) != Some(&value.scalar_type))
                {
                    return Err(invalid("scalar qualification carrier mismatch"));
                }
            }
        }
        // The value scope an erased actual on an edge may name: every value
        // the source machine admits, including its erased scalar formals.
        let admitted: BTreeSet<ValueId> = crate::validation::machine_value_types(machine)
            .map(|(id, _)| id)
            .collect();
        for block in &machine.blocks {
            for operation in &block.operations {
                match &operation.kind {
                    OperationKind::Call {
                        callee, arguments, ..
                    }
                    | OperationKind::CallUnit {
                        callee, arguments, ..
                    }
                    | OperationKind::CallStructuralScalar {
                        callee, arguments, ..
                    }
                    | OperationKind::CallStructuralWithScalarArguments {
                        callee, arguments, ..
                    } => {
                        let callee = module
                            .machines
                            .iter()
                            .find(|candidate| candidate.id == *callee)
                            .ok_or_else(|| invalid("unknown scalar qualification callee"))?;
                        if arguments.len() != callee.parameters.len()
                            || arguments.iter().zip(&callee.parameters).any(
                                |(argument, parameter)| {
                                    values.get(argument).is_none_or(|value| {
                                        value.qualifications != parameter.qualifications
                                    })
                                },
                            )
                        {
                            return Err(invalid("call argument scalar qualification mismatch"));
                        }
                        if operation
                            .result
                            .scalar_ref()
                            .map(|value| value.qualifications)
                            != callee.result.scalar_ref().map(|value| value.qualifications)
                        {
                            return Err(invalid("call result scalar qualification mismatch"));
                        }
                        // A ranged callee parameter admits only deliveries
                        // that are provably inside the authored range: a
                        // constant whose retained bits the range contains, or
                        // a caller parameter whose own retained range is
                        // subsumed. Anything else — a computed value, a block
                        // parameter, an unranged caller parameter — cannot
                        // prove membership and is rejected.
                        for (argument, parameter) in arguments.iter().zip(&callee.parameters) {
                            let Some(range) = float_ranges.get(&(callee.id, parameter.id)) else {
                                continue;
                            };
                            let admitted = match producers.get(argument) {
                                Some(OperationKind::IeeeFloatConstant { value }) => {
                                    range.contains(*value)
                                }
                                _ => float_ranges
                                    .get(&(machine.id, *argument))
                                    .is_some_and(|caller| range.contains_range(caller)),
                            };
                            if !admitted {
                                return Err(ModuleError::ScalarFloatRangeDelivery {
                                    caller: machine.id,
                                    operation: operation.id,
                                    callee: callee.id,
                                    parameter: parameter.id,
                                });
                            }
                        }
                        // A ranged integer parameter publishes the same bounds
                        // as `requires` propositions, so call composition
                        // already reconstructs the delivery obligation and its
                        // proof replays constants, forwarded parameters, and
                        // computed arguments alike. The one delivery shape the
                        // obligation cannot make more precise is an exact
                        // `IntegerConstant` outside the authored interval:
                        // reject it here so the failure names the delivery
                        // instead of surfacing as an unprovable obligation.
                        for (argument, parameter) in arguments.iter().zip(&callee.parameters) {
                            let Some(range) = integer_ranges.get(&(callee.id, parameter.id)) else {
                                continue;
                            };
                            if let Some(OperationKind::IntegerConstant { value }) =
                                producers.get(argument)
                                && !range.contains(*value)
                            {
                                return Err(ModuleError::ScalarIntegerRangeDelivery {
                                    caller: machine.id,
                                    operation: operation.id,
                                    callee: callee.id,
                                    parameter: parameter.id,
                                });
                            }
                        }
                    }
                    _ => {
                        if operation
                            .result
                            .scalar_ref()
                            .is_some_and(|value| !value.qualifications.is_empty())
                        {
                            return Err(invalid("operation cannot establish scalar qualification"));
                        }
                    }
                }
            }
            let mut arrival = |edge,
                               target,
                               arguments: &[ValueId],
                               erased_arguments: &[semantic_vocabulary::ScalarTerm],
                               erased_proof_arguments: &[semantic_vocabulary::ProofTerm]|
             -> Result<(), ModuleError> {
                let target = machine
                    .blocks
                    .iter()
                    .find(|block| block.id == target)
                    .ok_or_else(|| invalid("unknown scalar qualification successor"))?;
                if arguments.len() != target.parameters.len()
                    || erased_arguments.len() != target.erased_scalar_formals.len()
                    || erased_proof_arguments.len() != target.erased_proof_formals.len()
                {
                    return Err(invalid("scalar qualification successor arity"));
                }
                // The entry block's roster lives on the machine contract;
                // every other block redeclares it.
                let source_formals = if block.erased_proof_formals.is_empty() {
                    machine.contract.erased_proof_formals.as_slice()
                } else {
                    block.erased_proof_formals.as_slice()
                };
                for (formal, term) in target
                    .erased_proof_formals
                    .iter()
                    .zip(erased_proof_arguments.iter())
                {
                    term.validate().map_err(ModuleError::MalformedProposition)?;
                    let mut in_scope = true;
                    term.visit_formal_positions(|position| {
                        in_scope &= source_formals
                            .get(usize::try_from(position).unwrap_or(usize::MAX))
                            .is_some();
                    });
                    if !in_scope {
                        return Err(invalid("erased proof argument out of scope"));
                    }
                    // A `Scalar` leaf inside a record/enum carrier names the
                    // source machine's own values — the admitted scope is
                    // the same one erased scalar actuals draw on.
                    if !term.visit_scalar_value_ids(|value| admitted.contains(&value)) {
                        return Err(invalid("erased proof argument scalar value out of scope"));
                    }
                    match term {
                        semantic_vocabulary::ProofTerm::Construction { type_identity, .. }
                            if *type_identity == formal.type_identity => {}
                        semantic_vocabulary::ProofTerm::Formal { position }
                            if source_formals
                                .get(usize::try_from(*position).unwrap_or(usize::MAX))
                                .is_some_and(|source| {
                                    source.type_identity == formal.type_identity
                                }) => {}
                        _ => {
                            return Err(invalid("erased proof successor argument mismatch"));
                        }
                    }
                }
                for (ordinal, (argument, destination)) in
                    arguments.iter().zip(&target.parameters).enumerate()
                {
                    let source = values
                        .get(argument)
                        .ok_or_else(|| invalid("unknown scalar qualification argument"))?;
                    let ordinal = u32::try_from(ordinal)
                        .map_err(|_| invalid("scalar qualification argument ordinal overflow"))?;
                    let coercion = coercions.remove(&(machine.id, edge, ordinal));
                    if let Some(coercion) = coercion {
                        validate_coercion(
                            machine.id,
                            coercion,
                            source,
                            destination,
                            &sets,
                            &domain_routes,
                            &machine_requirements,
                            &machine_boundaries,
                            &machine_identities,
                        )?;
                    } else if source.qualifications != destination.qualifications {
                        return Err(invalid(
                            "scalar edge changed qualification without coercion",
                        ));
                    }
                }
                Ok(())
            };
            match &block.terminator {
                Terminator::Jump {
                    edge,
                    target,
                    arguments,
                    erased_arguments,
                    erased_proof_arguments,
                    ..
                } => {
                    arrival(
                        *edge,
                        *target,
                        arguments,
                        erased_arguments,
                        erased_proof_arguments,
                    )?;
                }
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    // Observing a Boolean does not produce an erased value or
                    // change its declaration. Only the successor transfers
                    // establish new bindings and must preserve membership.
                    for successor in [when_true, when_false] {
                        arrival(
                            successor.edge,
                            successor.target,
                            &successor.arguments,
                            &successor.erased_arguments,
                            &successor.erased_proof_arguments,
                        )?;
                    }
                }
                Terminator::StructuralCase { cases, .. } => {
                    for successor in cases {
                        let target = machine
                            .blocks
                            .iter()
                            .find(|block| block.id == successor.target)
                            .ok_or_else(|| invalid("unknown structural case successor"))?;
                        if target
                            .parameters
                            .iter()
                            .any(|value| !value.qualifications.is_empty())
                        {
                            return Err(invalid(
                                "structural payload cannot establish scalar qualification",
                            ));
                        }
                    }
                }
                Terminator::Return { value, .. }
                    if values.get(value).map(|value| value.qualifications)
                        != machine
                            .result
                            .scalar_ref()
                            .map(|value| value.qualifications) =>
                {
                    return Err(invalid("return scalar qualification mismatch"));
                }
                _ => {}
            }
        }
    }
    if !coercions.is_empty() {
        return Err(invalid("orphan scalar qualification coercion"));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_coercion(
    machine: MachineId,
    coercion: &ScalarQualificationCoercion,
    source: &ValueDeclaration,
    destination: &ValueDeclaration,
    sets: &BTreeMap<ScalarQualificationSetId, &[ScalarDomainId]>,
    domain_routes: &BTreeMap<ScalarDomainId, &[ScalarDomainEstablishmentRoute]>,
    machine_requirements: &BTreeMap<MachineId, BTreeSet<&str>>,
    machine_boundaries: &BTreeMap<MachineId, BTreeSet<&str>>,
    machine_identities: &BTreeMap<MachineId, BTreeSet<&str>>,
) -> Result<(), ModuleError> {
    let source_members = if source.qualifications.is_empty() {
        &[][..]
    } else {
        sets.get(&source.qualifications)
            .copied()
            .ok_or_else(|| invalid("unknown coercion input set"))?
    };
    let destination_members = if destination.qualifications.is_empty() {
        &[][..]
    } else {
        sets.get(&destination.qualifications)
            .copied()
            .ok_or_else(|| invalid("unknown coercion output set"))?
    };
    // A single edge cannot replace unrelated meaning. Source must explicitly
    // erase and then introduce it through two independently checked bindings.
    let strict_subset = |smaller: &[ScalarDomainId], larger: &[ScalarDomainId]| {
        smaller.len() < larger.len()
            && smaller
                .iter()
                .all(|member| larger.binary_search(member).is_ok())
    };
    if coercion.source != source.id
        || coercion.destination != destination.id
        || source.id == destination.id
        || source.scalar_type != destination.scalar_type
        || !(strict_subset(source_members, destination_members)
            || strict_subset(destination_members, source_members))
    {
        return Err(invalid(
            "scalar coercion is not an exact fresh same-carrier introduction or erasure",
        ));
    }
    if strict_subset(source_members, destination_members) {
        // Introduction establishes membership: every gained domain with
        // named issuers must resolve to a route bound to the enclosing
        // machine. A domain without retained routes is a vacuous tag any
        // machine may introduce; a routed domain rejects a coercion whose
        // machine is only a caller of the issuer's public requirement.
        let requirements = machine_requirements.get(&machine);
        let boundaries = machine_boundaries.get(&machine);
        let identities = machine_identities.get(&machine);
        let unauthorized = destination_members
            .iter()
            .filter(|domain| !source_members.contains(domain))
            .any(|domain| {
                let routes = domain_routes.get(domain).copied().unwrap_or(&[]);
                !routes.is_empty()
                    && !routes.iter().any(|route| match route {
                        ScalarDomainEstablishmentRoute::CheckedRequirement {
                            requirement_identity,
                        } => requirements
                            .is_some_and(|bound| bound.contains(requirement_identity.as_str())),
                        ScalarDomainEstablishmentRoute::BoundaryRequirement {
                            requirement_identity,
                        } => boundaries
                            .is_some_and(|bound| bound.contains(requirement_identity.as_str())),
                        ScalarDomainEstablishmentRoute::ExactMachine { machine_identity } => {
                            identities
                                .is_some_and(|bound| bound.contains(machine_identity.as_str()))
                        }
                    })
            });
        if unauthorized {
            return Err(invalid(
                "scalar qualification introduction has no issuer route bound to this machine",
            ));
        }
    }
    Ok(())
}
