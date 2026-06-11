//! Interim equality rule (frozen decision 8): equality is trait-resolved core
//! `Equatable` with a synthesized structural `equals` LATER; until synthesis
//! exists, `==`/`!=` against a PAYLOAD-BEARING case in VALUE position is a
//! compile error (a tag compare would silently ignore the payload).
//! Payload-less sums keep the free tag compare.
//!
//! Scope: STATEMENT-position expressions only (assignments, value-call
//! arguments, local initializers, terminal expressions). Transition
//! statements are excluded on purpose -- a case match arm desugars to an
//! equality against the case in the guard layer, and that tag-dispatch with
//! payload BINDING is exactly the blessed form.

use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::DataMember;
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::statement::StatementNode;

pub(crate) fn validate_equality_operands(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Assignment(assignment) => {
                        scan_expression(program, assignment.target, diagnostics);
                        scan_expression(program, assignment.value, diagnostics);
                    }
                    StatementNode::Call(call) => {
                        for argument in program.statement_table.expression_handles(call.arguments) {
                            scan_expression(program, *argument, diagnostics);
                        }
                    }
                    StatementNode::Expression(expression) => {
                        scan_expression(program, *expression, diagnostics);
                    }
                    StatementNode::LocalData(local_data) => {
                        scan_expression(program, local_data.initial_value, diagnostics);
                    }
                    // A case match arm desugars to an equality against the
                    // case inside the transition guard layer; that tag
                    // dispatch (with payload binding) is the blessed form.
                    StatementNode::Transition(_) => {}
                }
            }
        }
    }
}

fn scan_expression(
    program: &TypedTrees,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) {
                for operand in [binary.left, binary.right] {
                    if let Some((data_name, case_name)) =
                        payload_bearing_case_operand(program, operand)
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "cannot compare against payload-bearing case `{data_name}::{case_name}` with `==`: structural equality is not synthesized yet -- match on the case in a transition to bind its payload"
                        )));
                    }
                }
            }
            scan_expression(program, binary.left, diagnostics);
            scan_expression(program, binary.right, diagnostics);
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                scan_expression(program, *element, diagnostics);
            }
        }
        ExpressionNode::Cast(cast) => scan_expression(program, cast.value, diagnostics),
        ExpressionNode::Call(call) => {
            scan_expression(program, call.receiver, diagnostics);
            for argument in program.expression_table.expression_handles(call.arguments) {
                scan_expression(program, *argument, diagnostics);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            scan_expression(program, indexed.collection, diagnostics);
            scan_expression(program, indexed.index, diagnostics);
        }
        ExpressionNode::Member(member) => scan_expression(program, member.receiver, diagnostics),
        ExpressionNode::Mutable(inner) => scan_expression(program, *inner, diagnostics),
        ExpressionNode::Range(range) => {
            scan_expression(program, range.start, diagnostics);
            scan_expression(program, range.end, diagnostics);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                scan_expression(program, field.value, diagnostics);
            }
        }
        ExpressionNode::Unary(unary) => scan_expression(program, unary.operand, diagnostics),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

/// An operand names a payload-bearing case when it is a `Type::Case` path or
/// a `Type::Case { ... }` construction where `Case` declares payload fields.
fn payload_bearing_case_operand(
    program: &TypedTrees,
    operand: ExpressionHandle,
) -> Option<(String, String)> {
    match program.expression_table.expression(operand) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let [type_name, case_name] = members else {
                return None;
            };
            payload_bearing_case(program, type_name.as_str(), case_name.as_str())
        }
        ExpressionNode::StructLiteral(literal) => {
            let case_name = literal.case_name.as_ref()?;
            payload_bearing_case(program, literal.type_name.as_str(), case_name.as_str())
        }
        _ => None,
    }
}

fn payload_bearing_case(
    program: &TypedTrees,
    type_name: &str,
    case_name: &str,
) -> Option<(String, String)> {
    let data_definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == type_name)?;

    program
        .data_members(data_definition)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant)
                if variant.name.as_str() == case_name && variant.payload.count() > 0 =>
            {
                Some((type_name.to_owned(), case_name.to_owned()))
            }
            _ => None,
        })
}
