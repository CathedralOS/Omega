//! Lifetimes: declaration-level check that a view-returning machine has an
//! unambiguous source for the returned borrow, and that captured body origins
//! belong to that declaration's lifetime relation. Formation owns current
//! descriptor contents; source spelling cannot waive this relation.
//!
//! Explicit output lifetimes name matching input leaves; elided outputs need
//! one unambiguous input. Type-dependent declarations are templates, not closed
//! loan contracts: selected instances and closed bodyless calls must establish
//! the complete relation. Declaration checks and caller loan attribution share
//! `borrow::view_link` rather than deriving different structural frontiers.

use crate::borrow::returned_body_source_mismatch;
use diagnostics::Diagnostic;

use crate::borrow::view_link::{
    DeclarationLifetimeFrontier, ViewReturnAmbiguity, ViewReturnSource,
    declaration_lifetime_frontier, resolve_applied_signature_view_return_source,
    resolve_signature_view_return_source, resolve_view_return_source,
};
mod templates;

pub(super) fn check_view_return_elision(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut deferred = Vec::new();
    for machine in program.machines() {
        let (scope, _) =
            program.machine_type_reference_scope(machine, |left, right| left.name == right.name);
        let parameters = scope
            .iter()
            .filter(|parameter| {
                matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
            })
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        for state in program.machine_states(machine) {
            if let Some(parameter_index) = returned_body_source_mismatch(program, facts, state) {
                let parameter = &program.state_parameters(state)[parameter_index];
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` returns a view borrowing input `{}`, which is not permitted by the declared result lifetime relation",
                    super::escape::state_subject(machine, state),
                    parameter.name,
                )));
            }
            if frontier_requires_application(
                program,
                state.return_type,
                program.state_parameters(state),
                &parameters,
            ) {
                // Authored generic bodies and bodyless requirements obey the
                // same frontier rule: no raw dependent signature is a closed
                // loan contract. The call fence below admits only complete
                // instantiated result-to-input relations.
                deferred.push(state.symbol);
                continue;
            }
            let ViewReturnSource::Ambiguous(ambiguity) = resolve_view_return_source(program, state)
            else {
                continue;
            };

            // For a free machine the interesting name is the machine's; an
            // inner named state is more precise when it differs.
            let subject = if state.name.as_str() == "entry"
                || machine.name.as_str().ends_with(state.name.as_str())
            {
                machine.name.as_str()
            } else {
                state.name.as_str()
            };

            diagnostics.push(Diagnostic::error(ambiguity_message(subject, &ambiguity)));
        }

        for parameter in program.machine_type_parameters(machine) {
            let typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind else {
                continue;
            };
            let signature = program
                .machine_parameter_contract_view(contract)
                .expect("typed machine-parameter contract must retain a valid requirement identity")
                .signature();
            check_signature_tree(
                program,
                signature,
                &parameters,
                &mut deferred,
                diagnostics,
                &mut Vec::new(),
            );
        }
    }

    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            let parameters = program
                .trait_type_parameters(trait_definition)
                .iter()
                .filter(|parameter| {
                    matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                })
                .map(|parameter| parameter.symbol)
                .collect::<Vec<_>>();
            check_signature_tree(
                program,
                signature,
                &parameters,
                &mut deferred,
                diagnostics,
                &mut Vec::new(),
            );
        }
    }
    templates::check_calls(program, &deferred, diagnostics);
}

fn check_signature_tree(
    program: &dyn ::validation::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
    captured_types: &[symbols::SymbolHandle],
    deferred: &mut Vec<symbols::SymbolHandle>,
    diagnostics: &mut Vec<Diagnostic>,
    active: &mut Vec<symbols::SymbolHandle>,
) {
    // Applied nominal preparation rejects recursive expansion, but an
    // argument-free nominal contract may still point at its raw declaration.
    // Only the active path is recursive: sibling applications need their own
    // captured-scope judgment, not a global symbol-only visited exemption.
    if active.contains(&signature.symbol) {
        diagnostics.push(Diagnostic::error(
            "unimplemented: recursive callable lifetime frontier requires cycle-aware applied telescopes",
        ));
        return;
    }
    active.push(signature.symbol);
    let parameters = program.state_signature_type_parameters(signature);
    let mut types = captured_types.to_vec();
    types.extend(parameters.iter().filter_map(|parameter| {
        matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
            .then_some(parameter.symbol)
    }));
    if !defer_signature_frontier(program, signature, &types, deferred, diagnostics) {
        check_bodyless_signature(program, signature, diagnostics);
    }
    // Every retained contract is checked, not only a top-level callback slot.
    // Descendants capture this telescope before binding their own fresh regions.
    for parameter in parameters {
        if let typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind
            && let Some(contract) = program.machine_parameter_contract_view(contract)
        {
            check_signature_tree(
                program,
                contract.signature(),
                &types,
                deferred,
                diagnostics,
                active,
            );
        }
    }
    active.pop();
}

fn defer_signature_frontier(
    program: &dyn ::validation::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
    type_parameters: &[symbols::SymbolHandle],
    deferred: &mut Vec<symbols::SymbolHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if !frontier_requires_application(
        program,
        signature.return_type,
        program.state_signature_parameters(signature),
        type_parameters,
    ) {
        return false;
    }
    // Deferred structure is not deferred contradiction. Fresh invocation
    // separation can already prove that a shared input cannot supply a mutable
    // result, even while captured Context remains opaque and nobody calls it.
    if let ViewReturnSource::Ambiguous(ambiguity) = resolve_applied_signature_view_return_source(
        program,
        signature,
        &[],
        &[0],
        signature.symbol,
    ) && !matches!(ambiguity, ViewReturnAmbiguity::IncompleteStructure { .. })
    {
        diagnostics.push(Diagnostic::error(ambiguity_message(
            signature.name.as_str(),
            &ambiguity,
        )));
    }
    deferred.push(signature.symbol);
    true
}

fn frontier_requires_application(
    program: &dyn ::validation::CheckingInput,
    result: typed_trees::types::TypeReferenceHandle,
    parameters: &[typed_trees::signature::StateParameter],
    type_parameters: &[symbols::SymbolHandle],
) -> bool {
    let result_frontier = declaration_lifetime_frontier(program, result, type_parameters);
    let mut dependent = match result_frontier {
        DeclarationLifetimeFrontier::Incomplete
        | DeclarationLifetimeFrontier::Complete { has_borrows: false } => return false,
        DeclarationLifetimeFrontier::Complete { has_borrows: true } => false,
        DeclarationLifetimeFrontier::TemplateDependent { .. } => true,
    };
    for parameter in parameters {
        match declaration_lifetime_frontier(program, parameter.type_reference, type_parameters) {
            DeclarationLifetimeFrontier::Incomplete => return false,
            DeclarationLifetimeFrontier::TemplateDependent { .. } => dependent = true,
            DeclarationLifetimeFrontier::Complete { .. } => {}
        }
    }
    // A known reference output still needs every input's applied frontier:
    // an opaque Context may contain additional matching sources. Retain that
    // obligation for the selected call rather than declaring Context view-free
    // or rejecting its authored template as a malformed concrete signature.
    dependent
}

fn check_bodyless_signature(
    program: &dyn ::validation::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ViewReturnSource::Ambiguous(ambiguity) = resolve_signature_view_return_source(
        program,
        program.state_signature_parameters(signature),
        signature.return_type,
    ) else {
        return;
    };

    diagnostics.push(Diagnostic::error(ambiguity_message(
        signature.name.as_str(),
        &ambiguity,
    )));
}

fn ambiguity_message(subject: &str, ambiguity: &ViewReturnAmbiguity) -> String {
    match ambiguity {
        ViewReturnAmbiguity::IncompleteStructure { subject: source } => format!(
            "machine `{subject}` returns a view whose {source} has an incomplete structural lifetime frontier"
        ),
        ViewReturnAmbiguity::IncompatibleSourceAccess { input } => format!(
            "machine `{subject}` returns a view whose access cannot be supplied by matching input `{input}`"
        ),
        ViewReturnAmbiguity::ElidedMultipleInputs { candidates } => format!(
            "machine `{subject}` returns a view but takes {} candidate ref inputs ({}); \
             cannot infer which input the returned view borrows — annotate the return type \
             with the lifetime of the input it borrows, e.g. `-> &'a T` matching an input `&'a T`",
            candidates.len(),
            quote_join(candidates),
        ),
        ViewReturnAmbiguity::LifetimeMatchesNoInput { lifetime } => format!(
            "machine `{subject}` returns a view with lifetime `'{lifetime}`, but no input \
             borrows `'{lifetime}`; annotate the input the view comes from, e.g. \
             `buffer: &'{lifetime} T`",
        ),
    }
}

fn quote_join(names: &[String]) -> String {
    names
        .iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ")
}
