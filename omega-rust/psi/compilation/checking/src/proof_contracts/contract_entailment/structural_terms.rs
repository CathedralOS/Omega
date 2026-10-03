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
pub(super) use binary::binary_term_in_scope;
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

/// Read an expression as a structural term. Single-segment names are
/// variables; a resolved payload-free case path constructs its common fields.
/// Record and case literals retain complete sorted fields, including established
/// runtime defaults; unsupported expressions stay opaque or fail closed.
pub(super) fn structural_term(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<StructuralTerm> {
    structural_term_in_scope(program, None, expression)
}

/// `structural_term` with an explicit declaring `(machine, state)` fallback for
/// name-free binary operands (see `binary_term_in_scope`).
pub(super) fn structural_term_in_scope(
    program: &TypedTrees,
    scope: Option<(
        &typed_trees::machine::Machine,
        &typed_trees::state::State,
        typed_trees::types::TypeReferenceHandle,
    )>,
    expression: ExpressionHandle,
) -> Option<StructuralTerm> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            match members {
                [single] => {
                    // Provenance: an immutable state local's term is its
                    // declared initial value. A mutable local may have been
                    // reassigned before this use, so it stays an opaque
                    // variable rather than inherit a stale value.
                    if let Some((_, state, _)) = scope
                        && path.symbol.is_valid()
                        && let Some((_, local)) =
                            program.state_local_declaration(state, path.symbol)
                        && !local.is_mutable
                        && let Some(term) =
                            structural_term_in_scope(program, scope, local.initial_value)
                    {
                        return Some(term);
                    }
                    Some(StructuralTerm::Variable(single.as_str().to_owned()))
                }
                [_, _, ..] if case_classifier(program, expression).is_some() => {
                    case_value_term(program, expression)
                }
                _ => Some(StructuralTerm::Opaque(
                    program.expression_table.display_name(expression),
                )),
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            constructor_literal_term(program, literal, |value| {
                structural_term_in_scope(program, scope, value)
            })
        }
        ExpressionNode::Binary(binary) => binary_term_in_scope(
            program,
            scope,
            expression,
            structural_term_in_scope(program, scope, binary.left)?,
            structural_term_in_scope(program, scope, binary.right)?,
        ),
        ExpressionNode::Integer(value) => value.value_bignum().map(StructuralTerm::Integer),
        // A determinate float literal carries its exact decimal spelling; the
        // ordering arm widens it once at judgment. UndeterminedNaN stays
        // term-less so a NaN operand can never pose as a decided value.
        ExpressionNode::Float(value) => Some(StructuralTerm::Float(value.clone())),
        ExpressionNode::Call(call) => {
            if (!call.receiver.is_valid() || static_call_receiver(program, call))
                && call.evidence_arguments.is_empty()
                && call.static_requirement_dispatch.is_none()
            {
                let handles = program.expression_table.expression_handles(call.arguments);
                let arguments: Vec<StructuralTerm> = handles
                    .iter()
                    .filter_map(|argument| structural_term_in_scope(program, scope, *argument))
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
                && (!call.receiver.is_valid() || static_call_receiver(program, call))
                && call.evidence_arguments.is_empty()
                && call.static_requirement_dispatch.is_none()
            {
                let handles = program.expression_table.expression_handles(call.arguments);
                let arguments = handles
                    .iter()
                    .filter_map(|argument| structural_term_in_scope(program, scope, *argument))
                    .collect::<Vec<_>>();
                if arguments.len() == handles.len() {
                    let result_type = program.state_by_symbol(call.target_symbol)
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
            // A `.len` over declared fixed storage reads a closed extent in
            // the declaring scope: `main.nodes.len` on a `[Node; 4]` field is
            // the literal 4. Runtime-length receivers keep a substitutable
            // projection below so a call instantiation can still decide them.
            let is_length_member =
                language_semantics::declaration_selection::CollectionMeasure::from_authored_spelling(
                    member.member.as_str(),
                )
                    == Some(language_semantics::declaration_selection::CollectionMeasure::Length)
                    && !member.member_symbol.is_valid()
                    && member.case_variant.is_none();
            if is_length_member
                && let Some((machine, state, _)) = scope
                && let Some(receiver) = crate::value_custody::places::collection_length_receiver(
                    program,
                    machine,
                    Some(state),
                    expression,
                )
                && let Some(extent) = crate::value_custody::places::declared_collection_extent(
                    program,
                    machine,
                    Some(state),
                    receiver,
                )
            {
                return Some(StructuralTerm::Integer(numerics::bignum::BigInt::from_u64(
                    extent as u64,
                )));
            }
            // A field read off a value receiver: when the receiver termifies
            // to a closed constructor (a struct literal, an immutable local
            // tracing to one, or a nested projection already resolved), the
            // field term is recoverable by name. Collection length remains
            // a substitutable projection so call instantiation can recover
            // the actual extent. Ordinary symbolic fields retain the same
            // vocabulary as callee_term_with_machines and citation intake.
            match structural_term_in_scope(program, scope, member.receiver) {
                Some(StructuralTerm::Constructor { fields, .. }) => fields
                    .into_iter()
                    .find(|(name, _)| name == member.member.as_str())
                    .map(|(_, term)| term)
                    .or_else(|| {
                        Some(StructuralTerm::Opaque(
                            program.expression_table.display_name(expression),
                        ))
                    }),
                Some(
                    subject @ (StructuralTerm::Variable(_) | StructuralTerm::Projection { .. }),
                ) if is_length_member => Some(StructuralTerm::Projection {
                    subject: Box::new(subject),
                    path: member.member.as_str().to_owned(),
                }),
                _ => Some(StructuralTerm::Opaque(
                    program.expression_table.display_name(expression),
                )),
            }
        }
        ExpressionNode::ZeroValue(type_reference) => {
            zero_value_structural_term(program, *type_reference)
        }
        // A borrow's referent is the bound place itself — fact
        // instantiation strips it the same way (`&mut main.control`
        // instantiates to `main.control`), so the term names the
        // referent's storage and a `.len` over it folds to the declared
        // extent.
        ExpressionNode::Borrow(borrow) => structural_term_in_scope(program, scope, borrow.target),
        _ => None,
    }
}

/// Whether a call's receiver position carries a namespace path rather than a
/// value: `Point::zero()` spells its `Point` prefix as a receiver Name whose
/// symbol resolves to a type, module, or machine root — never a local,
/// parameter, or borrow. A namespace receiver contributes no runtime
/// argument, so the call termifies exactly like a receiverless application.
pub(super) fn static_call_receiver(
    program: &TypedTrees,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    call.receiver.is_valid()
        && matches!(
            program.expression_table.expression(call.receiver),
            ExpressionNode::Name(path)
                if matches!(
                    program.symbols.get(path.symbol).kind,
                    symbols::SymbolKind::BuiltinType
                        | symbols::SymbolKind::Data
                        | symbols::SymbolKind::Domain
                        | symbols::SymbolKind::Machine
                        | symbols::SymbolKind::Module
                        | symbols::SymbolKind::Trait
                        | symbols::SymbolKind::ConformanceParameter
                )
        )
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
