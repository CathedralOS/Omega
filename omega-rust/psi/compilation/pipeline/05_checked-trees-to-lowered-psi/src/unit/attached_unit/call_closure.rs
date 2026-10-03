//! Exact checked Unit call-closure and identity validation.

use super::super::CheckedTerminalSignatureEligibility;
use super::bodies::UnitPlans;
use super::{
    CheckedBoundaryMachinePlan, CheckedTrees, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, LoweringError, Multiplicity, UnitBody, structural_calls,
    unsupported,
};
use checked_trees::{
    CheckedUnitCallCoordinate, CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute,
    CheckedUnitScalarResultBindingPlan,
};
pub(crate) fn checked_unit_call_closure_including(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    entry: symbols::SymbolHandle,
    additional_roots: &[symbols::SymbolHandle],
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
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
        let name = checked_terminal_machine_name(checked, machine_symbol)?;
        let machine = UnitBody::find(plans, machine_symbol).map_err(|error| match error {
            LoweringError::Unsupported(reason) => LoweringError::InvalidUnitMachinePlan {
                machine: name.to_owned(),
                reason,
                omission: crate::unit::unit_plan_omission_explanation(checked, machine_symbol),
            },
            error => error,
        })?;
        for target in machine
            .operations()
            .filter_map(|operation| unit_closure_target(plans, operation))
        {
            if !closure.contains(&target) {
                closure.push(target);
            }
        }
        // A nominal consuming machine's return edge invokes its exact
        // owner-attached `::drop` hook; the hook is roster work, not an
        // operation in the synthetic body, so it enters the closure here.
        if let Some(nominal) = checked
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .machines
            .iter()
            .find(|nominal| nominal.machine.machine == machine_symbol)
        {
            for cleanup in &nominal.cleanups {
                if !closure.contains(&cleanup.cleanup_machine) {
                    closure.push(cleanup.cleanup_machine);
                }
            }
        }
    }
    Ok(closure)
}

pub(super) use crate::scalar_graph::scalar_call_closure::embedded::checked_scalar_call_closure_with_structural_roots;

pub(crate) fn unique_unit_machine(
    plans: UnitPlans<'_>,
    symbol: symbols::SymbolHandle,
) -> Result<&CheckedUnitEffectMachinePlan, LoweringError> {
    let mut matches = plans.machines().filter(|plan| plan.machine == symbol);
    let plan = matches.next().ok_or(LoweringError::Unsupported(
        "attached Unit closure is missing a checked transitive machine plan",
    ))?;
    if matches.next().is_some() {
        return unsupported("attached Unit closure contains duplicate checked machine plans");
    }
    Ok(plan)
}

pub(super) fn unique_unit_boundary(
    plans: UnitPlans<'_>,
    symbol: symbols::SymbolHandle,
) -> Result<&CheckedBoundaryMachinePlan, LoweringError> {
    let mut matches = plans
        .boundary_machines()
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
    let selection = unique_terminal_selection(checked, symbol)?;
    if !matches!(
        selection.signature,
        CheckedTerminalSignatureEligibility::Attached
            | CheckedTerminalSignatureEligibility::FreeUnitEffect
            | CheckedTerminalSignatureEligibility::Eligible
    ) {
        return unsupported("Unit-effect member has an invalid checked terminal selection");
    }
    Ok(&selection.name)
}

/// The exact selection naming `symbol`, whatever its signature class. A
/// bodyless boundary machine is never lowered as a Terminal root, so its
/// `Unsupported` class does not bear on the identity its calls name.
fn unique_terminal_selection(
    checked: &CheckedTrees,
    symbol: symbols::SymbolHandle,
) -> Result<&checked_trees::CheckedTerminalMachineSelection, LoweringError> {
    let mut matches = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .filter(|selection| selection.machine == symbol);
    // The selection is what names the machine, so a missing one cannot report
    // through it. Name the member from its own symbol instead: reaching here
    // means the closure called a machine the selected target never selected --
    // a declaration scoped to another target, for one -- and a reader given
    // only "no checked terminal selection" has nothing to look for.
    let selection = matches
        .next()
        .ok_or_else(|| LoweringError::InvalidUnitMachinePlan {
            machine: checked.symbols.name(symbol).to_owned(),
            reason: "is called by this closure but the selected target selects no \
                 terminal machine for it",
            omission: crate::unit::unit_plan_omission_explanation(checked, symbol),
        })?;
    if matches.next().is_some() || selection.name.is_empty() {
        return unsupported("Unit-effect member has an invalid checked terminal selection");
    }
    Ok(selection)
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
    // Top-level installation dependencies are keyed by the normalized machine
    // overload, not its display name. Boundary calls must use that same exact
    // requirement identity or independent root replay sees an unused bound.
    let mut requirements = checked.typed.machines().iter().filter(|machine| {
        machine.symbol == symbol
            && (machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
                || (machine.supply_mode == language_semantics::MachineSupplyMode::Boundary
                    && machine.service_reach_is_installation_bound))
    });
    if let Some(requirement) = requirements.next() {
        if requirements.next().is_some() {
            return unsupported("top-level boundary requirement identity is ambiguous");
        }
        return checked
            .typed
            .normalized_machine_overload_identity(requirement)
            .map(|identity| identity.identity())
            .ok_or(LoweringError::Unsupported(
                "top-level boundary has no normalized machine overload identity",
            ));
    }
    if let Some(operator) = checked
        .typed
        .operators()
        .iter()
        .find(|operator| operator.is_boundary && operator.symbol == symbol)
    {
        return Ok(
            typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator),
        );
    }
    // An owner-qualified boundary machine keeps its qualified display name.
    // Qualification contributes no receiver, so its selection class is the
    // bodyless `Unsupported` one rather than `Attached`.
    unique_terminal_selection(checked, symbol).map(|selection| selection.name.clone())
}

pub(super) fn validate_unit_operation_sequence(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    let Some(CheckedUnitEffectOperationPlan::Complete {
        statement_index, ..
    }) = machine.operations.last()
    else {
        return unsupported("Unit machine does not end in exactly one checked Unit return");
    };
    let mut previous = None;
    let mut previous_nested = false;
    let mut coordinates = Vec::new();
    let mut next_scalar_binding = 0_u32;
    let mut next_structural_binding = 0_u32;
    let mut has_argument_constructors = false;
    for (operation_index, operation) in machine.operations[..machine.operations.len() - 1]
        .iter()
        .enumerate()
    {
        let coordinate = match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, calls, .. } => {
                // Operand calls publish their own results before the enclosing
                // constructor. Source replay separately rejoins each occurrence.
                for call in calls {
                    let Some(result) = call
                        .operation()
                        .call()
                        .filter(|planned| !planned.is_boundary())
                        .and_then(CheckedUnitCallPlan::structural_result)
                    else {
                        return unsupported("structural operand lost its call operation");
                    };
                    if result.binding_ordinal != next_structural_binding {
                        return unsupported(
                            "structural operand result is not the next dense source binding",
                        );
                    }
                    next_structural_binding = next_structural_binding.checked_add(1).ok_or(
                        LoweringError::Unsupported("structural operand binding space is exhausted"),
                    )?;
                }
                structural_calls::validate_usage(checked, machine, result)?;
                // A later element of a whole array-literal replacement binds
                // the next dense ordinal and continues the statement its
                // first element began.
                if super::field_replacement::continues_array_elements(
                    &machine.operations,
                    operation_index,
                ) {
                    if result.binding_ordinal != next_structural_binding {
                        return unsupported(
                            "Unit structural result is not the next dense source binding",
                        );
                    }
                    next_structural_binding = next_structural_binding.checked_add(1).ok_or(
                        LoweringError::Unsupported(
                            "Unit structural result binding ordinal space is exhausted",
                        ),
                    )?;
                    continue;
                }
                checked_trees::CheckedUnitCallCoordinate {
                    statement_index: result.statement_index,
                    call_ordinal: 0,
                }
            }
            CheckedUnitEffectOperationPlan::EstablishReference { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishByteSequence { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. } => {
                checked_trees::CheckedUnitCallCoordinate {
                    statement_index: result.statement_index,
                    call_ordinal: 0,
                }
            }
            CheckedUnitEffectOperationPlan::CallContinuationCleanup { coordinate, .. } => {
                // An ordinary call's continuation may retire its argument
                // owners regardless of the returned value. Boundary calls
                // retain their distinct structural-result disposal route.
                let completed_call = operation_index
                    .checked_sub(1)
                    .and_then(|previous| machine.operations.get(previous))
                    .and_then(CheckedUnitEffectOperationPlan::call)
                    .is_some_and(|call| {
                        call.coordinate == *coordinate
                            && matches!(
                                (call.route(), call.result()),
                                (CheckedUnitCallRoute::Module { .. }, _)
                                    | (_, CheckedUnitCallResult::Structural { .. })
                            )
                    });
                if !completed_call
                    && !super::field_replacement::continues_with_cleanup(
                        &machine.operations,
                        operation_index,
                    )
                    && !super::field_replacement::continues_array_elements(
                        &machine.operations,
                        operation_index,
                    )
                {
                    return unsupported("call cleanup has no immediately preceding completed call");
                }
                continue;
            }
            CheckedUnitEffectOperationPlan::ReleaseReference { .. } => continue,
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index,
                declaration_ordinal,
                ..
            }
            | CheckedUnitEffectOperationPlan::EstablishStructuralLocal {
                statement_index,
                declaration_ordinal,
                ..
            } => checked_trees::CheckedUnitCallCoordinate {
                statement_index: *statement_index,
                call_ordinal: *declaration_ordinal,
            },
            CheckedUnitEffectOperationPlan::PortWrite { coordinate, .. } => *coordinate,
            CheckedUnitEffectOperationPlan::Call(call) => {
                let coordinate = call.coordinate;
                match (call.route(), call.result()) {
                    (_, CheckedUnitCallResult::Unit) => {}
                    (_, CheckedUnitCallResult::Scalar(result)) => {
                        if result.statement_index != coordinate.statement_index
                            || coordinate.call_ordinal != 0
                            || result.binding_ordinal != next_scalar_binding
                        {
                            return unsupported(
                                "Unit scalar result local or call coordinate is not canonical",
                            );
                        }
                        if !crate::emission::call_source_custody::initializers::discards_result(
                            checked,
                            machine.state,
                            coordinate,
                        )? {
                            next_scalar_binding = next_scalar_binding.checked_add(1).ok_or(
                                LoweringError::Unsupported(
                                    "Unit scalar result binding ordinal space is exhausted",
                                ),
                            )?;
                        }
                    }
                    (
                        CheckedUnitCallRoute::Module { .. },
                        CheckedUnitCallResult::Structural {
                            binding: result, ..
                        },
                    ) => {
                        if result.statement_index != coordinate.statement_index
                            || result.binding_ordinal != next_structural_binding
                        {
                            return unsupported(
                                "Unit structural result local or call coordinate is not canonical",
                            );
                        }
                        structural_calls::validate_usage(checked, machine, result)?;
                    }
                    (
                        CheckedUnitCallRoute::Boundary,
                        CheckedUnitCallResult::Structural {
                            binding: result,
                            discard_result_on_return,
                        },
                    ) => {
                        if result.statement_index != coordinate.statement_index
                            || result.binding_ordinal != next_structural_binding
                            || (result.multiplicity != Multiplicity::Affine
                                && *discard_result_on_return)
                            || (coordinate.call_ordinal != 0
                                && result.multiplicity != Multiplicity::Affine)
                        {
                            return unsupported(
                                "Unit boundary structural result local or call coordinate is not canonical",
                            );
                        }
                        if result.multiplicity == Multiplicity::Affine {
                            structural_calls::validate_usage(checked, machine, result)?;
                        }
                    }
                }
                coordinate
            }
            // An observing atomic event binds its prior as the next dense
            // scalar local, exactly like an established scalar local; the
            // event's own coordinate is its carrier statement.
            CheckedUnitEffectOperationPlan::AtomicAccess(access) => {
                if let Some(result) = &access.result {
                    if result.binding_ordinal != next_scalar_binding
                        || result.statement_index > access.statement_index
                    {
                        return unsupported(
                            "atomic event result is not the next dense source binding",
                        );
                    }
                    next_scalar_binding =
                        next_scalar_binding
                            .checked_add(1)
                            .ok_or(LoweringError::Unsupported(
                                "Unit scalar result binding ordinal space is exhausted",
                            ))?;
                }
                checked_trees::CheckedUnitCallCoordinate {
                    statement_index: access.statement_index,
                    call_ordinal: 0,
                }
            }
            CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, .. } => {
                if result.binding_ordinal != next_scalar_binding {
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
            // The window pair of a field replacement continues its producer's
            // statement: the move-out and the store share the producing
            // call's or establishment's coordinate rather than taking one of
            // their own. The displaced binding still takes the next dense
            // ordinal.
            CheckedUnitEffectOperationPlan::MoveStructuralField { result, .. }
                if super::field_replacement::continues(&machine.operations, operation_index)
                    || super::field_replacement::continues_bound_value(
                        &machine.operations,
                        operation_index,
                    )
                    || super::field_replacement::continues_array_elements(
                        &machine.operations,
                        operation_index,
                    ) =>
            {
                if result.binding_ordinal != next_structural_binding {
                    return unsupported(
                        "Unit structural result is not the next dense source binding",
                    );
                }
                next_structural_binding =
                    next_structural_binding
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "Unit structural result binding ordinal space is exhausted",
                        ))?;
                continue;
            }
            CheckedUnitEffectOperationPlan::StoreStructuralField {
                statement_index,
                value,
                ..
            } if super::field_replacement::continues(&machine.operations, operation_index)
                || super::field_replacement::continues_bound_value(
                    &machine.operations,
                    operation_index,
                )
                || super::field_replacement::continues_array_elements(
                    &machine.operations,
                    operation_index,
                ) =>
            {
                if !super::field_replacement::continues(&machine.operations, operation_index)
                    && !super::field_replacement::continues_array_elements(
                        &machine.operations,
                        operation_index,
                    )
                {
                    let (_, source) =
                        crate::expression_preparation::source_custody::authored_state(
                            checked,
                            machine.state,
                        )?;
                    let Some(checked_trees::statement::StatementNode::Assignment(assignment)) =
                        checked
                            .statement_table
                            .statements(source.statement_nodes)
                            .get(*statement_index as usize)
                    else {
                        return unsupported("bound field replacement has no authored assignment");
                    };
                    super::field_replacement::bound_value_type(
                        checked,
                        source,
                        *statement_index as usize,
                        assignment.value,
                        value,
                        &machine.structural_parameters,
                        &machine.operations,
                    )?;
                }
                continue;
            }
            CheckedUnitEffectOperationPlan::MoveStructuralField { result, .. } => {
                checked_trees::CheckedUnitCallCoordinate {
                    statement_index: result.statement_index,
                    call_ordinal: 0,
                }
            }
            CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
                statement_index, ..
            }
            | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                statement_index, ..
            }
            | CheckedUnitEffectOperationPlan::StoreStructuralField {
                statement_index, ..
            } => checked_trees::CheckedUnitCallCoordinate {
                statement_index: *statement_index,
                call_ordinal: 0,
            },
            CheckedUnitEffectOperationPlan::ByteSequenceStore(store) => {
                if let Some(binding_ordinal) =
                    crate::emission::structural_byte_sequence_store::completed_call_binding(
                        store,
                        machine.state,
                    )?
                {
                    // A byte consumer completes its own call, regardless of
                    // whether it copies contents or a shared descriptor.
                    let completed = operation_index
                        .checked_sub(1)
                        .and_then(|previous| machine.operations.get(previous))
                        .and_then(CheckedUnitEffectOperationPlan::call)
                        .filter(|call| !call.is_boundary())
                        .is_some_and(|call| {
                            call.coordinate.statement_index == store.statement_index
                                && call.coordinate.call_ordinal == 0
                                && call.structural_result().is_some_and(|result| {
                                    result.statement_index == store.statement_index
                                        && result.binding_ordinal == binding_ordinal
                                        && result.multiplicity == Multiplicity::Unrestricted
                                })
                        });
                    if !completed {
                        return unsupported(
                            "byte store has no immediately preceding exact completed call",
                        );
                    }
                    continue;
                }
                checked_trees::CheckedUnitCallCoordinate {
                    statement_index: store.statement_index,
                    call_ordinal: 0,
                }
            }
            // A byte store consuming its own statement's scalar call result
            // shares that call's coordinate, so it has no separate position in
            // the canonical order. Its place is reconstructed the same way a
            // call continuation cleanup's is: the producing call must be the
            // operation immediately before it.
            CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store)
                if matches!(
                    store.value,
                    checked_trees::CheckedStoreValue::ScalarResult { .. }
                ) =>
            {
                if !preceding_scalar_call(&machine.operations, operation_index).is_some_and(
                    |(coordinate, result)| {
                        coordinate.statement_index == store.statement_index
                            && coordinate.call_ordinal == 0
                            && result.statement_index == store.statement_index
                            && result.primitive_type == checked_trees::types::PrimitiveType::U8
                    },
                ) {
                    return unsupported(
                        "indexed byte store call result has no immediately preceding scalar call",
                    );
                }
                continue;
            }
            CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) => {
                checked_trees::CheckedUnitCallCoordinate {
                    statement_index: store.statement_index,
                    call_ordinal: 0,
                }
            }
            // A store consuming its own statement's scalar call result shares
            // that call's coordinate, so it has no separate position in the
            // canonical order. Its place is reconstructed the same way a call
            // continuation cleanup's is: the producing call must be the
            // operation immediately before it.
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store)
                if matches!(
                    store.value,
                    checked_trees::CheckedStoreValue::ScalarResult { .. }
                ) =>
            {
                if !preceding_scalar_call(&machine.operations, operation_index).is_some_and(
                    |(coordinate, result)| {
                        coordinate.statement_index == store.statement_index
                            && coordinate.call_ordinal == 0
                            && result.statement_index == store.statement_index
                            && result.primitive_type == store.primitive_type
                    },
                ) {
                    return unsupported(
                        "field store call result has no immediately preceding scalar call",
                    );
                }
                continue;
            }
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => {
                checked_trees::CheckedUnitCallCoordinate {
                    statement_index: store.statement_index,
                    call_ordinal: 0,
                }
            }
            CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(store) => {
                checked_trees::CheckedUnitCallCoordinate {
                    statement_index: store.statement_index,
                    call_ordinal: 0,
                }
            }
            CheckedUnitEffectOperationPlan::StructuralCaseLiteralFieldStore(store) => {
                checked_trees::CheckedUnitCallCoordinate {
                    statement_index: store.statement_index,
                    call_ordinal: 0,
                }
            }
            // A byte-view write consuming its own statement's scalar call
            // result shares that call's coordinate, exactly like the indexed
            // byte store above.
            CheckedUnitEffectOperationPlan::ByteSequenceWrite(write)
                if matches!(
                    write.value,
                    checked_trees::CheckedStoreValue::ScalarResult { .. }
                ) =>
            {
                if !preceding_scalar_call(&machine.operations, operation_index).is_some_and(
                    |(coordinate, result)| {
                        coordinate.statement_index == write.statement_index
                            && coordinate.call_ordinal == 0
                            && result.statement_index == write.statement_index
                            && result.primitive_type == checked_trees::types::PrimitiveType::U8
                    },
                ) {
                    return unsupported(
                        "byte-view write call result has no immediately preceding scalar call",
                    );
                }
                continue;
            }
            CheckedUnitEffectOperationPlan::ByteSequenceWrite(write) => {
                checked_trees::CheckedUnitCallCoordinate {
                    statement_index: write.statement_index,
                    call_ordinal: 0,
                }
            }
            // An element-view write consuming its own statement's scalar call
            // result shares that call's coordinate, like the byte write above.
            CheckedUnitEffectOperationPlan::ElementViewWrite(write)
                if matches!(
                    write.value,
                    checked_trees::CheckedStoreValue::ScalarResult { .. }
                ) =>
            {
                if !preceding_scalar_call(&machine.operations, operation_index).is_some_and(
                    |(coordinate, result)| {
                        coordinate.statement_index == write.statement_index
                            && coordinate.call_ordinal == 0
                            && result.statement_index == write.statement_index
                    },
                ) {
                    return unsupported(
                        "element-view write call result has no immediately preceding scalar call",
                    );
                }
                continue;
            }
            CheckedUnitEffectOperationPlan::ElementViewWrite(write) => {
                checked_trees::CheckedUnitCallCoordinate {
                    statement_index: write.statement_index,
                    call_ordinal: 0,
                }
            }
            // A store through a recast loan local consuming its own
            // statement's scalar call result shares that call's coordinate,
            // like the byte and element writes above; the produced result
            // carries the view's declared carrier.
            CheckedUnitEffectOperationPlan::RepresentationViewStore(write)
                if matches!(
                    write.value,
                    checked_trees::CheckedStoreValue::ScalarResult { .. }
                ) =>
            {
                if !preceding_scalar_call(&machine.operations, operation_index).is_some_and(
                    |(coordinate, result)| {
                        coordinate.statement_index == write.statement_index
                            && coordinate.call_ordinal == 0
                            && result.statement_index == write.statement_index
                            && result.primitive_type == write.view
                    },
                ) {
                    return unsupported(
                        "representation view store call result has no immediately preceding scalar call",
                    );
                }
                continue;
            }
            CheckedUnitEffectOperationPlan::RepresentationViewStore(write) => {
                checked_trees::CheckedUnitCallCoordinate {
                    statement_index: write.statement_index,
                    call_ordinal: 0,
                }
            }
            CheckedUnitEffectOperationPlan::Complete { .. } => {
                return unsupported("Unit machine contains a nonfinal Unit return");
            }
        };
        // Every structural producer takes the next dense binding.
        if let Some(result) = operation.structural_result_binding() {
            if result.binding_ordinal != next_structural_binding {
                return unsupported("Unit structural result is not the next dense source binding");
            }
            next_structural_binding =
                next_structural_binding
                    .checked_add(1)
                    .ok_or(LoweringError::Unsupported(
                        "Unit structural result binding ordinal space is exhausted",
                    ))?;
        }
        if matches!(
            operation,
            CheckedUnitEffectOperationPlan::EstablishScalarArray {
                source: checked_trees::CheckedArrayConstructionSource::CallArgument { .. },
                ..
            } | CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                operand_source: Some(
                    checked_trees::CheckedArrayConstructionSource::CallArgument { .. },
                ),
                ..
            }
        ) {
            // A constructor is an argument-position event, not another call
            // with ordinal zero. Keep its dense result binding check above;
            // the authored schedule below verifies its exact position among
            // nested calls and constructors, including missing/duplicate owners.
            has_argument_constructors = true;
            continue;
        }
        let key = (coordinate.statement_index, coordinate.call_ordinal);
        let call_result = operation.call().map(|call| call.result());
        let structural_call = matches!(call_result, Some(CheckedUnitCallResult::Structural { .. }));
        let nested = structural_call && coordinate.call_ordinal != 0;
        let same_statement = previous.is_some_and(|previous: (u32, u32)| previous.0 == key.0);
        let nested_consumer =
            structural_call || (call_result.is_some() && coordinate.call_ordinal == 0);
        // A whole-record replacement decomposes into one ordered field store
        // per member -- a scalar store, or a descriptor store for a shared
        // view member -- and a whole array-literal replacement into one
        // primitive store per element. Each store keeps the statement's own
        // coordinate, so consecutive same-coordinate stores stay canonical
        // only by operation order; the exact roster is rejoined against the
        // authored literal in `structural_scalar_store_source`.
        let previous_operation = operation_index
            .checked_sub(1)
            .and_then(|previous| machine.operations.get(previous));
        let record_member_store = |operation: &CheckedUnitEffectOperationPlan| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
            ) || matches!(operation, CheckedUnitEffectOperationPlan::ByteSequenceStore(store)
            if matches!(
                store.destination,
                checked_trees::CheckedByteSequenceStoreDestination::ViewField { .. }
            ))
        };
        let repeated_literal_store = previous == Some(key)
            && previous_operation.is_some_and(|previous_operation| {
                (record_member_store(operation) && record_member_store(previous_operation))
                    || matches!(
                        (operation, previous_operation),
                        (
                            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. },
                            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                        )
                    )
            });
        // Coordinates retain preorder identity. Same-statement producers are
        // published in postorder; exact syntax and argument ordering rejoin in
        // structural result consumer validation after this shape check.
        if !repeated_literal_store {
            if (previous.is_some_and(|previous| previous >= key)
                && !(same_statement && previous_nested && nested_consumer))
                || (previous_nested && (!same_statement || !nested_consumer))
                || (same_statement && nested && !previous_nested)
                || coordinates.contains(&key)
                || coordinate.statement_index >= *statement_index
            {
                return unsupported("Unit machine operation order is not canonical source order");
            }
            coordinates.push(key);
        }
        previous = Some(key);
        previous_nested = nested;
    }
    if previous_nested {
        return unsupported(
            "Unit machine has an anonymous structural result without its enclosing call",
        );
    }
    if has_argument_constructors {
        super::argument_schedule::build(checked, machine)?;
    }
    Ok(())
}

/// The machine an ordinary call enters inside the Unit roster: every ordinary
/// Unit call, and an ordinary structural call whose target has a Unit body.
/// Scalar and boundary calls reach their targets through other rosters.
fn unit_closure_target(
    plans: UnitPlans<'_>,
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<symbols::SymbolHandle> {
    let call = operation.call()?;
    match (call.route(), call.result()) {
        (CheckedUnitCallRoute::Module { .. }, CheckedUnitCallResult::Unit) => {
            Some(call.target.machine)
        }
        (CheckedUnitCallRoute::Module { .. }, CheckedUnitCallResult::Structural { .. })
            if UnitBody::contains(plans, call.target.machine) =>
        {
            Some(call.target.machine)
        }
        _ => None,
    }
}

/// The operation immediately before `operation_index` when it is a call with
/// a scalar result, ordinary or boundary: its coordinate and result binding.
fn preceding_scalar_call(
    operations: &[CheckedUnitEffectOperationPlan],
    operation_index: usize,
) -> Option<(
    CheckedUnitCallCoordinate,
    &CheckedUnitScalarResultBindingPlan,
)> {
    let call = operations.get(operation_index.checked_sub(1)?)?.call()?;
    Some((call.coordinate, call.scalar_result()?))
}

pub(super) fn reject_recursive_unit_closure(
    plans: UnitPlans<'_>,
    closure: &[symbols::SymbolHandle],
) -> Result<(), LoweringError> {
    fn visit(
        plans: UnitPlans<'_>,
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
        for target in UnitBody::find(plans, symbol)?
            .operations()
            .filter_map(|operation| unit_closure_target(plans, operation))
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
