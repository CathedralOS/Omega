//! Structural proof-term construction and normalization.
//!
//! This module owns the expression-to-term boundary used by contract
//! entailment. It deliberately recognizes only the closed structural shapes
//! the judge can compare soundly; every other expression remains opaque or
//! unsupported.

use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};

use super::{StructuralTerm, is_arm_pattern_marker};

/// Whether `haystack` contains `needle` as a subterm (occurs check for the
/// rewrite orientation: a rewrite whose replacement contains its own pattern
/// would loop; the resolution cap would still bound it, but skipping keeps
/// resolution productive).
pub(super) fn term_contains(haystack: &StructuralTerm, needle: &StructuralTerm) -> bool {
    if haystack == needle {
        return true;
    }
    match haystack {
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
        StructuralTerm::Application { machine, arguments } if arguments.is_empty() => {
            match constant_machine_constructor(program, &machine) {
                Some(constructor) => constructor,
                None => StructuralTerm::Application { machine, arguments },
            }
        }
        StructuralTerm::Application { machine, arguments } => StructuralTerm::Application {
            machine,
            arguments: arguments
                .into_iter()
                .map(|argument| unfold_constant_applications(program, argument))
                .collect(),
        },
        StructuralTerm::CallProjection {
            target,
            machine,
            result_type,
            field,
            field_name,
            arguments,
        } => StructuralTerm::CallProjection {
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
        other => other,
    }
}

/// The constructor value a trivial constant machine returns, when its shape
/// is exactly one state with one unguarded transition to a closed constructor.
fn constant_machine_constructor(program: &TypedTrees, name: &str) -> Option<StructuralTerm> {
    use psi_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

    let machine = program.machines().iter().find(|machine| {
        machine.name.as_str() == name
            || machine
                .name
                .as_str()
                .rsplit("::")
                .next()
                .is_some_and(|simple| simple == name)
    })?;
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
    machine_environment: &[(String, String)],
) -> String {
    let substitute = |name: String| {
        machine_environment
            .iter()
            .find(|(parameter, _)| parameter == &name)
            .map(|(_, selected)| selected.clone())
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
/// variables; a two-segment data path is a nullary case constructor; record
/// and case literals retain sorted fields; unsupported expressions stay
/// opaque or fail closed.
pub(super) fn structural_term(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<StructuralTerm> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            match members {
                [single] => Some(StructuralTerm::Variable(single.as_str().to_owned())),
                [first, second] => {
                    if program
                        .data_definitions()
                        .iter()
                        .any(|definition| definition.name.as_str() == first.as_str())
                    {
                        Some(StructuralTerm::Constructor {
                            data: first.as_str().to_owned(),
                            case: second.as_str().to_owned(),
                            fields: Vec::new(),
                        })
                    } else {
                        Some(StructuralTerm::Opaque(
                            program.expression_table.display_name(expression),
                        ))
                    }
                }
                _ => Some(StructuralTerm::Opaque(
                    program.expression_table.display_name(expression),
                )),
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            let case = literal
                .case_name
                .as_ref()
                .map(|case| case.as_str())
                .unwrap_or("");
            let mut fields: Vec<(String, StructuralTerm)> = Vec::new();
            for field in program.expression_table.struct_fields(literal.fields) {
                fields.push((
                    field.name.as_str().to_owned(),
                    structural_term(program, field.value)?,
                ));
            }
            fields.sort_by(|(left, _), (right, _)| left.cmp(right));
            Some(StructuralTerm::Constructor {
                data: literal.type_name.as_str().to_owned(),
                case: case.to_owned(),
                fields,
            })
        }
        ExpressionNode::Call(call) => {
            if !call.receiver.is_valid() {
                let handles = program.expression_table.expression_handles(call.arguments);
                let arguments: Vec<StructuralTerm> = handles
                    .iter()
                    .filter_map(|argument| structural_term(program, *argument))
                    .collect();
                if arguments.len() == handles.len() {
                    return Some(StructuralTerm::Application {
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
    result_type: psi_typed_trees::types::TypeReferenceHandle,
    field_name: &str,
) -> Option<psi_symbols::SymbolHandle> {
    let data = crate::places::data_definition_for_type(program, result_type)?;
    program.data_members(data).iter().find_map(|member| {
        let psi_typed_trees::data::DataMember::Field(field) = member else {
            return None;
        };
        (field.name.as_str() == field_name).then_some(field.symbol)
    })
}

/// Normalize proof-only `zero_value<T>()` through the same home-
/// representation rule used by layout.
fn zero_value_structural_term(
    program: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> Option<StructuralTerm> {
    use psi_typed_trees::data::DataMember;
    use psi_typed_trees::types::TypeReferenceNode;

    let (symbol, name) = match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            return zero_value_structural_term(program, *base_type);
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } => (*base_symbol, base_name.as_str()),
        TypeReferenceNode::Named { symbol, name } => (*symbol, name.as_str()),
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Unit => return None,
    };
    let definition = program.data_definitions().iter().find(|definition| {
        (symbol.is_valid() && definition.symbol == symbol) || definition.name.as_str() == name
    })?;
    if crate::data::data_requires_establishment(program, definition) {
        return None;
    }
    let variant = program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            DataMember::Field(_) => None,
        })?;
    if !program.data_payload_fields(variant).is_empty() {
        return None;
    }
    Some(StructuralTerm::Constructor {
        data: definition.name.as_str().to_owned(),
        case: variant.name.as_str().to_owned(),
        fields: Vec::new(),
    })
}
