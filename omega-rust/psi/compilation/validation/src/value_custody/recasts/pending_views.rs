//! Initial establishment of a target view is separate from borrow formation.
//!
//! Only initialized, unrestricted source bits can currently open this window.
//! Field validity, physical geometry and exclusive access are immediate; the
//! record predicate is proved over current contents at every valid-T boundary.
//! Original padding stays occupied source storage, never invented T fields.

use super::record_representation::{
    fields_cover_source_extent, mutable_record_representations_equivalent,
    pending_record_definition, pending_record_fields, source_supports_pending_fields,
    type_representation,
};
use diagnostics::Diagnostic;
use typed_trees::data::DataDefinition;
use typed_trees::expression::{ExpressionHandle, TableCastExpression};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

pub(super) struct PendingView<'program> {
    pub(super) definition: &'program DataDefinition,
    pub(super) fields_cover_source: bool,
}

/// This relation grants no reference permission. Formation checks access,
/// footprint and lifetimes independently before using it.
pub(super) fn pending_view<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    cast: &TableCastExpression,
) -> Option<PendingView<'program>> {
    if cast.form.recast_access() != Some(language_core::ReferenceAccess::Mutable) {
        return None;
    }
    let (definition, fields) = pending_record_fields(program, cast.target_type)?;
    let source = super::recast_judgments::strip_mutable(program, cast.value);
    let source =
        super::source_regions::source_representation(program, machine, state, source).ok()?;
    // Existing fully compatible views retain their ordinary valid-T contract.
    if type_representation(program, cast.target_type)
        .is_some_and(|target| mutable_record_representations_equivalent(program, &source, &target))
    {
        return None;
    }
    source_supports_pending_fields(program, &source, &fields).then_some(PendingView {
        definition,
        fields_cover_source: fields_cover_source_extent(&fields),
    })
}

/// Classify obligations across the full retained layout family. Failure to
/// recover geometry cannot erase a semantic pending predicate or prove that
/// target fields account for every initialized source bit.
fn formation_obligation<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    cast: &TableCastExpression,
) -> Option<PendingView<'program>> {
    if cast.form.recast_access() != Some(language_core::ReferenceAccess::Mutable) {
        return None;
    }
    let definition = pending_record_definition(program, cast.target_type)?;
    let source = super::recast_judgments::strip_mutable(program, cast.value);
    let roots = super::source_regions::source_layout_root(program, machine, state, source)
        .into_iter()
        .chain([cast.target_type]);
    let mut established = true;
    let mut fields_cover_source = true;
    match crate::layout_checking_targets(program, roots) {
        Ok(targets) if !targets.is_empty() => {
            for target in targets {
                let alternative = crate::LayoutCheckingContext::new(program, target);
                let source = super::source_regions::source_representation(
                    &alternative,
                    machine,
                    state,
                    source,
                )
                .ok();
                let target = type_representation(&alternative, cast.target_type);
                established &=
                    source
                        .as_ref()
                        .zip(target.as_ref())
                        .is_some_and(|(source, target)| {
                            mutable_record_representations_equivalent(&alternative, source, target)
                        });
                fields_cover_source &= pending_record_fields(&alternative, cast.target_type)
                    .is_some_and(|(_, fields)| fields_cover_source_extent(&fields));
            }
        }
        _ => {
            established = false;
            fields_cover_source = false;
        }
    }
    (!established).then_some(PendingView {
        definition,
        fields_cover_source,
    })
}

/// Declaration ownership of an exact formation occurrence. A checked callee's
/// returned reference has already crossed its validity boundary; its original
/// body must not be reinterpreted under the caller's premises.
fn formation_owner(
    program: &dyn crate::CheckingInput,
    formation: ExpressionHandle,
) -> Option<(&Machine, &State)> {
    let mut owners =
        program.machines().iter().flat_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .filter_map(move |state| {
                    program
                        .statement_table
                        .statements(state.statement_nodes)
                        .iter()
                        .any(|statement| {
                            program.tables.expression_roots(statement).into_iter().any(
                                |(_, root)| {
                                    root.is_valid()
                                        && crate::expression_owners::expression_contains(
                                            program, root, formation,
                                        )
                                },
                            )
                        })
                        .then_some((machine, state))
                })
        });
    let owner = owners.next()?;
    owners.next().is_none().then_some(owner)
}

/// Incoming values and checked call results are already valid. Only a local
/// exclusive recast can introduce this initial pending phase; every ordinary
/// boundary out of its body is checked below. This is a producer inventory,
/// not a validity judgment inferred from an annotation or unknown origins.
fn has_pending_formation(program: &dyn crate::CheckingInput, machine: &Machine) -> bool {
    program.machine_states(machine).iter().any(|state| {
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|statement| {
                let mut roots = Vec::new();
                super::statement_expression_roots(program, statement, &mut roots);
                crate::queries::expression_traversal::visit_pre_order(program, roots, |_, node| {
                    matches!(node, typed_trees::expression::ExpressionNode::Cast(cast)
                    if cast.form.recast_access() == Some(language_core::ReferenceAccess::Mutable)
                        && pending_record_definition(program, cast.target_type).is_some())
                })
            })
    })
}

fn pending_origins<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    index: usize,
    expression: ExpressionHandle,
) -> Result<Vec<PendingView<'program>>, &'static str> {
    if !has_pending_formation(program, machine) {
        return Ok(Vec::new());
    }
    program
        .statement_table
        .statements(state.statement_nodes)
        .get(index)
        .ok_or("reference operand has no owning statement")?;
    // Capture this exact operand, including its selected match-arm bindings
    // and earlier sibling effects. A statement-prefix inventory omits those
    // bindings and cannot stand in for a completed reference descriptor.
    let sources = crate::CallFrameResolver::new(program)
        .ok_or("reference interpretation resolver is unavailable")?
        .completed_reference_interpretation_sources(machine, state, expression)?;
    let mut obligations = Vec::new();
    for recast in sources.iter().flat_map(|source| &source.recasts) {
        if pending_record_definition(program, recast.target_type).is_none() {
            continue;
        }
        let formation = recast.formation.expression();
        // A missing or ambiguous issuer cannot turn a pending local into an
        // admitted value. Only an exact separately checked body is external.
        let (owner_machine, owner_state) =
            formation_owner(program, formation).ok_or("recast issuer has no unique body owner")?;
        if owner_machine.symbol != machine.symbol {
            continue;
        }
        // Origins retain the complete authored Borrow wrapper. Only the
        // syntax lookup peels it; issuer identity and access remain original.
        let operand = super::recast_judgments::strip_mutable(program, formation);
        let cast = program
            .expression_table
            .cast_node(operand)
            .ok_or("recast issuer has no cast operand")?;
        if let Some(obligation) = formation_obligation(program, machine, owner_state, cast) {
            obligations.push(obligation);
        }
    }
    Ok(obligations)
}

/// A pending view never contributes its own default predicate as a standing
/// premise. Explicit guards and completed stores remain available at their
/// actual occurrence through the prefix observer.
pub(crate) fn withholds_default_predicate(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    let reference = crate::expression_result_type_reference(program, machine, state, expression);
    if reference
        .and_then(|reference| pending_record_definition(program, reference))
        .is_none()
    {
        return false;
    }
    let index = program
        .statement_table
        .indexed_statements(state.statement_nodes)
        .find_map(|(index, statement)| {
            program
                .tables
                .expression_roots(statement)
                .into_iter()
                .any(|(_, root)| {
                    root.is_valid()
                        && crate::expression_owners::expression_contains(program, root, expression)
                })
                .then_some(index)
        });
    index.is_some_and(|index| {
        pending_origins(program, machine, state, index, expression)
            .map_or(true, |origins| !origins.is_empty())
    })
}

/// Calls, returns, construction and owned copies consume valid values. Local
/// reference bindings may retain a pending descriptor but cannot discharge it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn report_consumption(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    index: usize,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
    context: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let Some(actual) = crate::expression_result_type_reference(program, machine, state, expression)
    else {
        return false;
    };
    // Scalar field reads have independent validity. They do not consume the
    // enclosing record or require its coupled root predicate.
    // An owned result has already crossed its own construction/copy boundary.
    // Loading T through a pending reference still has that reference as its
    // authored operand and is checked before the copy completes.
    if crate::reference_shell(program, actual).is_none() {
        return false;
    }
    if pending_record_definition(program, actual).is_none() {
        return false;
    }
    let views = match pending_origins(program, machine, state, index, expression) {
        Ok(views) => views,
        Err(reason) => {
            diagnostics.push(
                Diagnostic::error(format!("{context}: unestablished recast target: {reason}"))
                    .with_source_span(program.expression_table.source_span(expression)),
            );
            return true;
        }
    };
    if views.is_empty() {
        return false;
    }
    let writable =
        crate::reference_shell(program, expected).is_some_and(|shell| shell.access.is_exclusive());
    let place = crate::value_custody::places::canonical_place_from_expression(program, expression);
    for view in views {
        if writable && !view.fields_cover_source {
            diagnostics.push(Diagnostic::error(format!(
                "{context}: mutable recast target escape has no complete source-storage handback; target padding is not initialized field storage"
            )).with_source_span(program.expression_table.source_span(expression)));
            return true;
        }
        let established = place.as_ref().is_some_and(|place| {
            program
                .proof_facts
                .span_or_empty(view.definition.where_facts)
                .iter()
                .all(|fact| {
                    fact.as_expression().is_some_and(|predicate| {
                        crate::proof_contracts::contract_entailment::completed_data_predicate_holds(
                            program,
                            machine,
                            state,
                            index,
                            expression,
                            place,
                            view.definition,
                            *predicate,
                        )
                    })
                })
        });
        if !established {
            diagnostics.push(Diagnostic::error(format!(
                "{context}: unestablished recast target `{}`; prove its predicate over current storage before consuming a valid value",
                view.definition.name,
            )).with_source_span(program.expression_table.source_span(expression)));
            return true;
        }
    }
    false
}

/// A whole-record store can overwrite source bytes outside T's field roster.
/// Scalar stores keep their exact occupied source coordinates instead.
pub(crate) fn report_whole_view_write(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    index: usize,
    target: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(reference) = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        target,
    ) else {
        return;
    };
    if pending_record_definition(program, reference).is_none() {
        return;
    }
    if pending_origins(program, machine, state, index, target).map_or(true, |views| {
        views.iter().any(|view| !view.fields_cover_source)
    }) {
        diagnostics.push(Diagnostic::error(
            "whole recast target replacement cannot establish initialized source padding for handback"
        ).with_source_span(program.expression_table.source_span(target)));
    }
}

/// Ordinary receiving owners check calls, constructors and statement results.
/// Other value operands (including collection elements and comparisons) must
/// not materialize a valid record from a pending descriptor either. Walk the
/// existing executable occurrence inventory; place receivers are not values.
pub(super) fn validate_operand_consumptions(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    index: usize,
    statement: &typed_trees::statement::StatementNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use typed_trees::expression::ExpressionNode;
    crate::visit_statement_value_occurrences(
        program,
        machine.symbol,
        state.symbol,
        index,
        statement,
        &mut |expression, origin, _| {
            let checked_trees::CheckedValueOrigin::NestedExpression { parent } = origin else {
                return;
            };
            let parent = program.expression_table.expression(parent);
            let place_or_received = match parent {
                ExpressionNode::Borrow(_) | ExpressionNode::Member(_) => true,
                ExpressionNode::Indexed(indexed) => indexed.collection == expression,
                ExpressionNode::Call(call) => call.receiver != expression,
                ExpressionNode::StructLiteral(_) => true,
                _ => false,
            };
            if place_or_received {
                return;
            }
            let Some(reference) =
                crate::expression_result_type_reference(program, machine, state, expression)
            else {
                return;
            };
            let expected = if matches!(parent, ExpressionNode::Cast(_)) {
                // Re-viewing reads the previous interpretation. Its resulting
                // permission is judged independently by recast formation.
                crate::value_custody::places::assignment_value_type(program, reference)
            } else {
                reference
            };
            report_consumption(
                program,
                machine,
                state,
                index,
                expression,
                expected,
                "value operand",
                diagnostics,
            );
        },
    );
}
