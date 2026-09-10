//! Reconstruct scalar membership at every definition and transport boundary.
//!
//! Declarations carry locally defined, obligation-free tags. Ordinary values
//! cannot implicitly gain or lose them: only a catalogued successor binding
//! introduces or erases membership, creating a distinct SSA value without an opcode.

use super::*;
use semantic_vocabulary::{ScalarDomainId, ScalarQualificationSetId};
use terminal_psi::{ScalarQualificationCoercion, ValueDeclaration};

fn invalid(reason: &'static str) -> ModuleError {
    ModuleError::InvalidScalarQualification(reason)
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
    // Boundary scalar signatures currently carry only payload types. A
    // provider installation cannot supply or forget membership through that
    // older interface, even when its in-module body has a valid signature.
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
            })
        {
            return Err(invalid("qualified scalar provider boundary is unsupported"));
        }
    }
    let catalog = &module.scalar_qualifications;
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
            let mut arrival = |edge, target, arguments: &[ValueId]| -> Result<(), ModuleError> {
                let target = machine
                    .blocks
                    .iter()
                    .find(|block| block.id == target)
                    .ok_or_else(|| invalid("unknown scalar qualification successor"))?;
                if arguments.len() != target.parameters.len() {
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
                    ..
                } => arrival(*edge, *target, arguments)?,
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    // Observing a Boolean does not produce an erased value or
                    // change its declaration. Only the successor transfers
                    // establish new bindings and must preserve membership.
                    for successor in [when_true, when_false] {
                        arrival(successor.edge, successor.target, &successor.arguments)?;
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
