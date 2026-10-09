//! Lifetimes: declaration-level check that a view-returning machine has an
//! unambiguous source for the returned borrow, and that reconstructed body
//! origins belong to that declaration's lifetime relation. Unknown body
//! reconstruction is not confused with a known contradictory source.
//!
//! Stage 1 (elision) links a `&self` method or a single-ref-input machine to its
//! one obvious source. Stage 2 (frozen decision 15) adds EXPLICIT lifetimes: a
//! `-> &'buf T` output names the input it borrows, so a machine with two or more
//! ref inputs is no longer inherently ambiguous — it is resolved by matching the
//! output lifetime to an input lifetime. The decision is shared with the loan
//! attributor (`borrow::view_link`) so the two never disagree.

use crate::borrow::returned_body_source_mismatch;
use diagnostics::Diagnostic;

use crate::borrow::view_link::{
    DeclarationLifetimeFrontier, ViewReturnAmbiguity, ViewReturnSource,
    declaration_lifetime_frontier, resolve_signature_view_return_source,
    resolve_view_return_source,
};
mod templates;

pub(super) fn check_view_return_elision(
    program: &dyn ::validation::CheckingInput,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut deferred = Vec::new();
    for machine in program.machines() {
        let parameters = program
            .machine_type_parameters(machine)
            .iter()
            .filter(|parameter| {
                matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
            })
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        for state in program.machine_states(machine) {
            if let Some(parameter_index) =
                returned_body_source_mismatch(program, state, call_frames)
            {
                let parameter = &program.state_parameters(state)[parameter_index];
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` returns a view borrowing input `{}`, which is not permitted by the declared result lifetime relation",
                    super::escape::state_subject(machine, state),
                    parameter.name,
                )));
            }
            if declaration_lifetime_frontier(program, state.return_type, &parameters)
                == DeclarationLifetimeFrontier::TemplateDependent
                && program.state_parameters(state).iter().all(|parameter| {
                    declaration_lifetime_frontier(program, parameter.type_reference, &parameters)
                        != DeclarationLifetimeFrontier::Incomplete
                })
            {
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
            check_bodyless_signature(program, signature, diagnostics);
        }
    }

    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            let parameters = program
                .trait_type_parameters(trait_definition)
                .iter()
                .chain(program.state_signature_type_parameters(signature))
                .filter(|parameter| {
                    matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                })
                .map(|parameter| parameter.symbol)
                .collect::<Vec<_>>();
            if declaration_lifetime_frontier(program, signature.return_type, &parameters)
                == DeclarationLifetimeFrontier::TemplateDependent
                && program
                    .state_signature_parameters(signature)
                    .iter()
                    .all(|parameter| {
                        declaration_lifetime_frontier(
                            program,
                            parameter.type_reference,
                            &parameters,
                        ) != DeclarationLifetimeFrontier::Incomplete
                    })
            {
                // A template is not an executable lifetime contract. Every
                // use that still selects this raw signature is fenced below;
                // concrete machine instances retain the ordinary full check.
                deferred.push(signature.symbol);
                continue;
            }
            check_bodyless_signature(program, signature, diagnostics);
        }
    }
    templates::check_calls(program, &deferred, diagnostics);
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
