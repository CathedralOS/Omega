//! Struct-literal field validation: every named field of a brace construction
//! must be a declared member of the constructed shape. Covers current-shape
//! record literals (`Counter { count: 0 }`), historical-shape literals
//! (`Counter::v1 { counter: 3 }` -- lowered upstream to record literals of the
//! version's root-level shape definition), and case-payload literals
//! (`Command::Say { text: ... }`). Literals whose head type is not a data
//! definition in this program (or is generic, where member types depend on
//! instantiation) are left to later layers.

use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::{DataDefinition, DataMember};
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
use omega_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

pub(crate) fn validate_struct_literal_fields(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
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
                    StatementNode::Transition(transition) => {
                        if let TransitionGuardNode::When(guard) = &transition.guard {
                            scan_expression(program, *guard, diagnostics);
                        }
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            scan_transition_target(
                                program,
                                program.statement_table.transition_target(target),
                                diagnostics,
                            );
                        }
                    }
                }
            }
        }
    }
}

fn scan_transition_target(
    program: &TypedTrees,
    target: &TransitionTargetNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match target {
        TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                scan_expression(program, *argument, diagnostics);
            }
        }
        TransitionTargetNode::Value(expression) => {
            scan_expression(program, *expression, diagnostics);
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
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
        ExpressionNode::StructLiteral(literal) => {
            validate_literal_field_names(program, &literal, diagnostics);
            for field in program.expression_table.struct_fields(literal.fields) {
                scan_expression(program, field.value, diagnostics);
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                scan_expression(program, *element, diagnostics);
            }
        }
        ExpressionNode::Binary(binary) => {
            scan_expression(program, binary.left, diagnostics);
            scan_expression(program, binary.right, diagnostics);
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
        ExpressionNode::Unary(unary) => scan_expression(program, unary.operand, diagnostics),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

/// Check one literal's named fields against the constructed shape's declared
/// members: record literals (current or historical shape) construct FIELD
/// members; case literals construct the named variant's PAYLOAD fields.
fn validate_literal_field_names(
    program: &TypedTrees,
    literal: &TableStructLiteral,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let type_name = literal.type_name.as_str();
    let Some(data_definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == type_name)
    else {
        return;
    };
    if data_definition.type_parameters.count() > 0 {
        return;
    }

    match &literal.case_name {
        None => {
            for field in program.expression_table.struct_fields(literal.fields) {
                if !data_declares_field(program, data_definition, field.name.as_str()) {
                    diagnostics.push(Diagnostic::error(format!(
                        "data `{type_name}` has no field `{}`",
                        field.name.as_str()
                    )));
                }
            }
        }
        Some(case_name) => {
            let Some(variant) = program
                .data_members(data_definition)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.name.as_str() == case_name.as_str() => {
                        Some(variant)
                    }
                    _ => None,
                })
            else {
                return;
            };
            for field in program.expression_table.struct_fields(literal.fields) {
                let declared = program
                    .data_payload_fields(variant)
                    .iter()
                    .any(|payload_field| payload_field.name.as_str() == field.name.as_str());
                if !declared {
                    diagnostics.push(Diagnostic::error(format!(
                        "case `{type_name}::{}` has no payload field `{}`",
                        case_name.as_str(),
                        field.name.as_str()
                    )));
                }
            }
        }
    }
}

fn data_declares_field(
    program: &TypedTrees,
    data_definition: &DataDefinition,
    field_name: &str,
) -> bool {
    program.data_members(data_definition).iter().any(
        |member| matches!(member, DataMember::Field(field) if field.name.as_str() == field_name),
    )
}
