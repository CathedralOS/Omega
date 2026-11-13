/*
Preparation of checked scalar and boolean expressions (`prepare_expression`), computation graphs,
byte literals, integer bit images and coercions, representation views, saturating shifts, and
scalar/return qualifications prior to operation emission.
*/

pub(crate) use crate::expression_bindings::bindings;

pub(crate) use crate::emission::scalar_types::terminal_scalar_type;
pub(crate) use crate::shared::lowering_error::{LoweringError, unsupported};
pub(crate) use checked_trees::types::PrimitiveType;
pub(crate) use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees,
    CheckedUnitStructuralPathSegment,
};
pub(crate) use semantic_vocabulary::{PlaceId, ScalarType, StructuralFieldId, StructuralTypeId};
pub(crate) use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralFieldType, StructuralParameterDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape,
};

pub(crate) mod byte_literals {

    use checked_trees::types::{
        FixedArrayLength, PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
    };
    use checked_trees::{
        CheckedTrees, CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    };
    use language_semantics::SemanticDomainId;

    use crate::shared::lowering_error::{LoweringError, unsupported};

    pub(crate) fn validate(
        checked: &CheckedTrees,
        expected: TypeReferenceHandle,
        argument: &CheckedUnitStructuralArgumentPlan,
    ) -> Result<Vec<SemanticDomainId>, LoweringError> {
        let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { bytes } =
            &argument.source
        else {
            return unsupported("record byte literal has no literal source");
        };
        let mut reference = expected;
        let mut visited = Vec::new();
        let mut domains = Vec::new();
        loop {
            if !checked
                .type_reference_table
                .contains_type_reference(reference)
                || visited.contains(&reference)
            {
                return unsupported("record byte literal has an invalid carrier chain");
            }
            visited.push(reference);
            match checked.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    let constraints = checked
                        .type_reference_table
                        .constraint_span(*constraints)
                        .ok_or(LoweringError::Unsupported(
                            "record byte literal has stale constraints",
                        ))?;
                    for constraint in constraints {
                        let TypeConstraintNode::Domain(domain) = constraint else {
                            return unsupported(
                                "record byte literal has an unsupported qualification",
                            );
                        };
                        let definition = checked.domain_definition(domain.symbol).ok_or(
                            LoweringError::Unsupported("record byte literal domain is absent"),
                        )?;
                        if !domain.semantic_id.is_valid()
                            || definition.semantic_id != domain.semantic_id
                            || !domain.arguments.is_empty()
                            || !domain.establishment_routes.is_empty()
                            || !definition.establishment_routes.is_empty()
                            || definition.semantic_roles != Default::default()
                            || !checked_trees::byte_predicates::domain_byte_predicate(
                                &checked.typed,
                                domain.symbol,
                            )
                            .is_some_and(|predicate| predicate.holds_for(bytes))
                        {
                            return unsupported(
                                "record byte literal does not establish its declared domain",
                            );
                        }
                        domains.push(domain.semantic_id);
                    }
                    reference = *base_type;
                }
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(_),
                } if !domains.is_empty()
                    && checked.primitive_type_reference(*element_type)
                        == Some(PrimitiveType::U8) =>
                {
                    domains.sort_by_key(|domain| domain.0);
                    domains.dedup();
                    return Ok(domains);
                }
                _ => {
                    return unsupported(
                        "record byte literal carrier is not a domain-qualified byte array",
                    );
                }
            }
        }
    }
}
pub(crate) mod computation_graph {

    use crate::shared::checked_facts::{flow_facts, value_facts};
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use checked_trees::CheckedTrees;
    use checked_trees::{
        CheckedScalarCaseComputationField, CheckedScalarCaseConstruction,
        CheckedScalarComputationKind, CheckedScalarComputationStructuralArgument,
    };
    type Computation = checked_trees::CheckedScalarComputationHandle;

    pub(crate) fn borrowed_arguments_from_roots<'checked>(
        checked: &'checked CheckedTrees,
        roots: &[Computation],
    ) -> Result<Vec<&'checked checked_trees::CheckedUnitStructuralArgumentPlan>, LoweringError>
    {
        let plans = &checked.facts.values.scalar_computations;
        let mut arguments = Vec::new();
        let mut structural_roots = Vec::new();
        for handle in collect_reachable_nodes(checked, roots, &mut structural_roots)? {
            if let CheckedScalarComputationKind::StorageAddress { subject, .. } =
                &plans.nodes.get(handle).kind
            {
                arguments.push(subject);
            }
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
        let mut visited = Vec::new();
        for root in structural_roots {
            for value in structural_dependencies(checked, root)?.values {
                if visited.contains(&value) {
                    continue;
                }
                visited.push(value);
                for call in value_facts(checked)
                    .structural_values
                    .call_schedule(value)
                    .map(|(_, calls)| calls)
                    .unwrap_or(&[])
                {
                    for operation in
                        call.operands().iter().chain(call.operation()).flat_map(
                            checked_trees::CheckedUnitEffectOperationPlan::with_value_calls,
                        )
                    {
                        arguments.extend(operation.structural_operands().iter().filter(
                            |argument| {
                                argument.access != checked_trees::CheckedStructuralAccess::Owned
                            },
                        ));
                    }
                }
                match &checked.facts.values.structural_values.nodes.get(value).kind {
                    checked_trees::CheckedStructuralValueKind::Reference { source, .. }
                    | checked_trees::CheckedStructuralValueKind::BorrowedSliceView {
                        source, ..
                    }
                    | checked_trees::CheckedStructuralValueKind::MutableSliceView {
                        source, ..
                    } => {
                        arguments.push(source);
                    }
                    _ => {}
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

    pub(crate) fn machine_computations(
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
    ) -> Result<Vec<Computation>, LoweringError> {
        let mut roots = value_facts(checked)
            .scalar_computations
            .roots_for_machine(machine);
        for root in value_facts(checked)
            .structural_values
            .roots_for_machine(machine)
        {
            roots.extend(structural_dependencies(checked, root.root)?.scalars);
        }
        reachable_nodes(checked, &roots)
    }

    /// Retain both invoked bodies and code stored as values, without turning a
    /// construction into an executable call edge.
    pub(crate) fn code_targets(
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
    ) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
        let mut targets = call_targets(checked, machine)?;
        for handle in machine_computations(checked, machine)? {
            if let CheckedScalarComputationKind::MachineValue { machine, .. } = checked
                .facts
                .values
                .scalar_computations
                .nodes
                .get(handle)
                .kind
                && !targets.contains(&machine)
            {
                targets.push(machine);
            }
        }
        Ok(targets)
    }

    pub(crate) fn static_realization_targets(
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
    ) -> Vec<symbols::SymbolHandle> {
        let control = &flow_facts(checked).control;
        let mut targets = Vec::new();
        for (_, state) in control
            .machine_states(machine)
            .into_iter()
            .map(|state| ((), state))
        {
            for call in control.calls.span_or_empty(state.calls) {
                if call.static_requirement_dispatch.is_none()
                    || control.is_retired(state.state_symbol, call)
                {
                    continue;
                }
                let Some(owner) = control
                    .state_by_symbol(call.target_symbol)
                    .map(|target_state| target_state.machine_symbol)
                else {
                    continue;
                };
                if owner != machine && !targets.contains(&owner) {
                    targets.push(owner);
                }
            }
        }
        targets
    }

    pub(crate) fn boundary_targets(
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
    ) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
        let plans = &checked.facts.values.scalar_computations;
        let roots = plans.roots_for_machine(machine);
        let mut targets = Vec::new();
        for handle in reachable_nodes(checked, &roots)? {
            if let CheckedScalarComputationKind::Call {
                callee:
                    checked_trees::CheckedScalarCallee::Boundary { machine, .. }
                    | checked_trees::CheckedScalarCallee::DescriptorTable { machine, .. },
                ..
            } = plans.nodes.get(handle).kind
                && !targets.contains(&machine)
            {
                targets.push(machine);
            }
        }
        for operation in value_call_operations(checked, machine, None)? {
            if let Some(call) = operation.call().filter(|call| call.is_boundary())
                && !targets.contains(&crate::shared::call_sites::declared_machine(&call.target)?)
            {
                targets.push(crate::shared::call_sites::declared_machine(&call.target)?);
            }
        }
        Ok(targets)
    }

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
        let transfers = flow_facts(checked)
            .terminal_unit_effects
            .composed_for_machine(machine)
            .into_iter()
            .flat_map(|graph| &graph.states)
            .flat_map(|state| {
                state.successors().into_iter().filter_map(|edge| {
                    if edge.is_continuation {
                        return None;
                    }
                    plans
                        .root_at(
                            state.state,
                            edge.statement_ordinal,
                            checked_trees::CheckedScalarExpressionRole::Return,
                        )
                        .map(|root| (root.root, edge.target_state))
                })
            })
            .collect::<Vec<_>>();
        let mut targets = Vec::new();
        for handle in reachable_nodes(checked, &roots)? {
            if let CheckedScalarComputationKind::Call {
                callee:
                    checked_trees::CheckedScalarCallee::Body {
                        machine: target_machine,
                        state: target_state,
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
                if !transfers.contains(&(handle, target_state))
                    && (!structural_only || !arguments.is_empty())
                {
                    targets.push(target_machine);
                }
            }
        }
        for operation in value_call_operations(checked, machine, None)? {
            if let Some(call) = operation
                .call()
                .filter(|call| call.is_module() && call.scalar_result().is_some())
                && (!structural_only
                    || !call.structural_arguments.is_empty()
                    || !call.claim_transfers().is_empty())
                && !targets.contains(&crate::shared::call_sites::declared_machine(&call.target)?)
            {
                targets.push(crate::shared::call_sites::declared_machine(&call.target)?);
            }
        }
        Ok(targets)
    }

    pub(crate) fn value_call_operations(
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: Option<symbols::SymbolHandle>,
    ) -> Result<Vec<&checked_trees::CheckedUnitEffectOperationPlan>, LoweringError> {
        let mut operations = Vec::new();
        for call in value_calls(checked, machine, state)? {
            for operation in call.operands().iter().chain(call.operation()) {
                operations.extend(operation.with_value_calls());
            }
        }
        Ok(operations)
    }

    pub(crate) fn reachable_structural_values(
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
    ) -> Result<Vec<checked_trees::CheckedStructuralValueHandle>, LoweringError> {
        let roots = value_facts(checked)
            .scalar_computations
            .roots_for_machine(machine);
        let mut values = Vec::new();
        collect_reachable_nodes(checked, &roots, &mut values)?;
        Ok(values)
    }

    fn value_calls(
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: Option<symbols::SymbolHandle>,
    ) -> Result<Vec<&checked_trees::CheckedStructuralValueCall>, LoweringError> {
        let scalar_plans = &checked.facts.values.scalar_computations;
        let roots = match state {
            Some(state) => scalar_plans
                .root_records_for_state(state)
                .into_iter()
                .filter_map(|root| (root.machine == machine).then_some(root.root))
                .collect::<Vec<_>>(),
            None => scalar_plans.roots_for_machine(machine),
        };
        let mut values = Vec::new();
        collect_reachable_nodes(checked, &roots, &mut values)?;
        let plans = &checked.facts.values.structural_values;
        let structural_roots = match state {
            Some(state) => plans.roots_for_state(state),
            None => plans.roots_for_machine(machine),
        };
        for root in structural_roots
            .into_iter()
            .filter(|root| root.machine == machine)
        {
            if !values.contains(&root.root) {
                values.push(root.root);
            }
        }
        let mut dependencies = Vec::new();
        for value in values {
            for dependency in structural_dependencies(checked, value)?.values {
                if !dependencies.contains(&dependency) {
                    dependencies.push(dependency);
                }
            }
        }
        let mut calls = Vec::new();
        for value in dependencies {
            calls.extend(
                value_facts(checked)
                    .structural_values
                    .call_schedule(value)
                    .map(|(_, calls)| calls)
                    .unwrap_or(&[]),
            );
        }
        Ok(calls)
    }

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
                    pending.extend(
                        subject.scalar_operands(&checked.facts.values.scalar_computations.operands),
                    );
                    let value = subject.structural_value();
                    if value.is_valid() {
                        if !structural_roots.contains(&value) {
                            structural_roots.push(value);
                        }
                        pending.extend(structural_dependencies(checked, value)?.scalars);
                    }
                    for arm in plans
                        .dispatch_arms
                        .span(*arms)
                        .ok_or(LoweringError::Unsupported(
                            "scalar dispatch closure has stale arms",
                        ))?
                    {
                        if let Some(pattern) = arm.pattern.computation() {
                            pending.push(pattern);
                        }
                        pending.push(arm.value);
                    }
                }
                CheckedScalarComputationKind::StorageAddress { subject, .. }
                | CheckedScalarComputationKind::ByteSequenceLiteralComparison { subject, .. }
                | CheckedScalarComputationKind::ByteSequencePredicate { subject, .. } => {
                    extend_range(checked, subject, &mut pending);
                }
                CheckedScalarComputationKind::RetainedConstant { .. }
                | CheckedScalarComputationKind::Value(_)
                | CheckedScalarComputationKind::MachineValue { .. } => {}
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
                            extend_range(checked, place, &mut pending);
                            if place.source.structural_value_dependency().is_some() {
                                let root = produced_value_root(
                                    checked,
                                    source_call.statement().ok_or(LoweringError::Unsupported(
                                        "declaration call has no structural producer custody",
                                    ))?,
                                    place,
                                )?
                                .root;
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
                            pending
                                .extend(fields(checked, subject)?.iter().map(|field| field.value));
                        }
                    }
                    pending.extend(callee.evaluated_callee());
                    pending.extend(plans.operands.span(*arguments).ok_or(
                        LoweringError::Unsupported(
                            "scalar computation closure has invalid arguments",
                        ),
                    )?);
                }
                CheckedScalarComputationKind::WithOperands {
                    operands, operand, ..
                } => {
                    pending.push(*operand);
                    for dependency in plans.evaluation_operands.span(*operands).ok_or(
                        LoweringError::Unsupported("scalar computation scope has invalid operands"),
                    )? {
                        match dependency {
                            checked_trees::CheckedScalarEvaluationOperand::Scalar(value) => {
                                pending.push(*value)
                            }
                            checked_trees::CheckedScalarEvaluationOperand::Structural(value) => {
                                if !structural_roots.contains(value) {
                                    structural_roots.push(*value);
                                }
                                pending.extend(structural_dependencies(checked, *value)?.scalars);
                            }
                        }
                    }
                }
                CheckedScalarComputationKind::Apply { operands, .. }
                | CheckedScalarComputationKind::StorageObservation { operands, .. } => {
                    pending.extend(plans.operands.span(*operands).ok_or(
                        LoweringError::Unsupported(
                            "scalar computation closure has invalid operands",
                        ),
                    )?)
                }
            }
        }
        Ok(visited)
    }

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
        let mut values = plans
            .value_dependencies(root)
            .ok_or(LoweringError::Unsupported(
                "structural computation has a stale value dependency",
            ))?;
        let mut scalars = Vec::new();
        let mut value_position = 0;
        while value_position < values.len() {
            let value = values[value_position];
            value_position += 1;
            for call in plans
                .call_schedule(value)
                .map(|(_, calls)| calls)
                .unwrap_or(&[])
            {
                for operation in call
                    .operands()
                    .iter()
                    .chain(call.operation())
                    .flat_map(checked_trees::CheckedUnitEffectOperationPlan::with_value_calls)
                {
                    if let Some((value, _)) = operation.structural_value_evaluation() {
                        for dependency in
                            plans
                                .value_dependencies(value)
                                .ok_or(LoweringError::Unsupported(
                                    "structural computation has a stale prepared value dependency",
                                ))?
                        {
                            if !values.contains(&dependency) {
                                values.push(dependency);
                            }
                        }
                    }
                    if let Some(call) = operation.call() {
                        scalars.extend(call.scalar_arguments.iter().filter_map(|argument| {
                            match argument {
                                checked_trees::CheckedCallScalarArgument::Computation(root) => {
                                    Some(*root)
                                }
                                checked_trees::CheckedCallScalarArgument::Pure(_) => None,
                            }
                        }));
                    }
                }
            }
            match &plans.nodes.get(value).kind {
                Value::NativeCodeLayout { .. } => {}
                Value::QuotientCall { .. } => {}
                Value::Record { fields, .. } | Value::StructuralCase { fields, .. } => {
                    for field in
                        plans
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
                    scalars.extend(
                        subject.scalar_operands(&checked.facts.values.scalar_computations.operands),
                    );
                    for arm in plans
                        .dispatch_arms
                        .span(*arms)
                        .ok_or(LoweringError::Unsupported(
                            "structural computation has stale arms",
                        ))?
                    {
                        if let Some(pattern) = arm.pattern.computation() {
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
                Value::ViewSubslice { start, end, .. } => {
                    scalars.extend(start.iter().chain(end.iter()).filter_map(
                        |value| match value {
                            checked_trees::CheckedCallScalarArgument::Computation(root) => {
                                Some(*root)
                            }
                            checked_trees::CheckedCallScalarArgument::Pure(_) => None,
                        },
                    ));
                }
                Value::BorrowedSliceView { source, region, .. }
                | Value::MutableSliceView { source, region, .. } => {
                    extend_range(checked, source, &mut scalars);
                    if let checked_trees::CheckedElementViewRegion::RepresentationBytes {
                        start,
                        ..
                    } = region
                    {
                        scalars.push(*start);
                    }
                }
                Value::Reference { selectors, .. } => scalars.extend(
                    value_facts(checked)
                        .scalar_computations
                        .operands
                        .span(*selectors)
                        .ok_or(LoweringError::Unsupported(
                            "reference has stale selector evaluations",
                        ))?,
                ),
                Value::Projection {
                    path,
                    projection_evaluations,
                    ..
                } => {
                    scalars.extend(checked_trees::CheckedUnitStructuralArgumentPlan::projection_computation_operands(
                        path, *projection_evaluations, &checked.facts.values.scalar_computations.operands,
                    ));
                }
                Value::Place(source) | Value::CopiedStructuralPlace { source, .. } => {
                    extend_range(checked, source, &mut scalars);
                }
                Value::QuotientAdaptation { .. }
                | Value::FixedArray { .. }
                | Value::Call { .. }
                | Value::ByteLiteralView { .. }
                | Value::ZeroedStructural { .. } => {}
            }
        }
        Ok(StructuralDependencies { values, scalars })
    }

    pub(crate) fn produced_value_root<'checked>(
        checked: &'checked CheckedTrees,
        source_call: arena::Handle<checked_trees::FlowCallFact>,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    ) -> Result<&'checked checked_trees::CheckedStructuralValueRoot, LoweringError> {
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
            expression,
            value_dependency: Some(value),
            ..
        } = argument.source
        else {
            return unsupported("computed structural operand has no produced expression");
        };
        if !matches!(
            argument.access,
            checked_trees::CheckedStructuralAccess::Owned
                | checked_trees::CheckedStructuralAccess::SharedBorrow
                | checked_trees::CheckedStructuralAccess::MutableBorrow
                | checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
        ) {
            return unsupported(
                "computed structural operand must transfer or lend its retained owner",
            );
        }
        let control = &checked.facts.flow.control;
        if !control.calls.is_valid(source_call) {
            return unsupported("computed structural operand lost its checked invocation");
        }
        let call = control.calls.get(source_call);
        let mut owners = control.call_owners(source_call).into_iter();
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
        let root =
            values
                .root_for_value_home(value, expression)
                .ok_or(LoweringError::Unsupported(
                    "computed structural operand has no unique authenticated value dependency",
                ))?;
        // The root keeps the declared value contract; the argument and catalog
        // name only its storage carrier's structural identity. Loan admission
        // belongs to the checked root, including a borrowed slice descriptor;
        // resolving that root does not manufacture an owned reference.
        if root.machine != state.machine_symbol
            || root.state != state.state_symbol
            || root.statement_ordinal != statement
            || !values.nodes.is_valid(root.root)
            || values.nodes.get(root.root).expression != expression
            || (argument.path.is_empty()
                && validation::structural_type_identity(
                    &checked.typed,
                    root.type_reference,
                    &[],
                    &[],
                ) != argument.type_identity)
        {
            return unsupported("computed structural operand disagrees with its retained value");
        }
        Ok(root)
    }

    pub(crate) fn fields<'a>(
        checked: &'a CheckedTrees,
        subject: &CheckedScalarCaseConstruction,
    ) -> Result<&'a [CheckedScalarCaseComputationField], LoweringError> {
        value_facts(checked)
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
        checked: &CheckedTrees,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        pending: &mut Vec<Computation>,
    ) {
        pending.extend(
            argument.computation_operands(&checked.facts.values.scalar_computations.operands),
        );
    }
}
pub(crate) mod integer_bit_image {

    use crate::emission::operation_emission::expressions::LoweredDirectExpression;
    use crate::emission::operation_emission::integer::LoweredIntegerBinaryKind;
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};

    pub(crate) fn integer_bit_image(
        operand: LoweredDirectExpression,
        source_type: IntegerType,
        target: ScalarType,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let Some(target_type) = target.as_integer() else {
            return unsupported("wrapping conversion requires fixed integer carriers");
        };
        if source_type == target_type {
            return Ok(operand);
        }
        if source_type.can_widen_to(target_type) {
            return Ok(LoweredDirectExpression::IntegerWiden {
                scalar_type: target,
                operand: Box::new(operand),
            });
        }
        match (source_type.sign(), target_type.sign()) {
            (IntegerSign::Unsigned, IntegerSign::Unsigned) => {
                unsigned_narrowing(operand, source_type, target, target_type)
            }
            (IntegerSign::Signed, IntegerSign::Unsigned) => {
                signed_source_unsigned_target(operand, source_type, target, target_type)
            }
            (IntegerSign::Unsigned, IntegerSign::Signed) => {
                assemble_halves(operand, source_type, target, target_type)
            }
            (IntegerSign::Signed, IntegerSign::Signed) => {
                let parts_carrier = unsigned_carrier(target_type.bits())?;
                let residue =
                    unsigned_residue(operand, source_type, target_type.bits(), parts_carrier)?;
                assemble_halves(residue, parts_carrier, target, target_type)
            }
        }
    }

    fn unsigned_narrowing(
        operand: LoweredDirectExpression,
        source_type: IntegerType,
        target: ScalarType,
        target_type: IntegerType,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let modulus =
            1_u128
                .checked_shl(u32::from(target_type.bits()))
                .ok_or(LoweringError::Unsupported(
                    "wrapping conversion modulus exceeds u128",
                ))?;
        Ok(exact_cast(
            binary(
                LoweredIntegerBinaryKind::ExactRemainder,
                source_type,
                operand,
                integer_literal(source_type, modulus)?,
            ),
            target,
        ))
    }

    fn signed_source_unsigned_target(
        operand: LoweredDirectExpression,
        source_type: IntegerType,
        target: ScalarType,
        target_type: IntegerType,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        if target_type.bits() < source_type.bits() {
            return unsigned_residue(operand, source_type, target_type.bits(), target_type);
        }
        if target_type.bits() < 64 {
            let carrier = signed_carrier_above(target_type.bits())?;
            let widened = widen(operand, carrier);
            return unsigned_residue(widened, carrier, target_type.bits(), target_type);
        }
        let carrier = signed_carrier(64)?;
        let residue = if source_type.bits() < 64 {
            widen(operand, carrier)
        } else {
            operand
        };
        assemble_halves(residue, carrier, target, target_type)
    }

    fn unsigned_residue(
        operand: LoweredDirectExpression,
        source_type: IntegerType,
        target_bits: u16,
        target_type: IntegerType,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let mask = mask_value(target_bits)?;
        Ok(exact_cast(
            and_mask(operand, source_type, mask)?,
            ScalarType::Integer(target_type),
        ))
    }

    fn assemble_halves(
        residue: LoweredDirectExpression,
        carrier: IntegerType,
        target: ScalarType,
        target_type: IntegerType,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let sign_position = u32::from(target_type.bits()) - 1;
        let low = and_mask(residue.clone(), carrier, (1_u128 << sign_position) - 1)?;
        let bit = and_mask(shift_right(residue, carrier, sign_position)?, carrier, 1)?;
        let low = exact_cast(low, target);
        let bit = exact_cast(bit, target);
        let shifted = shift_left(bit, target_type, sign_position)?;
        Ok(binary(
            LoweredIntegerBinaryKind::BitwiseOr,
            target_type,
            low,
            shifted,
        ))
    }

    fn unsigned_carrier(bits: u16) -> Result<IntegerType, LoweringError> {
        IntegerType::new(IntegerSign::Unsigned, bits)
            .map_err(|_| LoweringError::Unsupported("wrapping conversion requires a fixed carrier"))
    }

    fn signed_carrier(bits: u16) -> Result<IntegerType, LoweringError> {
        IntegerType::new(IntegerSign::Signed, bits)
            .map_err(|_| LoweringError::Unsupported("wrapping conversion requires a fixed carrier"))
    }

    fn signed_carrier_above(bits: u16) -> Result<IntegerType, LoweringError> {
        let carrier_bits = match bits {
            0..=8 => 16,
            9..=16 => 32,
            17..=32 => 64,
            _ => {
                return unsupported("wrapping conversion has no wider signed carrier");
            }
        };
        signed_carrier(carrier_bits)
    }

    pub(crate) fn mask_value(bits: u16) -> Result<u128, LoweringError> {
        1_u128
            .checked_shl(u32::from(bits))
            .and_then(|modulus| modulus.checked_sub(1))
            .ok_or(LoweringError::Unsupported(
                "wrapping conversion mask exceeds u128",
            ))
    }

    fn and_mask(
        operand: LoweredDirectExpression,
        carrier: IntegerType,
        mask: u128,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        Ok(binary(
            LoweredIntegerBinaryKind::BitwiseAnd,
            carrier,
            operand,
            integer_literal(carrier, mask)?,
        ))
    }

    pub(crate) fn shift_right(
        operand: LoweredDirectExpression,
        carrier: IntegerType,
        count: u32,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        Ok(binary(
            LoweredIntegerBinaryKind::WrappingShiftRight,
            carrier,
            operand,
            integer_literal(carrier, u128::from(count))?,
        ))
    }

    fn shift_left(
        operand: LoweredDirectExpression,
        carrier: IntegerType,
        count: u32,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        Ok(binary(
            LoweredIntegerBinaryKind::WrappingShiftLeft,
            carrier,
            operand,
            integer_literal(carrier, u128::from(count))?,
        ))
    }

    pub(crate) fn binary(
        kind: LoweredIntegerBinaryKind,
        carrier: IntegerType,
        left: LoweredDirectExpression,
        right: LoweredDirectExpression,
    ) -> LoweredDirectExpression {
        LoweredDirectExpression::IntegerBinary {
            kind,
            scalar_type: ScalarType::Integer(carrier),
            left: Box::new(left),
            right: Box::new(right),
            formation_decided: false,
        }
    }

    fn exact_cast(operand: LoweredDirectExpression, target: ScalarType) -> LoweredDirectExpression {
        LoweredDirectExpression::IntegerExactCast {
            scalar_type: target,
            operand: Box::new(operand),
            formation_decided: true,
        }
    }

    pub(crate) fn widen(
        operand: LoweredDirectExpression,
        carrier: IntegerType,
    ) -> LoweredDirectExpression {
        LoweredDirectExpression::IntegerWiden {
            scalar_type: ScalarType::Integer(carrier),
            operand: Box::new(operand),
        }
    }

    fn integer_literal(
        carrier: IntegerType,
        value: u128,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let value = match carrier.sign() {
            IntegerSign::Signed => IntegerValue::Signed(i128::try_from(value).map_err(|_| {
                LoweringError::Unsupported("wrapping conversion literal exceeds the carrier")
            })?),
            IntegerSign::Unsigned => IntegerValue::Unsigned(value),
        };
        if !carrier.admits(value) {
            return unsupported("wrapping conversion literal exceeds the carrier");
        }
        Ok(LoweredDirectExpression::IntegerLiteral {
            value,
            scalar_type: ScalarType::Integer(carrier),
        })
    }
}
pub(crate) mod integer_coercion {

    use crate::emission::operation_emission::expressions::LoweredDirectExpression;
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use semantic_vocabulary::ScalarType;

    pub(crate) fn coerce_integer_carrier(
        operand: LoweredDirectExpression,
        target: ScalarType,
        formation_decided: bool,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let Some(source_type) = operand.scalar_type().as_integer() else {
            return unsupported("exact integer coercion requires an integer or address source");
        };
        let Some(target_type) = target.as_integer() else {
            return unsupported("exact integer coercion requires an integer destination");
        };
        if source_type == target_type {
            return Ok(operand);
        }
        if !source_type.can_exact_cast_to(target_type) {
            return unsupported("exact integer coercion cannot change address models");
        }
        if source_type.can_widen_to(target_type) {
            Ok(LoweredDirectExpression::IntegerWiden {
                scalar_type: target,
                operand: Box::new(operand),
            })
        } else {
            Ok(LoweredDirectExpression::IntegerExactCast {
                scalar_type: target,
                operand: Box::new(operand),
                formation_decided,
            })
        }
    }
}
pub(crate) mod prepare_expression {

    use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
    use crate::emission::operation_emission::expressions::TERMINAL_NESTED_SELECTOR_LIMIT;
    use crate::emission::operation_emission::expressions::{
        LoweredDirectExpression, LoweredElementPathSegment,
    };
    use crate::emission::operation_emission::integer::{
        LoweredIntegerBinaryKind, LoweredIntegerComparisonKind,
    };
    use crate::emission::scalar_types::{
        integer_landing_scalar_type, integer_value, terminal_scalar_type,
    };
    use crate::expression_preparation::bindings::StructuralFieldBindings;
    use crate::expression_preparation::bindings::primitive_reads::captured_observation;
    use crate::expression_preparation::bindings::primitive_storage_read;
    use crate::expression_preparation::bindings::structural_cases::resolve;
    use crate::expression_preparation::bindings::structural_fields::CasePayloadRead;
    use crate::expression_preparation::bindings::structural_fields::boolean_observation;
    use crate::expression_preparation::bindings::view_locals::{ViewCarrier, ViewLocalBinding};
    use crate::expression_preparation::representation_view::replay;
    use crate::expression_preparation::{
        CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
        CheckedScalarExpression, LoweringError, PlaceId, PrimitiveType, ScalarType,
        StructuralTypeId, unsupported,
    };

    pub(crate) fn lower_checked_scalar_expression(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: &CheckedScalarExpression,
        proven_indexes: &[checked_trees::expression::ExpressionHandle],
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let structural_fields = StructuralFieldBindings::default();
        PreparedExpressionContext {
            structural_fields: &structural_fields,
            structural_cases: &[],
            primitive_storage: &[],
            view_locals: &[],
            structural_locals: &[],
            scalar_bindings: None,
            proven_indexes,
        }
        .lower_scalar(identities, expression)
    }

    pub(crate) fn selector_bound_is_decided(
        selector: &checked_trees::CheckedScalarSelector,
        proven_indexes: &[checked_trees::expression::ExpressionHandle],
    ) -> bool {
        if selector.entry_bound.is_some() {
            return !selector.occurrence.is_valid();
        }
        selector.occurrence.is_valid()
            && proven_indexes
                .binary_search_by_key(
                    &checked_trees::operators::handle_position(selector.occurrence),
                    |occurrence| checked_trees::operators::handle_position(*occurrence),
                )
                .is_ok()
    }

    pub(crate) struct PreparedExpressionContext<'a> {
        pub(crate) structural_fields: &'a StructuralFieldBindings,
        pub(crate) structural_cases:
            &'a [crate::expression_preparation::bindings::structural_cases::StructuralCaseBinding],
        pub(crate) primitive_storage: &'a [(symbols::SymbolHandle, PlaceId, ScalarType)],
        pub(crate) view_locals: &'a [ViewLocalBinding],
        pub(crate) structural_locals:
            &'a [crate::expression_preparation::bindings::StructuralLocalBinding],
        pub(crate) scalar_bindings: Option<&'a crate::expression_preparation::bindings::ScalarBindings>,
        pub(crate) proven_indexes: &'a [checked_trees::expression::ExpressionHandle],
    }

    #[derive(Clone, Copy)]
    enum SequenceObservationPurpose {
        Length,
        Contents,
    }

    impl<'a> PreparedExpressionContext<'a> {
        fn lower_scalar_type(
            &self,
            scalar: checked_trees::CheckedScalarType,
        ) -> Result<ScalarType, LoweringError> {
            if let Some(bindings) = self.scalar_bindings {
                return bindings.scalar_type(scalar);
            }
            match scalar {
                checked_trees::CheckedScalarType::Primitive(primitive) => {
                    terminal_scalar_type(primitive)
                }
                checked_trees::CheckedScalarType::Callable(_) => {
                    unsupported("callable read has no prepared type namespace")
                }
            }
        }

        pub(crate) fn lower_scalar(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &CheckedScalarExpression,
        ) -> Result<LoweredDirectExpression, LoweringError> {
            match expression {
                CheckedScalarExpression::ConstantApplication { .. } => {
                    Err(LoweringError::Unsupported(
                        "pending contract constant cannot be emitted as an executable value",
                    ))
                }
                CheckedScalarExpression::StructuralParameterField {
                    root:
                        root @ (checked_trees::CheckedStorageRoot::OwnedLocal { .. }
                        | checked_trees::CheckedStorageRoot::ViewLocal { .. }),
                    path,
                    scalar_type: checked_scalar_type,
                    ..
                } => {
                    // Psi 05 forms this root only for an erased call actual
                    // (`erased_actual_local_field`). It lowers like a
                    // formal's field, at the local's established home; the
                    // call site turns the result into a proof term.
                    let (home, structural_type) = self.storage_home(*root)?;
                    let binding = self.structural_fields.binding_at(&home, structural_type)?;
                    crate::expression_preparation::bindings::structural_fields::resolve(
                        identities,
                        std::slice::from_ref(&binding),
                        0,
                        path,
                        self.lower_scalar_type(*checked_scalar_type)?,
                    )
                }
                CheckedScalarExpression::StructuralSequenceLength { root, path } => self
                    .sequence_observation(
                        identities,
                        *root,
                        path,
                        SequenceObservationPurpose::Length,
                        |identities, selector| self.lower_scalar(identities, selector),
                    )?
                    .ok_or(LoweringError::Unsupported(
                        "length observation requires a live sequence descriptor",
                    )),
                CheckedScalarExpression::StorageRead {
                    symbol,
                    scalar_type: checked_scalar_type,
                } => {
                    let scalar_type = self.lower_scalar_type(*checked_scalar_type)?;
                    if let Some(bindings) = self.scalar_bindings
                        && let Some(observation) =
                            captured_observation(bindings, *symbol, scalar_type)?
                    {
                        return Ok(observation);
                    }
                    let (source, path) = primitive_storage_read(
                        self.primitive_storage,
                        self.structural_locals,
                        self.structural_fields.declarations(),
                        *symbol,
                        scalar_type,
                    )?;
                    Ok(LoweredDirectExpression::PrimitiveRead {
                        qualifications: Default::default(),
                        source: source.into(),
                        path,
                        scalar_type,
                    })
                }
                CheckedScalarExpression::Parameter {
                    position,
                    scalar_type: checked_scalar_type,
                } => Ok(LoweredDirectExpression::Parameter {
                    position: *position,
                    scalar_type: self.lower_scalar_type(*checked_scalar_type)?,
                }),
                CheckedScalarExpression::ErasedParameter {
                    position,
                    primitive_type,
                } => Ok(LoweredDirectExpression::ErasedParameter {
                    position: *position,
                    scalar_type: terminal_scalar_type(*primitive_type)?,
                }),
                CheckedScalarExpression::Local {
                    position,
                    scalar_type: checked_scalar_type,
                } => Ok(LoweredDirectExpression::Local {
                    position: *position,
                    scalar_type: self.lower_scalar_type(*checked_scalar_type)?,
                }),
                CheckedScalarExpression::StructuralParameterField {
                    root:
                        checked_trees::CheckedStorageRoot::Parameter {
                            index: parameter_position,
                        },
                    path,
                    scalar_type: checked_scalar_type,
                    ..
                } => {
                    let scalar_type = self.lower_scalar_type(*checked_scalar_type)?;
                    if let Some(bindings) = self.scalar_bindings
                        && let Some(observation) = bindings.captured_field_observation(
                            identities,
                            *parameter_position,
                            path,
                            scalar_type,
                        )?
                    {
                        return Ok(observation);
                    }
                    if matches!(
                        path.first(),
                        Some(checked_trees::CheckedStructuralPredicatePathSegment::Case(
                            _
                        ))
                    ) {
                        return Ok(
                match crate::expression_preparation::bindings::structural_fields::resolve_case_payload(
                            identities,
                            self.structural_fields,
                            *parameter_position,
                            path,
                            scalar_type,
                        )? {
                            CasePayloadRead::Observation { expression } => expression,
                            CasePayloadRead::Established { position } => {
                                LoweredDirectExpression::Parameter {
                                    position,
                                    scalar_type,
                                }
                            }
                        },
                    );
                    }
                    crate::expression_preparation::bindings::structural_fields::resolve(
                        identities,
                        self.structural_fields,
                        *parameter_position,
                        path,
                        scalar_type,
                    )
                }
                CheckedScalarExpression::IntegerLiteral { literal } => {
                    let scalar_type = integer_landing_scalar_type(literal)?;
                    Ok(LoweredDirectExpression::IntegerLiteral {
                        value: integer_value(literal, scalar_type)?,
                        scalar_type,
                    })
                }
                CheckedScalarExpression::IeeeFloatLiteral { value } => {
                    Ok(LoweredDirectExpression::IeeeFloatLiteral { value: *value })
                }
                CheckedScalarExpression::ImplementationSelection {
                    implementation,
                    interface_identity,
                } => Ok(LoweredDirectExpression::ImplementationSelection {
                    implementation: *implementation,
                    interface_identity: *interface_identity,
                    scalar_type: crate::emission::scalar_types::selector_scalar_type(),
                }),
                CheckedScalarExpression::IntegerBinary {
                    kind,
                    primitive_type,
                    left,
                    right,
                    formation_decided,
                    ..
                } => {
                    let scalar_type = terminal_scalar_type(*primitive_type)?;
                    let left = self.lower_scalar(identities, left)?;
                    let right = self.lower_scalar(identities, right)?;
                    if *kind == CheckedIntegerBinaryKind::SaturatingShiftLeft {
                        return super::saturating_shift::left(scalar_type, left, right);
                    }
                    Ok(LoweredDirectExpression::IntegerBinary {
                        kind: identical_lowered_integer_binary_kind(*kind),
                        scalar_type,
                        left: Box::new(left),
                        right: Box::new(right),
                        formation_decided: *formation_decided,
                    })
                }
                CheckedScalarExpression::IntegerBitwiseNot {
                    primitive_type,
                    operand,
                } => Ok(LoweredDirectExpression::IntegerBitwiseNot {
                    scalar_type: terminal_scalar_type(*primitive_type)?,
                    operand: Box::new(self.lower_scalar(identities, operand)?),
                }),
                CheckedScalarExpression::IntegerWiden {
                    primitive_type,
                    operand,
                } => Ok(LoweredDirectExpression::IntegerWiden {
                    scalar_type: terminal_scalar_type(*primitive_type)?,
                    operand: Box::new(self.lower_scalar(identities, operand)?),
                }),
                CheckedScalarExpression::StructuralParameterIndexedRead {
                    root,
                    path,
                    index,
                    element_path,
                    scalar_type: checked_scalar_type,
                    ..
                } => {
                    let bound_decided = selector_bound_is_decided(index, self.proven_indexes);
                    if let checked_trees::CheckedStorageRoot::ViewLocal { symbol } = *root
                        && let Some(bindings) = self.scalar_bindings
                        && let Some(observation) = bindings.byte_record_observation(
                            identities,
                            self.proven_indexes,
                            symbol,
                            path,
                            &index.expression,
                            element_path,
                            *checked_scalar_type,
                        )?
                    {
                        return Ok(observation);
                    }

                    let lower_selector =
                        |builder: &mut terminal_psi::TerminalIdentityTableBuilder,
                         selector: &CheckedScalarExpression| {
                            self.lower_scalar(builder, selector)
                                .and_then(exact_u64_index)
                        };
                    let index = Box::new(lower_selector(identities, &index.expression)?);
                    if element_path.is_empty()
                        && *checked_scalar_type == PrimitiveType::U8
                        && let Some(LoweredDirectExpression::ByteSequenceFieldLength {
                            source,
                            path,
                            field,
                            ..
                        }) = self.sequence_observation(
                            identities,
                            *root,
                            &path
                                .iter()
                                .cloned()
                                .map(checked_trees::CheckedScalarReadProjection::Static)
                                .collect::<Vec<_>>(),
                            SequenceObservationPurpose::Contents,
                            |_, _| {
                                unsupported("indexed byte observation has an unevaluated selector")
                            },
                        )?
                    {
                        return Ok(LoweredDirectExpression::ByteSequenceFieldRead {
                            source,
                            path,
                            field,
                            index,
                            scalar_type: self.lower_scalar_type(*checked_scalar_type)?,
                            bound_decided,
                        });
                    }
                    let mut selectors = Vec::new();
                    let geometry = element_path
                        .iter()
                        .map(|step| match step {
                            checked_trees::CheckedScalarReadProjection::Static(segment) => {
                                segment.clone()
                            }
                            checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) => {
                                let depth = selectors.len() as u32;
                                selectors.push(selector);
                                checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(
                                    checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth },
                                )
                            }
                        })
                        .collect::<Vec<_>>();
                    let (home, structural_type) = self.observation_home(*root)?;
                    let observation =
                    crate::expression_preparation::bindings::structural_fields::resolve_indexed_element(identities, self.structural_fields, &home, structural_type, path, &geometry)?;
                    let crate::expression_preparation::bindings::structural_fields::IndexedElementObservation {
                    source, path, element_path: explicit_path, scalar_type, extent,
                } = observation;
                    if scalar_type != self.lower_scalar_type(*checked_scalar_type)? {
                        return unsupported("indexed read changes its declared scalar carrier");
                    }
                    use crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment as Prepared;
                    let mut element_path = Vec::with_capacity(explicit_path.len());
                    for step in explicit_path {
                        let dynamic_extent = matches!(
                            element_path.last(),
                            Some(LoweredElementPathSegment::ViewExtent { .. })
                        );
                        element_path.push(match step {
                            Prepared::RuntimeIndex(
                                checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth },
                            ) => {
                                let selector = selectors.get(depth as usize).ok_or(
                                    LoweringError::Unsupported(
                                        "indexed read lost its selector position",
                                    ),
                                )?;
                                LoweredElementPathSegment::RuntimeIndex {
                                    selector: exact_u64_index(lower_selector(
                                        identities,
                                        &selector.expression,
                                    )?)?,
                                    extent: None,
                                    bound_decided: selector_bound_is_decided(
                                        selector,
                                        self.proven_indexes,
                                    ),
                                }
                            }
                            Prepared::Static(terminal_psi::StructuralPathSegment::FixedIndex(
                                position,
                            )) if dynamic_extent => LoweredElementPathSegment::RuntimeIndex {
                                selector: LoweredDirectExpression::IntegerLiteral {
                                    value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(
                                        position,
                                    )),
                                    scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
                                },
                                extent: None,
                                bound_decided: false,
                            },
                            Prepared::Static(segment) => LoweredElementPathSegment::Static(segment),
                            Prepared::ViewExtent { bytes } => {
                                LoweredElementPathSegment::ViewExtent { source, bytes }
                            }
                            _ => return unsupported("indexed read has an unsupported projection"),
                        });
                    }
                    let selectors = path
                        .iter()
                        .filter(|step| {
                            matches!(
                                step,
                                terminal_psi::StructuralPathSegment::RuntimeIndex { .. }
                            )
                        })
                        .count()
                        + element_path
                            .iter()
                            .filter(|step| {
                                matches!(step, LoweredElementPathSegment::RuntimeIndex { .. })
                            })
                            .count();
                    if selectors > TERMINAL_NESTED_SELECTOR_LIMIT {
                        return unsupported("indexed read exceeds its selector namespace");
                    }
                    Ok(LoweredDirectExpression::IndexedPrimitiveRead {
                        qualifications: Default::default(),
                        source,
                        path,
                        index,
                        extent,
                        element_path,
                        scalar_type,
                        bound_decided,
                    })
                }
                CheckedScalarExpression::RepresentationView {
                    primitive_type,
                    operand,
                } => {
                    let operand = self.lower_scalar(identities, operand)?;
                    replay(operand, terminal_scalar_type(*primitive_type)?)
                }
                CheckedScalarExpression::IntegerExactCast {
                    primitive_type,
                    operand,
                    formation_decided,
                    ..
                } => crate::expression_preparation::integer_coercion::coerce_integer_carrier(
                    self.lower_scalar(identities, operand)?,
                    terminal_scalar_type(*primitive_type)?,
                    *formation_decided,
                ),
                CheckedScalarExpression::Boolean(expression) => {
                    Ok(LoweredDirectExpression::Boolean {
                        expression: Box::new(self.lower_boolean(identities, expression)?),
                    })
                }
            }
        }

        fn lower_boolean(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &CheckedBooleanExpression,
        ) -> Result<LoweredBooleanReturnExpression, LoweringError> {
            let mut lower =
                |expression: &CheckedBooleanExpression| self.lower_boolean(identities, expression);
            Ok(match expression {
                CheckedBooleanExpression::Constant(value) => {
                    LoweredBooleanReturnExpression::Constant { value: *value }
                }
                CheckedBooleanExpression::Parameter { position } => {
                    LoweredBooleanReturnExpression::Parameter {
                        position: *position,
                    }
                }
                CheckedBooleanExpression::StorageRead { symbol } => {
                    if let Some(bindings) = self.scalar_bindings
                        && let Some(observation) =
                            captured_observation(bindings, *symbol, ScalarType::Boolean)?
                    {
                        return Ok(LoweredBooleanReturnExpression::ScalarObservation {
                            operand: Box::new(observation),
                        });
                    }
                    let (source, path) = primitive_storage_read(
                        self.primitive_storage,
                        self.structural_locals,
                        self.structural_fields.declarations(),
                        *symbol,
                        ScalarType::Boolean,
                    )?;
                    LoweredBooleanReturnExpression::PrimitiveRead {
                        qualifications: Default::default(),
                        source: source.into(),
                        path,
                    }
                }
                CheckedBooleanExpression::Local { position } => {
                    LoweredBooleanReturnExpression::Local {
                        position: *position,
                    }
                }
                CheckedBooleanExpression::StructuralParameterField {
                    parameter_position,
                    path,
                    ..
                } => {
                    if let Some(bindings) = self.scalar_bindings
                        && let Some(observation) = bindings.captured_field_observation(
                            identities,
                            *parameter_position,
                            path,
                            semantic_vocabulary::ScalarType::Boolean,
                        )?
                    {
                        return Ok(boolean_observation(observation));
                    }

                    if !self.structural_fields.is_empty()
                        && matches!(
                            path.first(),
                            Some(checked_trees::CheckedStructuralPredicatePathSegment::Case(
                                _
                            ))
                        )
                    {
                        return Ok(
                        match crate::expression_preparation::bindings::structural_fields::resolve_case_payload(
                            identities,
                            self.structural_fields,
                            *parameter_position,
                            path,
                            ScalarType::Boolean,
                        )? {
                            CasePayloadRead::Observation { expression } =>
                                boolean_observation(
                expression),
                            CasePayloadRead::Established { position } => {
                                LoweredBooleanReturnExpression::Parameter { position }
                            }
                        },
                    );
                    }
                    if !self.structural_fields.is_empty() {
                        let observation =
                            crate::expression_preparation::bindings::structural_fields::resolve(
                                identities,
                                self.structural_fields,
                                *parameter_position,
                                path,
                                ScalarType::Boolean,
                            )?;
                        return Ok(boolean_observation(observation));
                    }
                    let path = path
                        .iter()
                        .map(|segment| match segment {
                            checked_trees::CheckedStructuralPredicatePathSegment::Field(
                                identity,
                            ) => Ok(identity.spelling()),
                            checked_trees::CheckedStructuralPredicatePathSegment::Referent
                            | checked_trees::CheckedStructuralPredicatePathSegment::Case(_)
                            | checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(_)
                            | checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(
                                _,
                            ) => unsupported("case-payload predicates are contract-only"),
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    LoweredBooleanReturnExpression::UnresolvedStructuralParameterField {
                        parameter_position: *parameter_position,
                        path,
                    }
                }
                CheckedBooleanExpression::ByteSequencePredicate {
                    predicate,
                    root,
                    path,
                } => {
                    let operand = self.byte_sequence_observation(identities, *root, path)?;
                    LoweredBooleanReturnExpression::ByteSequencePredicate {
                        predicate: *predicate,
                        source: operand.source,
                        path: operand.path,
                        field: operand.field,
                    }
                }
                CheckedBooleanExpression::ByteSequenceEqual { left, right } => {
                    let operand =
                        |builder: &mut terminal_psi::TerminalIdentityTableBuilder,
                         operand: &checked_trees::CheckedByteSequenceOperand| {
                            self.byte_sequence_observation(builder, operand.root, &operand.path)
                        };
                    LoweredBooleanReturnExpression::ByteSequenceEqual {
                        left: operand(identities, left)?,
                        right: operand(identities, right)?,
                    }
                }
                CheckedBooleanExpression::Not(operand) => LoweredBooleanReturnExpression::Not {
                    operand: Box::new(lower(operand)?),
                },
                CheckedBooleanExpression::Equal { left, right } => {
                    LoweredBooleanReturnExpression::Equal {
                        left: Box::new(lower(left)?),
                        right: Box::new(lower(right)?),
                    }
                }
                CheckedBooleanExpression::IntegerMathComparison { .. } => {
                    return unsupported(
                        "mathematical contract predicate has no executable Boolean value",
                    );
                }
                CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
                    LoweredBooleanReturnExpression::IntegerComparison {
                        kind: match kind {
                            CheckedIntegerComparisonKind::Equal => {
                                LoweredIntegerComparisonKind::Equal
                            }
                            CheckedIntegerComparisonKind::LessThan => {
                                LoweredIntegerComparisonKind::LessThan
                            }
                            CheckedIntegerComparisonKind::LessOrEqual => {
                                LoweredIntegerComparisonKind::LessOrEqual
                            }
                        },
                        left: Box::new(self.lower_scalar(identities, left)?),
                        right: Box::new(self.lower_scalar(identities, right)?),
                    }
                }
                CheckedBooleanExpression::StructuralCaseMembership { subject, case } => {
                    let subject = subject.parameter_field().ok_or(LoweringError::Unsupported(
                        "contract result cannot be used as an executable entry observation",
                    ))?;
                    let (source, path, case) =
                        resolve(identities, self.structural_cases, &subject, case)?;
                    LoweredBooleanReturnExpression::StructuralCaseMembership { source, path, case }
                }
                CheckedBooleanExpression::IeeeFloatComparison { .. }
                | CheckedBooleanExpression::StructuralIdentity { .. }
                | CheckedBooleanExpression::ScalarIeeeFloatComparison { .. }
                | CheckedBooleanExpression::ErasedParameter { .. } => {
                    return unsupported("structural equality is contract-only terminal vocabulary");
                }
                CheckedBooleanExpression::And { left, right } => {
                    LoweredBooleanReturnExpression::And {
                        left: Box::new(lower(left)?),
                        right: Box::new(lower(right)?),
                    }
                }
                CheckedBooleanExpression::Or { left, right } => {
                    LoweredBooleanReturnExpression::Or {
                        left: Box::new(lower(left)?),
                        right: Box::new(lower(right)?),
                    }
                }
            })
        }

        fn storage_home(
            &self,
            root: checked_trees::CheckedStorageRoot,
        ) -> Result<(terminal_psi::StructuralArgument, StructuralTypeId), LoweringError> {
            let captured = self
                .scalar_bindings
                .map(|bindings| bindings.captured_storage_home(root))
                .transpose()?
                .flatten();
            let (home, structural_type) = match captured {
                Some(home) => home,
                None => self.structural_fields.storage_home(
                    root,
                    self.structural_locals,
                    self.view_locals,
                )?,
            };
            Ok((home, structural_type))
        }

        fn observation_home(
            &self,
            root: checked_trees::CheckedStorageRoot,
        ) -> Result<(terminal_psi::StructuralArgument, StructuralTypeId), LoweringError> {
            let (home, structural_type) = self.storage_home(root)?;
            if !matches!(
                home.access,
                terminal_psi::StructuralAccess::Owned
                    | terminal_psi::reading_borrow_pair_pattern!()
            ) {
                return unsupported("storage observation requires readable access");
            }
            Ok((home, structural_type))
        }

        fn byte_sequence_observation(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            root: checked_trees::CheckedStorageRoot,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
        ) -> Result<
            crate::emission::operation_emission::boolean::LoweredByteSequenceOperand,
            LoweringError,
        > {
            use crate::emission::operation_emission::boolean::LoweredByteSequenceOperand;
            let observation = self
                .sequence_observation(
                    identities,
                    root,
                    &path
                        .iter()
                        .cloned()
                        .map(checked_trees::CheckedScalarReadProjection::Static)
                        .collect::<Vec<_>>(),
                    SequenceObservationPurpose::Contents,
                    |_, _| unsupported("byte observation has an unevaluated selector"),
                )?
                .ok_or(LoweringError::Unsupported(
                    "byte observation has no readable byte extent",
                ))?;
            match observation {
                LoweredDirectExpression::ByteSequenceFieldLength {
                    source,
                    path,
                    field,
                    ..
                } => Ok(LoweredByteSequenceOperand {
                    source,
                    path,
                    field: Some(field),
                }),
                LoweredDirectExpression::ByteSequenceLength { source, path, .. } => {
                    Ok(LoweredByteSequenceOperand {
                        source,
                        path,
                        field: None,
                    })
                }
                _ => unsupported("byte observation requires byte elements"),
            }
        }

        fn sequence_observation(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            root: checked_trees::CheckedStorageRoot,
            path: &[checked_trees::CheckedScalarReadProjection],
            purpose: SequenceObservationPurpose,
            mut lower_selector: impl FnMut(
                &mut terminal_psi::TerminalIdentityTableBuilder,
                &CheckedScalarExpression,
            )
                -> Result<LoweredDirectExpression, LoweringError>,
        ) -> Result<Option<LoweredDirectExpression>, LoweringError> {
            use crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment as Prepared;
            use checked_trees::{CheckedRuntimeIndex, CheckedScalarReadProjection as Projection};
            let (home, structural_type) = match purpose {
                SequenceObservationPurpose::Length => self.storage_home(root)?,
                SequenceObservationPurpose::Contents => self.observation_home(root)?,
            };
            let mut selectors = Vec::new();
            let geometry = path
                .iter()
                .map(|step| match step {
                    Projection::Static(segment) => segment.clone(),
                    Projection::RuntimeIndex(selector) => {
                        let depth = selectors.len() as u32;
                        selectors.push(selector);
                        checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(
                            CheckedRuntimeIndex::AssignmentIndex { depth },
                        )
                    }
                })
                .collect::<Vec<_>>();
            let Some((steps, field, carrier)) =
            crate::expression_preparation::bindings::structural_fields::prepare_sequence_observation(
                identities,
                structural_type,
                self.structural_fields.declarations(),
                &geometry,
            )?
        else {
            return Ok(None);
        };
            let metadata_access = if home.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
            {
                terminal_psi::StructuralAccess::WriteOnlyBorrow
            } else {
                terminal_psi::StructuralAccess::SharedBorrow
            };
            let source = home.place;
            let mut path = home
                .path
                .into_iter()
                .map(LoweredElementPathSegment::Static)
                .collect::<Vec<_>>();
            for step in steps {
                let dynamic_extent = matches!(
                    path.last(),
                    Some(LoweredElementPathSegment::ViewExtent { .. })
                );
                path.push(match step {
                    Prepared::Static(terminal_psi::StructuralPathSegment::FixedIndex(position))
                        if dynamic_extent =>
                    {
                        LoweredElementPathSegment::RuntimeIndex {
                            selector: LoweredDirectExpression::IntegerLiteral {
                                value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(
                                    position,
                                )),
                                scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
                            },
                            extent: None,
                            bound_decided: false,
                        }
                    }
                    Prepared::Static(segment) => LoweredElementPathSegment::Static(segment),
                    Prepared::ViewExtent { bytes } => {
                        LoweredElementPathSegment::ViewExtent { source, bytes }
                    }
                    Prepared::RuntimeIndex(CheckedRuntimeIndex::AssignmentIndex { depth }) => {
                        let selector =
                            selectors
                                .get(depth as usize)
                                .ok_or(LoweringError::Unsupported(
                                    "length observation lost its evaluated selector",
                                ))?;
                        LoweredElementPathSegment::RuntimeIndex {
                            selector: exact_u64_index(lower_selector(
                                identities,
                                &selector.expression,
                            )?)?,
                            extent: None,
                            bound_decided: selector_bound_is_decided(selector, self.proven_indexes),
                        }
                    }
                    Prepared::RuntimeIndex(index) => LoweredElementPathSegment::RuntimeIndex {
                        selector: exact_u64_index(
                            self.element_path_runtime_selector(identities, &index)?,
                        )?,
                        extent: None,
                        bound_decided: false,
                    },
                    _ => return unsupported("length observation requires a completed selector"),
                });
            }
            if path
                .iter()
                .filter(|step| matches!(step, LoweredElementPathSegment::RuntimeIndex { .. }))
                .count()
                > TERMINAL_NESTED_SELECTOR_LIMIT
            {
                return unsupported("length observation exceeds its runtime selector namespace");
            }
            let scalar_type = terminal_scalar_type(PrimitiveType::U64)?;
            Ok(Some(match (carrier, field) {
                (ViewCarrier::Bytes, Some(field)) => {
                    LoweredDirectExpression::ByteSequenceFieldLength {
                        source,
                        path,
                        field,
                        scalar_type,
                    }
                }
                (ViewCarrier::Bytes | ViewCarrier::FixedBytes, None) => {
                    LoweredDirectExpression::ByteSequenceLength {
                        source,
                        path,
                        access: metadata_access,
                        scalar_type,
                    }
                }
                (ViewCarrier::Elements { .. }, None) => {
                    LoweredDirectExpression::ElementViewLength {
                        source,
                        path,
                        access: metadata_access,
                        scalar_type,
                    }
                }
                _ => {
                    return unsupported("length observation has an inconsistent sequence endpoint");
                }
            }))
        }

        fn element_path_runtime_selector(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            index: &checked_trees::CheckedRuntimeIndex,
        ) -> Result<LoweredDirectExpression, LoweringError> {
            match index {
                checked_trees::CheckedRuntimeIndex::StorageRead { symbol } => self
                    .scalar_bindings
                    .ok_or(LoweringError::Unsupported(
                        "storage selector has no established scalar homes",
                    ))?
                    .runtime_index_storage(*symbol),
                checked_trees::CheckedRuntimeIndex::Parameter { position } => self
                    .scalar_bindings
                    .ok_or(LoweringError::Unsupported(
                        "element selector has no typed scalar namespace",
                    ))?
                    .runtime_index_parameter(*position),
                checked_trees::CheckedRuntimeIndex::Local { position } => self
                    .scalar_bindings
                    .ok_or(LoweringError::Unsupported(
                        "element selector has no typed scalar namespace",
                    ))?
                    .runtime_index_local(*position),
                checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
                    let observation =
                        crate::expression_preparation::bindings::structural_fields::resolve_typed(
                            identities,
                            self.structural_fields,
                            field.parameter_position,
                            &field.path,
                        )?;
                    if !matches!(observation.scalar_type(), ScalarType::Integer(integer) if !integer.is_address())
                    {
                        return unsupported("element path runtime selector is not an integer read");
                    }
                    Ok(observation)
                }
                _ => unsupported("element path runtime selector is not a parameter or field read"),
            }
        }
    }

    pub(crate) fn exact_u64_index(
        index: LoweredDirectExpression,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let u64_type = terminal_scalar_type(PrimitiveType::U64)?;
        match index.scalar_type() {
            scalar_type if scalar_type == u64_type => Ok(index),
            ScalarType::Integer(integer)
                if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned =>
            {
                Ok(LoweredDirectExpression::IntegerWiden {
                    scalar_type: u64_type,
                    operand: Box::new(index),
                })
            }
            ScalarType::Integer(_) => Ok(LoweredDirectExpression::IntegerExactCast {
                scalar_type: u64_type,
                operand: Box::new(index),
                formation_decided: false,
            }),
            _ => unsupported("indexed reads require an integer index"),
        }
    }

    macro_rules! identical_lowered_integer_binary_kinds {
        ($($kind:ident),* $(,)?) => {
            fn identical_lowered_integer_binary_kind(
                kind: CheckedIntegerBinaryKind,
            ) -> LoweredIntegerBinaryKind {
                match kind {
                    $(CheckedIntegerBinaryKind::$kind => LoweredIntegerBinaryKind::$kind,)*
                    CheckedIntegerBinaryKind::SaturatingShiftRight => {
                        LoweredIntegerBinaryKind::ExactShiftRight
                    }
                    CheckedIntegerBinaryKind::SaturatingShiftLeft => {
                        unreachable!("saturating shift left lowers through its composed form")
                    }
                }
            }
        };
    }

    identical_lowered_integer_binary_kinds!(
        ExactAdd,
        ExactSubtract,
        ExactMultiply,
        ExactDivide,
        ExactRemainder,
        WrappingDivide,
        WrappingRemainder,
        SaturatingDivide,
        SaturatingRemainder,
        WrappingAdd,
        SaturatingAdd,
        WrappingSubtract,
        SaturatingSubtract,
        WrappingMultiply,
        SaturatingMultiply,
        BitwiseAnd,
        BitwiseOr,
        BitwiseXor,
        WrappingShiftLeft,
        WrappingShiftRight,
        ExactShiftLeft,
        ExactShiftRight,
        TrappingShiftLeft,
        TrappingShiftRight,
        TrappingAdd,
        TrappingSubtract,
        TrappingMultiply,
        TrappingDivide,
        TrappingRemainder,
    );
}
pub(crate) mod qualifications {

    use crate::emission::scalar_types::terminal_scalar_type;
    use crate::shared::checked_facts::{flow_facts, value_facts};
    use crate::shared::lowering_error::LoweringError;
    use crate::shared::lowering_error::unsupported;
    use checked_trees::types::{
        DomainConstraintSubject, PrimitiveType, TypeConstraintNode, TypeReferenceHandle,
        TypeReferenceNode,
    };
    use checked_trees::{CheckedScalarComputationKind, CheckedTrees};
    use language_semantics::{DomainEstablishmentRoute, SemanticDomainId};
    use semantic_vocabulary::{
        DomainSemanticId, QualifiedScalarType, ScalarDomainId, ScalarQualificationSetId,
    };
    use symbols::SymbolHandle;
    use terminal_psi::{
        ScalarDomainDeclaration, ScalarDomainEstablishmentRoute, ScalarQualificationCatalog,
        ScalarQualificationSet,
    };

    pub(crate) mod domain_predicates {

        use semantic_vocabulary::Proposition;
        use terminal_psi::{ScalarDomainDeclaration, ScalarDomainPredicate};

        pub(crate) fn complete_value_components(domains: &mut [ScalarDomainDeclaration]) {
            let represented = domains
                .iter()
                .map(|domain| domain.required_memberships_are_well_formed(domains))
                .collect::<Vec<_>>();
            for (domain, represented) in domains.iter_mut().zip(represented) {
                if !represented {
                    domain.required_memberships = None;
                    domain.predicate = None;
                }
            }
            let mut values = vec![None; domains.len()];
            for index in 0..domains.len() {
                complete(domains, index, &mut Vec::new(), &mut values);
            }
            for (domain, value) in domains.iter_mut().zip(values) {
                if domain.predicate.is_none()
                    && domain.has_no_required_memberships()
                    && domain.establishment_routes.is_empty()
                {
                    continue;
                }
                domain.predicate = value.flatten().map(|proposition| ScalarDomainPredicate {
                    subject: crate::shared::terminal_identities::value_id(1),
                    proposition,
                });
            }
        }

        fn complete(
            domains: &[ScalarDomainDeclaration],
            index: usize,
            active: &mut Vec<usize>,
            values: &mut [Option<Option<Proposition>>],
        ) -> Option<Proposition> {
            if let Some(value) = &values[index] {
                return value.clone();
            }
            if active.contains(&index) {
                return None;
            }
            let domain = &domains[index];
            let mut value = match &domain.predicate {
                Some(predicate) => predicate.proposition.clone(),
                None if domain.has_no_required_memberships()
                    && domain.establishment_routes.is_empty() =>
                {
                    Proposition::Truth
                }
                None => return None,
            };
            active.push(index);
            let result = (|| {
                if let Some(required) = &domain.required_memberships {
                    for parent in required {
                        let mut matches = domains.iter().enumerate().filter(|(_, candidate)| {
                            candidate.semantic_domain == *parent
                                && candidate.carrier == domain.carrier
                        });
                        let (parent, _) = matches.next()?;
                        if matches.next().is_some() {
                            return None;
                        }
                        let predicate = complete(domains, parent, active, values)?;
                        value = match (value, predicate) {
                            (Proposition::Truth, right) => right,
                            (left, Proposition::Truth) => left,
                            (Proposition::Falsehood, _) | (_, Proposition::Falsehood) => {
                                Proposition::Falsehood
                            }
                            (left, right) if left == right => left,
                            (left, right) => Proposition::Conjunction(vec![left, right]),
                        };
                    }
                }
                Some(value)
            })();
            active.pop();
            values[index] = Some(result.clone());
            result
        }
    }
    pub(crate) mod fields {

        use super::PreparedScalarQualifications;
        use crate::shared::lowering_error::{LoweringError, unsupported};
        use checked_trees::CheckedTrees;
        use checked_trees::types::TypeReferenceHandle;
        use semantic_vocabulary::StructuralFieldId;

        pub(crate) type FieldType = (StructuralFieldId, TypeReferenceHandle);

        impl PreparedScalarQualifications {
            pub(crate) fn retain_fields(
                &mut self,
                checked: &CheckedTrees,
                fields: &[FieldType],
            ) -> Result<(), LoweringError> {
                for (field, reference) in fields {
                    let value_type = self.value_type(checked, *reference)?;
                    if !value_type.qualifications.is_empty() {
                        self.catalog
                            .fields
                            .push(terminal_psi::ScalarFieldQualification {
                                field: *field,
                                value_type,
                            });
                    }
                }
                self.catalog.fields.sort_by_key(|row| row.field);
                if self
                    .catalog
                    .fields
                    .windows(2)
                    .any(|pair| pair[0].field == pair[1].field)
                {
                    return unsupported("scalar field membership has duplicate declarations");
                }
                Ok(())
            }
        }
    }
    pub(crate) mod issuers {

        use super::{
            CheckedTrees, DomainEstablishmentRoute, LoweringError, ScalarDomainEstablishmentRoute,
        };
        use crate::machine_lowering::declarations::semantic_identity;
        use crate::proofs::evidence_lowering::{
            checked_evidence_machine_identity, checked_evidence_requirement_identity,
        };

        pub(crate) fn establishment_route(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            route: &DomainEstablishmentRoute,
        ) -> Result<ScalarDomainEstablishmentRoute, LoweringError> {
            let declaration = terminal_psi::NominalDeclarationIdentity::new(
                semantic_identity(identities, checked, route.established_declaration()).map_err(
                    |_| LoweringError::Unsupported("domain issuer has no exact declaration owner"),
                )?,
            );
            Ok(match *route {
                DomainEstablishmentRoute::CheckedRequirement {
                    trait_definition,
                    requirement,
                } => ScalarDomainEstablishmentRoute::CheckedRequirement {
                    declaration,
                    requirement_identity: identities.intern(
                        &checked_evidence_requirement_identity(
                            checked,
                            trait_definition,
                            requirement,
                        )?,
                    ),
                },
                DomainEstablishmentRoute::BoundaryRequirement {
                    boundary_trait,
                    requirement,
                } => ScalarDomainEstablishmentRoute::BoundaryRequirement {
                    declaration,
                    requirement_identity: identities.intern(
                        &checked_evidence_requirement_identity(
                            checked,
                            boundary_trait,
                            requirement,
                        )?,
                    ),
                },
                DomainEstablishmentRoute::ExactMachine { machine } => {
                    ScalarDomainEstablishmentRoute::ExactMachine {
                        declaration,
                        machine_identity: identities
                            .intern(&checked_evidence_machine_identity(checked, machine)?),
                    }
                }
            })
        }

        pub(crate) fn establishment_routes(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            routes: &[language_semantics::DomainEstablishmentRoute],
        ) -> Result<Vec<terminal_psi::StructuralEstablishmentRoute>, LoweringError> {
            let mut lowered = Vec::new();
            for route in routes {
                let (owner, requirement) = match route {
                    language_semantics::DomainEstablishmentRoute::CheckedRequirement {
                        trait_definition,
                        requirement,
                    } => (*trait_definition, *requirement),
                    language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
                        boundary_trait,
                        requirement,
                    } => (*boundary_trait, *requirement),
                    language_semantics::DomainEstablishmentRoute::ExactMachine { machine } => {
                        lowered.extend(exact_machine_routes(identities, checked, *machine)?);
                        continue;
                    }
                };
                let definition =
                    checked
                        .trait_definition_by_symbol(owner)
                        .ok_or(LoweringError::Unsupported(
                            "domain establishment route names an absent trait",
                        ))?;
                let declared = checked
                    .trait_machine_signature_by_symbol(definition, requirement)
                    .ok_or(LoweringError::Unsupported(
                        "domain establishment route names an absent requirement",
                    ))?;
                for signature in checked.executable_requirement_machine_instances(declared) {
                    let mut application = *route;
                    match &mut application {
                        language_semantics::DomainEstablishmentRoute::CheckedRequirement {
                            requirement,
                            ..
                        }
                        | language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
                            requirement,
                            ..
                        } => *requirement = signature.symbol,
                        language_semantics::DomainEstablishmentRoute::ExactMachine { .. } => {}
                    }
                    lowered.push(establishment_route(identities, checked, &application)?);
                }
            }
            Ok(lowered)
        }

        pub(crate) fn exact_machine_routes(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            issuer: symbols::SymbolHandle,
        ) -> Result<Vec<ScalarDomainEstablishmentRoute>, LoweringError> {
            let mut routes = vec![establishment_route(
                identities,
                checked,
                &DomainEstablishmentRoute::ExactMachine { machine: issuer },
            )?];
            for candidate in checked.machines() {
                if candidate.symbol != issuer
                    && checked
                        .typed
                        .machine_invokes_declaration(candidate.symbol, issuer)
                        .map_err(LoweringError::Unsupported)?
                {
                    routes.push(establishment_route(
                        identities,
                        checked,
                        &DomainEstablishmentRoute::ExactMachine {
                            machine: candidate.symbol,
                        },
                    )?);
                }
            }
            Ok(routes)
        }

        pub(crate) fn return_route(
            checked: &CheckedTrees,
            returning: symbols::SymbolHandle,
            route: &DomainEstablishmentRoute,
        ) -> Result<DomainEstablishmentRoute, LoweringError> {
            let DomainEstablishmentRoute::ExactMachine { machine: issuer } = route else {
                return Ok(*route);
            };
            if !checked
                .typed
                .machine_invokes_declaration(returning, *issuer)
                .map_err(LoweringError::Unsupported)?
            {
                return Err(LoweringError::Unsupported(
                    "checked scalar return has no declaration correspondence to its issuer",
                ));
            }
            Ok(DomainEstablishmentRoute::ExactMachine { machine: returning })
        }
    }
    pub(crate) mod parameters {

        use super::{
            PreparedScalarQualifications, carries_scalar_qualifications, qualification_set,
            retain_domain, retained_domain_predicate, terminal_scalar_type, type_atoms,
        };
        use crate::shared::lowering_error::{LoweringError, unsupported};
        use checked_trees::CheckedTrees;
        use language_semantics::SemanticDomainId;
        use semantic_vocabulary::{QualifiedScalarType, ScalarType};
        use symbols::SymbolHandle;
        use terminal_psi::ScalarQualificationCatalog;

        pub(crate) struct ParameterAtoms {
            state: SymbolHandle,
            position: u32,
            carrier: ScalarType,
            domains: Vec<SemanticDomainId>,
        }

        #[derive(Clone)]
        pub(crate) struct ParameterQualification {
            pub state: SymbolHandle,
            pub position: u32,
            pub value_type: QualifiedScalarType,
        }

        pub(crate) fn prepare(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            constant_views: &crate::scalar_lowering::contracts::ContractViewNamespace<'_>,
            bodies: &[SymbolHandle],
            catalog: &mut ScalarQualificationCatalog,
            atom_sets: &mut Vec<Vec<String>>,
        ) -> Result<Vec<ParameterAtoms>, LoweringError> {
            let mut parameters = Vec::new();
            for symbol in bodies {
                let machine = checked.machine(*symbol).ok_or(LoweringError::Unsupported(
                    "scalar parameter namespace lost its machine",
                ))?;
                for (state_ordinal, state) in checked.machine_states(machine).iter().enumerate() {
                    for (position, parameter) in checked.state_parameters(state).iter().enumerate()
                    {
                        let Some(scalar) = checked_trees::CheckedScalarType::from_type_reference(
                            &checked.typed,
                            parameter.type_reference,
                        ) else {
                            continue;
                        };
                        if parameter.relevance.is_erased() {
                            continue;
                        }
                        let carrier = crate::emission::scalar_types::terminal_checked_scalar_type(
                            checked, scalar,
                        )?;
                        let Some(primitive) = scalar.primitive() else {
                            if carries_scalar_qualifications(checked, parameter.type_reference) {
                                return unsupported(
                                    "opaque callable parameters cannot acquire numeric domain predicates",
                                );
                            }
                            parameters.push(ParameterAtoms {
                                state: state.symbol,
                                position: u32::try_from(position).map_err(|_| {
                                    LoweringError::Unsupported(
                                        "callable parameter position exceeds u32",
                                    )
                                })?,
                                carrier,
                                domains: Vec::new(),
                            });
                            continue;
                        };
                        let mut atoms =
                            if carries_scalar_qualifications(checked, parameter.type_reference) {
                                type_atoms(checked, parameter.type_reference)?.1
                            } else {
                                Vec::new()
                            };
                        let contracts = checked.state_contracts(state).iter().chain(
                            checked
                                .machine_contracts(machine)
                                .iter()
                                .filter(|_| state_ordinal == 0),
                        );
                        for contract in contracts.filter(|contract| {
                            contract.kind == typed_trees::signature::SignatureContractKind::Requires
                                && contract.binding.is_none()
                        }) {
                            for fact in checked.proof_facts.span_or_empty(contract.facts) {
                                let Some(membership) = fact.as_membership() else {
                                    continue;
                                };
                                if validation::single_segment_name_symbol(
                                    &checked.typed,
                                    membership.value,
                                ) != Some(parameter.symbol)
                                {
                                    continue;
                                }
                                let Some(definition) =
                                    checked.domain_definition(membership.domain_symbol)
                                else {
                                    return unsupported("scalar entry membership lost its domain");
                                };
                                if definition.classification
                                    == Some(
                                        language_semantics::DomainClassification::ProgressProfile,
                                    )
                                {
                                    continue;
                                }
                                let routed = validation::field_domains::domain_requires_provenance(
                                    &checked.typed,
                                    membership.domain_symbol,
                                );
                                let tag = validation::scalar_membership_is_tag(
                                    &checked.typed,
                                    membership,
                                    primitive,
                                );
                                if !routed && !tag {
                                    continue;
                                }
                                if routed
                                    && validation::scalar_membership_value_carrier(
                                        &checked.typed,
                                        membership,
                                    ) != Some(primitive)
                                {
                                    return unsupported(
                                        "scalar entry membership has no exact primitive application",
                                    );
                                }
                                let predicate = if routed {
                                    let subject =
                                        checked_trees::CheckedScalarExpression::Parameter {
                                            position: 0,
                                            primitive_type: primitive,
                                        };
                                    let predicate = validation::scalar_membership_value_predicates(
                                        &checked.typed,
                                        membership,
                                        &subject,
                                    )
                                    .ok_or(LoweringError::Unsupported(
                                        "scalar entry membership has no complete predicate",
                                    ))?
                                    .into_iter()
                                    .reduce(|left, right| {
                                        checked_trees::CheckedBooleanExpression::And {
                                            left: Box::new(left),
                                            right: Box::new(right),
                                        }
                                    })
                                    .unwrap_or(checked_trees::CheckedBooleanExpression::Constant(
                                        true,
                                    ));
                                    Some(retained_domain_predicate(
                                        identities,
                                        constant_views,
                                        &predicate,
                                        carrier,
                                    )?)
                                } else {
                                    None
                                };
                                retain_domain(
                                    catalog,
                                    identities,
                                    checked,
                                    constant_views,
                                    membership.domain_symbol,
                                    membership.semantic_domain,
                                    carrier,
                                    predicate,
                                )?;
                                if !atoms.contains(&(
                                    membership.domain_symbol,
                                    membership.semantic_domain,
                                )) {
                                    atoms.push((
                                        membership.domain_symbol,
                                        membership.semantic_domain,
                                    ));
                                }
                            }
                        }
                        let domains = atoms.iter().map(|(_, domain)| *domain).collect::<Vec<_>>();
                        let mut names = domains
                            .iter()
                            .map(|domain| {
                                checked
                                    .semantic_domains
                                    .name(*domain)
                                    .map(str::to_owned)
                                    .ok_or(LoweringError::Unsupported(
                                        "scalar parameter lost a canonical domain",
                                    ))
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        names.sort();
                        names.dedup();
                        if !names.is_empty() {
                            atom_sets.push(names);
                        }
                        parameters.push(ParameterAtoms {
                            state: state.symbol,
                            position: u32::try_from(position).map_err(|_| {
                                LoweringError::Unsupported("scalar parameter position exceeds u32")
                            })?,
                            carrier,
                            domains,
                        });
                    }
                }
            }
            Ok(parameters)
        }

        pub(crate) fn resolve(
            catalog: &ScalarQualificationCatalog,
            parameters: Vec<ParameterAtoms>,
        ) -> Result<Vec<ParameterQualification>, LoweringError> {
            parameters
                .into_iter()
                .map(|parameter| {
                    let domains = parameter
                        .domains
                        .iter()
                        .map(|semantic| {
                            catalog
                                .domains
                                .iter()
                                .find(|domain| {
                                    domain.semantic_domain.get() == u64::from(semantic.0)
                                        && domain.carrier == parameter.carrier
                                })
                                .map(|domain| domain.id)
                                .ok_or(LoweringError::Unsupported(
                                    "scalar formal lost its prepared domain",
                                ))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let qualifications = qualification_set(catalog, domains)?;
                    Ok(ParameterQualification {
                        state: parameter.state,
                        position: parameter.position,
                        value_type: QualifiedScalarType {
                            scalar_type: parameter.carrier,
                            qualifications,
                        },
                    })
                })
                .collect()
        }

        impl PreparedScalarQualifications {
            pub(crate) fn parameter_value_type(
                &self,
                checked: &CheckedTrees,
                state: SymbolHandle,
                position: u32,
            ) -> Result<QualifiedScalarType, LoweringError> {
                self.parameters
                    .iter()
                    .find(|parameter| parameter.state == state && parameter.position == position)
                    .map(|parameter| parameter.value_type)
                    .ok_or_else(|| LoweringError::MissingScalarParameterQualification {
                        state: checked.symbols.display_path(state, "::"),
                        source_position: position,
                        prepared_positions: self
                            .parameters
                            .iter()
                            .filter(|parameter| parameter.state == state)
                            .map(|parameter| parameter.position)
                            .collect(),
                    })
            }
        }
    }
    pub(crate) use fields::FieldType;
    pub(crate) use issuers::{establishment_route, establishment_routes};
    pub(crate) mod returns {

        use super::{
            CheckedTrees, LoweringError, ScalarQualificationCatalog, SymbolHandle, unsupported,
        };
        use crate::shared::checked_facts::flow_facts;
        use checked_trees::CheckedScalarReturnOrigin;
        use semantic_vocabulary::MachineId;
        use terminal_psi::{
            ScalarReturnOrigin, ScalarReturnQualification, ScalarReturnSite, TerminalMachine,
            Terminator,
        };

        pub(crate) struct CheckedReturnCompletion {
            result: semantic_vocabulary::QualifiedScalarType,
        }

        impl CheckedReturnCompletion {
            pub(crate) fn destination(self) -> semantic_vocabulary::QualifiedScalarType {
                self.result
            }
        }

        impl super::PreparedScalarQualifications {
            pub(crate) fn return_completion(
                &self,
                checked: &CheckedTrees,
                machine: SymbolHandle,
                exit: &checked_trees::FlowClaimJoinExit,
                source: semantic_vocabulary::QualifiedScalarType,
                result: semantic_vocabulary::QualifiedScalarType,
            ) -> Result<CheckedReturnCompletion, LoweringError> {
                if source.scalar_type != result.scalar_type {
                    return unsupported("checked scalar return completion changes its carrier");
                }
                let domains = |qualification| {
                    if qualification == semantic_vocabulary::ScalarQualificationSetId::default() {
                        return Ok(&[][..]);
                    }
                    self.catalog
                        .sets
                        .iter()
                        .find(|set| set.id == qualification)
                        .map(|set| set.domains.as_slice())
                        .ok_or(LoweringError::Unsupported(
                            "scalar return completion lost its qualification set",
                        ))
                };
                let source_domains = domains(source.qualifications)?;
                let result_domains = domains(result.qualifications)?;
                if source_domains
                    .iter()
                    .any(|domain| !result_domains.contains(domain))
                {
                    return unsupported(
                        "checked scalar return completion cannot erase unrelated membership",
                    );
                }
                for domain_id in result_domains
                    .iter()
                    .filter(|domain| !source_domains.contains(domain))
                {
                    let domain = self
                        .catalog
                        .domains
                        .iter()
                        .find(|domain| domain.id == *domain_id)
                        .ok_or(LoweringError::Unsupported(
                            "scalar return completion lost its domain",
                        ))?;
                    if domain.predicate.is_none() && domain.establishment_routes.is_empty() {
                        continue;
                    }
                    checked_exit_decision(checked, machine, exit, domain.semantic_domain)?;
                }
                Ok(CheckedReturnCompletion { result })
            }
        }

        fn checked_exit_decision<'checked>(
            checked: &'checked CheckedTrees,
            machine: SymbolHandle,
            exit: &checked_trees::FlowClaimJoinExit,
            domain: semantic_vocabulary::DomainSemanticId,
        ) -> Result<&'checked checked_trees::CheckedScalarReturnQualification, LoweringError>
        {
            let mut decisions =
                checked
                    .facts
                    .qualifications
                    .scalar_returns
                    .iter()
                    .filter(|decision| {
                        decision.machine == machine
                            && u64::from(decision.semantic_domain.0) == domain.get()
                            && matches_exit(checked, exit, decision)
                    });
            let decision = decisions.next().ok_or(LoweringError::Unsupported(
                "scalar return has an exit without its checked membership decision",
            ))?;
            if decisions.next().is_some() {
                return unsupported("scalar return has ambiguous checked exit membership");
            }
            Ok(decision)
        }

        #[derive(Debug, Clone)]
        pub(crate) struct SourceScalarReturn {
            pub edge: semantic_vocabulary::EdgeId,
            pub exits: Vec<checked_trees::FlowClaimJoinExit>,
        }

        pub(crate) fn retain_returns(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            machine_ids: &[(SymbolHandle, MachineId)],
            machines: &[TerminalMachine],
            sources: &[SourceScalarReturn],
            catalog: &mut ScalarQualificationCatalog,
        ) -> Result<(), LoweringError> {
            for machine in machines {
                let Some(result) = machine.result.scalar() else {
                    continue;
                };
                let set = catalog
                    .sets
                    .iter()
                    .find(|set| set.id == result.qualifications);
                if set.is_none() && !result.qualifications.is_empty() {
                    return unsupported("scalar result lost its qualification set");
                }
                if set.is_none()
                    && !machine.blocks.iter().any(|block| {
                        matches!(block.terminator, Terminator::Jump { edge, .. }
                        if sources.iter().any(|source| source.edge == edge))
                    })
                {
                    continue;
                }
                let symbol = machine_ids
                    .iter()
                    .find(|(_, id)| *id == machine.id)
                    .map(|(symbol, _)| *symbol)
                    .ok_or(LoweringError::Unsupported(
                        "qualified result lost its source machine",
                    ))?;
                for domain in &catalog.domains {
                    let domain_id = &domain.id;
                    for block in &machine.blocks {
                        let (edge, value, result_id, site) = match &block.terminator {
                            Terminator::Return { edge, value, .. }
                                if !domain.establishment_routes.is_empty()
                                    && set.is_some_and(|set| set.domains.contains(domain_id)) =>
                            {
                                (*edge, *value, result.id, ScalarReturnSite::NormalExit)
                            }
                            Terminator::Jump {
                                edge,
                                target,
                                arguments,
                                ..
                            } if sources.iter().any(|source| source.edge == *edge) => {
                                let destination = machine
                                    .blocks
                                    .iter()
                                    .find(|block| block.id == *target)
                                    .and_then(|block| block.parameters.last())
                                    .ok_or(LoweringError::Unsupported(
                                        "scalar return completion lost its destination",
                                    ))?;
                                if !catalog.sets.iter().any(|set| {
                                    set.id == destination.qualifications
                                        && set.domains.contains(domain_id)
                                }) {
                                    continue;
                                }
                                let value = *arguments.last().ok_or(LoweringError::Unsupported(
                                    "scalar return completion lost its source",
                                ))?;
                                let declaration = machine
                                    .parameters
                                    .iter()
                                    .chain(machine.block_parameters())
                                    .chain(
                                        machine
                                            .operations()
                                            .filter_map(|operation| operation.result.scalar_ref()),
                                    )
                                    .find(|source| source.id == value)
                                    .ok_or(LoweringError::Unsupported(
                                        "scalar return completion source is undeclared",
                                    ))?;
                                if catalog.sets.iter().any(|set| {
                                    set.id == declaration.qualifications
                                        && set.domains.contains(domain_id)
                                }) {
                                    continue;
                                }
                                (
                                    *edge,
                                    value,
                                    destination.id,
                                    ScalarReturnSite::Completion {
                                        argument_ordinal: u32::try_from(arguments.len() - 1)
                                            .map_err(|_| {
                                                LoweringError::Unsupported(
                                                    "scalar completion argument exceeds u32",
                                                )
                                            })?,
                                    },
                                )
                            }
                            _ => continue,
                        };
                        let source_exits = sources
                            .iter()
                            .find(|source| source.edge == edge)
                            .ok_or(LoweringError::Unsupported(
                                "qualified return lost its checked source exits",
                            ))?;
                        let mut origins = Vec::new();
                        for exit in &source_exits.exits {
                            if domain.predicate.is_none() && domain.establishment_routes.is_empty()
                            {
                                origins.push(ScalarReturnOrigin::Vacuous);
                                continue;
                            }
                            let decision = checked_exit_decision(
                                checked,
                                symbol,
                                exit,
                                domain.semantic_domain,
                            )?;
                            origins.push(match &decision.origin {
                                CheckedScalarReturnOrigin::Forwarded => ScalarReturnOrigin::Forwarded,
                                CheckedScalarReturnOrigin::Established(route) => {
                                    let route = super::issuers::return_route(checked, symbol, route)?;
                                    let route = super::establishment_route(identities, checked, &route)?;
                                    let position = domain
                                        .establishment_routes
                                        .iter()
                                        .position(|row| *row == route)
                                        .ok_or(LoweringError::Unsupported(
                                            "checked scalar issuer is absent from its domain routes",
                                        ))?;
                                    ScalarReturnOrigin::Established {
                                        route: u32::try_from(position).map_err(|_| {
                                            LoweringError::Unsupported("scalar issuer route exceeds u32")
                                        })?,
                                    }
                                }
                                CheckedScalarReturnOrigin::Predicate => {
                                    return unsupported(
                                        "predicate-only checking cannot establish routed scalar provenance",
                                    );
                                }
                            });
                        }
                        origins.sort();
                        origins.dedup();
                        if origins.is_empty() {
                            return unsupported(
                                "routed scalar result has no checked return membership",
                            );
                        }
                        catalog.returns.push(ScalarReturnQualification {
                            site,
                            machine: machine.id,
                            edge,
                            source: value,
                            result: result_id,
                            domain: *domain_id,
                            origins,
                        });
                    }
                }
            }
            catalog
                .returns
                .sort_by_key(|row| (row.machine, row.edge, row.domain));
            Ok(())
        }

        fn matches_exit(
            checked: &CheckedTrees,
            exit: &checked_trees::FlowClaimJoinExit,
            decision: &checked_trees::CheckedScalarReturnQualification,
        ) -> bool {
            use checked_trees::FlowClaimJoinExitKind;
            if exit.state_symbol != decision.state
                || exit.statement_index != decision.statement_index
            {
                return false;
            }
            match exit.kind {
                FlowClaimJoinExitKind::SelectionArm { .. }
                | FlowClaimJoinExitKind::SelectionDecision { .. } => false,
                FlowClaimJoinExitKind::FinalExpression => !decision.transition_target.is_valid(),
                FlowClaimJoinExitKind::TransitionTarget
                | FlowClaimJoinExitKind::TransitionContinuation => {
                    decision.transition_target.is_valid()
                        && decision.is_continuation
                            == matches!(exit.kind, FlowClaimJoinExitKind::TransitionContinuation)
                }
                FlowClaimJoinExitKind::CallResult { call_ordinal } => flow_facts(checked)
                    .control
                    .machine_states(decision.machine)
                    .into_iter()
                    .filter(|state| state.state_symbol == decision.state)
                    .map(|state| ((), state))
                    .flat_map(|(_, state)| {
                        checked.facts.flow.control.calls.span_or_empty(state.calls)
                    })
                    .any(|call| {
                        call.statement_index == decision.statement_index
                            && call.call_ordinal == call_ordinal
                            && call.authored_expression == decision.expression
                    }),
            }
        }
    }
    pub(crate) use returns::SourceScalarReturn;
    pub(crate) use returns::retain_returns;

    #[derive(Clone)]
    pub(crate) struct PreparedScalarQualifications {
        catalog: ScalarQualificationCatalog,
        parameters: Vec<parameters::ParameterQualification>,
    }

    impl PreparedScalarQualifications {
        pub(crate) fn prepare(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            scalar_applications: &crate::unit::attached_unit::scalar_applications::SourceScalarApplications,
            signatures: &[SymbolHandle],
            bodies: &[SymbolHandle],
            boundaries: &[&checked_trees::CheckedBoundaryMachinePlan],
            fields: &[FieldType],
        ) -> Result<Self, LoweringError> {
            let constant_views = crate::scalar_lowering::contracts::ContractViewNamespace {
                scalar_applications,
                constant_values: &checked.typed.constant_recipe_closure.values,
                ..crate::scalar_lowering::contracts::ContractViewNamespace::EMPTY
            };
            let mut references = fields
                .iter()
                .map(|(_, reference)| *reference)
                .collect::<Vec<_>>();
            for row in checked.facts.callable_contracts.contracts() {
                let signature = &checked.runtime_callable_contract(row.source).signature;
                references.extend(
                    checked
                        .state_signature_parameters(signature)
                        .iter()
                        .map(|parameter| parameter.type_reference)
                        .chain([signature.return_type])
                        .filter(|reference| checked.primitive_type_reference(*reference).is_some()),
                );
            }
            for symbol in signatures {
                let machine = checked.machine(*symbol).ok_or(LoweringError::Unsupported(
                    "scalar qualification closure lost a machine",
                ))?;
                for state in checked.machine_states(machine) {
                    for parameter in checked.state_parameters(state) {
                        if checked
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                        {
                            references.push(parameter.type_reference);
                        }
                    }
                    if checked
                        .primitive_type_reference(state.return_type)
                        .is_some()
                    {
                        references.push(state.return_type);
                    }
                }
            }
            for symbol in bodies.iter().filter(|symbol| !signatures.contains(symbol)) {
                let machine = checked.machine(*symbol).ok_or(LoweringError::Unsupported(
                    "scalar qualification closure lost a machine",
                ))?;
                for state in checked.machine_states(machine) {
                    references.extend(
                        checked
                            .state_parameters(state)
                            .iter()
                            .map(|parameter| parameter.type_reference)
                            .chain([state.return_type])
                            .filter(|reference| carries_scalar_qualifications(checked, *reference)),
                    );
                }
            }
            for symbol in bodies {
                let machine = checked.machine(*symbol).ok_or(LoweringError::Unsupported(
                    "scalar qualification closure lost a local's machine",
                ))?;
                for state in checked.machine_states(machine) {
                    references.extend(
                        checked
                            .facts
                            .state_locals
                            .locals(state.symbol)
                            .iter()
                            .filter(|local| local.is_scalar_value && !local.type_is_inferred)
                            .map(|local| local.declared_type)
                            .filter(|reference| carries_scalar_qualifications(checked, *reference)),
                    );
                }
            }
            let plans = &checked.facts.values.scalar_computations;
            let roots = plans
                .roots
                .iter()
                .filter_map(|(_, root)| bodies.contains(&root.machine).then_some(root.root))
                .collect::<Vec<_>>();
            for handle in super::computation_graph::reachable_nodes(checked, &roots)? {
                match &plans.nodes.get(handle).kind {
                    CheckedScalarComputationKind::Qualification { result_type, .. }
                    | CheckedScalarComputationKind::StructuralField { result_type, .. }
                    | CheckedScalarComputationKind::AuthoredCast { result_type, .. } => {
                        references.push(*result_type)
                    }
                    _ => {}
                }
            }
            for boundary in boundaries {
                let signature = crate::shared::declarations::target_signature(
                    checked,
                    boundary.machine,
                    boundary.machine,
                )?;
                if carries_scalar_qualifications(checked, signature.return_type) {
                    references.push(signature.return_type);
                }
            }
            let mut catalog = ScalarQualificationCatalog::default();
            let mut atom_sets = Vec::new();
            for reference in references {
                let (primitive, atoms) = type_atoms(checked, reference)?;
                let scalar_type = terminal_scalar_type(primitive)?;
                let mut atom_identities = Vec::new();
                for (symbol, semantic_id) in atoms {
                    let predicate = domain_predicate(
                        identities,
                        checked,
                        &constant_views,
                        reference,
                        symbol,
                        semantic_id,
                    )?;
                    atom_identities.push(retain_domain(
                        &mut catalog,
                        identities,
                        checked,
                        &constant_views,
                        symbol,
                        semantic_id,
                        scalar_type,
                        predicate,
                    )?);
                }
                atom_identities.sort();
                atom_identities.dedup();
                if !atom_identities.is_empty() {
                    atom_sets.push(atom_identities);
                }
            }
            for boundary in boundaries {
                let mut parameter_sets = vec![Vec::new(); boundary.scalar_parameters.len()];
                for requirement in &boundary.scalar_domain_requirements {
                    let scalar_type = terminal_scalar_type(requirement.primitive_type)?;
                    let routed = validation::field_domains::domain_requires_provenance(
                        &checked.typed,
                        requirement.domain_symbol,
                    );
                    let predicate = if routed {
                        Some(retained_domain_predicate(
                            identities,
                            &constant_views,
                            &requirement.predicate,
                            scalar_type,
                        )?)
                    } else {
                        None
                    };
                    let identity = retain_domain(
                        &mut catalog,
                        identities,
                        checked,
                        &constant_views,
                        requirement.domain_symbol,
                        requirement.domain,
                        scalar_type,
                        predicate,
                    )?;
                    parameter_sets
                        .get_mut(requirement.argument_index as usize)
                        .ok_or(LoweringError::Unsupported(
                            "boundary membership has no scalar formal",
                        ))?
                        .push(identity);
                }
                for mut atoms in parameter_sets {
                    atoms.sort();
                    atoms.dedup();
                    if !atoms.is_empty() {
                        atom_sets.push(atoms);
                    }
                }
            }
            let parameter_atoms = parameters::prepare(
                identities,
                checked,
                &constant_views,
                bodies,
                &mut catalog,
                &mut atom_sets,
            )?;
            domain_predicates::complete_value_components(&mut catalog.domains);
            catalog.domains.sort_by(|left, right| {
                identities
                    .text(left.identity)
                    .unwrap_or_default()
                    .cmp(identities.text(right.identity).unwrap_or_default())
            });
            for (ordinal, domain) in catalog.domains.iter_mut().enumerate() {
                domain.id = ScalarDomainId::new(
                    u64::try_from(ordinal)
                        .ok()
                        .and_then(|value| value.checked_add(1))
                        .ok_or(LoweringError::Unsupported(
                            "scalar domain catalog exceeds identity capacity",
                        ))?,
                )
                .ok_or(LoweringError::Unsupported(
                    "scalar domain catalog has an invalid identity",
                ))?;
            }
            atom_sets.sort();
            atom_sets.dedup();
            for (ordinal, atom_identities) in atom_sets.into_iter().enumerate() {
                let domains = atom_identities
                    .iter()
                    .map(|identity| {
                        catalog
                            .domains
                            .iter()
                            .find(|domain| domain.identity == identities.intern(identity))
                            .map(|domain| domain.id)
                            .ok_or(LoweringError::Unsupported(
                                "scalar qualification set lost a domain",
                            ))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let id = u64::try_from(ordinal)
                    .ok()
                    .and_then(|value| value.checked_add(1))
                    .ok_or(LoweringError::Unsupported(
                        "scalar qualification sets exceed identity capacity",
                    ))?;
                catalog.sets.push(ScalarQualificationSet {
                    id: ScalarQualificationSetId::new(id),
                    domains,
                });
            }
            let parameters = parameters::resolve(&catalog, parameter_atoms)?;
            let mut prepared = Self {
                catalog,
                parameters,
            };
            prepared.retain_fields(checked, fields)?;
            Ok(prepared)
        }

        pub(crate) fn empty() -> Self {
            Self {
                catalog: ScalarQualificationCatalog::default(),
                parameters: Vec::new(),
            }
        }

        pub(crate) fn catalog(&self) -> &ScalarQualificationCatalog {
            &self.catalog
        }

        pub(crate) fn signature_value_type(
            &self,
            checked: &CheckedTrees,
            reference: TypeReferenceHandle,
        ) -> Result<QualifiedScalarType, LoweringError> {
            signature_value_type(checked, &self.catalog, reference)
        }

        pub(crate) fn value_type(
            &self,
            checked: &CheckedTrees,
            reference: TypeReferenceHandle,
        ) -> Result<QualifiedScalarType, LoweringError> {
            signature_value_type(checked, &self.catalog, reference)
        }

        pub(crate) fn boundary_domain_id(
            &self,
            requirement: &checked_trees::CheckedBoundaryScalarDomainRequirementPlan,
        ) -> Result<ScalarDomainId, LoweringError> {
            let carrier = terminal_scalar_type(requirement.primitive_type)?;
            self.catalog
                .domains
                .iter()
                .find(|domain| {
                    domain.semantic_domain.get() == u64::from(requirement.domain.0)
                        && domain.carrier == carrier
                })
                .map(|domain| domain.id)
                .ok_or(LoweringError::Unsupported(
                    "boundary scalar membership lost its prepared declaration",
                ))
        }

        pub(crate) fn boundary_signature_types(
            &self,
            boundary: &checked_trees::CheckedBoundaryMachinePlan,
        ) -> Result<Vec<QualifiedScalarType>, LoweringError> {
            boundary
                .scalar_parameters
                .iter()
                .enumerate()
                .map(|(position, parameter)| {
                    let domains = boundary
                        .scalar_domain_requirements
                        .iter()
                        .filter(|row| row.argument_index as usize == position)
                        .map(|row| self.boundary_domain_id(row))
                        .collect::<Result<Vec<_>, _>>()?;
                    let qualifications = qualification_set(&self.catalog, domains)?;
                    Ok(QualifiedScalarType {
                        scalar_type: terminal_scalar_type(parameter.primitive_type)?,
                        qualifications,
                    })
                })
                .collect()
        }

        pub(crate) fn boundary_scalar_formals(
            &self,
            checked: &CheckedTrees,
            machine: SymbolHandle,
        ) -> Option<Vec<QualifiedScalarType>> {
            let plan = flow_facts(checked)
                .terminal_unit_effects
                .boundary_machines
                .iter()
                .find(|plan| plan.machine == machine)?;
            self.boundary_signature_types(plan).ok()
        }

        pub(crate) fn scalar_state_types(
            &self,
            checked: &CheckedTrees,
            symbol: SymbolHandle,
        ) -> Result<(Vec<QualifiedScalarType>, QualifiedScalarType), LoweringError> {
            let mut states = checked.machine_states_named(symbol).map(|(_, state)| state);
            let state = states.next().ok_or(LoweringError::Unsupported(
                "scalar signature lost its state",
            ))?;
            if states.next().is_some() {
                return unsupported("scalar signature has ambiguous state identity");
            }
            let parameters = checked
                .state_parameters(state)
                .iter()
                .enumerate()
                .filter(|(_, parameter)| {
                    !parameter.relevance.is_erased()
                        && checked_trees::CheckedScalarType::from_type_reference(
                            &checked.typed,
                            parameter.type_reference,
                        )
                        .is_some()
                })
                .map(|(position, _)| {
                    self.parameter_value_type(
                        checked,
                        state.symbol,
                        u32::try_from(position).map_err(|_| {
                            LoweringError::Unsupported("scalar formal position exceeds u32")
                        })?,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok((parameters, self.value_type(checked, state.return_type)?))
        }
    }

    pub(crate) fn received_scalar_type(
        formal: Option<QualifiedScalarType>,
        actual: QualifiedScalarType,
    ) -> QualifiedScalarType {
        formal
            .filter(|formal| formal.scalar_type == actual.scalar_type)
            .unwrap_or(actual)
    }

    fn retain_domain(
        catalog: &mut ScalarQualificationCatalog,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        constant_views: &crate::scalar_lowering::contracts::ContractViewNamespace<'_>,
        symbol: SymbolHandle,
        semantic_id: SemanticDomainId,
        scalar_type: semantic_vocabulary::ScalarType,
        predicate: Option<terminal_psi::ScalarDomainPredicate>,
    ) -> Result<String, LoweringError> {
        let identity = checked
            .semantic_domains
            .name(semantic_id)
            .ok_or(LoweringError::Unsupported(
                "scalar qualification lost its canonical identity",
            ))?
            .to_owned();
        let semantic_domain = DomainSemanticId::new(u64::from(semantic_id.0)).ok_or(
            LoweringError::Unsupported("scalar qualification has an invalid semantic identity"),
        )?;
        let establishment_routes = match catalog.domains.iter().find(|domain| {
            domain.semantic_domain == semantic_domain && domain.carrier == scalar_type
        }) {
            Some(domain) => domain.establishment_routes.clone(),
            None => scalar_domain_establishment_routes(identities, checked, symbol, semantic_id)?,
        };
        let definition = checked
            .typed
            .domain_definition_for_instance(symbol, semantic_id)
            .or_else(|| checked.domain_definition(symbol))
            .ok_or(LoweringError::Unsupported(
                "scalar qualification lost its required-domain declaration",
            ))?;
        let requirements =
            typed_trees::domain::required_subject_memberships(&checked.typed, definition);
        let required_memberships = requirements
            .as_ref()
            .map(|requirements| {
                let mut domains = requirements
                    .iter()
                    .map(|membership| {
                        DomainSemanticId::new(u64::from(membership.semantic_domain.0))
                            .ok_or(LoweringError::InvalidContentDomainIdentity)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                domains.sort();
                domains.dedup();
                Ok::<_, LoweringError>(domains)
            })
            .transpose()?;
        let predicate = if required_memberships.is_some() {
            if definition.establishment_routes.is_empty()
                && checked.typed.proof_facts(definition).is_empty()
            {
                None
            } else {
                let local = validation::scalar_domain_local_predicate(&checked.typed, definition)
                    .ok_or(LoweringError::Unsupported(
                    "scalar required-domain theory has no complete local predicate",
                ))?;
                Some(retained_domain_predicate(
                    identities,
                    constant_views,
                    &local,
                    scalar_type,
                )?)
            }
        } else {
            validation::scalar_domain_definition_predicate(&checked.typed, symbol, semantic_id)
                .map(|predicate| {
                    retained_domain_predicate(identities, constant_views, &predicate, scalar_type)
                })
                .transpose()?
                .or(predicate)
        };
        let retained_identity = identities.intern(&identity);
        if let Some(existing) = catalog
            .domains
            .iter()
            .find(|domain| domain.identity == retained_identity)
        {
            if existing.carrier != scalar_type
                || existing.semantic_domain != semantic_domain
                || existing.establishment_routes != establishment_routes
                || existing.required_memberships != required_memberships
                || existing.predicate != predicate
            {
                return unsupported("scalar qualification identity has conflicting declarations");
            }
            return Ok(identity);
        } else {
            catalog.domains.push(ScalarDomainDeclaration {
                required_memberships,
                predicate,
                id: ScalarDomainId::new(1).ok_or(LoweringError::Unsupported(
                    "invalid initial scalar domain identity",
                ))?,
                semantic_domain,
                identity: retained_identity,
                carrier: scalar_type,
                establishment_routes,
            });
        }
        if let Some(requirements) = requirements {
            for membership in requirements {
                let parent = checked
                    .typed
                    .domain_definition_for_instance(
                        membership.domain_symbol,
                        membership.semantic_domain,
                    )
                    .or_else(|| checked.domain_definition(membership.domain_symbol))
                    .ok_or(LoweringError::Unsupported(
                        "required scalar domain is absent",
                    ))?;
                let predicate = if parent.establishment_routes.is_empty()
                    && checked.typed.proof_facts(parent).is_empty()
                {
                    None
                } else {
                    let local = validation::scalar_domain_local_predicate(&checked.typed, parent)
                        .or_else(|| {
                            let primitive_type =
                                checked.primitive_type_reference(parent.target_type)?;
                            let predicates = validation::scalar_membership_value_predicates(
                                &checked.typed,
                                membership,
                                &checked_trees::CheckedScalarExpression::Parameter {
                                    position: 0,
                                    primitive_type,
                                },
                            )?;
                            Some(
                                predicates
                                    .into_iter()
                                    .reduce(|left, right| {
                                        checked_trees::CheckedBooleanExpression::And {
                                            left: Box::new(left),
                                            right: Box::new(right),
                                        }
                                    })
                                    .unwrap_or(checked_trees::CheckedBooleanExpression::Constant(
                                        true,
                                    )),
                            )
                        })
                        .ok_or(LoweringError::Unsupported(
                            "required scalar domain has no complete local predicate",
                        ))?;
                    Some(retained_domain_predicate(
                        identities,
                        constant_views,
                        &local,
                        scalar_type,
                    )?)
                };
                retain_domain(
                    catalog,
                    identities,
                    checked,
                    constant_views,
                    membership.domain_symbol,
                    membership.semantic_domain,
                    scalar_type,
                    predicate,
                )?;
            }
        }
        Ok(identity)
    }

    fn retained_domain_predicate(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        constant_views: &crate::scalar_lowering::contracts::ContractViewNamespace<'_>,
        predicate: &checked_trees::CheckedBooleanExpression,
        carrier: semantic_vocabulary::ScalarType,
    ) -> Result<terminal_psi::ScalarDomainPredicate, LoweringError> {
        let subject = crate::shared::terminal_identities::value_id(1);
        let proposition = crate::scalar_lowering::contracts::proposition(
            identities,
            predicate,
            &[terminal_psi::ValueDeclaration {
                id: subject,
                scalar_type: carrier,
                qualifications: ScalarQualificationSetId::ZERO,
            }],
            &[],
            constant_views,
        )?;
        Ok(terminal_psi::ScalarDomainPredicate {
            subject,
            proposition,
        })
    }

    fn scalar_domain_establishment_routes(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        symbol: SymbolHandle,
        semantic_domain: SemanticDomainId,
    ) -> Result<Vec<ScalarDomainEstablishmentRoute>, LoweringError> {
        let domain = checked
            .domain_definition(symbol)
            .ok_or(LoweringError::Unsupported(
                "scalar qualification lost its domain declaration",
            ))?;
        let mut routes = Vec::new();
        for route in &domain.establishment_routes {
            routes.push(establishment_route(identities, checked, route)?);
        }
        for decision in checked
            .facts
            .qualifications
            .scalar_returns
            .iter()
            .filter(|decision| decision.semantic_domain == semantic_domain)
        {
            let checked_trees::CheckedScalarReturnOrigin::Established(route) = &decision.origin
            else {
                continue;
            };
            let route = issuers::return_route(checked, decision.machine, route)?;
            routes.push(establishment_route(identities, checked, &route)?);
        }
        routes.sort();
        routes.dedup();
        Ok(routes)
    }

    pub(crate) fn signature_value_type(
        checked: &CheckedTrees,
        catalog: &ScalarQualificationCatalog,
        reference: TypeReferenceHandle,
    ) -> Result<QualifiedScalarType, LoweringError> {
        if carries_scalar_qualifications(checked, reference) {
            return scalar_value_type(checked, catalog, reference);
        }
        Ok(crate::emission::scalar_types::terminal_type_reference(checked, reference)?.into())
    }

    fn qualification_set(
        catalog: &ScalarQualificationCatalog,
        mut domains: Vec<semantic_vocabulary::ScalarDomainId>,
    ) -> Result<ScalarQualificationSetId, LoweringError> {
        domains.sort();
        domains.dedup();
        if domains.is_empty() {
            return Ok(ScalarQualificationSetId::ZERO);
        }
        catalog
            .sets
            .iter()
            .find(|set| set.domains == domains)
            .map(|set| set.id)
            .ok_or(LoweringError::Unsupported(
                "scalar qualification set is absent from the prepared closure",
            ))
    }

    fn scalar_value_type(
        checked: &CheckedTrees,
        catalog: &ScalarQualificationCatalog,
        reference: TypeReferenceHandle,
    ) -> Result<QualifiedScalarType, LoweringError> {
        let (primitive, atoms) = type_atoms(checked, reference)?;
        let scalar_type = terminal_scalar_type(primitive)?;
        let mut domains = Vec::new();
        for (_, identity) in atoms {
            let domain = catalog
                .domains
                .iter()
                .find(|domain| {
                    domain.semantic_domain.get() == u64::from(identity.0)
                        && domain.carrier == scalar_type
                })
                .ok_or(LoweringError::Unsupported(
                    "scalar qualification is absent from the prepared closure",
                ))?;
            domains.push(domain.id);
        }
        let qualifications = qualification_set(catalog, domains)?;
        Ok(QualifiedScalarType {
            scalar_type,
            qualifications,
        })
    }

    fn carries_scalar_qualifications(
        checked: &CheckedTrees,
        reference: TypeReferenceHandle,
    ) -> bool {
        checked.primitive_type_reference(reference).is_some()
            && (!validation::scalar_type_tags(&checked.typed, reference).is_empty()
                || !validation::scalar_type_routed_domains(&checked.typed, reference).is_empty())
    }

    fn domain_predicate(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        constant_views: &crate::scalar_lowering::contracts::ContractViewNamespace<'_>,
        mut reference: TypeReferenceHandle,
        symbol: SymbolHandle,
        semantic_id: SemanticDomainId,
    ) -> Result<Option<terminal_psi::ScalarDomainPredicate>, LoweringError> {
        let primitive_type =
            checked
                .primitive_type_reference(reference)
                .ok_or(LoweringError::Unsupported(
                    "scalar domain predicate lost its carrier",
                ))?;
        let mut active = Vec::new();
        while !active.contains(&reference) {
            active.push(reference);
            let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = checked.type_reference_table.type_reference(reference)
            else {
                break;
            };
            for constraint in checked.type_reference_table.constraints(*constraints) {
                let TypeConstraintNode::Domain(domain) = constraint else {
                    continue;
                };
                if domain.symbol != symbol
                    || domain.semantic_id != semantic_id
                    || domain.establishment_routes.is_empty()
                {
                    continue;
                }
                let predicate = validation::scalar_domain_value_predicate(
                    &checked.typed,
                    domain,
                    checked_trees::CheckedScalarExpression::Parameter {
                        position: 0,
                        primitive_type,
                    },
                )
                .ok_or(LoweringError::Unsupported(
                    "routed scalar domain has no complete predicate",
                ))?;
                let subject = crate::shared::terminal_identities::value_id(1);
                let proposition = crate::scalar_lowering::contracts::proposition(
                    identities,
                    &predicate,
                    &[terminal_psi::ValueDeclaration {
                        id: subject,
                        scalar_type: terminal_scalar_type(primitive_type)?,
                        qualifications: ScalarQualificationSetId::ZERO,
                    }],
                    &[],
                    constant_views,
                )?;
                return Ok(Some(terminal_psi::ScalarDomainPredicate {
                    subject,
                    proposition,
                }));
            }
            reference = *base_type;
        }
        Ok(None)
    }

    pub(crate) fn type_atoms(
        checked: &CheckedTrees,
        reference: TypeReferenceHandle,
    ) -> Result<(PrimitiveType, Vec<(SymbolHandle, SemanticDomainId)>), LoweringError> {
        let primitive = observed_scalar_carrier(checked, reference)?;
        let mut current = reference;
        let mut active = Vec::new();
        let mut atoms = Vec::new();
        let mut projects_reference = false;
        loop {
            if !checked
                .type_reference_table
                .contains_type_reference(current)
                || active.contains(&current)
            {
                return unsupported("scalar qualification has a stale or cyclic type");
            }
            active.push(current);
            match checked.type_reference_table.type_reference(current) {
                TypeReferenceNode::Reference { referee, .. } => {
                    projects_reference = true;
                    current = *referee;
                }
                TypeReferenceNode::Named { symbol, .. } => {
                    if checked.symbols.builtin_type_atom(*symbol).is_none() {
                        return unsupported(
                            "scalar qualification carrier is not a builtin declaration",
                        );
                    }
                    break;
                }
                TypeReferenceNode::DynamicTrait { .. } => break,
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    for constraint in checked
                        .type_reference_table
                        .constraint_span(*constraints)
                        .ok_or(LoweringError::Unsupported(
                            "scalar qualification has stale constraints",
                        ))?
                    {
                        match constraint {
                            TypeConstraintNode::ArithmeticDomain(_) => {}
                            TypeConstraintNode::Domain(domain)
                                if !domain.establishment_routes.is_empty()
                                    && value_facts(checked)
                                        .scalar_type_refinement_predicates(current)
                                        .is_some() =>
                            {
                                atoms.push((domain.symbol, domain.semantic_id));
                            }
                            TypeConstraintNode::Domain(domain)
                                if matches!(domain.subject, DomainConstraintSubject::Value(value_domain)
                                    if validation::scalar_value_domain_predicate(value_domain,
                                        checked_trees::CheckedScalarExpression::Parameter { position: 0, primitive_type: primitive }
                                    ).is_some()) => {}
                            TypeConstraintNode::Domain(domain)
                                if validation::exact_declared_domain_carrier_interval(
                                    &checked.typed,
                                    primitive,
                                    domain,
                                )
                                .filter(|(minimum, maximum)| minimum <= maximum)
                                .is_some() => {}
                            TypeConstraintNode::Domain(domain)
                                if domain.predicate_presence.is_present()
                                    && value_facts(checked)
                                        .scalar_type_refinement_predicates(current)
                                        .is_some() => {}
                            TypeConstraintNode::Domain(domain)
                                if domain.subject == DomainConstraintSubject::Declared
                                    && !domain.predicate_presence.is_present() =>
                            {
                                declared_atoms(
                                    checked,
                                    domain.symbol,
                                    &domain.arguments,
                                    domain.semantic_id,
                                    primitive,
                                    &mut Vec::new(),
                                    &mut atoms,
                                )?;
                                if !checked.domain_definition(domain.symbol).is_some() {
                                    return unsupported(
                                        "scalar qualification lost its declaration",
                                    );
                                }
                            }
                            _ => {
                                return unsupported(
                                    "scalar qualification requires non-vacuous or unsupported evidence",
                                );
                            }
                        }
                    }
                    current = *base_type;
                }
                _ => {
                    return unsupported(
                        "scalar qualification cannot erase a reference or structural shell",
                    );
                }
            }
        }
        if projects_reference && !atoms.is_empty() {
            return unsupported("qualified scalar references require their own transport judgment");
        }
        atoms.sort_by_key(|(_, identity)| identity.0);
        atoms.dedup();
        Ok((primitive, atoms))
    }

    fn observed_scalar_carrier(
        checked: &CheckedTrees,
        mut reference: TypeReferenceHandle,
    ) -> Result<PrimitiveType, LoweringError> {
        for _ in 0..checked.type_reference_table.type_reference_count() {
            if !checked
                .type_reference_table
                .contains_type_reference(reference)
            {
                return unsupported("scalar qualification has a stale type");
            }
            match checked.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Reference {
                    referee, access, ..
                } if access.is_readable() => reference = *referee,
                TypeReferenceNode::Reference { .. } => {
                    return unsupported("scalar qualification cannot observe write-only storage");
                }
                TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                _ => {
                    return checked.primitive_type_reference(reference).ok_or(
                        LoweringError::Unsupported("scalar qualification has no primitive carrier"),
                    );
                }
            }
        }
        unsupported("scalar qualification has a cyclic type")
    }

    pub(crate) fn retained_scalar_restriction(
        checked: &CheckedTrees,
        reference: TypeReferenceHandle,
    ) -> Option<&checked_trees::CheckedUnitStructuralFieldType> {
        let primitive = checked.primitive_type_reference(reference)?;
        let identity = validation::structural_type_identity(&checked.typed, reference, &[], &[]);
        flow_facts(checked)
            .terminal_unit_effects
            .structural_types
            .iter()
            .find(|plan| {
                plan.identity == identity
                    && plan.shape
                        == checked_trees::CheckedUnitStructuralTypeShape::PrimitiveScalar(primitive)
            })
            .and_then(|plan| plan.scalar_restriction.as_ref())
    }

    pub(crate) fn declared_atoms(
        checked: &CheckedTrees,
        symbol: symbols::SymbolHandle,
        arguments: &[TypeReferenceHandle],
        semantic_id: SemanticDomainId,
        primitive: PrimitiveType,
        active: &mut Vec<symbols::SymbolHandle>,
        atoms: &mut Vec<(symbols::SymbolHandle, SemanticDomainId)>,
    ) -> Result<(), LoweringError> {
        if !symbol.is_valid() || active.contains(&symbol) || !semantic_id.is_valid() {
            return unsupported("scalar qualification has a stale or cyclic domain definition");
        }
        let mut declarations = checked
            .domain_definitions()
            .iter()
            .filter(|domain| domain.symbol == symbol);
        let domain = declarations.next().ok_or(LoweringError::Unsupported(
            "scalar qualification has no domain definition",
        ))?;
        if declarations.next().is_some()
            || domain.predicate_presence.is_present()
            || !domain.facts.is_empty()
            || (!checked_trees::domain::has_generic_carrier(checked, domain)
                && checked.primitive_type_reference(domain.target_type) != Some(primitive))
        {
            return unsupported(
                "scalar qualification declaration needs non-vacuous evidence or another carrier",
            );
        }
        let parameters = checked_trees::domain::index_parameters(checked, domain);
        checked_trees::domain::indexed_domain_instance_name(checked, domain, parameters, arguments)
            .map_err(|_| {
                LoweringError::Unsupported("scalar qualification has unresolved domain indices")
            })?;
        if let Some(alias) = &domain.alias {
            if alias.constituents.is_empty() || !arguments.is_empty() {
                return unsupported("scalar qualification has an empty or indexed alias expansion");
            }
            active.push(symbol);
            for constituent in &alias.constituents {
                let identity = checked
                    .domain_definition(constituent.domain_symbol)
                    .map(|domain| domain.semantic_id)
                    .ok_or(LoweringError::Unsupported(
                        "scalar qualification has an unresolved alias atom",
                    ))?;
                declared_atoms(
                    checked,
                    constituent.domain_symbol,
                    &[],
                    identity,
                    primitive,
                    active,
                    atoms,
                )?;
            }
            active.pop();
        } else {
            atoms.push((symbol, semantic_id));
        }
        Ok(())
    }
}
pub(crate) mod representation_view {

    use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
    use crate::emission::operation_emission::expressions::LoweredDirectExpression;
    use crate::emission::operation_emission::integer::LoweredIntegerBinaryKind;
    use crate::expression_preparation::integer_bit_image::integer_bit_image;
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};

    pub(crate) fn replay(
        operand: LoweredDirectExpression,
        view: ScalarType,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let source = operand.scalar_type();
        if source == view {
            return unsupported("a retained representation re-view must change its carrier");
        }
        if source
            .as_integer()
            .is_some_and(|integer| integer.bits() == 8 && integer.sign() == IntegerSign::Unsigned)
            && matches!(
                operand,
                LoweredDirectExpression::IndexedPrimitiveRead { .. }
            )
            && scalar_storage_bits(view).is_some_and(|bits| bits > 8)
        {
            return byte_region(operand, view);
        }
        match (source, view) {
            (ScalarType::Integer(source_type), ScalarType::Integer(view_type)) => {
                if source_type.bits() != view_type.bits() {
                    return unsupported("a representation re-view cannot change its carrier width");
                }
                if !observes_one_place(&operand) {
                    return unsupported("a representation re-view must observe one scalar place");
                }
                integer_bit_image(operand, source_type, view)
            }
            _ if terminal_semantics::ScalarRepresentationViewPair::classify(source, view)
                .is_some() =>
            {
                if !observes_one_place(&operand) {
                    return unsupported("a representation re-view must observe one scalar place");
                }
                Ok(LoweredDirectExpression::RepresentationView {
                    scalar_type: view,
                    operand: Box::new(operand),
                })
            }
            _ => unsupported(
                "a representation re-view pairs carriers of unequal width or without a bit view",
            ),
        }
    }

    fn byte_region(
        operand: LoweredDirectExpression,
        view: ScalarType,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let LoweredDirectExpression::IndexedPrimitiveRead {
            source,
            path,
            index,
            extent,
            element_path,
            scalar_type,
            ..
        } = operand
        else {
            return unsupported("a byte-region re-view must start at one runtime-selected byte");
        };
        if !element_path.is_empty() {
            return unsupported("a byte-region re-view must select a byte of one flat array");
        }
        let bits = scalar_storage_bits(view).ok_or(LoweringError::Unsupported(
            "a byte-region re-view needs an integer or IEEE carrier",
        ))?;
        if bits % 8 != 0 {
            return unsupported("a byte-region re-view needs a whole-byte carrier");
        }
        let region_type = IntegerType::new(IntegerSign::Unsigned, bits).map_err(|_| {
            LoweringError::Unsupported("a byte-region carrier has no unsigned twin")
        })?;
        let region = ScalarType::Integer(region_type);
        let index_type = index.scalar_type();
        let mut assembled: Option<LoweredDirectExpression> = None;
        for byte in 0..u128::from(bits / 8) {
            let byte_index = if byte == 0 {
                index.clone()
            } else {
                Box::new(LoweredDirectExpression::IntegerBinary {
                    kind: LoweredIntegerBinaryKind::ExactAdd,
                    scalar_type: index_type,
                    left: index.clone(),
                    right: Box::new(LoweredDirectExpression::IntegerLiteral {
                        value: IntegerValue::Unsigned(byte),
                        scalar_type: index_type,
                    }),
                    formation_decided: false,
                })
            };
            let widened = LoweredDirectExpression::IntegerWiden {
                scalar_type: region,
                operand: Box::new(LoweredDirectExpression::IndexedPrimitiveRead {
                    qualifications: Default::default(),
                    source,
                    path: path.clone(),
                    index: byte_index,
                    extent: extent.clone(),
                    element_path: Vec::new(),
                    scalar_type,
                    bound_decided: extent.is_none(),
                }),
            };
            let placed = if byte == 0 {
                widened
            } else {
                LoweredDirectExpression::IntegerBinary {
                    kind: LoweredIntegerBinaryKind::WrappingShiftLeft,
                    scalar_type: region,
                    left: Box::new(widened),
                    right: Box::new(LoweredDirectExpression::IntegerLiteral {
                        value: IntegerValue::Unsigned(byte * 8),
                        scalar_type: region,
                    }),
                    formation_decided: false,
                }
            };
            assembled = Some(match assembled {
                None => placed,
                Some(lower) => LoweredDirectExpression::IntegerBinary {
                    kind: LoweredIntegerBinaryKind::BitwiseOr,
                    scalar_type: region,
                    left: Box::new(lower),
                    right: Box::new(placed),
                    formation_decided: false,
                },
            });
        }
        let assembled = assembled.ok_or(LoweringError::Unsupported(
            "a byte-region re-view spans no bytes",
        ))?;
        if region == view {
            return Ok(assembled);
        }
        if matches!(view, ScalarType::IeeeFloat(_)) {
            return Ok(LoweredDirectExpression::RepresentationView {
                scalar_type: view,
                operand: Box::new(assembled),
            });
        }
        integer_bit_image(assembled, region_type, view)
    }

    fn scalar_storage_bits(scalar_type: ScalarType) -> Option<u16> {
        match scalar_type {
            ScalarType::Integer(integer) => Some(integer.bits()),
            ScalarType::IeeeFloat(format) => Some(format.byte_size() * 8),
            ScalarType::Boolean => None,
        }
    }

    fn observes_one_place(operand: &LoweredDirectExpression) -> bool {
        match operand {
            LoweredDirectExpression::PrimitiveRead { .. }
            | LoweredDirectExpression::StructuralField { .. }
            | LoweredDirectExpression::AtomicLoad { .. }
            | LoweredDirectExpression::IndexedPrimitiveRead { .. }
            | LoweredDirectExpression::ElementViewRead { .. }
            | LoweredDirectExpression::Parameter { .. }
            | LoweredDirectExpression::Local { .. } => true,
            LoweredDirectExpression::Boolean { expression } => matches!(
                expression.as_ref(),
                LoweredBooleanReturnExpression::PrimitiveRead { .. }
                    | LoweredBooleanReturnExpression::StructuralField { .. }
                    | LoweredBooleanReturnExpression::Parameter { .. }
                    | LoweredBooleanReturnExpression::Local { .. }
            ),
            _ => false,
        }
    }
}
pub(crate) mod saturating_shift {

    use crate::emission::operation_emission::expressions::LoweredDirectExpression;
    use crate::emission::operation_emission::integer::LoweredIntegerBinaryKind;
    use crate::expression_preparation::integer_bit_image::binary;
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};

    pub(crate) fn left(
        scalar_type: ScalarType,
        operand: LoweredDirectExpression,
        count: LoweredDirectExpression,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let Some(carrier) = scalar_type.as_integer() else {
            return unsupported("saturating shift requires a fixed integer carrier");
        };
        if carrier.is_address() || !matches!(carrier.bits(), 8 | 16 | 32 | 64) {
            return unsupported("saturating shift requires a supported fixed integer width");
        }
        let Some(count_carrier) = count.scalar_type().as_integer() else {
            return unsupported("saturating shift requires an integer count");
        };
        if count_carrier.is_address() || count_carrier.bits() < 8 {
            return unsupported("saturating shift requires a fixed integer count carrier");
        }
        Ok(LoweredDirectExpression::SaturatingShiftLeft {
            scalar_type,
            left: Box::new(operand),
            right: Box::new(count),
        })
    }

    pub(crate) fn expansion(
        scalar_type: ScalarType,
        operand: LoweredDirectExpression,
        count: LoweredDirectExpression,
    ) -> LoweredDirectExpression {
        let Some(carrier) = scalar_type.as_integer() else {
            unreachable!("prepared saturating shift has an integer carrier");
        };
        let Some(count_carrier) = count.scalar_type().as_integer() else {
            unreachable!("prepared saturating shift has an integer count");
        };
        let even = binary(
            LoweredIntegerBinaryKind::BitwiseAnd,
            count_carrier,
            count.clone(),
            literal(count_carrier, carrier.bits() - 2),
        );
        let low = binary(
            LoweredIntegerBinaryKind::BitwiseAnd,
            count_carrier,
            count,
            literal(count_carrier, 1),
        );
        let mut product = operand;
        for exponent in [even, low] {
            let factor = binary(
                LoweredIntegerBinaryKind::WrappingShiftLeft,
                carrier,
                literal(carrier, 1),
                exponent,
            );
            product = binary(
                LoweredIntegerBinaryKind::SaturatingMultiply,
                carrier,
                product,
                factor,
            );
        }
        product
    }

    fn literal(carrier: IntegerType, value: u16) -> LoweredDirectExpression {
        LoweredDirectExpression::IntegerLiteral {
            value: match carrier.sign() {
                IntegerSign::Signed => IntegerValue::Signed(i128::from(value)),
                IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(value)),
            },
            scalar_type: ScalarType::Integer(carrier),
        }
    }
}
pub(crate) mod structural_value_requirements {

    use crate::shared::checked_facts::value_facts;
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};
    use checked_trees::{CheckedTrees, CheckedUnitStructuralArgumentSourcePlan};
    use language_semantics::SemanticDomainId;

    pub(crate) fn collect_operation_requirements(
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        operation: &checked_trees::CheckedUnitEffectOperationPlan,
        domains: &mut Vec<SemanticDomainId>,
    ) -> Result<bool, LoweringError> {
        if let checked_trees::CheckedUnitEffectOperationPlan::EstablishReference {
            source_contract,
            ..
        } = operation
        {
            collect_reference_contract(source_contract, domains);
        }
        let Some((value, _)) = operation.structural_value_evaluation() else {
            return Ok(false);
        };
        let statement = operation
            .source_statement_index()
            .ok_or(LoweringError::Unsupported(
                "structural value evaluation has no source statement",
            ))?;
        let values = &checked.facts.values.structural_values;
        let mut roots = values.roots.iter().map(|(_, root)| root).filter(|root| {
            root.machine == machine && root.statement_ordinal == statement && root.root == value
        });
        let reference = roots
            .next()
            .map(|root| root.type_reference)
            .unwrap_or_default();
        if roots.next().is_some() {
            return unsupported("structural domain catalog has ambiguous value roots");
        }
        collect_domains(checked, value, reference, domains)
    }

    pub(crate) fn collect_value_requirements(
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        value: checked_trees::CheckedStructuralValueHandle,
        domains: &mut Vec<SemanticDomainId>,
    ) -> Result<bool, LoweringError> {
        let reference = value_facts(checked)
            .structural_values
            .roots
            .iter()
            .map(|(_, root)| root)
            .find(|root| root.machine == machine && root.root == value)
            .map(|root| root.type_reference)
            .unwrap_or_default();
        collect_domains(checked, value, reference, domains)
    }

    fn collect_domains(
        checked: &CheckedTrees,
        value: checked_trees::CheckedStructuralValueHandle,
        expected: TypeReferenceHandle,
        domains: &mut Vec<SemanticDomainId>,
    ) -> Result<bool, LoweringError> {
        use checked_trees::{CheckedStructuralRecordFieldValue, CheckedStructuralValueKind};
        let plans = &checked.facts.values.structural_values;
        let mut pending = vec![(value, expected)];
        let mut visited = Vec::new();
        let mut requires_literal_view = false;
        while let Some((value, reference)) = pending.pop() {
            if !plans.nodes.is_valid(value) {
                return unsupported("structural domain catalog has stale construction");
            }
            if visited.contains(&(value, reference)) {
                continue;
            }
            visited.push((value, reference));
            let node = plans.nodes.get(value);
            for call in plans
                .call_schedule(value)
                .map(|(_, calls)| calls)
                .unwrap_or(&[])
            {
                for operation in call.operands().iter() {
                    if let Some((value, _)) = operation.structural_value_evaluation() {
                        let child = plans.nodes.get(value);
                        pending.push((value, child.type_reference));
                    }
                }
            }
            for decision in checked
                .facts
                .qualifications
                .value_formations
                .span(node.qualification_decisions)
                .ok_or(LoweringError::Unsupported(
                    "structural value membership decisions are stale",
                ))?
            {
                if !domains.contains(&decision.semantic_domain) {
                    domains.push(decision.semantic_domain);
                }
            }
            match &node.kind {
                CheckedStructuralValueKind::Reference {
                    backing_type,
                    source_contract,
                    source_value,
                    ..
                } => {
                    collect_reference_contract(source_contract, domains);
                    if source_value.is_valid() {
                        if !plans.nodes.is_valid(*source_value) {
                            return Err(LoweringError::InvalidReferenceContract {
                                supplying_type: *backing_type,
                                reason: "reference domain catalog has stale source value"
                                    .to_owned(),
                            });
                        }
                        let child = plans.nodes.get(*source_value);
                        pending.push((*source_value, child.type_reference));
                    }
                }

                CheckedStructuralValueKind::ZeroedStructural {
                    projected_qualifications,
                } => {
                    for row in projected_qualifications {
                        if !domains.contains(&row.domain) {
                            domains.push(row.domain);
                        }
                    }
                }
                CheckedStructuralValueKind::ByteLiteralView { .. } => {
                    requires_literal_view = true;
                }
                CheckedStructuralValueKind::Place(argument)
                    if matches!(
                        argument.source,
                        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }
                    ) =>
                {
                    requires_literal_view = true;
                    for domain in super::byte_literals::validate(checked, reference, argument)? {
                        if !domains.contains(&domain) {
                            domains.push(domain);
                        }
                    }
                }
                checked_trees::record_or_structural_case_pattern!(fields, ..) => {
                    for field in
                        plans
                            .record_fields
                            .span(*fields)
                            .ok_or(LoweringError::Unsupported(
                                "structural domain catalog has stale fields",
                            ))?
                    {
                        if let CheckedStructuralRecordFieldValue::Structural(child) = field.value {
                            pending.push((child, field.type_reference));
                        }
                    }
                }
                CheckedStructuralValueKind::Dispatch { arms, .. } => {
                    for arm in plans
                        .dispatch_arms
                        .span(*arms)
                        .ok_or(LoweringError::Unsupported(
                            "structural domain catalog has stale arms",
                        ))?
                    {
                        pending.push((arm.value, reference));
                    }
                }
                CheckedStructuralValueKind::FixedArray { elements } => {
                    let element = if checked
                        .type_reference_table
                        .contains_type_reference(reference)
                        && let TypeReferenceNode::FixedArray { element_type, .. } =
                            checked.type_reference_table.type_reference(reference)
                    {
                        *element_type
                    } else {
                        TypeReferenceHandle::default()
                    };
                    pending.extend(
                        elements
                            .iter()
                            .map(|element_value| (*element_value, element)),
                    );
                }
                CheckedStructuralValueKind::ViewSubslice { source, .. } => {
                    let child = plans.nodes.get(*source);
                    pending.push((*source, child.type_reference));
                }
                CheckedStructuralValueKind::Projection { source, .. } => {
                    if !plans.nodes.is_valid(*source) {
                        return unsupported(
                            "structural domain catalog has stale projection source",
                        );
                    }
                    if !matches!(
                        plans.nodes.get(*source).kind,
                        CheckedStructuralValueKind::CopiedStructuralPlace { .. }
                    ) {
                        let source_reference = plans
                            .roots
                            .iter()
                            .map(|(_, root)| root)
                            .find(|root| root.root == *source)
                            .map(|root| root.type_reference)
                            .unwrap_or_default();
                        pending.push((*source, source_reference));
                    }
                }
                _ => {}
            }
        }
        Ok(requires_literal_view)
    }

    pub(crate) fn collect_reference_contract(
        contract: &checked_trees::CheckedReferenceSourceContract,
        domains: &mut Vec<SemanticDomainId>,
    ) {
        if let checked_trees::CheckedReferenceSourceContract::Standing { contract, .. } = contract {
            for (domain, _) in &contract.domains {
                if !domains.contains(domain) {
                    domains.push(*domain);
                }
            }
        }
    }
}
