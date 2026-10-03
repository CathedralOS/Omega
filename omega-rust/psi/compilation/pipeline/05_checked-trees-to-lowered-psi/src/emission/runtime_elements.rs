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
    CheckedScalarExpressionRole, CheckedTrees, LoweringError, ScalarType, StructuralPathSegment,
    ValueDeclaration, ValueId, unsupported,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;

/// One resolved step of a destination path: a static Terminal segment, or a
/// runtime element whose selector the emitter evaluates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProjectionStep {
    Static(StructuralPathSegment),
    /// A literal still needs a runtime bound when selecting a dynamic view.
    RuntimeIndexLiteral {
        index: u64,
    },
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
    /// A runtime element whose selector is an immutable integer local of the
    /// calling state — the dense position after its scalar parameters, the
    /// coordinate `CheckedScalarExpression::Local` uses. The scalar source
    /// frame reads it directly, like a parameter position.
    RuntimeIndexLocal {
        position: u32,
    },
    /// A runtime element whose selector is a stored-field snapshot — one
    /// integer scalar read below a structural entry parameter of the calling
    /// state. The emitter mints the field read at the program point and
    /// selects with its value.
    RuntimeIndexField(checked_trees::CheckedStructuralParameterField),
}

/// The step one checked runtime element lowers through. Assignment selectors
/// evaluate the retained bound expression or computation; parameter and
/// local positions read the dense scalar frame directly; a stored-field
/// selector emits the field read at the program point.
pub(crate) fn selector_step(runtime: &checked_trees::CheckedRuntimeIndex) -> ProjectionStep {
    match runtime {
        checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth } => {
            ProjectionStep::AssignmentIndex { depth: *depth }
        }
        checked_trees::CheckedRuntimeIndex::Parameter { position } => {
            ProjectionStep::RuntimeIndexParameter {
                position: *position,
            }
        }
        checked_trees::CheckedRuntimeIndex::Local { position } => {
            ProjectionStep::RuntimeIndexLocal {
                position: *position,
            }
        }
        checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
            ProjectionStep::RuntimeIndexField(field.clone())
        }
    }
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
            | ProjectionStep::RuntimeIndexLiteral { .. }
            | ProjectionStep::RuntimeIndexParameter { .. }
            | ProjectionStep::RuntimeIndexLocal { .. }
            | ProjectionStep::RuntimeIndexField(_) => {
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
        let (index, scalar_type) = match step {
            ProjectionStep::AssignmentIndex { depth } => {
                let role = CheckedScalarExpressionRole::AssignmentIndex { depth: *depth };
                let source = selector_source(checked, state, statement_index, role)?;
                let index = evaluation.source_value(
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
                )?;
                (index.id, index.scalar_type)
            }
            ProjectionStep::RuntimeIndexParameter { position }
            | ProjectionStep::RuntimeIndexLocal { position } => {
                // A parameter or immutable-local selector is already a scalar
                // source value of the calling state: the dense source prefix
                // holds it, in the same order the checker numbers
                // `RuntimeIndex` positions.
                let Some(index) = values.get(*position as usize) else {
                    return unsupported(
                        "store runtime index position exceeds the scalar source frame",
                    );
                };
                (index.id, index.scalar_type)
            }
            ProjectionStep::RuntimeIndexLiteral { index } => {
                let scalar_type = crate::emission::scalar_types::terminal_scalar_type(
                    checked_trees::types::PrimitiveType::U64,
                )?;
                let value = super::emit_direct_expression(
                    &LoweredDirectExpression::IntegerLiteral {
                        value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(*index)),
                        scalar_type,
                    },
                    values,
                    next_value,
                    operations,
                );
                (value, scalar_type)
            }
            ProjectionStep::RuntimeIndexField(field) => {
                let (source, path, field, scalar_type) =
                    crate::expression_preparation::bindings::structural_fields::resolve_typed(
                        &evaluation.structural_fields,
                        field.parameter_position,
                        &field.path,
                    )?;
                if !matches!(scalar_type, ScalarType::Integer(integer) if !integer.is_address()) {
                    return unsupported(
                        "store runtime index field selector is not an integer read",
                    );
                }
                let index = super::emit_direct_expression(
                    &LoweredDirectExpression::StructuralField {
                        source: source.into(),
                        path,
                        field,
                        scalar_type,
                    },
                    values,
                    next_value,
                    operations,
                );
                (index, scalar_type)
            }
            _ => continue,
        };
        indexes.push(super::emit_u64_coordinate(
            index,
            scalar_type,
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
            | ProjectionStep::RuntimeIndexLiteral { .. }
            | ProjectionStep::RuntimeIndexParameter { .. }
            | ProjectionStep::RuntimeIndexLocal { .. }
            | ProjectionStep::RuntimeIndexField(_) => Ok(StructuralPathSegment::RuntimeIndex {
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
