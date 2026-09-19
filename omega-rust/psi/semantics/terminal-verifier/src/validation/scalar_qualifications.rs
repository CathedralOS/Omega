//! Reconstruct scalar membership at every definition and transport boundary.
//!
//! Declarations carry locally defined, obligation-free tags. Ordinary values
//! cannot implicitly gain or lose them: only a catalogued successor binding
//! introduces or erases membership, creating a distinct SSA value without an opcode.
use super::{
    BTreeMap, BTreeSet, MachineId, ModuleError, OperationKind, TerminalMachine, TerminalModule,
    Terminator, ValueId,
};
use semantic_vocabulary::{ScalarDomainId, ScalarQualificationSetId, ScalarType};
use terminal_psi::{ScalarFloatRange, ScalarQualificationCoercion, ValueDeclaration};

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

pub(super) fn declarations(machine: &TerminalMachine) -> impl Iterator<Item = &ValueDeclaration> {
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

pub(super) fn validate(module: &TerminalModule) -> Result<(), ModuleError> {
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
    // Boundary scalar signatures currently carry only payload types. A
    // provider installation cannot supply or forget membership — or a retained
    // floating range — through that older interface, even when its in-module
    // body has a valid signature.
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
                    || machine
                        .parameters
                        .iter()
                        .any(|parameter| float_ranges.contains_key(&(machine.id, parameter.id)))
            })
        {
            return Err(invalid("qualified scalar provider boundary is unsupported"));
        }
    }
    let mut domains = BTreeMap::new();
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
        previous = Some(domain.id);
        domains.insert(domain.id, domain.carrier);
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
                               erased_arguments: &[semantic_vocabulary::ScalarTerm]|
             -> Result<(), ModuleError> {
                let target = machine
                    .blocks
                    .iter()
                    .find(|block| block.id == target)
                    .ok_or_else(|| invalid("unknown scalar qualification successor"))?;
                if arguments.len() != target.parameters.len()
                    || erased_arguments.len() != target.erased_scalar_formals.len()
                {
                    return Err(invalid("scalar qualification successor arity"));
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
                        validate_coercion(coercion, source, destination, &sets)?;
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
                    ..
                } => arrival(*edge, *target, arguments, erased_arguments)?,
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

fn validate_coercion(
    coercion: &ScalarQualificationCoercion,
    source: &ValueDeclaration,
    destination: &ValueDeclaration,
    sets: &BTreeMap<ScalarQualificationSetId, &[ScalarDomainId]>,
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
    Ok(())
}
