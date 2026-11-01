//! Structural values retain a type namespace without inventing source parameters.

use super::super::terminal_unit::reference_backing_shapes;
use super::super::terminal_unit::scalar_case_value_shapes;
use super::super::terminal_unit::scalar_operand_shapes;
use checked_trees::{
    CheckedScalarComputationKind as ComputationKind, CheckedScalarComputationPlans,
    CheckedUnitStructuralTypePlan as TypePlan,
};
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;

/// Retain a Case subject's value shapes and enqueue its field values.
fn retain_case_subject_fields(
    shapes: &mut Vec<TypePlan>,
    plans: &CheckedScalarComputationPlans,
    program: &dyn ::validation::CheckingInput,
    subject: &checked_trees::CheckedScalarCaseConstruction,
    pending: &mut Vec<arena::Handle<checked_trees::CheckedScalarComputation>>,
) -> Result<(), String> {
    retain_declarations(
        program,
        shapes,
        scalar_case_value_shapes(program, subject.type_reference).ok_or_else(|| {
            format!(
                "cannot collect case subject storage for {}",
                type_description(program, subject.type_reference)
            )
        })?,
    )?;
    pending.extend(
        plans
            .case_fields
            .span(subject.fields)
            .ok_or_else(|| {
                format!(
                    "case subject {} has no retained field span",
                    type_description(program, subject.type_reference)
                )
            })?
            .iter()
            .map(|field| field.value),
    );
    Ok(())
}

pub(crate) fn retain_shapes(
    program: &dyn ::validation::CheckingInput,
    plans: &CheckedScalarComputationPlans,
    structural_values: &checked_trees::CheckedStructuralValuePlans,
    state: SymbolHandle,
    is_executable: impl Fn(u32) -> bool,
    shapes: &mut Vec<TypePlan>,
) -> Result<(), String> {
    let retained = (|| {
        macro_rules! retain_operand {
            ($type_reference:expr, $context:expr) => {{
                let reference = $type_reference;
                let declarations = scalar_operand_shapes(program, reference).ok_or_else(|| {
                    format!(
                        "{}: cannot collect operand storage for {}",
                        $context,
                        type_description(program, reference)
                    )
                })?;
                retain_declarations(program, shapes, declarations)
                    .map_err(|reason| format!("{}: {reason}", $context))
            }};
        }
        // A stored reference publishes only its pointer shape. Forming one also
        // demands actual backing storage, whether named or computed. Follow the
        // retained formation/value edges, never transitive pointee types or
        // speculative arena nodes; recursive pointer catalogs stay finite.
        let mut retained_values = Vec::new();
        for root in structural_values.roots_for_state(state) {
            // Erased argument roots retain proof/custody identity, but own no
            // executable constructor storage. Live dependencies still follow
            // their complete retained value graph below.
            if !is_executable(root.statement_ordinal) {
                continue;
            }
            for value in structural_values
                .value_dependencies(root.root)
                .ok_or_else(|| {
                    format!(
                        "structural root {:?} at statement {} has an invalid dependency graph",
                        root.root, root.statement_ordinal
                    )
                })?
            {
                if retained_values.contains(&value) {
                    continue;
                }
                retained_values.push(value);
                if let checked_trees::CheckedStructuralValueKind::Record { fields, .. }
                | checked_trees::CheckedStructuralValueKind::StructuralCase {
                    fields, ..
                } = &structural_values.nodes.get(value).kind
                {
                    for field in structural_values
                        .record_fields
                        .span(*fields)
                        .ok_or_else(|| {
                            format!("constructor {value:?} has no retained field span")
                        })?
                    {
                        let (_, declaration) =
                            program.any_field_owner(field.field).ok_or_else(|| {
                                format!(
                                    "constructor {value:?} field {:?} has no declaration owner",
                                    field.field
                                )
                            })?;
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
                        retain_operand!(
                            field.type_reference,
                            format_args!("constructor {value:?} field {:?}", field.field)
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
                    retain_operand!(
                        structural_values.nodes.get(value).type_reference,
                        format_args!("reference formation {value:?} descriptor")
                    )?;
                    retain_declarations(
                        program,
                        shapes,
                        reference_backing_shapes(program, *backing_type, source.type_identity)
                            .ok_or_else(|| {
                                format!(
                                    "reference formation {value:?}: cannot collect backing {} for identity {:?}",
                                    type_description(program, *backing_type),
                                    source.type_identity
                                )
                            })?,
                    )
                    .map_err(|reason| format!("reference formation {value:?} backing: {reason}"))?;
                    if source_value.is_valid() {
                        // A projected computed source also owns its complete
                        // containing storage, independently of the borrowed leaf.
                        retain_operand!(
                            structural_values.nodes.get(*source_value).type_reference,
                            format_args!("reference formation {value:?} source {source_value:?}")
                        )?;
                    }
                }
            }
        }
        let mut pending: Vec<_> = plans
            .root_records_for_state(state)
            .into_iter()
            .filter(|root| is_executable(root.statement_ordinal))
            .map(|root| root.root)
            .collect();
        let mut visited = Vec::new();
        while let Some(handle) = pending.pop() {
            if !plans.nodes.is_valid(handle) {
                return Err(format!("scalar computation {handle:?} is not retained"));
            }
            if visited.contains(&handle) {
                continue;
            }
            visited.push(handle);
            match &plans.nodes.get(handle).kind {
                ComputationKind::CaseMembership {
                    subject: checked_trees::CheckedScalarComputationStructuralArgument::Place(_),
                    receiver_value,
                    ..
                } if !receiver_value.is_valid() => {}
                ComputationKind::CaseMembership {
                    subject:
                        checked_trees::CheckedScalarComputationStructuralArgument::Case(subject),
                    ..
                } => retain_case_subject_fields(shapes, plans, program, subject, &mut pending)
                    .map_err(|reason| format!("case membership {handle:?}: {reason}"))?,
                ComputationKind::SelectedComparison { left, right, .. } => {
                    pending.extend([*left, *right])
                }
                checked_trees::cast_or_qualification_triple_pattern!(operand, ..) => {
                    pending.push(*operand)
                }
                ComputationKind::ByteSequenceLiteralComparison { operand_type, .. }
                | ComputationKind::StorageAddress { operand_type, .. }
                | ComputationKind::ByteSequencePredicate { operand_type, .. } => {
                    retain_operand!(
                        *operand_type,
                        format_args!("scalar computation {handle:?} operand")
                    )?;
                }
                ComputationKind::StructuralField { receiver_value, .. }
                | ComputationKind::CaseMembership { receiver_value, .. } => {
                    if !receiver_value.is_valid() {
                        continue;
                    }
                    if !structural_values.nodes.is_valid(*receiver_value) {
                        return Err(format!(
                            "scalar computation {handle:?} receiver {receiver_value:?} is not retained"
                        ));
                    }
                    let mut roots = structural_values.roots.iter().filter_map(|(_, root)| {
                        (root.state == state && root.root == *receiver_value).then_some(root)
                    });
                    let root = roots.next().ok_or_else(|| {
                        format!(
                            "scalar computation {handle:?} receiver {receiver_value:?} has no root in this state"
                        )
                    })?;
                    if roots.next().is_some() {
                        return Err(format!(
                            "scalar computation {handle:?} receiver {receiver_value:?} has multiple roots in this state"
                        ));
                    }
                    if structural_values.nodes.get(*receiver_value).expression != root.expression {
                        return Err(format!(
                            "scalar computation {handle:?} receiver {receiver_value:?} expression {:?} differs from root expression {:?}",
                            structural_values.nodes.get(*receiver_value).expression,
                            root.expression
                        ));
                    }
                    // The evaluated dependency owns the whole receiver type. The
                    // ordinary shape collector retains its transitive fields;
                    // neither the observed leaf nor a source local supplies it.
                    retain_operand!(
                        root.type_reference,
                        format_args!("scalar computation {handle:?} receiver {receiver_value:?}")
                    )?;
                }
                ComputationKind::RetainedConstant { .. } | ComputationKind::Value(_) => {}
                ComputationKind::Dispatch { subject, arms, .. } => {
                    pending.extend(subject.scalar_operands(&plans.operands));
                    if let checked_trees::CheckedDispatchSubject::Structural { value, .. } = subject
                    {
                        retain_operand!(
                            structural_values.nodes.get(*value).type_reference,
                            format_args!("dispatch {handle:?} subject {value:?}")
                        )?;
                    }
                    for arm in plans
                        .dispatch_arms
                        .span(*arms)
                        .ok_or_else(|| format!("dispatch {handle:?} has no retained arm span"))?
                    {
                        if let Some(value) = arm.pattern.computation() {
                            pending.push(value);
                        }
                        pending.push(arm.value);
                    }
                }
                ComputationKind::Call {
                    callee,
                    arguments,
                    structural_arguments,
                    ..
                } => {
                    pending.extend(callee.selector());
                    pending.extend(plans.operands.span(*arguments).ok_or_else(|| {
                        format!("call {handle:?} has no retained scalar argument span")
                    })?);
                    for argument in plans
                        .structural_arguments
                        .span(*structural_arguments)
                        .ok_or_else(|| {
                            format!("call {handle:?} has no retained structural argument span")
                        })?
                    {
                        if let Some(place) = argument.as_place() {
                            pending.extend(place.computation_operands(&plans.operands));
                            // A borrowed view does not define its scalar ingress's
                            // storage carrier. Retain the actual authored formal
                            // so lowering can establish that original home.
                            if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ScalarParameter {
                                parameter_index,
                            } = place.source
                            {
                                let authored = program.state_by_symbol(state).ok_or_else(|| {
                                    format!("call {handle:?} scalar ingress has no authored state")
                                })?;
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
                                    .nth(parameter_index as usize)
                                    .ok_or_else(|| {
                                        format!(
                                            "call {handle:?} scalar ingress parameter {parameter_index} is absent"
                                        )
                                    })?;
                                retain_operand!(
                                    parameter.type_reference,
                                    format_args!("call {handle:?} scalar ingress parameter {parameter_index}")
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
                            ).ok_or_else(|| {
                                format!("call {handle:?} expression {expression:?} has no completed structural dependency")
                            })?;
                            if argument.path.is_empty() {
                                let producer_identity = validation::structural_type_identity(
                                    program, root.type_reference, &[], &[],
                                );
                                if producer_identity != argument.type_identity {
                                    return Err(format!(
                                        "call {handle:?} expression {expression:?} producer {} differs from argument identity `{}` ({:?})",
                                        type_description(program, root.type_reference),
                                        program.type_identity_text(argument.type_identity).as_deref().unwrap_or("<unretained>"),
                                        argument.type_identity
                                    ));
                                }
                            }
                            if !matches!(argument.access,
                                    checked_trees::CheckedStructuralAccess::Owned
                                        | checked_trees::CheckedStructuralAccess::SharedBorrow
                                        | checked_trees::CheckedStructuralAccess::MutableBorrow
                                        | checked_trees::CheckedStructuralAccess::WriteOnlyBorrow)
                            {
                                return Err(format!(
                                    "call {handle:?} expression {expression:?} has unsupported structural access {:?}",
                                    argument.access
                                ));
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
                            retain_operand!(
                                root.type_reference,
                                format_args!("call {handle:?} expression {expression:?} completed producer")
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
                            )
                            .map_err(|reason| format!("call {handle:?} case argument: {reason}"))?;
                        }
                    }
                }
                ComputationKind::Select {
                    condition,
                    when_true,
                    when_false,
                    ..
                } => pending.extend([*condition, *when_true, *when_false]),
                ComputationKind::WithOperands {
                    operands, operand, ..
                } => {
                    pending.push(*operand);
                    for dependency in
                        plans.evaluation_operands.span(*operands).ok_or_else(|| {
                            format!(
                                "computation {handle:?} has no retained evaluation operand span"
                            )
                        })?
                    {
                        match dependency {
                            checked_trees::CheckedScalarEvaluationOperand::Scalar(value) => {
                                pending.push(*value)
                            }
                            checked_trees::CheckedScalarEvaluationOperand::Structural(value) => {
                                let root = structural_values.root_for_value(*value).ok_or_else(|| {
                                    format!("computation {handle:?} structural evaluation operand {value:?} has no retained root")
                                })?;
                                retain_operand!(
                                    root.type_reference,
                                    format_args!(
                                        "computation {handle:?} structural evaluation operand {value:?}"
                                    )
                                )?;
                            }
                        }
                    }
                }
                ComputationKind::Apply { operands, .. }
                | ComputationKind::StorageObservation { operands, .. } => {
                    pending.extend(plans.operands.span(*operands).ok_or_else(|| {
                        format!("computation {handle:?} has no retained operand span")
                    })?)
                }
            }
        }
        Ok(())
    })();
    retained
}

fn retain_declarations(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut Vec<TypePlan>,
    declarations: impl IntoIterator<Item = TypePlan>,
) -> Result<(), String> {
    for declaration in declarations {
        if let Some(existing) =
            checked_trees::checked_structural_type_by_identity(shapes, declaration.identity)
        {
            if existing != &declaration {
                return Err(format!(
                    "conflicting storage plans for semantic identity `{}` ({:?}): differing fields [{}]",
                    program
                        .type_identity_text(declaration.identity)
                        .as_deref()
                        .unwrap_or("<unretained>"),
                    declaration.identity,
                    differing_plan_fields(existing, &declaration)
                ));
            }
        } else {
            shapes.push(declaration);
        }
    }
    Ok(())
}

fn differing_plan_fields(existing: &TypePlan, incoming: &TypePlan) -> String {
    macro_rules! differences {
        ($($field:ident),+ $(,)?) => {
            [$(
                (existing.$field != incoming.$field).then_some(stringify!($field)),
            )+]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(", ")
        };
    }
    differences!(
        identity,
        native_identity,
        native_policy_identity,
        owns_linear_claim,
        copyable,
        no_code_affine_cleanup,
        descriptor_cleanup,
        shape,
        scalar_restriction,
    )
}

fn type_description(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> String {
    if !program
        .type_reference_table
        .contains_type_reference(reference)
    {
        return format!("invalid type reference {reference:?}");
    }
    let identity = validation::structural_type_identity(program, reference, &[], &[]);
    format!(
        "type `{}` (reference {reference:?}, identity {identity:?})",
        program
            .type_identity_text(identity)
            .as_deref()
            .unwrap_or("<unretained>")
    )
}
