//! Exact checked Unit call-closure and identity validation.

use super::*;

pub(crate) fn checked_unit_call_closure_including(
    checked: &CheckedTrees,
    entry: symbols::SymbolHandle,
    additional_roots: &[symbols::SymbolHandle],
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    let plans = &checked.facts.flow.terminal_unit_effects;
    let mut closure = vec![entry];
    for root in additional_roots {
        if closure.contains(root) {
            return unsupported("attached Unit closure contains a duplicate explicit root");
        }
        closure.push(*root);
    }
    let mut next = 0_usize;
    while let Some(machine_symbol) = closure.get(next).copied() {
        next += 1;
        checked_terminal_machine_name(checked, machine_symbol)?;
        let machine = unique_unit_machine(plans, machine_symbol)?;
        for target in machine
            .operations
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. } => {
                    Some(*target_machine)
                }
                _ => None,
            })
        {
            if !closure.contains(&target) {
                closure.push(target);
            }
        }
    }
    Ok(closure)
}

/// Discover the scalar graphs rooted by exact result-bearing calls in an
/// attached Unit plan. Embedded roots retain their caller attachment; every
/// transitive attached callee needs an exact source-validated static computation
/// edge. Other transitive scalar callees pass the generic signature fence.
pub(super) fn checked_scalar_call_closure(
    checked: &CheckedTrees,
    roots: &[symbols::SymbolHandle],
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    let mut closure = Vec::new();
    for root in roots {
        if !closure.contains(root) {
            closure.push(*root);
        }
    }
    let embedded_roots = closure.clone();
    let mut computation_targets = Vec::new();
    let mut attached_members = Vec::new();
    let mut next = 0_usize;
    while let Some(machine) = closure.get(next).copied() {
        next += 1;
        let selections = checked
            .facts
            .flow
            .terminal_machines
            .machines
            .iter()
            .filter(|selection| selection.machine == machine)
            .collect::<Vec<_>>();
        let [selection] = selections.as_slice() else {
            return unsupported("embedded scalar call has no unique checked terminal selection");
        };
        let supported_signature = matches!(
            selection.signature,
            CheckedTerminalSignatureEligibility::Eligible
                | CheckedTerminalSignatureEligibility::Attached
        );
        if selection.name.is_empty() || !supported_signature {
            return unsupported(
                "embedded scalar call closure has an unsupported terminal signature",
            );
        }
        if selection.signature == CheckedTerminalSignatureEligibility::Attached {
            attached_members.push(machine);
        }
        let graph = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine)
            .ok_or(LoweringError::Unsupported(
                "embedded scalar call has no checked scalar graph",
            ))?;
        let computed = source_checked_computation_targets(checked, machine)?;
        for target in &computed {
            if !computation_targets.contains(target) {
                computation_targets.push(*target);
            }
        }
        for target in graph
            .states
            .iter()
            .flat_map(|state| {
                state.bindings.iter().filter_map(|binding| {
                    let CheckedScalarBindingValue::DirectCall { target_machine, .. } =
                        &binding.value
                    else {
                        return None;
                    };
                    Some(*target_machine)
                })
            })
            .chain(computed)
        {
            if !closure.contains(&target) {
                closure.push(target);
            }
        }
    }
    // Check after discovery so two callers reaching the same callee do not make
    // static eligibility depend on the closure's worklist order.
    if attached_members
        .iter()
        .any(|machine| !embedded_roots.contains(machine) && !computation_targets.contains(machine))
    {
        return unsupported("embedded scalar call closure has an unsupported terminal signature");
    }
    Ok(closure)
}

fn source_checked_computation_targets(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    for (_, root) in plans
        .roots
        .iter()
        .filter(|(_, root)| root.machine == machine)
    {
        if plans.root_at(root.state, root.statement_ordinal, root.role) != Some(root)
            || !plans.nodes.is_valid(root.root)
        {
            return unsupported("embedded computation has no unique live source root");
        }
        let source = crate::scalar_source_custody::locate(
            checked,
            root.state,
            root.statement_ordinal,
            root.role,
        )?;
        let node = plans.nodes.get(root.root);
        if source.machine != machine
            || source.expression != node.authored_root
            || source.primitive_type != node.primitive_type
        {
            return unsupported("embedded computation disagrees with its authored source root");
        }
        crate::scalar_source_custody::validate_computation_calls(
            checked,
            machine,
            root.state,
            root.statement_ordinal,
            root.root,
            source.expression,
        )?;
    }
    crate::scalar_computations::call_targets(checked, machine)
}

pub(crate) fn unique_unit_machine(
    plans: &checked_trees::CheckedUnitEffectPlans,
    symbol: symbols::SymbolHandle,
) -> Result<&CheckedUnitEffectMachinePlan, LoweringError> {
    let mut matches = plans.machines.iter().filter(|plan| plan.machine == symbol);
    let plan = matches.next().ok_or(LoweringError::Unsupported(
        "attached Unit closure is missing a checked transitive machine plan",
    ))?;
    if matches.next().is_some() {
        return unsupported("attached Unit closure contains duplicate checked machine plans");
    }
    Ok(plan)
}

pub(super) fn unique_unit_boundary(
    plans: &checked_trees::CheckedUnitEffectPlans,
    symbol: symbols::SymbolHandle,
) -> Result<&CheckedBoundaryMachinePlan, LoweringError> {
    let mut matches = plans
        .boundary_machines
        .iter()
        .filter(|plan| plan.machine == symbol);
    let plan = matches.next().ok_or(LoweringError::Unsupported(
        "attached Unit closure is missing a checked boundary machine plan",
    ))?;
    if matches.next().is_some() {
        return unsupported("attached Unit closure contains duplicate boundary machine plans");
    }
    Ok(plan)
}

pub(super) fn checked_terminal_machine_name(
    checked: &CheckedTrees,
    symbol: symbols::SymbolHandle,
) -> Result<&str, LoweringError> {
    let mut matches = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .filter(|selection| selection.machine == symbol);
    let selection = matches.next().ok_or(LoweringError::Unsupported(
        "attached Unit member has no checked terminal selection",
    ))?;
    if matches.next().is_some()
        || !matches!(
            selection.signature,
            CheckedTerminalSignatureEligibility::Attached
                | CheckedTerminalSignatureEligibility::FreeUnitEffect
                | CheckedTerminalSignatureEligibility::Eligible
        )
        || selection.name.is_empty()
    {
        return unsupported("Unit-effect member has an invalid checked terminal selection");
    }
    Ok(&selection.name)
}

pub(crate) fn checked_unit_boundary_identity(
    checked: &CheckedTrees,
    symbol: symbols::SymbolHandle,
) -> Result<String, LoweringError> {
    let requirements = checked
        .typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| {
            checked
                .typed
                .trait_machine_signatures(definition)
                .iter()
                .filter(move |signature| signature.symbol == symbol)
                .map(move |signature| (definition, signature))
        })
        .collect::<Vec<_>>();
    if let [(definition, signature)] = requirements.as_slice() {
        let identity = checked
            .typed
            .normalized_trait_requirement_overload_identity(definition, signature)
            .identity();
        if !identity.is_empty() {
            return Ok(identity);
        }
    }
    checked_terminal_machine_name(checked, symbol).map(str::to_owned)
}

pub(super) fn validate_unit_operation_sequence(
    machine: &CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    let Some(CheckedUnitEffectOperationPlan::ReturnUnit {
        statement_index, ..
    }) = machine.operations.last()
    else {
        return unsupported("Unit machine does not end in exactly one checked Unit return");
    };
    let mut previous = None;
    let mut next_scalar_binding = 0_u32;
    for operation in &machine.operations[..machine.operations.len() - 1] {
        let coordinate = match operation {
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index,
                declaration_ordinal,
                ..
            }
            | CheckedUnitEffectOperationPlan::EstablishAffineScalarRecordLocal {
                statement_index,
                declaration_ordinal,
                ..
            } => checked_trees::CheckedUnitCallCoordinate {
                statement_index: *statement_index,
                call_ordinal: *declaration_ordinal,
            },
            CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. }
            | CheckedUnitEffectOperationPlan::BoundaryCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::PortWrite { coordinate, .. } => *coordinate,
            CheckedUnitEffectOperationPlan::ScalarCall {
                coordinate, result, ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                coordinate, result, ..
            }
            | CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                coordinate,
                result,
                ..
            }
            | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                coordinate,
                result,
                ..
            }
            | CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd {
                coordinate,
                result,
                ..
            } => {
                if result.statement_index != coordinate.statement_index
                    || coordinate.call_ordinal != 0
                    || result.binding_ordinal != next_scalar_binding
                {
                    return unsupported(
                        "Unit scalar result local or call coordinate is not canonical",
                    );
                }
                next_scalar_binding =
                    next_scalar_binding
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "Unit scalar result binding ordinal space is exhausted",
                        ))?;
                *coordinate
            }
            CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, .. } => {
                if result.statement_index != result.binding_ordinal
                    || result.binding_ordinal != next_scalar_binding
                {
                    return unsupported(
                        "Unit scalar expression local is not the next dense source binding",
                    );
                }
                next_scalar_binding =
                    next_scalar_binding
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "Unit scalar result binding ordinal space is exhausted",
                        ))?;
                checked_trees::CheckedUnitCallCoordinate {
                    statement_index: result.statement_index,
                    call_ordinal: 0,
                }
            }
            CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
                coordinate,
                result,
                discard_result_on_return,
                ..
            } => {
                if result.statement_index != coordinate.statement_index
                    || coordinate.call_ordinal != 0
                    || result.binding_ordinal != 0
                    || !discard_result_on_return
                {
                    return unsupported(
                        "Unit structural result local or call coordinate is not canonical",
                    );
                }
                *coordinate
            }
            CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate,
                result,
                discard_result_on_return,
                ..
            } => {
                if result.statement_index != coordinate.statement_index
                    || coordinate.call_ordinal != 0
                    || result.binding_ordinal != 0
                    || *discard_result_on_return != (result.multiplicity == Multiplicity::Affine)
                {
                    return unsupported(
                        "Unit boundary structural result local or call coordinate is not canonical",
                    );
                }
                *coordinate
            }
            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                statement_index, ..
            } => checked_trees::CheckedUnitCallCoordinate {
                statement_index: *statement_index,
                call_ordinal: 0,
            },
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => {
                checked_trees::CheckedUnitCallCoordinate {
                    statement_index: store.statement_index,
                    call_ordinal: 0,
                }
            }
            CheckedUnitEffectOperationPlan::ReturnUnit { .. } => {
                return unsupported("Unit machine contains a nonfinal Unit return");
            }
        };
        let key = (coordinate.statement_index, coordinate.call_ordinal);
        if previous.is_some_and(|previous| previous >= key)
            || coordinate.statement_index >= *statement_index
        {
            return unsupported("Unit machine operation order is not canonical source order");
        }
        previous = Some(key);
    }
    Ok(())
}

pub(super) fn reject_recursive_unit_closure(
    plans: &checked_trees::CheckedUnitEffectPlans,
    closure: &[symbols::SymbolHandle],
) -> Result<(), LoweringError> {
    fn visit(
        plans: &checked_trees::CheckedUnitEffectPlans,
        symbol: symbols::SymbolHandle,
        active: &mut Vec<symbols::SymbolHandle>,
        complete: &mut Vec<symbols::SymbolHandle>,
    ) -> Result<(), LoweringError> {
        if active.contains(&symbol) {
            return unsupported("recursive Unit call closure is not yet terminal");
        }
        if complete.contains(&symbol) {
            return Ok(());
        }
        active.push(symbol);
        for target in unique_unit_machine(plans, symbol)?
            .operations
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. } => {
                    Some(*target_machine)
                }
                _ => None,
            })
        {
            visit(plans, target, active, complete)?;
        }
        active.pop();
        complete.push(symbol);
        Ok(())
    }

    let mut active = Vec::new();
    let mut complete = Vec::new();
    for symbol in closure {
        visit(plans, *symbol, &mut active, &mut complete)?;
    }
    Ok(())
}
