//! Checked whole-primitive stores through exact exclusive reference parameters.
use super::{
    CheckFacts, CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedStructuralAccess, CheckedStructuralScalarParameterPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypeShape, DataMember, ExpressionNode,
    Multiplicity, PrimitiveType, StatementNode, SymbolHandle, TypeConstraintNode,
    TypeReferenceNode, TypedTrees,
};
use crate::execution::terminal_unit::types::ShapeCollector;

pub(super) fn build_write_only_primitive_store(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    statements: &[StatementNode],
    scalar_result_local: Option<&CheckedUnitScalarResultBindingPlan>,
    selected_scalar_result_local: Option<&CheckedUnitScalarResultBindingPlan>,
) -> Option<CheckedUnitEffectOperationPlan> {
    let result_local = scalar_result_local.or(selected_scalar_result_local);
    let (statement_index, assignment) = match (result_local, statements) {
        (None, [StatementNode::Assignment(assignment)]) => (0, assignment),
        (
            Some(result),
            [
                StatementNode::LocalData(_),
                StatementNode::Assignment(assignment),
            ],
        ) if result.statement_index == 0 && result.binding_ordinal == 0 => (1, assignment),
        _ => return None,
    };
    let [destination] = structural_parameters else {
        return None;
    };
    if destination.is_self
        || destination.position != 0
        || destination.multiplicity != Multiplicity::Unrestricted
        || !matches!(
            destination.access,
            CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::WriteOnlyBorrow
        )
        || !destination.qualifications.is_empty()
    {
        return None;
    }
    let CheckedUnitStructuralTypeShape::PrimitiveScalar(destination_type) = shapes
        .types
        .get(&destination.type_identity)
        .map(|declaration| &declaration.shape)?
    else {
        return None;
    };
    let parameter = program
        .state_parameters(state)
        .get(usize::try_from(destination.position).ok()?)?;
    if crate::execution::terminal_unit::types::abi_parameter_count(program.state_parameters(state))
        != structural_parameters.len() + scalar_parameters.len()
    {
        return None;
    }
    if parameter.is_self || parameter.is_const || !parameter.is_mutable {
        return None;
    }
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return None;
    };
    let expected_access = match access {
        language_semantics::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
        language_semantics::ReferenceAccess::WriteOnly => CheckedStructuralAccess::WriteOnlyBorrow,
        language_semantics::ReferenceAccess::Shared => return None,
    };
    if destination.access != expected_access
        || program.primitive_type_reference(*referee) != Some(*destination_type)
    {
        return None;
    }
    let target = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        usize::try_from(statement_index).ok()?,
        assignment.target,
    )?;
    if target.root != facts::PlaceRoot::Symbol(parameter.symbol) || !target.segments.is_empty() {
        return None;
    }
    let frame = &facts
        .mutation
        .for_machine(machine.symbol)?
        .state_write_frames
        .iter()
        .find(|frame| frame.state == state.symbol)?
        .frame;
    let expected_frame_path = format!("$P{}", destination.position);
    let exact_frame =
        matches!(frame.complete_paths(), Some([path]) if path == &expected_frame_path);
    // Before provider selection, a boundary-operator initializer makes the
    // source write frame opaque. The selected scalar lane replaces that one
    // initializer with a checked-body call whose complete scalar-only shape is
    // replayed below; the exact two-statement body leaves the destination
    // assignment as its only caller-visible write.
    let unresolved_selected_frame = selected_scalar_result_local.is_some()
        && frame.completeness() == facts::WriteFrameCompleteness::Opaque;
    if !exact_frame && !unresolved_selected_frame {
        return None;
    }
    let value = facts.values.scalar_expressions.expression_at(
        state.symbol,
        statement_index,
        CheckedScalarExpressionRole::AssignmentValue,
    )?;
    let direct_literal = matches!(value, CheckedScalarExpression::IntegerLiteral { .. })
        || matches!(value, CheckedScalarExpression::IeeeFloatLiteral { .. })
        || matches!(
            value,
            CheckedScalarExpression::Boolean(expression)
                if matches!(
                    expression.as_ref(),
                    checked_trees::CheckedBooleanExpression::Constant(_)
                )
        );
    let direct_parameter = match value {
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => Some((*position, *primitive_type)),
        CheckedScalarExpression::Boolean(expression) => match expression.as_ref() {
            checked_trees::CheckedBooleanExpression::Parameter { position } => {
                Some((*position, PrimitiveType::Bool))
            }
            _ => None,
        },
        _ => None,
    };
    let direct_parameter_is_exact = direct_parameter.is_some_and(|(position, primitive_type)| {
        scalar_parameters
            .get(position)
            .is_some_and(|parameter| parameter.primitive_type == primitive_type)
            && *destination_type == primitive_type
    });
    // The admitted result is binding zero, after the dense scalar inputs;
    // its source binding ordinal is not its scalar-expression position.
    let direct_result_is_exact = matches!(
        (result_local, value),
        (
            Some(result),
            CheckedScalarExpression::Local {
                position,
                primitive_type,
            },
        ) if *position == scalar_parameters.len()
            && *primitive_type == result.primitive_type
            && *destination_type == result.primitive_type
            && matches!(
                result.primitive_type,
                PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
            )
    );
    if !(direct_literal || direct_parameter_is_exact || direct_result_is_exact)
        || crate::values::scalar_expression_type(value) != Some(*destination_type)
    {
        return None;
    }
    Some(CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
        statement_index,
        path: Vec::new(),
        destination: checked_trees::CheckedPrimitiveStoreDestination::Parameter {
            parameter_index: 0,
        },
        value: checked_trees::CheckedCallScalarArgument::Pure(value.clone()),
    })
}

/// Find only initialized primitive storage established before this occurrence.
pub(super) fn primitive_local_before<'program>(
    program: &'program TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    symbol: SymbolHandle,
) -> Option<&'program typed_trees::statement::TableLocalData> {
    let mut locals = program
        .statement_table
        .statements(state.statement_nodes)
        .get(..statement_index)?
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == symbol => Some(local),
            _ => None,
        });
    let local = locals.next()?;
    if locals.next().is_some()
        || !local.is_mutable
        || !program
            .expression_table
            .expression_is_valid(local.initial_value)
        || !matches!(
            program
                .type_reference_table
                .type_reference(local.type_reference),
            TypeReferenceNode::Named { .. }
        )
        || program
            .primitive_type_reference(local.type_reference)
            .is_none()
    {
        return None;
    }
    Some(local)
}

/// The shared statement roster validates the complete state write frame.
pub(super) fn build_primitive_store_at(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
) -> Option<CheckedUnitEffectOperationPlan> {
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        usize::try_from(statement_index).ok()?,
        assignment.target,
    )?;
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    // An erased borrow carrier names its captured referent's storage:
    // substitute the checked alias root and prepend its captured projection.
    // The scalar binding's destination keeps the authored root spelling.
    let authored_symbol = symbol;
    let (symbol, segments) = match super::receiver_aliases::aliases(program, facts, machine, state)
        .unwrap_or_default()
        .iter()
        .find(|alias| alias.owner == symbol)
    {
        Some(alias) => {
            let mut segments = alias.segments.clone();
            segments.extend_from_slice(&place.segments);
            (alias.root, segments)
        }
        None => (symbol, place.segments.clone()),
    };
    // A trailing runtime index is an operand, not a path segment: it carries
    // its retained bounds proof into a distinct checked operation. Literal
    // tails keep the existing all-static projection.
    let (path, index) = if matches!(segments.last(), Some(facts::PlaceSegment::Index { .. })) {
        let (path, index) = primitive_indexed_tail(
            program,
            facts,
            machine,
            state,
            statement_index,
            assignment,
            &segments,
        )?;
        (path, Some(index))
    } else {
        (
            primitive_projection(program, machine, state, assignment.target, &segments)?,
            None,
        )
    };
    let whole = path.is_empty() && index.is_none();
    let (destination, primitive_type) = if let Some(local) = primitive_local_before(
        program,
        state,
        usize::try_from(statement_index).ok()?,
        symbol,
    ) {
        if !whole {
            return None;
        }
        (
            checked_trees::CheckedPrimitiveStoreDestination::Local { symbol },
            program.primitive_type_reference(local.type_reference)?,
        )
    } else {
        let (parameter_index, destination) =
            structural_parameters
                .iter()
                .enumerate()
                .find(|(_, destination)| {
                    program
                        .state_parameters(state)
                        .get(destination.position as usize)
                        .is_some_and(|parameter| parameter.symbol == symbol)
                })?;
        if (whole
            && (destination.is_self || destination.multiplicity != Multiplicity::Unrestricted))
            || destination.multiplicity == Multiplicity::Linear
            || !destination.qualifications.is_empty()
            || !matches!(
                destination.access,
                CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::WriteOnlyBorrow
            )
        {
            return None;
        }
        let parameter = program
            .state_parameters(state)
            .get(destination.position as usize)?;
        if parameter.is_self != destination.is_self || parameter.is_const || !parameter.is_mutable {
            return None;
        }
        let TypeReferenceNode::Reference {
            access, referee, ..
        } = program
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            return None;
        };
        let expected_access = match access {
            language_semantics::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
            language_semantics::ReferenceAccess::WriteOnly => {
                CheckedStructuralAccess::WriteOnlyBorrow
            }
            language_semantics::ReferenceAccess::Shared => return None,
        };
        if destination.access != expected_access
            || (whole
                && !matches!(
                    program.type_reference_table.type_reference(*referee),
                    TypeReferenceNode::Named { .. }
                ))
        {
            return None;
        }
        (
            checked_trees::CheckedPrimitiveStoreDestination::Parameter {
                parameter_index: u32::try_from(parameter_index).ok()?,
            },
            program.primitive_type_reference(if whole {
                *referee
            } else {
                validation::declared_place_type_raw(
                    program,
                    machine,
                    Some(state),
                    assignment.target,
                )?
            })?,
        )
    };
    // Computed RHS values share the ordinary scalar evaluator. Keep the
    // authored assignment root until emission completes it, then replace storage.
    let computations = &facts.values.scalar_computations;
    let mut roots = computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .filter(|root| {
            root.state == state.symbol
                && root.statement_ordinal == statement_index
                && root.role == CheckedScalarExpressionRole::AssignmentValue
        });
    if let Some(root) = roots.next() {
        if roots.next().is_some()
            || root.machine != machine.symbol
            || !computations.nodes.is_valid(root.root)
            || computations.nodes.get(root.root).authored_root != assignment.value
            || computations.nodes.get(root.root).primitive_type != primitive_type
            || facts
                .values
                .scalar_expressions
                .expression_at(
                    state.symbol,
                    statement_index,
                    CheckedScalarExpressionRole::AssignmentValue,
                )
                .is_some()
        {
            return None;
        }
        return Some(primitive_store_plan(
            statement_index,
            destination,
            path,
            index,
            checked_trees::CheckedCallScalarArgument::Computation(root.root),
        ));
    }
    let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
        state.symbol,
        statement_index,
        CheckedScalarExpressionRole::AssignmentValue,
    )?;
    if binding.expression != assignment.value
        || binding.destination
            != if whole {
                authored_symbol
            } else {
                SymbolHandle::invalid()
            }
        || crate::values::scalar_expression_type(value) != Some(primitive_type)
        || !scalar_custody_is_exact(program, facts, state, binding, value, primitive_type)
    {
        return None;
    }
    Some(primitive_store_plan(
        statement_index,
        destination,
        path,
        index,
        checked_trees::CheckedCallScalarArgument::Pure(value.clone()),
    ))
}

/// The one checked operation for a primitive store: a retained runtime index
/// selects the indexed variant, and a static or empty path keeps the literal
/// operation.
fn primitive_store_plan(
    statement_index: u32,
    destination: checked_trees::CheckedPrimitiveStoreDestination,
    path: Vec<CheckedUnitStructuralPathSegment>,
    index: Option<CheckedScalarExpression>,
    value: checked_trees::CheckedCallScalarArgument,
) -> CheckedUnitEffectOperationPlan {
    match index {
        Some(index) => CheckedUnitEffectOperationPlan::WriteOnlyIndexedPrimitiveStore {
            statement_index,
            destination,
            path,
            index,
            value,
        },
        None => CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index,
            destination,
            path,
            value,
        },
    }
}

/// Whole primitive storage and indexed primitive leaves use one operation.
/// Field-only destinations retain their existing bounded-field write owner;
/// an indexed leaf carries its real path, never an invented terminal field.
/// Every segment here is literal; a trailing runtime index takes the
/// `primitive_indexed_tail` route instead.
fn primitive_projection(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    segments: &[facts::PlaceSegment],
) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
    if segments.is_empty() {
        return Some(Vec::new());
    }
    if !matches!(
        segments.last(),
        Some(facts::PlaceSegment::FixedIndex { .. })
    ) || !validation::place_has_builtin_coordinates(program, machine, Some(state), expression)
    {
        return None;
    }
    static_index_chain(program, machine, state, expression)?;
    primitive_leaf(program, machine, state, expression)?;
    checked_unit_path(program, segments)
}

/// A trailing runtime index stays a scalar operand rather than a path
/// segment. The retained `AssignmentIndex` binding must name this statement's
/// authored selector, and the selector's closed integer entry range must
/// discharge `0 <= index < extent` exactly. A literal index keeps the static
/// route; a computed, mutable-carrier, or unproven selector keeps the
/// rejection.
fn primitive_indexed_tail(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    segments: &[facts::PlaceSegment],
) -> Option<(
    Vec<CheckedUnitStructuralPathSegment>,
    CheckedScalarExpression,
)> {
    let facts::PlaceSegment::Index {
        expression: index_expression,
    } = segments.last()?
    else {
        return None;
    };
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(assignment.target)
    else {
        return None;
    };
    if *index_expression != indexed.index
        || !matches!(
            program.expression_table.expression(indexed.index),
            ExpressionNode::Name(_)
        )
        || !validation::place_has_builtin_coordinates(
            program,
            machine,
            Some(state),
            assignment.target,
        )
    {
        return None;
    }
    // The collection chain keeps the same static-geometry contract as the
    // literal route: every earlier selector is a proven literal or field.
    static_index_chain(program, machine, state, indexed.collection)?;
    let collection =
        validation::declared_place_type_raw(program, machine, Some(state), indexed.collection)?;
    let collection = validation::unwrapped_type_reference(program, collection)?;
    let TypeReferenceNode::FixedArray {
        length: typed_trees::types::FixedArrayLength::Literal(extent),
        ..
    } = program.type_reference_table.type_reference(collection)
    else {
        return None;
    };
    primitive_leaf(program, machine, state, assignment.target)?;
    let index = proven_runtime_index(
        program,
        facts,
        machine,
        state,
        statement_index,
        indexed.index,
        *extent,
    )?;
    let path = checked_unit_path(program, &segments[..segments.len() - 1])?;
    Some((path, index))
}

/// The checked proof a runtime array index stays in bounds: the retained
/// `AssignmentIndex` expression must be this authored selector, resolved to
/// one immutable scalar entry parameter whose closed integer entry range
/// proves `0 <= index < extent`. Entry ranges live on the machine's entry
/// state, so the proof applies only when the store runs there; every other
/// shape keeps the rejection.
fn proven_runtime_index(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    authored_index: typed_trees::expression::ExpressionHandle,
    extent: usize,
) -> Option<CheckedScalarExpression> {
    if program.machine_states(machine).first()?.symbol != state.symbol {
        return None;
    }
    let (binding, index) = facts.values.scalar_expressions.bound_expression_at(
        state.symbol,
        statement_index,
        CheckedScalarExpressionRole::AssignmentIndex,
    )?;
    if binding.expression != authored_index {
        return None;
    }
    let &CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    } = index
    else {
        return None;
    };
    // `CheckedScalarExpression::Parameter` positions skip erased primitive
    // formals while the retained range roster counts every primitive
    // parameter; translate through the authored parameter itself so both
    // namespaces name one declared carrier.
    let parameters = program.state_parameters(state);
    let authored = parameters
        .iter()
        .filter(|parameter| crate::values::occupies_scalar_position(program, parameter))
        .nth(position)?;
    if program.primitive_type_reference(authored.type_reference) != Some(primitive_type) {
        return None;
    }
    let authored_position = parameters
        .iter()
        .position(|parameter| parameter.symbol == authored.symbol)?;
    let roster_position = parameters[..authored_position]
        .iter()
        .filter(|parameter| {
            program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        })
        .count();
    let contract = &facts
        .contract_plans
        .for_machine(machine.symbol)?
        .closed_scalar_values;
    let requirement = contract.integer_entry_ranges()?.iter().find(|requirement| {
        requirement.position == roster_position && requirement.primitive_type == primitive_type
    });
    let maximum = match requirement {
        Some(requirement) => {
            // A negative minimum fails `to_u64`, and the normalized inclusive
            // maximum must fit strictly below the declared extent.
            requirement.minimum.value_bignum()?.to_u64()?;
            requirement.maximum.value_bignum()?.to_u64()?
        }
        // The declared range suffix rode the retained roster; its contract
        // replacement is the authored `requires` clause, which the closed
        // scalar plan carries as predicate conjuncts on the same entry
        // parameter. An `index <= K` conjunct is the same caller-discharged
        // entry obligation, so its literal endpoint bounds the selector
        // exactly like the roster row did.
        None => requires_entry_bound(contract, position, primitive_type)?,
    };
    (maximum < u64::try_from(extent).ok()?).then(|| index.clone())
}

/// The contract-fact replacement for a declared range suffix: authored
/// `requires` conjuncts on this entry scalar parameter fold into the closed
/// inclusive interval the retained range roster would carry. The clause was
/// already lowered into the closed scalar namespace and is caller-discharged
/// at every call site, so its literal endpoints prove the entry bound with
/// the same strength as a declared range. An unsigned carrier supplies its
/// own `0 <=` half; a signed carrier still owes an explicit `>=` conjunct.
/// Conjuncts this lane cannot read stay outside the interval rather than
/// declining it; a missing upper half or an empty interval declines.
fn requires_entry_bound(
    contract: &checked_trees::ClosedScalarValueContractPlan,
    position: usize,
    primitive_type: PrimitiveType,
) -> Option<u64> {
    let mut minimum: Option<i128> = (!primitive_type.is_signed_integer()).then_some(0);
    let mut maximum = None;
    for clause in contract.authored_requires() {
        let Some(checked_trees::ClosedScalarContractValue::Predicate(predicate)) = clause else {
            continue;
        };
        fold_requires_bound_conjunct(
            predicate,
            position,
            primitive_type,
            &mut minimum,
            &mut maximum,
        );
    }
    let (minimum, maximum) = minimum.zip(maximum)?;
    if 0 <= minimum && minimum <= maximum {
        u64::try_from(maximum).ok()
    } else {
        None
    }
}

/// Meet one `requires` conjunct's literal bound on the selector into the
/// running interval. Canonical lowering leaves only `Equal`, `LessThan` and
/// `LessOrEqual` integer comparisons: `p <= k` on the left is the upper half
/// and `k <= p` the lower. Conjuncts over other parameters, composed terms,
/// or non-literal endpoints are proof facts this admission does not read,
/// not a reason to decline.
fn fold_requires_bound_conjunct(
    predicate: &CheckedBooleanExpression,
    position: usize,
    primitive_type: PrimitiveType,
    minimum: &mut Option<i128>,
    maximum: &mut Option<i128>,
) {
    match predicate {
        CheckedBooleanExpression::And { left, right } => {
            fold_requires_bound_conjunct(left, position, primitive_type, minimum, maximum);
            fold_requires_bound_conjunct(right, position, primitive_type, minimum, maximum);
        }
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            let (endpoint, subject_is_left) = if conjunct_subject(left, position, primitive_type) {
                (conjunct_literal(right), true)
            } else if conjunct_subject(right, position, primitive_type) {
                (conjunct_literal(left), false)
            } else {
                return;
            };
            let Some(endpoint) = endpoint else {
                return;
            };
            let (lower, upper) = match (kind, subject_is_left) {
                (checked_trees::CheckedIntegerComparisonKind::Equal, _) => {
                    (Some(endpoint), Some(endpoint))
                }
                (checked_trees::CheckedIntegerComparisonKind::LessOrEqual, true) => {
                    (None, Some(endpoint))
                }
                (checked_trees::CheckedIntegerComparisonKind::LessThan, true) => {
                    (None, endpoint.checked_sub(1))
                }
                (checked_trees::CheckedIntegerComparisonKind::LessOrEqual, false) => {
                    (Some(endpoint), None)
                }
                (checked_trees::CheckedIntegerComparisonKind::LessThan, false) => {
                    (endpoint.checked_add(1), None)
                }
            };
            if let Some(lower) = lower {
                *minimum = Some(minimum.map_or(lower, |bound| bound.max(lower)));
            }
            if let Some(upper) = upper {
                *maximum = Some(maximum.map_or(upper, |bound| bound.min(upper)));
            }
        }
        _ => {}
    }
}

/// The conjunct subject is exactly this entry scalar parameter, in the same
/// dense scalar-parameter namespace the retained `AssignmentIndex` binding
/// and the closed contract predicates share.
fn conjunct_subject(
    expression: &CheckedScalarExpression,
    position: usize,
    primitive_type: PrimitiveType,
) -> bool {
    matches!(
        expression,
        CheckedScalarExpression::Parameter {
            position: subject,
            primitive_type: carrier,
        } if *subject == position && *carrier == primitive_type
    )
}

/// A conjunct endpoint lands as a literal only when it carries an exact
/// integer value; anything wider than both machine carriers stays unread.
fn conjunct_literal(expression: &CheckedScalarExpression) -> Option<i128> {
    let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
        return None;
    };
    let value = literal.value_bignum()?;
    value
        .to_i64()
        .map(i128::from)
        .or_else(|| value.to_u64().map(i128::from))
}

/// Every selector in this expression chain is a proven literal index or a
/// member; a runtime index never walks this loop.
fn static_index_chain(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<()> {
    let mut cursor = expression;
    loop {
        match program.expression_table.expression(cursor) {
            ExpressionNode::Indexed(indexed) => {
                let ExpressionNode::Integer(index) =
                    program.expression_table.expression(indexed.index)
                else {
                    return None;
                };
                let index = index.value_bignum()?.to_u64()?;
                let collection = validation::declared_place_type_raw(
                    program,
                    machine,
                    Some(state),
                    indexed.collection,
                )?;
                let collection = validation::unwrapped_type_reference(program, collection)?;
                let TypeReferenceNode::FixedArray {
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                    ..
                } = program.type_reference_table.type_reference(collection)
                else {
                    return None;
                };
                if usize::try_from(index).ok()? >= *length {
                    return None;
                }
                cursor = indexed.collection;
            }
            ExpressionNode::Member(member) => cursor = member.receiver,
            ExpressionNode::Name(_) => break,
            _ => return None,
        }
    }
    Some(())
}

/// The assignment leaf's declared type must be a named primitive, after the
/// arithmetic-policy shell peel.
fn primitive_leaf(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<PrimitiveType> {
    let mut leaf = validation::declared_place_type_raw(program, machine, Some(state), expression)?;
    // An arithmetic-policy shell (`i32 in Wrapping`) qualifies the element's
    // operations, not its storage identity, so it peels here like the primitive
    // leaf of the record-field store route. Range, named, and domain
    // constraints carry their own write obligations and keep their own owners.
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(leaf)
    {
        if !program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .all(|constraint| matches!(constraint, TypeConstraintNode::ArithmeticDomain(_)))
        {
            break;
        }
        leaf = *base_type;
    }
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(leaf)
    else {
        return None;
    };
    let atom = program.symbols.builtin_type_atom(*symbol)?;
    if name.as_str() != atom.symbol_name() {
        return None;
    }
    program.primitive_type_reference(leaf)
}

/// Literal path segments keep their exact source identity; fields reject an
/// erased or domain-constrained declaration.
fn checked_unit_path(
    program: &TypedTrees,
    segments: &[facts::PlaceSegment],
) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
    segments
        .iter()
        .map(|segment| match segment {
            facts::PlaceSegment::FixedIndex { index } => Some(
                CheckedUnitStructuralPathSegment::FixedIndex(u64::try_from(*index).ok()?),
            ),
            facts::PlaceSegment::Field { symbol } => {
                let mut fields = program
                    .data_definitions()
                    .iter()
                    .flat_map(|owner| program.data_members(owner))
                    .filter_map(|member| match member {
                        DataMember::Field(field) if field.symbol == *symbol => Some(field),
                        _ => None,
                    });
                let field = fields.next()?;
                if fields.next().is_some()
                    || field.relevance.is_erased()
                    || !crate::facts::field_domain::domain_constraint_symbols(
                        program,
                        field.type_reference,
                    )
                    .is_empty()
                {
                    return None;
                }
                Some(CheckedUnitStructuralPathSegment::Field(
                    field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned()),
                ))
            }
            _ => None,
        })
        .collect()
}

pub(super) fn scalar_custody_is_exact(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    binding: &checked_trees::CheckedScalarExpressionBindings,
    value: &CheckedScalarExpression,
    primitive_type: PrimitiveType,
) -> bool {
    let Ok(statement_index) = usize::try_from(binding.statement_ordinal) else {
        return false;
    };
    let Some(prefix) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(..statement_index)
    else {
        return false;
    };
    let symbols = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        })
        .map(|parameter| parameter.symbol)
        .chain(prefix.iter().filter_map(|statement| {
            match statement {
                StatementNode::LocalData(local)
                    if !local.is_mutable
                        && program
                            .primitive_type_reference(local.type_reference)
                            .is_some() =>
                {
                    Some(local.symbol)
                }
                _ => None,
            }
        }));
    if !symbols.eq(facts
        .values
        .scalar_expressions
        .binding_symbols
        .span_or_empty(binding.symbols)
        .iter()
        .copied())
    {
        return false;
    }
    crate::values::lower_unit_scalar_argument(
        program,
        &facts.operators,
        state,
        statement_index,
        binding.expression,
        primitive_type,
    )
    .as_ref()
        == Some(value)
}
