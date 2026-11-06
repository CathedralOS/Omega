//! A scalar loan returns the original source's complete predicate.
//!
//! Invocation subjects retain the performed reference's representation and
//! physical origin. The ordinary normal-call owner applies checked guarantees
//! to fresh output cells, after all argument effects and before later siblings.
//! Handback consumes that current storage state; it neither reprojects callee
//! contracts into unrelated intervals nor assumes the goal as an entry fact.

use super::captured_predicates::{CapturedPredicatePoint, captured_data_predicate_holds};
use super::specification_calls::{
    call_parameter_scope, owner_leading_operands, selected_call_site_contract,
};
use crate::{CallFrameResolver, CallSite, CanonicalPlace};
use diagnostics::Diagnostic;
use facts::PlaceSegment;
use typed_trees::{machine::Machine, state::State};

pub(crate) fn validate_scalar_loan_handbacks(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    prepared_frames: Option<&CallFrameResolver<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let local_frames = prepared_frames
        .is_none()
        .then(|| CallFrameResolver::new(program))
        .flatten();
    let frames = prepared_frames.or(local_frames.as_ref());
    let Some(statement) = program.statement_table.statement_at(state, statement_index) else {
        return;
    };
    for site in crate::collect_call_sites_in_statement(program, machine, state, statement_index) {
        let Some(owner) = selected_call_site_contract(program, machine.symbol, site) else {
            continue;
        };
        let Some(parameters) = call_parameter_scope(program, machine.symbol, &owner, site) else {
            continue;
        };
        if !parameters.iter().any(|parameter| {
            crate::reference_shell(program, parameter.type_reference).is_some_and(|shell| {
                shell.access.is_exclusive()
                    && program.primitive_type_reference(shell.referee).is_some()
            })
        }) {
            continue;
        }
        let leading = owner_leading_operands(&owner);
        let Some(arguments) = crate::call_site_argument_expressions(program, &site).get(leading..)
        else {
            continue;
        };
        if arguments.len() != parameters.len() {
            continue;
        }
        let point = match site {
            CallSite::Statement(_) => CapturedPredicatePoint::StatementReturn(statement_index),
            CallSite::Expression { expression, .. } => CapturedPredicatePoint::ExpressionReturn {
                statement_index,
                expression,
            },
            // Named local control edges are not normal-return invocations.
            // Callable terminal targets use the normalized expression owner.
            CallSite::TransitionNamed { .. } => continue,
        };
        let origins = frames.and_then(|frames| match site {
            CallSite::Statement(call) => frames.call_write_origins(machine, call),
            CallSite::Expression { expression, .. } => {
                frames.expression_call_write_origins(machine, expression)
            }
            CallSite::TransitionNamed { .. } => None,
        });
        let mut judged_parents = Vec::new();
        for (index, (argument, parameter)) in arguments.iter().zip(parameters.iter()).enumerate() {
            let Some(shell) = crate::reference_shell(program, parameter.type_reference) else {
                continue;
            };
            if !shell.access.is_exclusive()
                || program.primitive_type_reference(shell.referee).is_none()
            {
                continue;
            }
            let sources = origins
                .as_ref()
                .and_then(|origins| origins.arguments.get(index + leading))
                .and_then(|sources| sources.as_ref());
            let mut places: Vec<CanonicalPlace> = sources
                .into_iter()
                .flatten()
                .filter_map(|source| {
                    source
                        .scalar_storage_coordinate(program, state)
                        .map(|scalar| scalar.place)
                        .or_else(|| {
                            source.root.is_valid().then(|| CanonicalPlace {
                                root: facts::PlaceRoot::Symbol(source.root),
                                segments: source.segments.clone(),
                            })
                        })
                })
                .collect();
            // An unavailable interpretation cannot erase an already identified
            // source obligation. The direct coordinate still owes proof.
            if places.is_empty() {
                let target = program
                    .expression_table
                    .borrow_expression(*argument)
                    .map_or(*argument, |borrow| borrow.target);
                if let Some(mut place) = crate::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    target,
                ) {
                    if place.segments.is_empty()
                        && let Some(root) = place.root.as_symbol()
                        && let Some((root, segments)) = frames.and_then(|frames| {
                            frames.local_reference_origin_before_statement(machine, statement, root)
                        })
                    {
                        place.root = facts::PlaceRoot::Symbol(root);
                        place.segments = segments;
                    }
                    places.push(place);
                }
            }
            for mut parent in places {
                let Some(PlaceSegment::Field { symbol }) = parent.segments.pop() else {
                    continue;
                };
                let Some((definition, field)) = program.any_field_owner(symbol) else {
                    continue;
                };
                let variant = program.data_members(definition).iter().find_map(|member| {
                    let typed_trees::data::DataMember::Variant(variant) = member else {
                        return None;
                    };
                    program
                        .data_payload_fields(variant)
                        .iter()
                        .any(|field| field.symbol == symbol)
                        .then_some(variant)
                });
                if crate::declarations::data_queries::data_field_is_where_independent(
                    program, definition, field,
                ) || judged_parents
                    .contains(&(parent.clone(), variant.map(|variant| variant.symbol)))
                {
                    continue;
                }
                judged_parents.push((parent.clone(), variant.map(|variant| variant.symbol)));
                let receiver = crate::value_custody::places::canonical_place_type_reference(
                    program,
                    state.symbol,
                    statement_index,
                    &parent,
                );
                let mut encoded = parent.clone();
                encoded.segments.push(PlaceSegment::Field { symbol });
                let encoding_preserves = variant.is_none()
                    && program
                        .primitive_type_reference(shell.referee)
                        .zip(program.primitive_type_reference(field.type_reference))
                        .is_some_and(|(view, source)| {
                            super::structural_judgment::stored_encoding_preserves_field(
                                program,
                                &encoded,
                                view,
                                source,
                                definition,
                                field.symbol,
                            )
                        });
                let preserved = encoding_preserves
                    || receiver.is_some_and(|receiver| {
                        program
                            .proof_facts
                            .span_or_empty(definition.where_facts)
                            .iter()
                            .chain(variant.into_iter().flat_map(|variant| {
                                program
                                    .proof_facts
                                    .span_or_empty(variant.where_facts)
                                    .iter()
                            }))
                            .all(|fact| {
                                fact.as_expression().is_some_and(|predicate| {
                                    captured_data_predicate_holds(
                                        program, machine, state, point, &parent, receiver,
                                        definition, *predicate,
                                    )
                                })
                            })
                    });
                if !preserved {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` cannot lend restricted field `{}` to `{}`: the selected scalar referent's normal-return guarantees do not preserve the field's standing predicate",
                        machine.name, field.name, program.symbols.name(site.target_symbol(program)),
                    )));
                }
            }
        }
    }
}
