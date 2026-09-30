//! Runtime elements of an assignment's destination path.
//!
//! A checked store path names each runtime element by the assignment's
//! `AssignmentIndex { depth }` coordinate. The emitter evaluates those
//! selectors in path order, before the stored value, through the same scalar
//! evaluator as the value; converts each to Terminal's `u64` coordinate; and,
//! once the value is complete, spells each element as
//! `RuntimeIndex { index, obligation }`. The store owns every such
//! obligation, and the verifier reconstructs it as `index < extent` for the
//! array that step selects from.
use super::{
    CheckedScalarExpressionRole, CheckedTrees, LoweringError, StructuralPathSegment,
    ValueDeclaration, ValueId, unsupported,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;

/// One resolved step of a destination path: a static Terminal segment, or a
/// runtime element whose selector the emitter evaluates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProjectionStep {
    Static(StructuralPathSegment),
    AssignmentIndex {
        depth: u32,
    },
    /// A runtime element whose selector is already the calling state's own
    /// operand — a dense scalar parameter position a borrow or an argument
    /// mirror minted, not an assignment selector. The scalar source frame
    /// reads it directly; no bound-expression row exists for it.
    RuntimeIndexParameter {
        position: u32,
    },
    /// A runtime element whose selector is a program-point read of one
    /// integer field below a structural entry parameter — `bag.at` in
    /// `cells[bag.at]`. The checked coordinate names the parameter's authored
    /// position and field path; the evaluator resolves that observation
    /// against the state's field bindings and emits the read itself.
    RuntimeIndexStructuralField(checked_trees::CheckedStructuralParameterField),
    /// A runtime element whose selector is an immutable scalar local of the
    /// calling state — `let at: u64 = ...` in `cells[at]` — at the dense
    /// namespace position the checker minted, the same coordinate
    /// `CheckedScalarExpression::Local` uses. The scalar source frame reads
    /// it directly.
    RuntimeIndexLocal {
        position: u32,
    },
}

/// The steps as a static Terminal path, for routes that evaluate no selector.
pub(crate) fn static_segments(
    steps: Vec<ProjectionStep>,
) -> Result<Vec<StructuralPathSegment>, LoweringError> {
    steps
        .into_iter()
        .map(|step| match step {
            ProjectionStep::Static(segment) => Ok(segment),
            ProjectionStep::AssignmentIndex { .. }
            | ProjectionStep::RuntimeIndexParameter { .. }
            | ProjectionStep::RuntimeIndexStructuralField(_)
            | ProjectionStep::RuntimeIndexLocal { .. } => {
                unsupported("this route evaluates no assignment runtime element")
            }
        })
        .collect()
}

/// Evaluate every runtime element's selector in path order -- the authored
/// left-to-right order of the target's selectors -- and convert each to the
/// `u64` coordinate.
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_selectors(
    steps: &[ProjectionStep],
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: u32,
    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
    source_value_count: usize,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
) -> Result<Vec<ValueId>, LoweringError> {
    let mut indexes = Vec::new();
    for step in steps {
        let index = match step {
            ProjectionStep::AssignmentIndex { depth } => {
                let role = CheckedScalarExpressionRole::AssignmentIndex { depth: *depth };
                let source = selector_source(checked, state, statement_index, role)?;
                evaluation.source_value(
                    checked,
                    machine,
                    state,
                    statement_index,
                    role,
                    &source,
                    source_value_count,
                    values,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                    calls,
                )?
            }
            ProjectionStep::RuntimeIndexParameter { position }
            | ProjectionStep::RuntimeIndexLocal { position } => {
                // A parameter or immutable-local selector is already a scalar
                // source value of the calling state: the dense source frame
                // holds it, in the same order the checker numbers
                // `RuntimeIndex` positions.
                let Some(index) = values.get(*position as usize) else {
                    return unsupported(
                        "store runtime index position exceeds the scalar source frame",
                    );
                };
                index.clone()
            }
            ProjectionStep::RuntimeIndexStructuralField(field) => {
                // A field selector is a program-point read of that parameter
                // member — `bag.at` — resolved against the state's field
                // bindings and emitted before the stored value, exactly as a
                // scalar observation on the same field lowers.
                let (source, path, leaf, scalar_type) =
                    crate::expression_preparation::bindings::structural_fields::resolve_typed(
                        &evaluation.structural_fields,
                        field.parameter_position,
                        &field.path,
                    )?;
                let index =
                    crate::emission::operation_emission::expressions::emit_direct_expression(
                        &crate::emission::operation_emission::expressions::LoweredDirectExpression::StructuralField {
                            source,
                            path,
                            field: leaf,
                            scalar_type,
                        },
                        values,
                        next_value,
                        operations,
                    );
                ValueDeclaration {
                    id: index,
                    scalar_type,
                    qualifications: Default::default(),
                }
            }
            _ => continue,
        };
        indexes.push(super::emit_u64_coordinate(
            index.id,
            index.scalar_type,
            next_value,
            &mut calls.next_obligation_identity,
            operations,
        )?);
    }
    Ok(indexes)
}

/// Join the evaluated selectors into the Terminal path. Called after the
/// stored value is complete, so each segment's obligation follows every
/// obligation its operands allocated.
pub(crate) fn complete_path(
    steps: Vec<ProjectionStep>,
    indexes: Vec<ValueId>,
    calls: &mut CallEmissionContext<'_>,
) -> Result<Vec<StructuralPathSegment>, LoweringError> {
    let mut indexes = indexes.into_iter();
    let path = steps
        .into_iter()
        .map(|step| match step {
            ProjectionStep::Static(segment) => Ok(segment),
            ProjectionStep::AssignmentIndex { .. }
            | ProjectionStep::RuntimeIndexParameter { .. }
            | ProjectionStep::RuntimeIndexStructuralField(_)
            | ProjectionStep::RuntimeIndexLocal { .. } => Ok(StructuralPathSegment::RuntimeIndex {
                index: indexes.next().ok_or(LoweringError::Unsupported(
                    "store path lost an evaluated runtime element",
                ))?,
                obligation: calls.allocate_requirement()?,
            }),
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    if indexes.next().is_some() {
        return unsupported("store path evaluated an extra runtime element");
    }
    Ok(path)
}

/// The retained source of one `AssignmentIndex` coordinate: its bound pure
/// expression, or the computation root the scalar evaluator completes. The
/// checked producer admitted the runtime element only when exactly one of
/// them names that selector.
fn selector_source(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement_index: u32,
    role: CheckedScalarExpressionRole,
) -> Result<checked_trees::CheckedCallScalarArgument, LoweringError> {
    if let Some((_, expression)) =
        checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(state, statement_index, role)
    {
        return Ok(checked_trees::CheckedCallScalarArgument::Pure(
            expression.clone(),
        ));
    }
    let root = checked
        .facts
        .values
        .scalar_computations
        .root_at(state, statement_index, role)
        .ok_or(LoweringError::Unsupported(
            "store runtime element lost its retained selector",
        ))?;
    Ok(checked_trees::CheckedCallScalarArgument::Computation(
        root.root,
    ))
}
