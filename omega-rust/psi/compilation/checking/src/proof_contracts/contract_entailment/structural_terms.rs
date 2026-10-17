//! Structural proof-term construction and normalization.
//!
//! This module owns the expression-to-term boundary used by contract
//! entailment. It deliberately recognizes only the closed structural shapes
//! the judge can compare soundly; every other expression remains opaque or
//! unsupported.

use symbols::JoinedPath;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};

use super::StructuralTerm;

mod binary;
mod collection_measures;
pub(super) use collection_measures::{
    collection_measure_term, descriptor_only_observation, observed_collection_measure,
    reference_argument_observation,
};
mod constructors;
mod equality;
mod indexed;
pub(super) use indexed::indexed_term_in_scope;
mod scalar_operands;
pub(super) use binary::{binary_term_in_scope, logical_not_term_in_scope};
pub(super) use constructors::{
    case_classifier, case_guard_classifier, case_value_term, constructor_literal_term,
    is_case_observation, zero_value_structural_term,
};
pub(super) use equality::{has_identity_equality_meaning, identity_equality_terms};
pub(super) use scalar_operands::{
    scalar_operand_is_observed, scalar_operand_meaning, term_is_unobserved_descriptor,
};

/// An admitted integer conversion preserves its mathematical value; policy
/// qualification changes later operations, never the conversion's denotation.
/// Formation remains an occurrence obligation, observed before the conversion
/// result exists. Plain domain qualification on the same owned carrier also
/// preserves the value. Reference re-views and carrier-changing noninteger
/// conversions retain their separate authority and denotation rules.
pub(super) fn denotation_preserving_conversion(
    program: &dyn crate::CheckingInput,
    cast: &typed_trees::expression::TableCastExpression,
    source: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let integer = |reference| {
        program
            .primitive_type_reference(reference)
            .is_some_and(|primitive| {
                primitive.is_fixed_integer() || primitive == typed_trees::types::PrimitiveType::Addr
            })
    };
    !cast.form.is_recast()
        && ((integer(source) && integer(cast.target_type))
            || (crate::reference_shell(program, source).is_none()
                && crate::reference_shell(program, cast.target_type).is_none()
                && crate::declared_carriers_match(program, source, cast.target_type)
                && program.arithmetic_domain_for_type_reference(source)
                    == program.arithmetic_domain_for_type_reference(cast.target_type)))
}

/// The builtin embedding observes the authenticated payload, not the source
/// expression at a later proof query. Shared and owned views retain their
/// original observation authority through the raw argument reference.
pub(super) fn integer_embedding_term(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    value: StructuralTerm,
) -> Option<StructuralTerm> {
    let (_, argument) =
        crate::proof_contracts::proof_embeddings::integer_embedding_argument(program, expression)?;
    let reference =
        crate::proof_contracts::proof_embeddings::expression_type_reference(program, argument)?;
    Some(StructuralTerm::IntegerEmbedding {
        reference,
        value: Box::new(value),
    })
}

/// Both anonymous peers compare as exact rationals after builtin selection,
/// without choosing a fixed carrier for either intermediate computation.
pub(super) fn builtin_anonymous_comparison_term(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<StructuralTerm> {
    crate::value_custody::literals::evaluate_anonymous_numeric_comparison(
        program,
        expression,
        &[],
        |source| {
            source == expression
                || crate::value_custody::literals::has_anonymous_operator_meaning(program, source)
        },
    )
    .map(|value| StructuralTerm::Constructor {
        data: "bool".to_owned(),
        case: value.to_string(),
        fields: Vec::new(),
    })
}

/// Whether `haystack` contains `needle` as a subterm (occurs check for the
/// rewrite orientation: a rewrite whose replacement contains its own pattern
/// would loop; the resolution cap would still bound it, but skipping keeps
/// resolution productive).
pub(super) fn term_contains(haystack: &StructuralTerm, needle: &StructuralTerm) -> bool {
    if haystack == needle {
        return true;
    }
    match haystack.semantic_value() {
        StructuralTerm::CollectionMeasure {
            collection: subject,
            ..
        }
        | StructuralTerm::Projection { subject, .. }
        | StructuralTerm::IntegerEmbedding { value: subject, .. } => term_contains(subject, needle),
        StructuralTerm::IndexedValue {
            collection, index, ..
        } => term_contains(collection, needle) || term_contains(index, needle),
        StructuralTerm::ScalarBinary { left, right, .. }
        | StructuralTerm::MathematicalBinary { left, right, .. } => {
            term_contains(left, needle) || term_contains(right, needle)
        }
        StructuralTerm::Constructor { fields, .. } => {
            fields.iter().any(|(_, value)| term_contains(value, needle))
        }
        StructuralTerm::Application { arguments, .. }
        | StructuralTerm::CallProjection { arguments, .. } => arguments
            .iter()
            .any(|argument| term_contains(argument, needle)),
        _ => false,
    }
}

/// Normalize eligible selected applications to complete closed values through
/// the shared body/environment evaluator. Runtime prefix resolution remains
/// occurrence-based: symbolic or effectful calls gain no replayable identity.
pub(super) fn unfold_constant_applications(
    program: &dyn crate::CheckingInput,
    term: StructuralTerm,
) -> StructuralTerm {
    match term {
        StructuralTerm::IntegerEmbedding { reference, value } => StructuralTerm::IntegerEmbedding {
            reference,
            value: Box::new(unfold_constant_applications(program, *value)),
        },
        StructuralTerm::MathematicalBinary {
            operator,
            left,
            right,
        } => StructuralTerm::MathematicalBinary {
            operator,
            left: Box::new(unfold_constant_applications(program, *left)),
            right: Box::new(unfold_constant_applications(program, *right)),
        },
        StructuralTerm::ObservedValue {
            observed_type,
            value,
            descriptor,
        } => StructuralTerm::ObservedValue {
            observed_type,
            value: Box::new(unfold_constant_applications(program, *value)),
            descriptor: descriptor
                .map(|descriptor| Box::new(unfold_constant_applications(program, *descriptor))),
        },
        StructuralTerm::Application {
            result_type,
            target,
            selections,
            machine,
            arguments,
        } => {
            let arguments: Vec<_> = arguments
                .into_iter()
                .map(|argument| unfold_constant_applications(program, argument))
                .collect();
            super::StructuralJudge::closed_application_value(
                program,
                result_type,
                target,
                &selections,
                &arguments,
                None,
            )
            .unwrap_or(StructuralTerm::Application {
                result_type,
                target,
                selections,
                machine,
                arguments,
            })
        }
        term @ StructuralTerm::CallProjection { .. } => {
            let term =
                term.map_argument_terms(|argument| unfold_constant_applications(program, argument));
            if let StructuralTerm::CallProjection {
                result_type,
                target,
                selections,
                arguments,
                field,
                ..
            } = &term
            {
                super::StructuralJudge::closed_application_value(
                    program,
                    *result_type,
                    *target,
                    selections,
                    arguments,
                    Some(*field),
                )
                .unwrap_or(term)
            } else {
                unreachable!()
            }
        }
        StructuralTerm::Constructor { data, case, fields } => StructuralTerm::Constructor {
            data,
            case,
            fields: fields
                .into_iter()
                .map(|(name, value)| (name, unfold_constant_applications(program, value)))
                .collect(),
        },
        StructuralTerm::ScalarBinary {
            references,
            operator,
            meaning,
            left,
            right,
        } => StructuralTerm::ScalarBinary {
            references,
            operator,
            meaning,
            left: Box::new(unfold_constant_applications(program, *left)),
            right: Box::new(unfold_constant_applications(program, *right)),
        },
        StructuralTerm::IndexedValue {
            element_type,
            element_identity,
            collection,
            index,
        } => StructuralTerm::IndexedValue {
            element_type,
            element_identity,
            collection: Box::new(unfold_constant_applications(program, *collection)),
            index: Box::new(unfold_constant_applications(program, *index)),
        },
        StructuralTerm::CollectionMeasure {
            measure,
            collection,
        } => StructuralTerm::CollectionMeasure {
            measure,
            collection: Box::new(
                unfold_constant_applications(program, *collection).into_collection_descriptor(),
            ),
        },
        StructuralTerm::Projection { subject, path } => StructuralTerm::Projection {
            subject: Box::new(unfold_constant_applications(program, *subject)),
            path,
        },
        other => other,
    }
}

/// The authored spelling of a term for a diagnostic. Identity stays with the
/// term itself; this text only tells a reader which values the judgment
/// compared, in the source grammar they wrote.
pub(super) fn term_spelling(program: &dyn crate::CheckingInput, term: &StructuralTerm) -> String {
    let arguments = |arguments: &[StructuralTerm]| {
        arguments
            .iter()
            .map(|argument| term_spelling(program, argument))
            .collect::<Vec<_>>()
            .join(", ")
    };
    match term {
        StructuralTerm::Variable(name) => name.clone(),
        StructuralTerm::ObservedValue { value, .. } => term_spelling(program, value),
        StructuralTerm::BoundValue(symbol) => program.symbols.name(*symbol).to_owned(),
        StructuralTerm::CapturedValue { expression, .. } => {
            program.expression_table.display_name(*expression)
        }
        StructuralTerm::BoundProjection { subject, path } => {
            format!("{}.{path}", program.symbols.name(*subject))
        }
        StructuralTerm::Integer(value) => value.to_string(),
        StructuralTerm::Float(value) => value.to_string(),
        StructuralTerm::Constructor { data, case, fields } => {
            let head = if case.is_empty() || data == "bool" {
                if data == "bool" {
                    case.clone()
                } else {
                    data.clone()
                }
            } else {
                format!("{data}::{case}")
            };
            if fields.is_empty() {
                return head;
            }
            let fields = fields
                .iter()
                .map(|(name, value)| format!("{name}: {}", term_spelling(program, value)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{head} {{ {fields} }}")
        }
        StructuralTerm::Application {
            machine,
            arguments: values,
            ..
        } => format!("{machine}({})", arguments(values)),
        StructuralTerm::CallProjection {
            machine,
            field_name,
            arguments: values,
            ..
        } => format!("{machine}({}).{field_name}", arguments(values)),
        StructuralTerm::CollectionMeasure {
            measure,
            collection,
        } => format!(
            "{}.{}",
            term_spelling(program, collection),
            measure.authored_spelling(),
        ),
        StructuralTerm::Projection { subject, path } => {
            format!("{}.{path}", term_spelling(program, subject))
        }
        StructuralTerm::IndexedValue {
            collection, index, ..
        } => format!(
            "{}[{}]",
            term_spelling(program, collection),
            term_spelling(program, index)
        ),
        StructuralTerm::IntegerEmbedding { value, .. } => {
            format!("embed({})", term_spelling(program, value))
        }
        StructuralTerm::MathematicalBinary {
            operator,
            left,
            right,
        }
        | StructuralTerm::ScalarBinary {
            operator,
            left,
            right,
            ..
        } => format!(
            "{} {} {}",
            term_spelling(program, left),
            operator.display_name(),
            term_spelling(program, right)
        ),
        StructuralTerm::Opaque(display) => display.clone(),
    }
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
            .map(|(_, _, selected)| selected.path.joined_path())
            .unwrap_or(name)
    };
    let target = substitute(target.to_owned());
    if machine_arguments.is_empty() {
        return target;
    }
    let selected: Vec<String> = machine_arguments
        .iter()
        .map(|argument| {
            let name = argument.path.joined_path();
            substitute(name)
        })
        .collect();
    format!("{target}<{}>", selected.join(","))
}

/// A call's retained target can name a machine, its entry state or a public
/// requirement surface. Structural applications use the same exact public
/// contract owner as guarantee import. Symbolic selected-machine parameters
/// keep their retained handle when no concrete contract can be projected.
pub(super) fn structural_call_target(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> symbols::SymbolHandle {
    super::specification_calls::selected_call_contract(program, call)
        .map(|owner| super::specification_calls::owner_symbol(&owner))
        .unwrap_or(call.target_symbol)
}

/// Read an expression as a structural term. Single-segment names are
/// variables; a resolved payload-free case path constructs its common fields.
/// Record and case literals retain complete sorted fields, including established
/// runtime defaults; unsupported expressions stay opaque or fail closed.
/// Without a declaring frame, selected applications retain absent result-type
/// authority. This route serves closed literals and structural shape matching;
/// semantic premises/goals must use the state or parameter-frame entry point.

/// Both operands of `binary` read as structural terms, or `None` when
/// either stays opaque — the pair every equality/ordering consumer opens.
pub(super) fn structural_term_pair(
    program: &dyn crate::CheckingInput,
    binary: &typed_trees::expression::TableBinaryExpression,
) -> Option<(StructuralTerm, StructuralTerm)> {
    let left = structural_term(program, None, None, binary.left, None)?;
    let right = structural_term(program, None, None, binary.right, None)?;
    let references = [binary.left, binary.right].map(|expression| {
        crate::proof_contracts::proof_embeddings::expression_type_reference(program, expression)
            .unwrap_or_default()
    });
    (scalar_operand_is_observed(program, references[0], &left)
        && scalar_operand_is_observed(program, references[1], &right))
    .then_some((left, right))
}

/// Bind both sides in the declaration's own telescope before substituting
/// actual values. Applications retain the selected result carrier and policy.
pub(super) fn structural_term_pair_for_parameters(
    program: &dyn crate::CheckingInput,
    owner: symbols::SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    binary: &typed_trees::expression::TableBinaryExpression,
) -> Option<(StructuralTerm, StructuralTerm)> {
    let left = structural_term(program, None, Some((owner, parameters)), binary.left, None)?;
    let right = structural_term(program, None, Some((owner, parameters)), binary.right, None)?;
    let references = [binary.left, binary.right].map(|expression| {
        crate::parameter_expression_result_type_reference(program, owner, parameters, expression)
            .unwrap_or_default()
    });
    (scalar_operand_is_observed(program, references[0], &left)
        && scalar_operand_is_observed(program, references[1], &right))
    .then_some((left, right))
}

/// `structural_term` with an explicit declaring `(machine, state)` fallback for
/// name-free binary operands and selected application result references. The
/// runtime state binds actual source storage; abstract declaration telescopes
/// use the parameter-frame form of `structural_term` and never borrow a caller's state.

/// Compare selected scalar value operations in one exact declaration scope.
/// Symbol leaves stay exact even though the specification equation vocabulary
/// also supports named binders. Unsupported/opaque observations grant no
/// congruence; this query applies no arithmetic or float equality law.
pub(crate) fn source_operations_have_same_meaning(
    program: &dyn crate::CheckingInput,
    owner: symbols::SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    let bindings = |expression| {
        let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
            return None;
        };
        if path.symbol.is_valid()
            && ((program
                .expression_table
                .name_path_members(path.members)
                .len()
                == 1
                && parameters
                    .iter()
                    .any(|parameter| parameter.symbol == path.symbol))
                || program.symbols.get(path.symbol).kind == symbols::SymbolKind::Const)
        {
            Some(StructuralTerm::BoundValue(path.symbol))
        } else {
            None
        }
    };
    fn complete(term: &StructuralTerm) -> bool {
        match term {
            StructuralTerm::BoundValue(symbol) => symbol.is_valid(),
            StructuralTerm::Integer(_) | StructuralTerm::Float(_) => true,
            StructuralTerm::ScalarBinary { left, right, .. }
            | StructuralTerm::MathematicalBinary { left, right, .. } => {
                complete(left) && complete(right)
            }
            StructuralTerm::IntegerEmbedding { value, .. } => complete(value),
            _ => false,
        }
    }
    let form = |expression| {
        structural_term(
            program,
            None,
            Some((owner, parameters)),
            expression,
            Some(&bindings),
        )
    };
    match (form(left), form(right)) {
        (Some(left), Some(right)) => complete(&left) && complete(&right) && left == right,
        _ => false,
    }
}

pub(super) fn structural_term(
    program: &dyn crate::CheckingInput,
    scope: Option<(
        &typed_trees::machine::Machine,
        &typed_trees::state::State,
        typed_trees::types::TypeReferenceHandle,
    )>,
    frame: Option<(
        symbols::SymbolHandle,
        &[typed_trees::signature::StateParameter],
    )>,
    expression: ExpressionHandle,
    bindings: Option<&dyn Fn(ExpressionHandle) -> Option<StructuralTerm>>,
) -> Option<StructuralTerm> {
    if let Some(term) = bindings.and_then(|bindings| bindings(expression)) {
        return Some(term);
    }
    if let Some((_, argument)) =
        crate::proof_contracts::proof_embeddings::integer_embedding_argument(program, expression)
    {
        return integer_embedding_term(
            program,
            expression,
            structural_term(program, scope, frame, argument, bindings)?,
        );
    }
    if let Some(value) = super::proof_integer::anonymous_integer_value(program, expression) {
        return Some(StructuralTerm::Integer(value));
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            if program.symbols.builtin_type_member(path.symbol)
                == Some(symbols::BuiltinTypeMember::AddressBound)
            {
                return Some(StructuralTerm::BoundValue(path.symbol));
            }
            let members = program.expression_table.name_path_members(path.members);
            match members {
                [single] => {
                    // Immutability preserves the stored value, not the source
                    // observations used to construct it. Runtime contracts
                    // consume retained flow captures rather than replaying an
                    // initializer at the use point.
                    Some(StructuralTerm::Variable(single.as_str().to_owned()))
                }
                [_, _, ..] if case_classifier(program, expression).is_some() => {
                    case_value_term(program, expression)
                }
                _ if path.symbol.is_valid()
                    && program.symbols.get(path.symbol).kind == symbols::SymbolKind::Const =>
                {
                    Some(StructuralTerm::BoundValue(path.symbol))
                }
                _ => Some(StructuralTerm::Opaque(
                    program.expression_table.display_name(expression),
                )),
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            constructor_literal_term(program, literal, |field| {
                structural_term(program, scope, frame, field.value, bindings)
            })
        }
        ExpressionNode::Binary(binary) => binary_term_in_scope(
            program,
            scope,
            frame,
            expression,
            structural_term(program, scope, frame, binary.left, bindings)?,
            structural_term(program, scope, frame, binary.right, bindings)?,
        ),
        ExpressionNode::Unary(unary) => logical_not_term_in_scope(
            program,
            scope,
            frame,
            expression,
            structural_term(program, scope, frame, unary.operand, bindings)?,
        ),
        ExpressionNode::Integer(value) => value.value_bignum().map(StructuralTerm::Integer),
        ExpressionNode::Cast(cast) => {
            let source = if let Some((owner, parameters)) = frame {
                crate::parameter_expression_result_type_reference(
                    program, owner, parameters, cast.value,
                )
            } else {
                scope.and_then(|(machine, state, _)| {
                    crate::expression_result_type_reference(program, machine, state, cast.value)
                })
            }?;
            denotation_preserving_conversion(program, cast, source)
                .then(|| structural_term(program, scope, frame, cast.value, bindings))
                .flatten()
        }
        // A determinate float literal carries its exact decimal spelling; the
        // ordering arm widens it once at judgment. UndeterminedNaN stays
        // term-less so a NaN operand can never pose as a decided value.
        ExpressionNode::Float(value) => Some(StructuralTerm::Float(value.clone())),
        ExpressionNode::Call(call) => {
            let target = structural_call_target_in_frame(program, call, frame);
            if (!call.receiver.is_valid() || static_call_receiver_for_target(program, call, target))
                && call.evidence_arguments.is_empty()
                && call.static_requirement_dispatch.is_none()
            {
                let handles = program.expression_table.expression_handles(call.arguments);
                let arguments: Vec<StructuralTerm> = handles
                    .iter()
                    .filter_map(|argument| {
                        structural_term(program, scope, frame, *argument, bindings)
                    })
                    .collect();
                if arguments.len() == handles.len() {
                    let result_type = frame
                        .and_then(|(owner, parameters)| {
                            crate::parameter_expression_result_type_reference(
                                program, owner, parameters, expression,
                            )
                        })
                        .or_else(|| {
                            scope.and_then(|(machine, state, _)| {
                                crate::expression_result_type_reference(
                                    program, machine, state, expression,
                                )
                            })
                        })
                        .unwrap_or_default();
                    return Some(StructuralTerm::Application {
                        result_type,
                        target,
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
            // Contract projections can retain an unresolved member handle even
            // when the receiver's declared record fixes the field. Rejoin that
            // declaration rather than turning the projection into opaque source
            // text: substitution must still be able to reach a returned record.
            let receiver_type = if let Some((owner, parameters)) = frame {
                crate::parameter_expression_result_type_reference(
                    program,
                    owner,
                    parameters,
                    member.receiver,
                )
            } else {
                scope.and_then(|(machine, state, _)| {
                    crate::expression_result_type_reference(
                        program,
                        machine,
                        state,
                        member.receiver,
                    )
                })
            }
            .or_else(|| {
                // Declaration-wide conformance checks have no invocation
                // frame. The reserved result still has an exact authored
                // owner; recover its carrier rather than losing a functional
                // postcondition's projections to opaque display text.
                crate::reserved_result_place(program, member.receiver)
                    .map(|place| place.type_reference)
            });
            if let Some(reference) = receiver_type
                && let Some(measure) = collection_measure_term(program, member, reference, || {
                    structural_term(program, scope, frame, member.receiver, bindings)
                })
            {
                return Some(measure);
            }
            if let Some(call) = program.expression_table.call_node(member.receiver)
                && (!call.receiver.is_valid()
                    || static_call_receiver_for_target(
                        program,
                        call,
                        structural_call_target_in_frame(program, call, frame),
                    ))
                && call.evidence_arguments.is_empty()
                && call.static_requirement_dispatch.is_none()
                && program.state_by_symbol(call.target_symbol).is_some()
            {
                let handles = program.expression_table.expression_handles(call.arguments);
                let arguments = handles
                    .iter()
                    .filter_map(|argument| {
                        structural_term(program, scope, frame, *argument, bindings)
                    })
                    .collect::<Vec<_>>();
                if arguments.len() == handles.len() {
                    let result_type = program
                        .state_by_symbol(call.target_symbol)
                        .map(|state| state.return_type)?;
                    let field =
                        call_projection_field_symbol(program, result_type, member.member.as_str())?;
                    return Some(StructuralTerm::CallProjection {
                        target: structural_call_target_in_frame(program, call, frame),
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
            let declared_field = receiver_type
                .and_then(|reference| {
                    let data =
                        crate::value_custody::places::data_definition_for_type(program, reference)?;
                    crate::value_custody::places::exact_data_member_field(
                        program,
                        data,
                        member.member_symbol,
                        member.member.as_str(),
                        member.case_variant.as_ref().map(|case| case.as_str()),
                    )
                })
                .is_some();
            // A field read off a value receiver: when the receiver termifies
            // to a closed constructor (a struct literal, an immutable local
            // tracing to one, or a nested projection already resolved), the
            // field term is recoverable by name. Ordinary symbolic fields retain the same
            // vocabulary as callee_term and citation intake.
            match structural_term(program, scope, frame, member.receiver, bindings) {
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
                    subject @ (StructuralTerm::Variable(_)
                    | StructuralTerm::Projection { .. }
                    | StructuralTerm::Application { .. }
                    | StructuralTerm::CapturedValue { .. }
                    | StructuralTerm::CallProjection { .. }
                    | StructuralTerm::IndexedValue { .. }),
                ) if declared_field && !term_is_unobserved_descriptor(program, &subject) => {
                    Some(StructuralTerm::Projection {
                        subject: Box::new(subject),
                        path: member.member.as_str().to_owned(),
                    })
                }
                _ => Some(StructuralTerm::Opaque(
                    program.expression_table.display_name(expression),
                )),
            }
        }
        ExpressionNode::Indexed(indexed) => indexed_term_in_scope(
            program,
            scope,
            frame,
            expression,
            structural_term(program, scope, frame, indexed.collection, bindings)?,
            structural_term(program, scope, frame, indexed.index, bindings)?,
        ),
        ExpressionNode::ZeroValue(type_reference) => {
            zero_value_structural_term(program, *type_reference)
        }
        // A borrow's referent is the bound place itself — fact
        // instantiation strips it the same way (`&mut main.control`
        // instantiates to `main.control`), so the term names the
        // referent's storage and a `.len` over it folds to the declared
        // extent.
        ExpressionNode::Borrow(borrow) => {
            structural_term(program, scope, frame, borrow.target, bindings)
        }
        _ => None,
    }
}

/// Rejoin the namespace to the exact selected public declaration. Source
/// qualifiers may still lack an early namespace symbol; declaration kinds are
/// not sufficient authority and an instance receiver stays observable.
pub(super) fn static_call_receiver(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    static_call_receiver_for_target(program, call, structural_call_target(program, call))
}

fn static_call_receiver_for_target(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
    target: symbols::SymbolHandle,
) -> bool {
    // Invocation targets and public contract owners can differ. Keep the
    // exact selected invocation's namespace authority (including its selector
    // telescope) before querying the normalized declaration used by terms.
    if program.call_has_no_runtime_receiver_for_target(call) {
        return true;
    }
    let mut selected = call.clone();
    selected.target_symbol = target;
    program.call_has_no_runtime_receiver_for_target(&selected)
}

fn structural_call_target_in_frame(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
    frame: Option<(
        symbols::SymbolHandle,
        &[typed_trees::signature::StateParameter],
    )>,
) -> symbols::SymbolHandle {
    frame
        .and_then(|(owner, parameters)| {
            super::specification_calls::selected_call_contract_for_parameters(
                program, call, owner, parameters,
            )
        })
        .map(|owner| super::specification_calls::owner_symbol(&owner))
        .unwrap_or_else(|| structural_call_target(program, call))
}

fn call_projection_field_symbol(
    program: &dyn crate::CheckingInput,
    result_type: typed_trees::types::TypeReferenceHandle,
    field_name: &str,
) -> Option<symbols::SymbolHandle> {
    let data = crate::value_custody::places::data_definition_for_type(program, result_type)?;
    program
        .data_field_named(data, field_name)
        .map(|field| field.symbol)
}

/// Rejoin the exact selected entry. Names never recover an absent selection.
pub(super) fn selected_application_machine(
    program: &dyn crate::CheckingInput,
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
