//! Structural values retain a type namespace without inventing source parameters.

use checked_trees::{
    CheckedScalarComputationKind, CheckedScalarComputationPlans, CheckedUnitStructuralTypePlan,
};
use symbols::SymbolHandle;

/// Retain a Case subject's value shapes and enqueue its field values.
fn retain_case_subject_fields(
    shapes: &mut Vec<CheckedUnitStructuralTypePlan>,
    plans: &CheckedScalarComputationPlans,
    program: &dyn ::validation::CheckingInput,
    subject: &checked_trees::CheckedScalarCaseConstruction,
    pending: &mut Vec<arena::Handle<checked_trees::CheckedScalarComputation>>,
) -> Option<()> {
    retain_declarations(
        shapes,
        super::super::terminal_unit::scalar_case_value_shapes(program, subject.type_reference)?,
    )?;
    pending.extend(
        plans
            .case_fields
            .span(subject.fields)?
            .iter()
            .map(|field| field.value),
    );
    Some(())
}

pub(crate) fn retain_shapes(
    program: &dyn ::validation::CheckingInput,
    plans: &CheckedScalarComputationPlans,
    structural_values: &checked_trees::CheckedStructuralValuePlans,
    state: SymbolHandle,
    shapes: &mut Vec<CheckedUnitStructuralTypePlan>,
) -> Option<()> {
    let mut failure_phase = "initial inventory";
    let mut failure_structural = arena::Handle::<checked_trees::CheckedStructuralValue>::invalid();
    let mut failure_scalar = arena::Handle::<checked_trees::CheckedScalarComputation>::invalid();
    let retained = (|| {
        // A stored reference publishes only its pointer shape. Forming one also
        // demands actual backing storage, whether named or computed. Follow the
        // retained formation/value edges, never transitive pointee types or
        // speculative arena nodes; recursive pointer catalogs stay finite.
        let mut retained_values = Vec::new();
        for root in structural_values.roots_for_state(state) {
            failure_phase = "structural dependency inventory";
            failure_structural = root.root;
            for value in structural_values.value_dependencies(root.root)? {
                failure_phase = "structural dependency fields/reference shapes";
                failure_structural = value;
                if retained_values.contains(&value) {
                    continue;
                }
                retained_values.push(value);
                if let checked_trees::CheckedStructuralValueKind::Record { fields, .. }
                | checked_trees::CheckedStructuralValueKind::StructuralCase {
                    fields, ..
                } = &structural_values.nodes.get(value).kind
                {
                    for field in structural_values.record_fields.span(*fields)? {
                        let (_, declaration) = program.any_field_owner(field.field)?;
                        if declaration.relevance.is_erased()
                            || !matches!(
                                field.value,
                                checked_trees::CheckedStructuralRecordFieldValue::Structural(_)
                            )
                        {
                            continue;
                        }
                        // Constructor operands complete in their own storage before
                        // joining the field. Inline field geometry (including byte
                        // views) cannot replace that operand's exact descriptor.
                        retain_declarations(
                            shapes,
                            super::super::terminal_unit::scalar_operand_shapes(
                                program,
                                field.type_reference,
                            )?,
                        )?;
                    }
                }
                if let checked_trees::CheckedStructuralValueKind::Reference {
                    source,
                    source_value,
                    backing_type,
                    ..
                } = &structural_values.nodes.get(value).kind
                {
                    // Formation materializes its own permission carrier before a
                    // consumer crosses Referent. A nested producer's descriptor
                    // need not occur in the outer call's signature; its backing
                    // declaration alone cannot supply that stored pointer type.
                    retain_declarations(
                        shapes,
                        super::super::terminal_unit::scalar_operand_shapes(
                            program,
                            structural_values.nodes.get(value).type_reference,
                        )?,
                    )?;
                    retain_declarations(
                        shapes,
                        super::super::terminal_unit::reference_backing_shapes(
                            program,
                            *backing_type,
                            source.type_identity,
                        )?,
                    )?;
                    if source_value.is_valid() {
                        // A projected computed source also owns its complete
                        // containing storage, independently of the borrowed leaf.
                        retain_declarations(
                            shapes,
                            super::super::terminal_unit::scalar_operand_shapes(
                                program,
                                structural_values.nodes.get(*source_value).type_reference,
                            )?,
                        )?;
                    }
                }
            }
        }
        let mut pending = plans.roots_for_state(state);
        let mut visited = Vec::new();
        while let Some(handle) = pending.pop() {
            failure_phase = "scalar node inventory";
            failure_scalar = handle;
            if !plans.nodes.is_valid(handle) {
                return None;
            }
            if visited.contains(&handle) {
                continue;
            }
            visited.push(handle);
            failure_phase = "scalar node dependencies/shapes";
            match &plans.nodes.get(handle).kind {
                CheckedScalarComputationKind::CaseMembership {
                    subject: checked_trees::CheckedScalarComputationStructuralArgument::Place(_),
                    receiver_value,
                    ..
                } if !receiver_value.is_valid() => {}
                CheckedScalarComputationKind::CaseMembership {
                    subject:
                        checked_trees::CheckedScalarComputationStructuralArgument::Case(subject),
                    ..
                } => retain_case_subject_fields(shapes, plans, program, subject, &mut pending)?,
                CheckedScalarComputationKind::SelectedComparison { left, right, .. } => {
                    pending.extend([*left, *right])
                }
                checked_trees::cast_or_qualification_triple_pattern!(operand, ..) => {
                    pending.push(*operand)
                }
                CheckedScalarComputationKind::ByteSequenceLiteralComparison {
                    operand_type,
                    ..
                }
                | CheckedScalarComputationKind::StorageAddress { operand_type, .. }
                | CheckedScalarComputationKind::ByteSequencePredicate { operand_type, .. } => {
                    retain_declarations(
                        shapes,
                        super::super::terminal_unit::scalar_operand_shapes(program, *operand_type)?,
                    )?;
                }
                CheckedScalarComputationKind::StructuralField { receiver_value, .. }
                | CheckedScalarComputationKind::CaseMembership { receiver_value, .. } => {
                    if !receiver_value.is_valid() {
                        continue;
                    }
                    if !structural_values.nodes.is_valid(*receiver_value) {
                        return None;
                    }
                    failure_phase = "scalar receiver exact root";
                    failure_structural = *receiver_value;
                    let mut roots = structural_values.roots.iter().filter_map(|(_, root)| {
                        (root.state == state && root.root == *receiver_value).then_some(root)
                    });
                    let root = roots.next()?;
                    if roots.next().is_some()
                        || structural_values.nodes.get(*receiver_value).expression
                            != root.expression
                    {
                        return None;
                    }
                    // The evaluated dependency owns the whole receiver type. The
                    // ordinary shape collector retains its transitive fields;
                    // neither the observed leaf nor a source local supplies it.
                    retain_declarations(
                        shapes,
                        super::super::terminal_unit::scalar_operand_shapes(
                            program,
                            root.type_reference,
                        )?,
                    )?;
                }
                CheckedScalarComputationKind::RetainedConstant { .. }
                | CheckedScalarComputationKind::Value(_) => {}
                CheckedScalarComputationKind::Dispatch { subject, arms, .. } => {
                    pending.extend(subject.scalar_operands(&plans.operands));
                    if let checked_trees::CheckedDispatchSubject::Structural { value, .. } = subject
                    {
                        retain_declarations(
                            shapes,
                            super::super::terminal_unit::scalar_operand_shapes(
                                program,
                                structural_values.nodes.get(*value).type_reference,
                            )?,
                        )?;
                    }
                    for arm in plans.dispatch_arms.span(*arms)? {
                        if let Some(value) = arm.pattern.computation() {
                            pending.push(value);
                        }
                        pending.push(arm.value);
                    }
                }
                CheckedScalarComputationKind::Call {
                    callee,
                    arguments,
                    structural_arguments,
                    ..
                } => {
                    pending.extend(callee.selector());
                    pending.extend(plans.operands.span(*arguments)?);
                    for argument in plans.structural_arguments.span(*structural_arguments)? {
                        if let Some(place) = argument.as_place() {
                            pending.extend(place.computation_operands(&plans.operands));
                            // A borrowed view does not define its scalar ingress's
                            // storage carrier. Retain the actual authored formal
                            // so lowering can establish that original home.
                            if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ScalarParameter {
                                parameter_index,
                            } = place.source
                            {
                                let authored = program.state_by_symbol(state)?;
                                let parameter = program
                                    .state_parameters(authored)
                                    .iter()
                                    .filter(|parameter| {
                                        !parameter.is_const
                                            && !parameter.relevance.is_erased()
                                            && program
                                                .primitive_type_reference(parameter.type_reference)
                                                .is_some()
                                    })
                                    .nth(parameter_index as usize)?;
                                retain_declarations(
                                    shapes,
                                    super::super::terminal_unit::scalar_operand_shapes(
                                        program,
                                        parameter.type_reference,
                                    )?,
                                )?;
                            }
                        }
                        if let checked_trees::CheckedScalarComputationStructuralArgument::Place(
                            argument,
                        ) = argument
                            && let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                                expression,
                            } = argument.source
                        {
                            // A saved home is an alias of a completed subject,
                            // not another producer root. Its catalog dependency
                            // is the original retained value; executable reads
                            // still use the saved anchor and never replay it.
                            let root = structural_values.completed_expression_dependency(
                                &program.expression_table, state, None, expression,
                            )?;
                            if (argument.path.is_empty()
                                    && validation::structural_type_identity(program, root.type_reference, &[], &[])
                                        != argument.type_identity)
                                || !matches!(argument.access,
                                    checked_trees::CheckedStructuralAccess::Owned
                                        | checked_trees::CheckedStructuralAccess::SharedBorrow
                                        | checked_trees::CheckedStructuralAccess::MutableBorrow
                                        | checked_trees::CheckedStructuralAccess::WriteOnlyBorrow)
                            {
                                return None;
                            }
                            // The produced value owns its complete declaration,
                            // including when the invocation only borrows it. The
                            // formal's reference type does not replace this owner;
                            // a projected argument likewise retains its exact path
                            // and transfer without replacing the owner's catalog.
                            // A view root owns descriptor storage, not its
                            // referent: retain its admitted carrier without
                            // reapplying an owned-result admission fence.
                            // its scalar children have their own retained roots in
                            // this same state's computation inventory.
                            retain_declarations(
                                shapes,
                                super::super::terminal_unit::scalar_operand_shapes(
                                    program,
                                    root.type_reference,
                                )?,
                            )?;
                        }
                        if let checked_trees::CheckedScalarComputationStructuralArgument::Case(
                            subject,
                        ) = argument
                        {
                            retain_case_subject_fields(
                                shapes,
                                plans,
                                program,
                                subject,
                                &mut pending,
                            )?;
                        }
                    }
                }
                CheckedScalarComputationKind::Select {
                    condition,
                    when_true,
                    when_false,
                    ..
                } => pending.extend([*condition, *when_true, *when_false]),
                CheckedScalarComputationKind::WithOperands {
                    operands, operand, ..
                } => {
                    pending.push(*operand);
                    for dependency in plans.evaluation_operands.span(*operands)? {
                        match dependency {
                            checked_trees::CheckedScalarEvaluationOperand::Scalar(value) => {
                                pending.push(*value)
                            }
                            checked_trees::CheckedScalarEvaluationOperand::Structural(value) => {
                                failure_phase = "ordered evaluation structural operand exact root";
                                failure_structural = *value;
                                let root = structural_values.root_for_value(*value)?;
                                retain_declarations(
                                    shapes,
                                    super::super::terminal_unit::scalar_operand_shapes(
                                        program,
                                        root.type_reference,
                                    )?,
                                )?;
                            }
                        }
                    }
                }
                CheckedScalarComputationKind::Apply { operands, .. }
                | CheckedScalarComputationKind::StorageObservation { operands, .. } => {
                    pending.extend(plans.operands.span(*operands)?)
                }
            }
        }
        Some(())
    })();
    if retained.is_none() && std::env::var_os("OMEGA_LOCAL_PRODUCTION_TRACE").is_some() {
        let call_arguments = plans
            .nodes
            .is_valid(failure_scalar)
            .then(|| match &plans.nodes.get(failure_scalar).kind {
                CheckedScalarComputationKind::Call {
                    structural_arguments,
                    ..
                } => plans.structural_arguments.span(*structural_arguments),
                _ => None,
            })
            .flatten();
        let argument_sources = call_arguments.map(|arguments| arguments.iter().filter_map(|argument| {
            let checked_trees::CheckedScalarComputationStructuralArgument::Place(argument) = argument else {
                return None;
            };
            let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression } = argument.source else {
                return None;
            };
            let node = program.expression_table.expression_is_valid(expression)
                .then(|| program.expression_table.expression(expression));
            let saved_source = match node {
                Some(typed_trees::expression::ExpressionNode::SavedSubject(saved)) => {
                    program.expression_table.saved_subject_source(saved)
                }
                _ => None,
            };
            let saved_node = saved_source.filter(|expression| program.expression_table.expression_is_valid(*expression))
                .map(|expression| program.expression_table.expression(expression));
            Some((expression, node, saved_source, saved_node))
        }).collect::<Vec<_>>());
        eprintln!(
            "VIEW_CONSTRUCTION_SHAPES_REFUSAL state={:?} phase={} structural={:?} structural_node={:?} structural_exact_roots={:?} scalar={:?} scalar_node={:?} retained_shape_count={} call_arguments={:?} argument_sources={:?} state_root_count={} state_roots_prefix={:?}",
            state,
            failure_phase,
            failure_structural,
            structural_values
                .nodes
                .is_valid(failure_structural)
                .then(|| structural_values.nodes.get(failure_structural)),
            structural_values
                .roots
                .iter()
                .filter(|(_, root)| root.state == state && root.root == failure_structural)
                .take(4)
                .map(|(_, root)| root)
                .collect::<Vec<_>>(),
            failure_scalar,
            plans
                .nodes
                .is_valid(failure_scalar)
                .then(|| plans.nodes.get(failure_scalar)),
            shapes.len(),
            call_arguments,
            argument_sources,
            structural_values.roots_for_state(state).len(),
            structural_values
                .roots_for_state(state)
                .iter()
                .take(32)
                .collect::<Vec<_>>(),
        );
    }
    retained
}

fn retain_declarations(
    shapes: &mut Vec<CheckedUnitStructuralTypePlan>,
    declarations: impl IntoIterator<Item = CheckedUnitStructuralTypePlan>,
) -> Option<()> {
    for declaration in declarations {
        if let Some(existing) =
            checked_trees::checked_structural_type_by_identity(shapes, declaration.identity)
        {
            if existing != &declaration {
                return None;
            }
        } else {
            shapes.push(declaration);
        }
    }
    Some(())
}
