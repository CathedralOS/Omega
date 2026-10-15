use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedTrees;
use checked_trees::{
    CheckedScalarCaseComputationField, CheckedScalarCaseConstruction, CheckedScalarComputationKind,
    CheckedScalarComputationStructuralArgument,
};
type Computation = checked_trees::CheckedScalarComputationHandle;

pub(crate) fn borrowed_arguments_from_roots<'checked>(
    checked: &'checked CheckedTrees,
    roots: &[Computation],
) -> Result<Vec<&'checked checked_trees::CheckedUnitStructuralArgumentPlan>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut arguments = Vec::new();
    for handle in reachable_nodes(checked, roots)? {
        if let CheckedScalarComputationKind::Call {
            structural_arguments,
            ..
        } = plans.nodes.get(handle).kind
        {
            for argument in plans
                .structural_arguments
                .span(structural_arguments)
                .ok_or(LoweringError::Unsupported(
                    "computed address demand has an invalid argument span",
                ))?
            {
                if let Some(argument) = argument.as_place()
                    && argument.access != checked_trees::CheckedStructuralAccess::Owned
                {
                    arguments.push(argument);
                }
            }
        }
    }
    Ok(arguments)
}

pub(crate) fn call_targets(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    collect_call_targets(checked, machine, false)
}

/// Boundary dependencies of retained computations and their constructed values.
/// Discovery preserves the declaration identity; it neither selects a provider
/// nor schedules an invocation.
pub(crate) fn boundary_targets(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans.roots_for_machine(machine);
    let mut targets = Vec::new();
    for handle in reachable_nodes(checked, &roots)? {
        if let CheckedScalarComputationKind::Call {
            callee: checked_trees::CheckedScalarCallee::Boundary { machine, .. },
            ..
        } = plans.nodes.get(handle).kind
            && !targets.contains(&machine)
        {
            targets.push(machine);
        }
    }
    for operation in value_call_operations(checked, machine, None)? {
        if let Some(call) = operation.call().filter(|call| call.is_boundary())
            && !targets.contains(&call.target.machine)
        {
            targets.push(call.target.machine);
        }
    }
    Ok(targets)
}

/// Discovery only: source replay and each call's signature independently
/// validate the structural lane before any invocation is published.
pub(crate) fn structural_call_targets(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    collect_call_targets(checked, machine, true)
}

fn collect_call_targets(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    structural_only: bool,
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans.roots_for_machine(machine);
    let mut targets = Vec::new();
    for handle in reachable_nodes(checked, &roots)? {
        if let CheckedScalarComputationKind::Call {
            callee:
                checked_trees::CheckedScalarCallee::Body {
                    machine: target_machine,
                    ..
                },
            structural_arguments,
            ..
        } = plans.nodes.get(handle).kind
        {
            let arguments = plans
                .structural_arguments
                .span(structural_arguments)
                .ok_or(LoweringError::Unsupported(
                    "scalar computation closure has invalid structural arguments",
                ))?;
            if !structural_only || !arguments.is_empty() {
                targets.push(target_machine);
            }
        }
    }
    for operation in value_call_operations(checked, machine, None)? {
        if let Some(call) = operation
            .call()
            .filter(|call| call.is_module() && call.scalar_result().is_some())
        {
            if !structural_only
                || !call.structural_arguments.is_empty()
                || !call.claim_transfers().is_empty()
            {
                if !targets.contains(&call.target.machine) {
                    targets.push(call.target.machine);
                }
            }
        }
    }
    Ok(targets)
}

/// Ordinary call operations owned by retained structural computation values.
/// Scalar and operation-body consumers discover these targets through the same
/// retained dependency walk; discovery does not schedule or authorize a call.
pub(crate) fn value_call_operations(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: Option<symbols::SymbolHandle>,
) -> Result<Vec<&checked_trees::CheckedUnitEffectOperationPlan>, LoweringError> {
    let mut operations = Vec::new();
    for call in value_calls(checked, machine, state)? {
        for operation in call
            .operands()
            .iter()
            .chain(std::iter::once(call.operation()))
        {
            operations.extend(operation.with_value_calls());
        }
    }
    Ok(operations)
}

/// The structural values a machine's retained scalar computations construct
/// or observe: each computed call's produced operand and each field read's
/// receiver value, in discovery order.
pub(crate) fn reachable_structural_values(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<checked_trees::CheckedStructuralValueHandle>, LoweringError> {
    let roots = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .filter_map(|(_, root)| (root.machine == machine).then_some(root.root))
        .collect::<Vec<_>>();
    let mut values = Vec::new();
    collect_reachable_nodes(checked, &roots, &mut values)?;
    Ok(values)
}

fn value_calls(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: Option<symbols::SymbolHandle>,
) -> Result<Vec<&checked_trees::CheckedStructuralValueCall>, LoweringError> {
    let roots = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .filter_map(|(_, root)| {
            (root.machine == machine && state.is_none_or(|state| root.state == state))
                .then_some(root.root)
        })
        .collect::<Vec<_>>();
    let mut values = Vec::new();
    collect_reachable_nodes(checked, &roots, &mut values)?;
    let mut calls = Vec::new();
    for value in values {
        calls.extend(
            checked
                .facts
                .values
                .structural_values
                .calls_for_root(value)
                .unwrap_or(&[]),
        );
    }
    Ok(calls)
}

/// Discovery walks only retained roots, never abandoned speculative arena nodes.
/// Source correspondence and cycle rejection remain independent checks.
pub(crate) fn reachable_nodes(
    checked: &CheckedTrees,
    roots: &[Computation],
) -> Result<Vec<Computation>, LoweringError> {
    collect_reachable_nodes(checked, roots, &mut Vec::new())
}

fn collect_reachable_nodes(
    checked: &CheckedTrees,
    roots: &[Computation],
    structural_roots: &mut Vec<checked_trees::CheckedStructuralValueHandle>,
) -> Result<Vec<Computation>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut pending = roots.to_vec();
    let mut visited = Vec::new();
    while let Some(handle) = pending.pop() {
        if visited.contains(&handle) {
            continue;
        }
        if !plans.nodes.is_valid(handle) {
            return unsupported("scalar computation closure has an invalid node");
        }
        visited.push(handle);
        match &plans.nodes.get(handle).kind {
            CheckedScalarComputationKind::CaseMembership {
                subject,
                receiver_value,
                ..
            } => {
                if receiver_value.is_valid() {
                    if !structural_roots.contains(receiver_value) {
                        structural_roots.push(*receiver_value);
                    }
                    pending.extend(structural_dependencies(checked, *receiver_value)?.scalars);
                }
                pending.extend(
                    operand_fields(checked, subject)?
                        .iter()
                        .map(|field| field.value),
                );
            }
            CheckedScalarComputationKind::SelectedComparison { left, right, .. } => {
                pending.extend([*left, *right])
            }
            checked_trees::cast_or_qualification_triple_pattern!(operand, ..) => {
                pending.push(*operand);
            }
            CheckedScalarComputationKind::Dispatch { subject, arms, .. } => {
                pending.push(*subject);
                for arm in plans
                    .dispatch_arms
                    .span(*arms)
                    .ok_or(LoweringError::Unsupported(
                        "scalar dispatch closure has stale arms",
                    ))?
                {
                    if let checked_trees::CheckedScalarDispatchPattern::Value(pattern) = arm.pattern
                    {
                        pending.push(pattern);
                    }
                    pending.push(arm.value);
                }
            }
            CheckedScalarComputationKind::ByteSequenceLiteralComparison { subject, .. }
            | CheckedScalarComputationKind::ByteSequencePredicate { subject, .. } => {
                extend_range(subject, &mut pending);
            }
            CheckedScalarComputationKind::RetainedConstant { .. }
            | CheckedScalarComputationKind::Value(_) => {}
            CheckedScalarComputationKind::StructuralField { receiver_value, .. } => {
                if receiver_value.is_valid() {
                    if !structural_roots.contains(receiver_value) {
                        structural_roots.push(*receiver_value);
                    }
                    pending.extend(structural_dependencies(checked, *receiver_value)?.scalars);
                }
            }
            CheckedScalarComputationKind::Select {
                condition,
                when_true,
                when_false,
                ..
            } => {
                pending.extend([*condition, *when_true, *when_false]);
            }
            CheckedScalarComputationKind::Call {
                source_call,
                callee,
                arguments,
                structural_arguments,
                ..
            } => {
                let authority = callee.authority_value();
                if authority.is_valid() {
                    if !structural_roots.contains(&authority) {
                        structural_roots.push(authority);
                    }
                    pending.extend(structural_dependencies(checked, authority)?.scalars);
                }
                let structural_arguments = plans
                    .structural_arguments
                    .span(*structural_arguments)
                    .ok_or(LoweringError::Unsupported(
                    "scalar computation closure has invalid structural arguments",
                ))?;
                for argument in structural_arguments {
                    if let Some(place) = argument.as_place() {
                        extend_range(place, &mut pending);
                        if matches!(place.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. }) {
                            let root = produced_value_root(checked, *source_call, place)?.root;
                            if !structural_roots.contains(&root) {
                                structural_roots.push(root);
                            }
                            pending.extend(structural_dependencies(checked, root)?.scalars);
                        }
                    }
                    if let checked_trees::CheckedScalarComputationStructuralArgument::Case(
                        subject,
                    ) = argument
                    {
                        pending.extend(fields(checked, subject)?.iter().map(|field| field.value));
                    }
                }
                pending.extend(callee.selector());
                pending.extend(plans.operands.span(*arguments).ok_or(
                    LoweringError::Unsupported("scalar computation closure has invalid arguments"),
                )?);
            }
            CheckedScalarComputationKind::Apply { operands, .. } => pending.extend(
                plans
                    .operands
                    .span(*operands)
                    .ok_or(LoweringError::Unsupported(
                        "scalar computation closure has invalid operands",
                    ))?,
            ),
        }
    }
    Ok(visited)
}

/// Follow retained value dependencies, not source syntax. Reservation and
/// scalar discovery consume the same structural graph; neither may replace a
/// complete receiver with the field eventually observed from it.
pub(crate) struct StructuralDependencies {
    pub(crate) values: Vec<checked_trees::CheckedStructuralValueHandle>,
    pub(crate) scalars: Vec<Computation>,
}

pub(crate) fn structural_dependencies(
    checked: &CheckedTrees,
    root: checked_trees::CheckedStructuralValueHandle,
) -> Result<StructuralDependencies, LoweringError> {
    use checked_trees::{
        CheckedStructuralRecordFieldValue as Field, CheckedStructuralValueKind as Value,
    };
    let plans = &checked.facts.values.structural_values;
    let values = plans
        .value_dependencies(root)
        .ok_or(LoweringError::Unsupported(
            "structural computation has a stale value dependency",
        ))?;
    let mut scalars = Vec::new();
    for value in &values {
        match &plans.nodes.get(*value).kind {
            Value::QuotientCall { .. } => {}
            Value::Record { fields, .. } | Value::StructuralCase { fields, .. } => {
                for field in plans
                    .record_fields
                    .span(*fields)
                    .ok_or(LoweringError::Unsupported(
                        "structural computation has stale fields",
                    ))?
                {
                    match field.value {
                        Field::Scalar(root) => scalars.push(root),
                        Field::Structural(_) => {}
                    }
                }
            }
            Value::Dispatch { subject, arms } => {
                scalars.push(*subject);
                for arm in plans
                    .dispatch_arms
                    .span(*arms)
                    .ok_or(LoweringError::Unsupported(
                        "structural computation has stale arms",
                    ))?
                {
                    if let checked_trees::CheckedScalarDispatchPattern::Value(pattern) = arm.pattern
                    {
                        scalars.push(pattern);
                    }
                }
            }
            Value::Case(subject) => {
                scalars.extend(fields(checked, subject)?.iter().map(|field| field.value))
            }
            Value::ScalarArray { elements } => scalars.extend(elements),
            Value::Scalar { value: element } | Value::ZeroedScalarArray { element, .. } => {
                scalars.push(*element)
            }
            Value::BorrowedSliceView { source, region, .. }
            | Value::MutableSliceView { source, region, .. } => {
                extend_range(source, &mut scalars);
                if let checked_trees::CheckedElementViewRegion::RepresentationBytes {
                    start, ..
                } = region
                {
                    scalars.push(*start);
                }
            }
            Value::Reference { .. }
            | Value::Projection { .. }
            | Value::QuotientAdaptation { .. }
            | Value::FixedArray { .. }
            | Value::Call { .. }
            | Value::Place(_)
            | Value::ByteLiteralView { .. }
            | Value::CopiedStructuralPlace { .. }
            | Value::ZeroedStructural { .. } => {}
        }
    }
    Ok(StructuralDependencies { values, scalars })
}

/// Join a fresh operand to the structural producer retained at this exact call
/// site. Equal expressions or types in another state do not share storage.
pub(crate) fn produced_value_root<'checked>(
    checked: &'checked CheckedTrees,
    source_call: arena::Handle<checked_trees::FlowCallFact>,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
) -> Result<&'checked checked_trees::CheckedStructuralValueRoot, LoweringError> {
    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression } =
        argument.source
    else {
        return unsupported("computed structural operand has no produced expression");
    };
    if !matches!(
        argument.access,
        checked_trees::CheckedStructuralAccess::Owned
            | checked_trees::CheckedStructuralAccess::SharedBorrow
            | checked_trees::CheckedStructuralAccess::MutableBorrow
    ) {
        return unsupported("computed structural operand must transfer or lend its retained owner");
    }
    let control = &checked.facts.flow.control;
    if !control.calls.is_valid(source_call) {
        return unsupported("computed structural operand lost its checked invocation");
    }
    let call = control.calls.get(source_call);
    let mut owners = control
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|state| {
            control
                .calls
                .span_or_empty(state.calls)
                .iter()
                .any(|candidate| std::ptr::eq(candidate, call))
        });
    let state = owners.next().ok_or(LoweringError::Unsupported(
        "computed structural operand has no invocation state",
    ))?;
    if owners.next().is_some() {
        return unsupported("computed structural operand has ambiguous invocation ownership");
    }
    let statement = u32::try_from(call.statement_index).map_err(|_| {
        LoweringError::Unsupported("computed structural operand statement exceeds u32")
    })?;
    let values = &checked.facts.values.structural_values;
    let root = values
        .root_for_expression(state.state_symbol, statement, expression)
        .ok_or(LoweringError::Unsupported(
            "computed structural operand lost its complete value root",
        ))?;
    // The root keeps the declared value contract; the argument and catalog
    // name only its storage carrier's structural identity. Loan admission
    // belongs to the checked root, including a borrowed slice descriptor;
    // resolving that root does not manufacture an owned reference.
    if root.machine != state.machine_symbol
        || !values.nodes.is_valid(root.root)
        || (argument.path.is_empty()
            && validation::structural_type_identity(&checked.typed, root.type_reference, &[], &[])
                != argument.type_identity)
    {
        return unsupported("computed structural operand disagrees with its retained value");
    }
    // A projected actual keeps this complete owner's storage contract. Its
    // selected type was checked at admission; lowering resolves the retained
    // path against the owner's catalog rather than minting another value.
    Ok(root)
}

pub(crate) fn fields<'a>(
    checked: &'a CheckedTrees,
    subject: &CheckedScalarCaseConstruction,
) -> Result<&'a [CheckedScalarCaseComputationField], LoweringError> {
    checked
        .facts
        .values
        .scalar_computations
        .case_fields
        .span(subject.fields)
        .ok_or(LoweringError::Unsupported(
            "computed case has a stale field span",
        ))
}

pub(crate) fn operand_fields<'a>(
    checked: &'a CheckedTrees,
    subject: &CheckedScalarComputationStructuralArgument,
) -> Result<&'a [CheckedScalarCaseComputationField], LoweringError> {
    match subject {
        CheckedScalarComputationStructuralArgument::Case(subject) => fields(checked, subject),
        CheckedScalarComputationStructuralArgument::Place(_) => Ok(&[]),
    }
}

fn extend_range(
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    pending: &mut Vec<Computation>,
) {
    pending.extend(argument.computation_operands());
}
