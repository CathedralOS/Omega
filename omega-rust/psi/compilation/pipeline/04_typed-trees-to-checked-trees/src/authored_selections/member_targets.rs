//! Contextual domain, value and statement member targets.

/// Membership sets over expression handles; never iterated for output order.
type HashSet<T> = std::collections::HashSet<T, symbols::BuildSymbolHasher>;

use crate::authored_selections::CheckedResolutionTarget;
use crate::authored_selections::call_targets::declaration_target;
use crate::authored_selections::contexts;
use crate::authored_selections::operator_targets::{
    authored_operand_descriptor, authored_operand_type, type_reference_for_symbol,
};
use crate::semantic::calls::MeasureReceiver;
use checked_trees::CheckFacts;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;

pub(crate) fn checked_member_target(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
    member: &typed_trees::expression::TableMemberExpression,
    owner_index: Option<&contexts::OwnerEnvironmentIndex>,
) -> Option<CheckedResolutionTarget> {
    declaration_target(crate::flow::effective_member_symbol(
        program,
        member.receiver,
        member,
    ))
    .or_else(|| {
        contexts::checked_member_target_from_exact_owner(
            program,
            facts,
            expression,
            member,
            owner_index,
        )
        .map(|target| match target {
            contexts::OwnerMemberTarget::Declaration(symbol) => {
                CheckedResolutionTarget::Declaration(symbol)
            }
            contexts::OwnerMemberTarget::CollectionMeasure(measure) => {
                CheckedResolutionTarget::Intrinsic(measure.intrinsic())
            }
        })
    })
    .or_else(|| {
        let matching = facts
            .fact_call_projections
            .iter()
            .filter(|projection| projection.projection_expression == expression)
            .collect::<Vec<_>>();
        let [projection] = matching.as_slice() else {
            return None;
        };
        declaration_target(projection.field)
    })
    .or_else(|| checked_value_member_target(program, facts, member))
    .or_else(|| authored_member_target(program, member))
    .or_else(|| contextual_statement_member_target(program, expression, member))
    .map(|target| match target {
        CheckedResolutionTarget::Declaration(symbol) => {
            CheckedResolutionTarget::Declaration(authored_data_member(program, symbol))
        }
        target => target,
    })
}

/// The source-selection ledger names the template declaration, while typed
/// expressions keep the instantiated field and its substituted type. Rejoin
/// only through the exact instance's retained generic origin and member slot;
/// same-spelled fields belonging to other declarations remain distinct.
pub(super) fn authored_data_member(program: &TypedTrees, symbol: SymbolHandle) -> SymbolHandle {
    let owner = program.symbols.get(symbol).parent;
    let Some(instance) = program.data_definition_by_symbol(owner) else {
        return symbol;
    };
    let Some(origin) = instance.generic_instance else {
        return symbol;
    };
    let typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. } =
        program.type_reference_table.type_reference(origin)
    else {
        return symbol;
    };
    let Some(template) = program.data_definition_by_symbol(*base_symbol) else {
        return symbol;
    };
    for (instance_member, template_member) in program
        .data_members(instance)
        .iter()
        .zip(program.data_members(template))
    {
        if let (
            typed_trees::data::DataMember::Field(instance_field),
            typed_trees::data::DataMember::Field(template_field),
        ) = (instance_member, template_member)
            && instance_field.symbol == symbol
            && instance_field.name == template_field.name
        {
            return template_field.symbol;
        }
    }
    symbol
}

fn checked_value_member_target(
    program: &TypedTrees,
    facts: &CheckFacts,
    member: &typed_trees::expression::TableMemberExpression,
) -> Option<CheckedResolutionTarget> {
    let mut resolved = None;
    for (_, value) in facts.values.expression_values(member.receiver) {
        if !value.type_reference.is_valid() {
            continue;
        }
        let target = member_symbol_from_type_reference(
            program,
            value.type_reference,
            member.member.as_str(),
        )
        .and_then(declaration_target)
        .or_else(|| collection_measure_target(program, value.type_reference, member));
        let Some(target) = target else {
            continue;
        };
        if resolved.is_some_and(|candidate| candidate != target) {
            return None;
        }
        resolved = Some(target);
    }
    resolved
}

fn authored_member_target(
    program: &TypedTrees,
    member: &typed_trees::expression::TableMemberExpression,
) -> Option<CheckedResolutionTarget> {
    let receiver_type = authored_operand_type(program, member.receiver)?;
    member_symbol_from_type_reference(program, receiver_type, member.member.as_str())
        .and_then(declaration_target)
        .or_else(|| collection_measure_target(program, receiver_type, member))
}

/// Run `visit` on every `(state, statement_index, statement)` whose
/// statement expression tree contains `needle`; `None` aborts the walk.
fn for_each_statement_containing(
    program: &TypedTrees,
    needle: typed_trees::expression::ExpressionHandle,
    mut visit: impl FnMut(
        &typed_trees::state::State,
        usize,
        &typed_trees::statement::StatementNode,
    ) -> Option<()>,
) -> Option<()> {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let mut expressions = Vec::new();
                crate::monomorphization::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                if !expressions.contains(&needle) {
                    continue;
                }
                visit(state, statement_index, statement)?;
            }
        }
    }
    Some(())
}

fn contextual_statement_member_target(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    member: &typed_trees::expression::TableMemberExpression,
) -> Option<CheckedResolutionTarget> {
    let mut resolved = None;
    for_each_statement_containing(program, expression, |state, statement_index, _| {
        let receiver_type = crate::flow::expression_type_reference_in_state(
            program,
            state.symbol,
            statement_index,
            member.receiver,
        )?;
        let target =
            member_symbol_from_type_reference(program, receiver_type, member.member.as_str())
                .and_then(declaration_target)
                .or_else(|| collection_measure_target(program, receiver_type, member))?;
        if resolved.is_some_and(|candidate| candidate != target) {
            return None;
        }
        resolved = Some(target);
        Some(())
    })?;
    resolved
}

fn member_symbol_from_type_reference(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    use typed_trees::types::TypeReferenceNode;

    let (symbol, name) = match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            return member_symbol_from_type_reference(program, *referee, member_name);
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            return member_symbol_from_type_reference(program, *base_type, member_name);
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } => (*base_symbol, base_name),
        TypeReferenceNode::Named { symbol, name } => (*symbol, name),
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => return None,
    };
    let data = program.data_definitions().iter().find(|definition| {
        (symbol.is_valid() && definition.symbol == symbol) || definition.name == *name
    })?;
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == member_name => {
                Some(field.symbol)
            }
            typed_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == member_name =>
            {
                Some(variant.symbol)
            }
            typed_trees::data::DataMember::Variant(variant) => program
                .data_payload_fields(variant)
                .iter()
                .find_map(|field| (field.name.as_str() == member_name).then_some(field.symbol)),
            _ => None,
        })
}

/// A `len` or `capacity` member on a fixed array or slice selects the
/// compiler-owned standing measure of that name; neither is a package
/// declaration. Anything else yields no intrinsic target.
fn collection_measure_target(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    member: &typed_trees::expression::TableMemberExpression,
) -> Option<CheckedResolutionTarget> {
    let measure = crate::semantic::calls::collection_measure_member(
        program,
        member,
        MeasureReceiver::Declared(type_reference),
    )?;
    Some(CheckedResolutionTarget::Intrinsic(measure.intrinsic()))
}

/// With a walk's `owners` index the statements containing the expression come
/// from its recorded sites instead of a whole-program rescan; the verdict
/// (every containing statement types the operand as a primitive, and one
/// exists) does not depend on the order they are visited in.
pub(crate) fn expression_is_contextual_statement_primitive(
    program: &TypedTrees,
    facts: &CheckFacts,
    containing_expression: typed_trees::expression::ExpressionHandle,
    operand: typed_trees::expression::ExpressionHandle,
    owners: Option<&contexts::OwnerEnvironmentIndex>,
) -> bool {
    if let Some(owners) = owners {
        let sites = owners.containing_statement_sites(program, facts, containing_expression);
        let mut found = false;
        for (machine, state, statement_index) in sites {
            let machine = &program.machines()[machine];
            let state = &program.machine_states(machine)[state];
            let Some(type_reference) = crate::flow::expression_type_reference_in_state(
                program,
                state.symbol,
                statement_index,
                operand,
            ) else {
                return false;
            };
            if program.primitive_type_reference(type_reference).is_none() {
                return false;
            }
            found = true;
        }
        return found;
    }
    let mut found = false;
    for_each_statement_containing(
        program,
        containing_expression,
        |state, statement_index, _| {
            let Some(type_reference) = crate::flow::expression_type_reference_in_state(
                program,
                state.symbol,
                statement_index,
                operand,
            ) else {
                return None;
            };
            if program.primitive_type_reference(type_reference).is_none() {
                return None;
            }
            found = true;
            Some(())
        },
    )
    .is_some()
        && found
}

/// Whether the containment walk from `root` reaches `target`.
pub(crate) fn expression_contains(
    program: &TypedTrees,
    root: typed_trees::expression::ExpressionHandle,
    target: typed_trees::expression::ExpressionHandle,
) -> bool {
    reaches(program, root, target, &mut HashSet::default())
}

/// Every expression the containment walk reaches from `root`.
///
/// `expression_contains(program, root, target)` holds exactly when this set
/// contains `target`: the walk records each expression it enters before
/// comparing it, and an address no expression occupies never ends the walk
/// early, so the recorded set is the whole reachable set. A caller asking
/// the same containment question of many targets indexes this once instead
/// of rewalking `root` per target.
pub(crate) fn reachable_expressions(
    program: &TypedTrees,
    root: typed_trees::expression::ExpressionHandle,
) -> HashSet<typed_trees::expression::ExpressionHandle> {
    let mut reached = HashSet::default();
    // Arena index zero is the invalid address; `is_valid` rejects it below,
    // so no recorded expression can equal it.
    reaches(
        program,
        root,
        typed_trees::expression::ExpressionHandle::invalid(),
        &mut reached,
    );
    reached
}

/// The shared walk: a membership set, never an ordered history, so an
/// expression reached twice through different parents is entered once.
fn reaches(
    program: &TypedTrees,
    root: typed_trees::expression::ExpressionHandle,
    target: typed_trees::expression::ExpressionHandle,
    visited: &mut HashSet<typed_trees::expression::ExpressionHandle>,
) -> bool {
    if !root.is_valid() || !visited.insert(root) {
        return false;
    }
    if root == target {
        return true;
    }
    match program.expression_table.expression(root) {
        ExpressionNode::Match(dispatch) => {
            reaches(program, dispatch.subject, target, visited)
                || program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .any(|arm| {
                        (matches!(arm.pattern, typed_trees::expression::MatchPattern::Value(pattern)
                        if reaches(program, pattern, target, visited)))
                            || reaches(program, arm.value, target, visited)
                    })
        }
        ExpressionNode::Atomic(atomic) => reaches(program, atomic.value, target, visited),
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|child| reaches(program, *child, target, visited)),
        ExpressionNode::Binary(binary) => {
            reaches(program, binary.left, target, visited)
                || reaches(program, binary.right, target, visited)
        }
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && reaches(program, call.receiver, target, visited))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|child| reaches(program, *child, target, visited))
        }
        ExpressionNode::Cast(cast) => reaches(program, cast.value, target, visited),
        ExpressionNode::Indexed(indexed) => {
            reaches(program, indexed.collection, target, visited)
                || reaches(program, indexed.index, target, visited)
        }
        ExpressionNode::Member(member) => reaches(program, member.receiver, target, visited),
        ExpressionNode::Borrow(inner) => reaches(program, inner.target, target, visited),
        ExpressionNode::Range(range) => {
            reaches(program, range.start, target, visited)
                || reaches(program, range.end, target, visited)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| reaches(program, field.value, target, visited)),
        ExpressionNode::Unary(unary) => reaches(program, unary.operand, target, visited),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

/// `program_owners`, when a walk supplies one, is its index over no checked
/// facts (`OwnerEnvironmentIndex::program_only`).
pub(crate) fn expression_is_intrinsic_primitive_without_origin(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    program_owners: Option<&contexts::OwnerEnvironmentIndex>,
) -> bool {
    let type_reference = match program.tables.expression_table.expression(expression) {
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_) => {
            return true;
        }
        ExpressionNode::Name(path) => type_reference_for_symbol(program, path.symbol),
        ExpressionNode::Call(call) => program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine))
            .find_map(|state| (state.symbol == call.target_symbol).then_some(state.return_type)),
        ExpressionNode::Cast(cast) => Some(cast.target_type),
        ExpressionNode::Member(member) => {
            if matches!(
                contexts::checked_member_target_from_exact_owner(
                    program,
                    &CheckFacts::default(),
                    expression,
                    member,
                    program_owners,
                ),
                Some(contexts::OwnerMemberTarget::CollectionMeasure(_))
            ) {
                return true;
            }
            type_reference_for_symbol(
                program,
                crate::flow::effective_member_symbol(program, member.receiver, member),
            )
        }
        ExpressionNode::Binary(binary)
            if crate::operators::binary_operator_spelling(binary.operator).is_none() =>
        {
            return true;
        }
        ExpressionNode::Binary(binary) => {
            let Some(spelling) = crate::operators::binary_operator_spelling(binary.operator) else {
                unreachable!("handled above")
            };
            let operand_types = [
                authored_operand_descriptor(program, binary.left),
                authored_operand_descriptor(program, binary.right),
            ];
            if operand_types.iter().all(|operand| operand.is_unknown()) {
                // Neither operand names an authored type. When both are
                // anonymous -- literals or nested anonymous arithmetic --
                // the operator is exact builtin arithmetic that lands after
                // evaluation (language guide chapter 5), so an authored
                // spelling for some carrier never selects it. Any other
                // unknown operand keeps the question unanswered here.
                return expression_is_anonymous_arithmetic(program, binary.left)
                    && expression_is_anonymous_arithmetic(program, binary.right);
            }
            if !typed_trees::operator::resolve_spelling_for_operand_types(
                program,
                spelling,
                &operand_types,
                program.tables.expression_table.source_span(expression),
            )
            .is_empty()
            {
                return false;
            }
            // An operand whose carrier is known without a type reference --
            // a comparison result, a boolean literal -- answers this question
            // directly; it is exactly a builtin primitive with nothing to
            // point at.
            return operand_types.iter().any(|operand| {
                matches!(operand, typed_trees::operator::OperandType::Primitive(_))
            }) || expression_is_intrinsic_primitive_without_origin(
                program,
                binary.left,
                program_owners,
            ) || expression_is_intrinsic_primitive_without_origin(
                program,
                binary.right,
                program_owners,
            );
        }
        ExpressionNode::Unary(_) => return true,
        ExpressionNode::Borrow(inner) => {
            return expression_is_intrinsic_primitive_without_origin(
                program,
                inner.target,
                program_owners,
            );
        }
        _ => None,
    };
    type_reference
        .and_then(|type_reference| program.primitive_type_reference(type_reference))
        .is_some()
}

/// Whether an expression is anonymous arithmetic: a literal that has not
/// landed (no width suffix and no captured carrier), or arithmetic whose
/// operands are all anonymous. Such an expression carries no type of its own
/// until it lands, so no authored operator spelling applies to it.
fn expression_is_anonymous_arithmetic(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    use typed_trees::expression::BinaryOperator;
    match program.tables.expression_table.expression(expression) {
        ExpressionNode::Float(literal) => literal.landing().is_none(),
        ExpressionNode::Integer(literal) => literal.landing().is_none(),
        ExpressionNode::Binary(binary) => {
            matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
            ) && expression_is_anonymous_arithmetic(program, binary.left)
                && expression_is_anonymous_arithmetic(program, binary.right)
        }
        _ => false,
    }
}
