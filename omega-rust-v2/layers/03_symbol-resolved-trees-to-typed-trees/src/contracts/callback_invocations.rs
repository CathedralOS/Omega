use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::type_error::SourceTypeError;
use crate::{BoundarySlot, InvocationTarget, RequirementSlot, SlotDirection, TypeError, TypedCall, TypedStage, ValueType};
use omega_resolved::DeclarationId;
use std::collections::{HashMap, HashSet};

fn declared(id: &DeclarationId) -> InvocationTarget {
    InvocationTarget::Declaration(RequirementSlot {
        declaration: id.clone(),
        trait_arguments: Vec::new(),
    })
}
pub(crate) fn through(
    call: &TypedCall,
    entry: &InvocationTarget,
) -> InvocationTarget {
    match entry {
        InvocationTarget::RuntimeParameter { owner, index } if owner == &call.machine => {
            let operand = &call.operands[*index];
            operand
                .callable
                .clone()
                .unwrap_or_else(|| InvocationTarget::StoredCallable(operand.site.clone()))
        }
        InvocationTarget::StaticParameter { owner, index } => match call.generic_argument(owner, *index) {
            Some(ValueType::MachineSelection(selection)) if selection.all_arguments().next().is_none() => declared(&selection.machine),
            Some(ValueType::MachineSelection(selection)) => InvocationTarget::AppliedMachine(selection.clone()),
            Some(ValueType::Parameter { owner, index }) => InvocationTarget::StaticParameter {
                owner: *owner.clone(),
                index: *index,
            },
            _ => entry.clone(),
        },
        InvocationTarget::AppliedMachine(selection) => InvocationTarget::AppliedMachine(selection.with_substitutions(|value| call.instantiate(value))),
        InvocationTarget::Declaration(slot) => InvocationTarget::Declaration(RequirementSlot {
            declaration: slot.declaration.clone(),
            trait_arguments: slot
                .trait_arguments
                .iter()
                .map(|t| call.instantiate(t))
                .collect(),
        }),
        _ => entry.clone(),
    }
}
pub(crate) fn compose_callback_invocations<'a>(
    catalog: &DeclarationCatalog<'a>,
    graph: &super::effect_propagation::ExecutionCalls,
    boundary_slots: &[BoundarySlot],
    out: &mut TypedStage,
) -> Result<HashMap<&'a DeclarationId, HashSet<InvocationTarget>>, TypeError> {
    let mut rows: HashMap<_, HashSet<_>> = catalog
        .signatures
        .keys()
        .map(|&id| (id, HashSet::new()))
        .collect();
    for imported in &out.machine_effects {
        rows.get_mut(&imported.machine)
            .unwrap()
            .extend(imported.invokes.iter().cloned());
    }
    let mut ceilings: HashMap<&DeclarationId, Vec<&InvocationTarget>> = HashMap::new();
    for (owner, _, entry) in &out.invocation_ceilings {
        ceilings.entry(owner).or_default().push(entry);
        rows.get_mut(owner).unwrap().insert(entry.clone());
    }
    let admits = |owner: &DeclarationId, entry: &InvocationTarget| {
        let Some(allowed) = ceilings.get(owner) else {
            return true;
        };
        if allowed.contains(&entry) {
            return true;
        }
        if let InvocationTarget::Declaration(actual) = entry {
            return allowed.iter().any(|a| matches!(a,InvocationTarget::Declaration(slot) if slot.trait_arguments==actual.trait_arguments && catalog.requirements.get(&slot.declaration).is_some_and(|r|r.contains(&&actual.declaration))));
        }
        let InvocationTarget::StaticParameter { owner: bound, index } = entry else {
            return false;
        };
        let Some(ValueType::Callable(c)) = catalog.types.bound(bound, *index) else {
            return false;
        };
        let Some(req) = &c.requirement else {
            return false;
        };
        allowed.iter().any(|a|matches!(a,InvocationTarget::Declaration(slot) if slot==req || catalog.requirements.get(&slot.declaration).is_some_and(|r|r.contains(&&req.declaration)) && slot.trait_arguments==req.trait_arguments))
    };
    const EXCEEDS_CEILING: &str = "call exceeds the declared synchronous invocation ceiling";
    for &index in &graph.direct {
        let call = &out.calls[index];
        let slots = boundary_slots.iter().filter(|s| {
            s.requirement.declaration == call.machine
                && s.arguments == call.type_arguments
                && call
                    .enclosing_arguments
                    .iter()
                    .find(|(owner, _)| {
                        catalog
                            .requirements
                            .get(owner)
                            .is_some_and(|r| r.contains(&&call.machine))
                    })
                    .map_or(&[][..], |(_, a)| a.as_slice())
                    == s.requirement.trait_arguments.as_slice()
        });
        let mut slots = slots.peekable();
        if let Some(slot) = slots.next() {
            if slots.peek().is_some() {
                return Err(TypeError::for_site(&call.site, "competing directions for an applied boundary slot"));
            }
            out.boundary_calls.push((call.site.clone(), slot.clone()));
            if slot.direction == SlotDirection::EnvironmentToProgram
                && let Some(caller) = &call.caller
            {
                let target = InvocationTarget::Declaration(slot.requirement.clone());
                if !admits(caller, &target) {
                    return Err(TypeError::for_site(&call.site, EXCEEDS_CEILING));
                }
                rows.get_mut(caller).unwrap().insert(target);
            }
        }
    }
    for &index in &graph.indirect {
        let call = &out.indirect_calls[index];
        if let Some(owner) = &call.caller {
            if !admits(owner, &call.entry) {
                return Err(TypeError::for_site(&call.site, EXCEEDS_CEILING));
            }
            rows.get_mut(owner).unwrap().insert(call.entry.clone());
        }
    }
    graph.propagate(
        &out.calls,
        &mut rows,
        "invocation inference exceeds implementation capacity",
        |call, caller, entry| {
            let supplied = through(call, entry);
            if !admits(caller, &supplied) {
                return Err(TypeError::for_site(&call.site, EXCEEDS_CEILING));
            }
            Ok(Some(supplied))
        },
    )?;
    Ok(rows)
}
