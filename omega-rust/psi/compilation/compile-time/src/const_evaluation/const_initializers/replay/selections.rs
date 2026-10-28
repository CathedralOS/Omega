//! Detached initializer selection and its transactional receiving-program handoff.
//!
//! A replay owner supplies ordinary operand context, not selection authority.
//! Close the selected execution dependencies before freezing the replay roster;
//! the source and materialized values still owe every custody and value check.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::references::{DeclarationSelection, collect_selected_program_references};

pub(super) fn prepare(
    typed: &TypedTrees,
    domain: &crate::build_time_evaluation::EvaluationDomain,
) -> Result<(TypedTrees, DeclarationSelection), Vec<Diagnostic>> {
    let mut probe = typed.clone();
    let source_machines = probe.roots.machines;
    let mut declarations = DeclarationSelection::from_symbols(
        typed
            .const_declarations()
            .iter()
            .filter(|declaration| domain.contains(declaration.symbol))
            .map(|declaration| declaration.symbol),
    );
    let mut owners = Vec::new();
    let mut owned = DeclarationSelection::from_symbols([]);
    loop {
        for declaration in typed.const_declarations() {
            if !declarations.contains(declaration.symbol) || owned.contains(declaration.symbol) {
                continue;
            }
            owned.include(declaration.symbol);
            // Invalid roots are diagnosed by the declaration validator. Do not
            // manufacture an expression to give them an evaluation context.
            if !probe
                .expression_table
                .expression_is_valid(declaration.authored_initializer)
            {
                continue;
            }
            if !crate::const_evaluation::const_generic_expressions::initializer_contains_call(
                &probe,
                declaration.authored_initializer,
            )
            .map_err(|reason| super::super::failure(declaration.initializer_source_span, reason))?
            {
                continue;
            }
            owners.push(super::super::invocations::append_probe(
                &mut probe,
                declaration.symbol,
                format!("@const-selection-{}", owners.len()),
                declaration.authored_initializer,
                declaration.declared_type,
            ));
        }
        if !owners.is_empty() {
            while !validation::resolve_named_result_overloads(&mut probe)?.is_empty() {}
        }
        // ROOT -> selected helper -> LATER is an ordinary execution dependency.
        // LATER needs its own detached initializer context before its call can
        // be selected; walking only direct const references misses this edge.
        let execution = (!owners.is_empty()).then(|| {
            crate::syntax_probes::PreparedEvaluationInputs::domain_for_program(&probe, &owners, &[])
        });
        let references = collect_selected_program_references(&probe, &declarations);
        let mut changed = false;
        for declaration in typed.const_declarations() {
            if !declarations.contains(declaration.symbol)
                && (execution
                    .as_ref()
                    .is_some_and(|execution| execution.contains(declaration.symbol))
                    || references.symbols.contains(&declaration.symbol))
            {
                declarations.include(declaration.symbol);
                changed = true;
            }
        }
        if !changed {
            // Selection contexts never become runtime declarations. The caller
            // now supplies the exact scalar/structured replay leaf owners.
            probe.roots.machines = source_machines;
            return Ok((probe, declarations));
        }
    }
}

pub(super) fn publish(
    typed: &mut TypedTrees,
    selected: &TypedTrees,
    declarations: &DeclarationSelection,
) -> Result<(), Vec<Diagnostic>> {
    // Both programs retain the same source arenas. Private specialization is a
    // later program and must never supply handles to this receiving handoff.
    let references = collect_selected_program_references(typed, declarations);
    let mut occurrences = references
        .authored_selection_occurrences
        .into_iter()
        .collect::<Vec<_>>();
    occurrences.sort_unstable_by_key(|occurrence| occurrence.ordinal());
    let mut ledger = typed.authored_declaration_selections().clone();
    for occurrence in occurrences {
        let original = ledger.get(occurrence).copied().ok_or_else(|| {
            super::super::failure(
                source::SourceSpan::default(),
                "receiving initializer lost its authored selection occurrence",
            )
        })?;
        let produced = selected
            .authored_declaration_selections()
            .get(occurrence)
            .ok_or_else(|| {
                super::super::failure(
                    original.source_span(),
                    "replay selection lost its receiving occurrence",
                )
            })?;
        if original == *produced {
            continue;
        }
        let AuthoredDeclarationSelectionTarget::LateBound(binding) = original.target() else {
            return Err(super::super::failure(
                original.source_span(),
                "replay selection conflicts with its receiving declaration selection",
            ));
        };
        let result = match produced.target() {
            AuthoredDeclarationSelectionTarget::Resolved(resolved)
                if typed
                    .symbols
                    .symbols()
                    .nodes()
                    .is_valid(resolved.selected_symbol()) =>
            {
                ledger.finalize_late_bound(occurrence, binding, resolved.selected_symbol())
            }
            AuthoredDeclarationSelectionTarget::Intrinsic(intrinsic) => {
                ledger.finalize_intrinsic(occurrence, binding, intrinsic)
            }
            _ => {
                return Err(super::super::failure(
                    original.source_span(),
                    "replay selection did not retain an exact receiving declaration target",
                ));
            }
        };
        result.map_err(|error| {
            super::super::failure(
                original.source_span(),
                format!("replay declaration selection finalization failed: {error:?}"),
            )
        })?;
        // This also pins kind, exposure, compiler partition and static operand
        // position. Equal ordinal or source text alone is not a custody join.
        if ledger.get(occurrence) != Some(produced) {
            return Err(super::super::failure(
                original.source_span(),
                "replay selection changed its receiving occurrence identity",
            ));
        }
    }

    let mut expressions = references.expressions.into_iter().collect::<Vec<_>>();
    expressions.sort_unstable_by_key(|expression| expression.arena_index());
    let mut targets = Vec::new();
    let mut registrations = Vec::new();
    for expression in expressions {
        let ExpressionNode::Call(original) = typed.expression_table.expression(expression) else {
            continue;
        };
        let ExpressionNode::Call(produced) = selected.expression_table.expression(expression)
        else {
            return Err(super::super::failure(
                typed.expression_table.source_span(expression),
                "replay selection changed its receiving call shape",
            ));
        };
        let mut expected = original.clone();
        expected.target_symbol = produced.target_symbol;
        if expected != *produced
            || (original.target_symbol != produced.target_symbol
                && !typed
                    .symbols
                    .symbols()
                    .nodes()
                    .is_valid(produced.target_symbol))
        {
            return Err(super::super::failure(
                typed.expression_table.source_span(expression),
                "replay call selection changed more than its exact source target",
            ));
        }
        if original.target_symbol != produced.target_symbol {
            targets.push((expression, produced.target_symbol));
        }
        let application = original.authored_machine_application;
        if !application.is_valid() {
            continue;
        }
        let original_application = typed
            .expression_table
            .static_machine_application(application);
        let produced_application = selected
            .expression_table
            .static_machine_application(application);
        let mut expected_application = original_application.clone();
        expected_application.registration_operation = produced_application.registration_operation;
        if expected_application != *produced_application
            || typed
                .expression_table
                .static_machine_application_arguments(application)
                != selected
                    .expression_table
                    .static_machine_application_arguments(application)
            || typed
                .expression_table
                .static_machine_application_selected_machines(application)
                != selected
                    .expression_table
                    .static_machine_application_selected_machines(application)
            || typed
                .expression_table
                .static_machine_application_bindings(application)
                != selected
                    .expression_table
                    .static_machine_application_bindings(application)
            || typed
                .expression_table
                .static_machine_application_lifetimes(application)
                != selected
                    .expression_table
                    .static_machine_application_lifetimes(application)
        {
            return Err(super::super::failure(
                typed.expression_table.source_span(expression),
                "replay selection changed its receiving source application operands",
            ));
        }
        if original_application.registration_operation
            != produced_application.registration_operation
        {
            let registration = produced_application.registration_operation;
            if original_application.registration_operation.is_valid()
                || registration != produced.target_symbol
                || !typed.symbols.symbols().nodes().is_valid(registration)
            {
                return Err(super::super::failure(
                    typed.expression_table.source_span(expression),
                    "replay selection changed its receiving source application registration",
                ));
            }
            registrations.push((application, registration));
        }
    }
    // Selection and its typed call targets publish together. No error path can
    // leave a partially finalized receiver or donate a private probe symbol.
    for (expression, target) in targets {
        if let ExpressionNode::Call(call) = typed.expression_table.expression_mut(expression) {
            call.target_symbol = target;
        }
    }
    for (application, registration) in registrations {
        typed
            .expression_table
            .static_machine_application_mut(application)
            .registration_operation = registration;
    }
    typed.retain_authored_declaration_selections(ledger);
    Ok(())
}
