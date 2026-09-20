//! Structural proof-term construction and normalization.
//!
//! This module owns the expression-to-term boundary used by contract
//! entailment. It deliberately recognizes only the closed structural shapes
//! the judge can compare soundly; every other expression remains opaque or
//! unsupported.

use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};

use super::{StructuralTerm, is_arm_pattern_marker};

mod binary;
mod constructors;
pub(super) use binary::binary_term;
pub(super) use constructors::{
    case_classifier, case_guard_classifier, case_value_term, constructor_literal_term,
    is_case_observation, zero_value_structural_term,
};

/// Whether `haystack` contains `needle` as a subterm (occurs check for the
/// rewrite orientation: a rewrite whose replacement contains its own pattern
/// would loop; the resolution cap would still bound it, but skipping keeps
/// resolution productive).
pub(super) fn term_contains(haystack: &StructuralTerm, needle: &StructuralTerm) -> bool {
    if haystack == needle {
        return true;
    }
    match haystack {
        StructuralTerm::Projection { subject, .. } => term_contains(subject, needle),
        StructuralTerm::ScalarBinary { left, right, .. } => {
            term_contains(left, needle) || term_contains(right, needle)
        }
        StructuralTerm::Constructor { fields, .. } => {
            fields.iter().any(|(_, value)| term_contains(value, needle))
        }
        StructuralTerm::Application { arguments, .. } => arguments
            .iter()
            .any(|argument| term_contains(argument, needle)),
        StructuralTerm::CallProjection { arguments, .. } => arguments
            .iter()
            .any(|argument| term_contains(argument, needle)),
        _ => false,
    }
}

/// Normalize nullary applications of trivial constant machines to the closed
/// constructor value returned by that machine.
pub(super) fn unfold_constant_applications(
    program: &TypedTrees,
    term: StructuralTerm,
) -> StructuralTerm {
    match term {
        StructuralTerm::Application {
            target,
            selections,
            machine,
            arguments,
        } if arguments.is_empty() => match constant_machine_constructor(program, target) {
            Some(constructor) => constructor,
            None => StructuralTerm::Application {
                target,
                selections,
                machine,
                arguments,
            },
        },
        StructuralTerm::Application {
            target,
            selections,
            machine,
            arguments,
        } => StructuralTerm::Application {
            target,
            selections,
            machine,
            arguments: arguments
                .into_iter()
                .map(|argument| unfold_constant_applications(program, argument))
                .collect(),
        },
        StructuralTerm::CallProjection {
            selections,
            target,
            machine,
            result_type,
            field,
            field_name,
            arguments,
        } => StructuralTerm::CallProjection {
            selections,
            target,
            machine,
            result_type,
            field,
            field_name,
            arguments: arguments
                .into_iter()
                .map(|argument| unfold_constant_applications(program, argument))
                .collect(),
        },
        StructuralTerm::Constructor { data, case, fields } => StructuralTerm::Constructor {
            data,
            case,
            fields: fields
                .into_iter()
                .map(|(name, value)| (name, unfold_constant_applications(program, value)))
                .collect(),
        },
        StructuralTerm::ScalarBinary {
            operator,
            meaning,
            left,
            right,
        } => StructuralTerm::ScalarBinary {
            operator,
            meaning,
            left: Box::new(unfold_constant_applications(program, *left)),
            right: Box::new(unfold_constant_applications(program, *right)),
        },
        StructuralTerm::Projection { subject, path } => StructuralTerm::Projection {
            subject: Box::new(unfold_constant_applications(program, *subject)),
            path,
        },
        other => other,
    }
}

/// The constructor value a trivial constant machine returns, when its shape
/// is exactly one state with one unguarded transition to a closed constructor.
fn constant_machine_constructor(
    program: &TypedTrees,
    target: symbols::SymbolHandle,
) -> Option<StructuralTerm> {
    use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

    let machine = selected_application_machine(program, target)?;
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let non_marker: Vec<&StatementNode> = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter(|statement| !is_arm_pattern_marker(statement))
        .collect();
    let [statement] = non_marker[..] else {
        return None;
    };
    let StatementNode::Transition(transition) = statement else {
        return None;
    };
    if !matches!(transition.guard, TransitionGuardNode::Always) {
        return None;
    }
    let TransitionTargetNode::Value(value) =
        program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    let term = structural_term(program, *value)?;

    fn is_closed(term: &StructuralTerm) -> bool {
        match term {
            StructuralTerm::Integer(_) => true,
            StructuralTerm::Constructor { fields, .. } => {
                fields.iter().all(|(_, value)| is_closed(value))
            }
            _ => false,
        }
    }

    is_closed(&term).then_some(term)
}

/// Preserve compile-time machine selections in structural application
/// identity. Static machine arguments are part of a call's meaning.
pub(super) fn structural_call_machine_name(
    target: &str,
    machine_arguments: &[StaticMachineArgument],
    machine_environment: &[(symbols::SymbolHandle, String, StaticMachineArgument)],
) -> String {
    let substitute = |name: String| {
        machine_environment
            .iter()
            .find(|(_, parameter, _)| parameter == &name)
            .map(|(_, _, selected)| {
                selected
                    .path
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("::")
            })
            .unwrap_or(name)
    };
    let target = substitute(target.to_owned());
    if machine_arguments.is_empty() {
        return target;
    }
    let selected: Vec<String> = machine_arguments
        .iter()
        .map(|argument| {
            let name = argument
                .path
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            substitute(name)
        })
        .collect();
    format!("{target}<{}>", selected.join(","))
}

pub(super) fn split_structural_machine_name(name: &str) -> (&str, Vec<&str>) {
    let Some((base, selected)) = name.split_once('<') else {
        return (name, Vec::new());
    };
    let Some(selected) = selected.strip_suffix('>') else {
        return (name, Vec::new());
    };
    if selected.is_empty() {
        (base, Vec::new())
    } else {
        (base, selected.split(',').collect())
    }
}

/// Read an expression as a structural term. Single-segment names are
/// variables; a resolved payload-free case path constructs its common fields.
/// Record and case literals retain complete sorted fields, including established
/// runtime defaults; unsupported expressions stay opaque or fail closed.
pub(super) fn structural_term(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<StructuralTerm> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            match members {
                [single] => Some(StructuralTerm::Variable(single.as_str().to_owned())),
                [_, _, ..] if case_classifier(program, expression).is_some() => {
                    case_value_term(program, expression)
                }
                _ => Some(StructuralTerm::Opaque(
                    program.expression_table.display_name(expression),
                )),
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            constructor_literal_term(program, literal, |value| structural_term(program, value))
        }
        ExpressionNode::Binary(binary) => binary_term(
            program,
            expression,
            structural_term(program, binary.left)?,
            structural_term(program, binary.right)?,
        ),
        ExpressionNode::Integer(value) => value.value_bignum().map(StructuralTerm::Integer),
        ExpressionNode::Call(call) => {
            if !call.receiver.is_valid()
                && call.evidence_arguments.is_empty()
                && call.static_requirement_dispatch.is_none()
            {
                let handles = program.expression_table.expression_handles(call.arguments);
                let arguments: Vec<StructuralTerm> = handles
                    .iter()
                    .filter_map(|argument| structural_term(program, *argument))
                    .collect();
                if arguments.len() == handles.len() {
                    return Some(StructuralTerm::Application {
                        target: call.target_symbol,
                        selections: call.machine_arguments.to_vec(),
                        machine: structural_call_machine_name(
                            call.target.as_str(),
                            &call.machine_arguments,
                            &[],
                        ),
                        arguments,
                    });
                }
            }
            Some(StructuralTerm::Opaque(
                program.expression_table.display_name(expression),
            ))
        }
        ExpressionNode::Boolean(value) => Some(StructuralTerm::Constructor {
            data: "bool".to_owned(),
            case: value.to_string(),
            fields: Vec::new(),
        }),
        ExpressionNode::Member(member) => {
            if let ExpressionNode::Call(call) = program.expression_table.expression(member.receiver)
                && !call.receiver.is_valid()
                && call.evidence_arguments.is_empty()
                && call.static_requirement_dispatch.is_none()
            {
                let handles = program.expression_table.expression_handles(call.arguments);
                let arguments = handles
                    .iter()
                    .filter_map(|argument| structural_term(program, *argument))
                    .collect::<Vec<_>>();
                if arguments.len() == handles.len() {
                    let result_type = program
                        .machines()
                        .iter()
                        .flat_map(|machine| program.machine_states(machine))
                        .find(|state| state.symbol == call.target_symbol)
                        .map(|state| state.return_type)?;
                    let field =
                        call_projection_field_symbol(program, result_type, member.member.as_str())?;
                    return Some(StructuralTerm::CallProjection {
                        target: call.target_symbol,
                        selections: call.machine_arguments.to_vec(),
                        machine: structural_call_machine_name(
                            call.target.as_str(),
                            &call.machine_arguments,
                            &[],
                        ),
                        result_type,
                        field,
                        field_name: member.member.as_str().to_owned(),
                        arguments,
                    });
                }
            }
            Some(StructuralTerm::Opaque(
                program.expression_table.display_name(expression),
            ))
        }
        ExpressionNode::ZeroValue(type_reference) => {
            zero_value_structural_term(program, *type_reference)
        }
        _ => None,
    }
}

fn call_projection_field_symbol(
    program: &TypedTrees,
    result_type: typed_trees::types::TypeReferenceHandle,
    field_name: &str,
) -> Option<symbols::SymbolHandle> {
    let data = crate::value_custody::places::data_definition_for_type(program, result_type)?;
    program.data_members(data).iter().find_map(|member| {
        let typed_trees::data::DataMember::Field(field) = member else {
            return None;
        };
        (field.name.as_str() == field_name).then_some(field.symbol)
    })
}

/// Rejoin the exact selected entry. Names never recover an absent selection.
pub(super) fn selected_application_machine(
    program: &TypedTrees,
    target: symbols::SymbolHandle,
) -> Option<&typed_trees::machine::Machine> {
    if !target.is_valid() {
        return None;
    }
    let mut matches = program.machines().iter().filter(|machine| {
        program
            .machine_states(machine)
            .first()
            .is_some_and(|state| state.symbol == target)
    });
    let machine = matches.next()?;
    matches.next().is_none().then_some(machine)
}
