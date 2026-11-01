//! Contextual domain, value and statement member targets.

use arena::Arena;
use typed_trees::data::DataMember;
use typed_trees::machine::Machine;
use typed_trees::operator::OperandType;
use typed_trees::operator::binary_operator_spelling;
use typed_trees::operator::resolve_spelling_for_operand_types;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceHandle;
use validation::expression_owners::OwnerEnvironmentIndex;
use validation::expression_owners::OwnerMemberTarget;
use validation::expression_owners::checked_member_target_from_exact_owner;
use validation::expression_traversal::collect_statement_expression_trees;
use validation::field_domains::data_definition_by_symbol_or_name;
use validation::selection_resolution::declaration_target;
use validation::selection_resolution::{
    AuthoredSelectionResolutionTarget as ResolutionTarget, authored_data_member,
};

use crate::authored_selections::operator_targets::{
    authored_operand_descriptor, authored_operand_type,
};
use crate::flow::indexed_statements;
use checked_trees::CheckFacts;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableMemberExpression};
use validation::collection_operations::{MeasureReceiver, collection_measure_member};
use validation::{
    data_member_symbol_by_name, declaration_type_reference, effective_member_symbol,
    expression_type_reference_in_state,
};

pub(crate) fn checked_member_target(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: ExpressionHandle,
    member: &TableMemberExpression,
    owner_index: Option<&OwnerEnvironmentIndex>,
) -> Option<ResolutionTarget> {
    declaration_target(effective_member_symbol(program, member.receiver, member))
        .or_else(|| {
            checked_member_target_from_exact_owner(
                program,
                &facts.proof.contract_facts,
                expression,
                member,
                owner_index,
            )
            .map(|target| match target {
                OwnerMemberTarget::Declaration(symbol) => ResolutionTarget::Declaration(symbol),
                OwnerMemberTarget::CollectionMeasure(measure) => {
                    ResolutionTarget::Intrinsic(measure.intrinsic())
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
            ResolutionTarget::Declaration(symbol) => {
                ResolutionTarget::Declaration(authored_data_member(program, symbol))
            }
            target => target,
        })
}

fn checked_value_member_target(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    member: &TableMemberExpression,
) -> Option<ResolutionTarget> {
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
    program: &dyn ::validation::CheckingInput,
    member: &TableMemberExpression,
) -> Option<ResolutionTarget> {
    let receiver_type = authored_operand_type(program, member.receiver)?;
    member_symbol_from_type_reference(program, receiver_type, member.member.as_str())
        .and_then(declaration_target)
        .or_else(|| collection_measure_target(program, receiver_type, member))
}

/// Run `visit` on every `(state, statement_index, statement)` whose
/// statement expression tree contains `needle`; `None` aborts the walk.
pub(super) fn for_each_statement_containing(
    program: &dyn ::validation::CheckingInput,
    needle: ExpressionHandle,
    mut visit: impl FnMut(&Machine, &State, usize, &StatementNode) -> Option<()>,
) -> Option<()> {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in indexed_statements(program, state.statement_nodes) {
                let mut expressions = Vec::new();
                collect_statement_expression_trees(program, statement, &mut expressions);
                if !expressions.contains(&needle) {
                    continue;
                }
                visit(machine, state, statement_index, statement)?;
            }
        }
    }
    Some(())
}

fn contextual_statement_member_target(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    member: &TableMemberExpression,
) -> Option<ResolutionTarget> {
    let mut resolved = None;
    for_each_statement_containing(program, expression, |_, state, statement_index, _| {
        let receiver_type = expression_type_reference_in_state(
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
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    use typed_trees::types::TypeReferenceNode;

    let (symbol, name) = match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::CaseView { .. } => return None,
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
    let data = data_definition_by_symbol_or_name(program, symbol, name)?;
    data_member_symbol_by_name(program, data, member_name).or_else(|| {
        program
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                DataMember::Variant(variant) => program
                    .data_payload_fields(variant)
                    .iter()
                    .find_map(|field| (field.name.as_str() == member_name).then_some(field.symbol)),
                _ => None,
            })
    })
}

/// A `len` or `capacity` member on a fixed array or slice selects the
/// compiler-owned standing measure of that name; neither is a package
/// declaration. Anything else yields no intrinsic target.
fn collection_measure_target(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
    member: &TableMemberExpression,
) -> Option<ResolutionTarget> {
    let measure =
        collection_measure_member(program, member, MeasureReceiver::Declared(type_reference))?;
    Some(ResolutionTarget::Intrinsic(measure.intrinsic()))
}

/// With a walk's `owners` index the statements containing the expression come
/// from its recorded sites instead of a whole-program rescan; the verdict
/// (every containing statement types the operand as a primitive, and one
/// exists) does not depend on the order they are visited in.
pub(crate) fn expression_is_contextual_statement_primitive(
    program: &dyn ::validation::CheckingInput,
    _facts: &CheckFacts,
    containing_expression: ExpressionHandle,
    operand: ExpressionHandle,
    owners: Option<&OwnerEnvironmentIndex>,
) -> bool {
    if let Some(owners) = owners {
        let sites = owners.containing_statement_sites(containing_expression);
        let mut found = false;
        for (machine, state, statement_index) in sites {
            let machine = &program.machines()[machine];
            let state = &program.machine_states(machine)[state];
            let Some(type_reference) =
                expression_type_reference_in_state(program, state.symbol, statement_index, operand)
            else {
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
        |_, state, statement_index, _| {
            let Some(type_reference) =
                expression_type_reference_in_state(program, state.symbol, statement_index, operand)
            else {
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

/// `program_owners`, when a walk supplies one, is its index over no checked
/// facts (`OwnerEnvironmentIndex::program_only`).
pub(crate) fn expression_is_intrinsic_primitive_without_origin(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    program_owners: Option<&OwnerEnvironmentIndex>,
) -> bool {
    let type_reference = match program.tables.expression_table.expression(expression) {
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_) => {
            return true;
        }
        ExpressionNode::Name(path) => declaration_type_reference(program, path.symbol),
        ExpressionNode::Call(call) => program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine))
            .find_map(|state| (state.symbol == call.target_symbol).then_some(state.return_type)),
        ExpressionNode::Cast(cast) => Some(cast.target_type),
        ExpressionNode::Member(member) => {
            if matches!(
                checked_member_target_from_exact_owner(
                    program,
                    &Arena::default(),
                    expression,
                    member,
                    program_owners,
                ),
                Some(OwnerMemberTarget::CollectionMeasure(_))
            ) {
                return true;
            }
            declaration_type_reference(
                program,
                effective_member_symbol(program, member.receiver, member),
            )
        }
        ExpressionNode::Binary(binary) if binary_operator_spelling(binary.operator).is_none() => {
            return true;
        }
        ExpressionNode::Binary(binary) => {
            let Some(spelling) = binary_operator_spelling(binary.operator) else {
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
            if !resolve_spelling_for_operand_types(
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
            return operand_types
                .iter()
                .any(|operand| matches!(operand, OperandType::Primitive(_)))
                || expression_is_intrinsic_primitive_without_origin(
                    program,
                    binary.left,
                    program_owners,
                )
                || expression_is_intrinsic_primitive_without_origin(
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
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
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
