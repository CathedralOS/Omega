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
//!
//! Range checking already decided that bound for each authored `[]`
//! occurrence it proved in every context (`CheckFacts.proven_scalar_indexes`),
//! after the statement's operand effects. `AssignmentIndex { depth }` names
//! the target's occurrence at that outside-in depth
//! (`validation::assignment_target_indexes`), so `a[i][i]` keeps two
//! occurrences with their own collections and bounds. `complete_path`
//! publishes the obligation of a decided element as `SourceChecked` evidence
//! instead of repeating certificate search across the representation
//! boundary. Only the bound travels: destination custody, declared value
//! constraints and stored-value formation stay their own admissions.
//! Parameter, local, field and literal selectors are generated and have no
//! assignment occurrence, so their bounds stay searched.
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
    RuntimeIndexStorage {
        symbol: symbols::SymbolHandle,
    },
}

/// The step one checked runtime element lowers through. Assignment selectors
/// evaluate the retained bound expression or computation; parameter and
/// local positions read the dense scalar frame directly; a stored-field
/// selector emits the field read at the program point.
pub(crate) fn selector_step(runtime: &checked_trees::CheckedRuntimeIndex) -> ProjectionStep {
    match runtime {
        checked_trees::CheckedRuntimeIndex::StorageRead { symbol } => {
            ProjectionStep::RuntimeIndexStorage { symbol: *symbol }
        }
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
            | ProjectionStep::RuntimeIndexField(_)
            | ProjectionStep::RuntimeIndexStorage { .. } => {
                unsupported("this route evaluates no assignment runtime element")
            }
        })
        .collect()
}

/// One runtime element's `u64` coordinate, and whether range checking proved
/// its authored selector inside the array that element selects from.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EvaluatedSelector {
    pub(crate) index: ValueId,
    decided: bool,
}

/// Evaluate every runtime element's selector in path order -- the authored
/// left-to-right order of the target's selectors -- and convert each to the
/// `u64` coordinate.
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_selectors(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
) -> Result<Vec<EvaluatedSelector>, LoweringError> {
    let mut indexes = Vec::new();
    for step in steps {
        let mut decided = false;
        let (index, scalar_type) = match step {
            ProjectionStep::AssignmentIndex { depth } => {
                let role = CheckedScalarExpressionRole::AssignmentIndex { depth: *depth };
                let source = selector_source(checked, state, statement_index, role)?;
                decided = checked
                    .facts
                    .assignment_index_is_proven(state, statement_index, *depth);

                let index = evaluation.source_value(
                    identities,
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
            ProjectionStep::RuntimeIndexStorage { symbol } => {
                let observation = evaluation
                    .contextual_bindings()
                    .runtime_index_storage(*symbol)?;
                let scalar_type = observation.scalar_type();
                let index =
                    super::emit_direct_expression(&observation, values, next_value, operations);
                (index, scalar_type)
            }
            ProjectionStep::RuntimeIndexField(field) => {
                let observation =
                    crate::expression_preparation::bindings::structural_fields::resolve_typed(
                        identities,
                        &evaluation.structural_fields,
                        field.parameter_position,
                        &field.path,
                    )?;
                let scalar_type = observation.scalar_type();
                if !matches!(scalar_type, ScalarType::Integer(integer) if !integer.is_address()) {
                    return unsupported(
                        "store runtime index field selector is not an integer read",
                    );
                }
                let index =
                    super::emit_direct_expression(&observation, values, next_value, operations);
                (index, scalar_type)
            }
            _ => continue,
        };
        indexes.push(EvaluatedSelector {
            index: super::emit_u64_coordinate(
                index,
                scalar_type,
                decided,
                next_value,
                &mut calls.next_obligation_identity,
                operations,
            )?,
            decided,
        });
    }
    Ok(indexes)
}

/// Join the evaluated selectors into the Terminal path. Called after the
/// stored value is complete, so each segment's obligation follows every
/// obligation its operands allocated. A decided selector's obligation travels
/// as `SourceChecked` evidence.
pub(crate) fn complete_path(
    steps: Vec<ProjectionStep>,
    indexes: Vec<EvaluatedSelector>,
    calls: &mut CallEmissionContext<'_>,
    operations: &mut OperationBuffer,
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
            | ProjectionStep::RuntimeIndexField(_)
            | ProjectionStep::RuntimeIndexStorage { .. } => {
                let selector = indexes.next().ok_or(LoweringError::Unsupported(
                    "store path lost an evaluated runtime element",
                ))?;
                let obligation = calls.allocate_requirement()?;
                if selector.decided {
                    operations.source_checked_obligations.push(obligation);
                }
                Ok(StructuralPathSegment::RuntimeIndex {
                    index: selector.index,
                    obligation,
                })
            }
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    if indexes.next().is_some() {
        return unsupported("store path evaluated an extra runtime element");
    }
    Ok(path)
}

/// Observe each selected view's own live extent before accessing its elements.
/// Completed paths retain the already captured selectors and their bounds;
/// observing prefixes outside-in preserves that evidence for nested views.
/// A containing record's identity cannot donate a sibling descriptor's length.
/// Fixed-array selections need no observation because their extent is declared.
pub(crate) fn observe_element_extent(
    destination: semantic_vocabulary::PlaceId,
    structural_type: semantic_vocabulary::StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    for (position, segment) in path.iter().enumerate() {
        if !matches!(segment, StructuralPathSegment::RuntimeIndex { .. }) {
            continue;
        }
        let prefix = &path[..position];
        let owner = terminal_semantics::runtime_structural_path_tip(
            declarations.iter(),
            structural_type,
            prefix,
        )
        .ok_or(LoweringError::Unsupported(
            "runtime element extent has an unresolved structural prefix",
        ))?;
        let source = terminal_psi::StructuralArgument {
            place: destination,
            path: prefix.to_vec(),
            access: terminal_psi::StructuralAccess::SharedBorrow,
        };
        let kind = match owner.shape {
            terminal_psi::StructuralTypeShape::ElementView { .. } => {
                terminal_psi::OperationKind::ElementViewLength { source }
            }
            terminal_psi::StructuralTypeShape::ByteSequence(_) => {
                terminal_psi::OperationKind::ByteSequenceLength { source }
            }
            _ => continue,
        };
        let length = crate::terminal_identities::value_id(
            crate::terminal_identities::allocate_dense(next_value)?,
        );
        let observation = operations.allocate();
        operations.push(terminal_psi::Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: observation,
            result: crate::emission::operation_emission::buffer::scalar_result_declaration(
                length,
                crate::emission::scalar_types::terminal_scalar_type(
                    checked_trees::types::PrimitiveType::U64,
                )?,
            ),
            kind,
        });
    }
    Ok(())
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
