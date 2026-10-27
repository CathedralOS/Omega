//! Complete scalar declarations after their target observations have closed.
//!
//! Readiness preserves the whole expression and its selected dependencies.
//! Omega supplies layout answers under the selected target; it does not supply
//! arithmetic meaning or invocation permission. The ordinary scalar evaluator
//! checks the declared carrier, every authored operator and admitted helper.
//! Only completed declaration values return from the private evaluation forest.

use super::invocations::{CheckedInitializers, append_probe};
use crate::build_time_evaluation::EvaluationDomain;
use crate::const_evaluation::const_generic_expressions::{scalar_probe_destination, value};
use diagnostics::Diagnostic;
use std::sync::Arc;
use typed_trees::TypedTrees;

pub(crate) fn settle(
    typed: &mut TypedTrees,
    pending: &[typed_trees::constant::ConstDeclaration],
    authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
    domain: &EvaluationDomain,
    selected: crate::SelectedBuildTimeOperators<'_>,
) -> Result<(), Vec<Diagnostic>> {
    let mut declarations = typed
        .const_declarations()
        .iter()
        .filter(|declaration| {
            domain.contains(declaration.symbol)
                && declaration.evaluated_value.is_none()
                && pending
                    .iter()
                    .any(|expected| expected.symbol == declaration.symbol)
        })
        .filter_map(|declaration| {
            scalar_probe_destination(typed, declaration.declared_type)
                .map(|destination| (declaration.clone(), destination))
        })
        .collect::<Vec<_>>();
    if declarations.is_empty() {
        return Ok(());
    }
    while !declarations.is_empty() {
        let mut ready = None;
        for (ordinal, (declaration, _)) in declarations.iter().enumerate() {
            let mut probes = typed.clone();
            let owner = append_probe(
                &mut probes,
                declaration.symbol,
                format!("@target-constant-{ordinal}"),
                declaration.materialized_initializer,
                declaration.declared_type,
            );
            let mut work = domain.clone();
            work.include(owner);
            let work = work.demanded_roots(&probes, &[owner]);
            // Source selections, including helper bodies and unexecuted arms,
            // order aliases before consumers. A provisional value is never a
            // canonical dependency, regardless of declaration order in source.
            let references = work.references(&probes);
            let pending_dependency =
                references
                    .authored_selection_occurrences
                    .iter()
                    .any(|occurrence| {
                        probes
                            .authored_declaration_selections()
                            .get(*occurrence)
                            .and_then(|selection| {
                                selection
                                    .target()
                                    .as_resolved()
                                    .map(|selected| selected.selected_symbol())
                            })
                            .and_then(|symbol| probes.const_declaration_by_symbol(symbol))
                            .is_some_and(|dependency| dependency.evaluated_value.is_none())
                    });
            if !pending_dependency {
                ready = Some((ordinal, probes, owner, work));
                break;
            }
        }
        let Some((ordinal, mut probes, owner, work)) = ready else {
            return Err(super::failure(
                declarations[0].0.initializer_source_span,
                "target constant dependency cycle or unavailable canonical input",
            ));
        };
        let (declaration, destination) = declarations.remove(ordinal);
        let failure = |reason: String| super::failure(declaration.initializer_source_span, reason);
        for root in [
            declaration.authored_initializer,
            declaration.materialized_initializer,
        ] {
            if root.is_valid() {
                crate::const_evaluation::const_generic_expressions::retain_current_initializer_calls(
                    &mut probes, owner, root,
                ).map_err(&failure)?;
            }
        }
        // Extending work with a private probe preserves the original execution
        // selection: a Product description in a Build forest grants no body demand.
        let (prepared, _) = crate::const_evaluation::prepared_admission(
            &probes,
            selected.provider_bodies,
            selected.operators,
            authority.clone(),
            &work,
        )?;
        let checked = CheckedInitializers::prepare_selected(
            &prepared,
            authority.clone(),
            &[owner],
            &work.accounting,
            selected.provider_bodies,
        )?;
        let execution = checked.typed();
        let machine = execution
            .machine(owner)
            .ok_or_else(|| failure("target constant lost its private evaluation owner".into()))?;
        let [state] = execution.machine_states(machine) else {
            return Err(failure(
                "target constant lost its private value exit".into(),
            ));
        };
        let calls = checked.calls_for_symbol(owner).map_err(&failure)?;
        let (value, warnings) = value::evaluate_scalar_with_policy(
            execution,
            machine,
            state,
            declaration.materialized_initializer,
            destination,
            execution.arithmetic_domain_for_type_reference(declaration.declared_type),
            Some(&calls),
        )
        .map_err(&failure)?;
        for warning in warnings {
            eprintln!("{warning}");
        }
        // Preserve the source-owned roster, not private specialization symbols.
        // The receiving replay specializes and checks that exact inventory.
        for root in [
            declaration.authored_initializer,
            declaration.materialized_initializer,
        ] {
            if root.is_valid() {
                let occurrences = probes
                    .expression_table
                    .authored_selection_occurrences(root)
                    .collect::<Vec<_>>();
                for occurrence in occurrences {
                    let selection = probes
                        .authored_declaration_selections()
                        .get(occurrence)
                        .ok_or_else(|| {
                            failure("pending initializer lost retained call custody".into())
                        })?;
                    if selection.kind() == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call {
                        let selected = selection.target().as_resolved().ok_or_else(|| failure("pending initializer call lost its source target".into()))?.selected_symbol();
                        let retained = typed.record_resolved_authored_declaration_selection_once(selection.source_span(), selection.exposure(), selection.kind(), selected)
                            .map_err(|reason| failure(format!("pending initializer call custody cannot return: {reason:?}")))?;
                        typed.expression_table.attach_authored_selection_occurrences(root, [retained]);
                    }
                }
            }
        }
        super::completed_values::install(typed, &declaration, &value).map_err(&failure)?;
    }
    Ok(())
}
