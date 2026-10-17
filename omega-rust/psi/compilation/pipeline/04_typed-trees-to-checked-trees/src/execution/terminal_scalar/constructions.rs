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
    // A stored reference publishes only its pointer shape. Forming one also
    // demands actual backing storage, whether named or computed. Follow the
    // retained formation/value edges, never transitive pointee types or
    // speculative arena nodes; recursive pointer catalogs stay finite.
    let mut retained_values = Vec::new();
    for (_, root) in structural_values
        .roots
        .iter()
        .filter(|(_, root)| root.state == state)
    {
        for value in structural_values.value_dependencies(root.root)? {
            if retained_values.contains(&value) {
                continue;
            }
            retained_values.push(value);
            if let checked_trees::CheckedStructuralValueKind::Record { fields, .. }
            | checked_trees::CheckedStructuralValueKind::StructuralCase { fields, .. } =
                &structural_values.nodes.get(value).kind
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
    let mut pending = plans
        .roots
        .iter()
        .filter_map(|(_, root)| (root.state == state).then_some(root.root))
        .collect::<Vec<_>>();
    let mut visited = Vec::new();
    while let Some(handle) = pending.pop() {
        if !plans.nodes.is_valid(handle) {
            return None;
        }
        if visited.contains(&handle) {
            continue;
        }
        visited.push(handle);
        match &plans.nodes.get(handle).kind {
            CheckedScalarComputationKind::CaseMembership {
                subject: checked_trees::CheckedScalarComputationStructuralArgument::Place(_),
                receiver_value,
                ..
            } if !receiver_value.is_valid() => {}
            CheckedScalarComputationKind::CaseMembership {
                subject: checked_trees::CheckedScalarComputationStructuralArgument::Case(subject),
                ..
            } => retain_case_subject_fields(shapes, plans, program, subject, &mut pending)?,
            CheckedScalarComputationKind::SelectedComparison { left, right, .. } => {
                pending.extend([*left, *right])
            }
            checked_trees::cast_or_qualification_triple_pattern!(operand, ..) => {
                pending.push(*operand)
            }
            CheckedScalarComputationKind::ByteSequenceLiteralComparison {
                operand_type, ..
            }
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
                let mut roots = structural_values.roots.iter().filter_map(|(_, root)| {
                    (root.state == state && root.root == *receiver_value).then_some(root)
                });
                let root = roots.next()?;
                if roots.next().is_some()
                    || structural_values.nodes.get(*receiver_value).expression != root.expression
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
                pending.push(*subject);
                for arm in plans.dispatch_arms.span(*arms)? {
                    if let checked_trees::CheckedScalarDispatchPattern::Value(value) = arm.pattern {
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
                        pending.extend(place.computation_operands());
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
                        let mut roots = structural_values.roots.iter().filter_map(|(_, root)| {
                            (root.state == state && root.expression == expression).then_some(root)
                        });
                        let root = roots.next()?;
                        if roots.next().is_some()
                            || !structural_values.nodes.is_valid(root.root)
                            || structural_values.nodes.get(root.root).expression != expression
                            || (argument.path.is_empty()
                                && validation::structural_type_identity(program, root.type_reference, &[], &[])
                                    != argument.type_identity)
                            || !matches!(argument.access,
                                checked_trees::CheckedStructuralAccess::Owned
                                    | checked_trees::CheckedStructuralAccess::SharedBorrow
                                    | checked_trees::CheckedStructuralAccess::MutableBorrow)
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
                        retain_case_subject_fields(shapes, plans, program, subject, &mut pending)?;
                    }
                }
            }
            CheckedScalarComputationKind::Select {
                condition,
                when_true,
                when_false,
                ..
            } => pending.extend([*condition, *when_true, *when_false]),
            CheckedScalarComputationKind::Apply { operands, .. }
            | CheckedScalarComputationKind::StorageObservation { operands, .. } => {
                pending.extend(plans.operands.span(*operands)?)
            }
        }
    }
    Some(())
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
