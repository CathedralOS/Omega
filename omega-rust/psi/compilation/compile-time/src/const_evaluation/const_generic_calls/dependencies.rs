//! Argument readiness uses exact selected declarations and original use keys.
//!
//! Provisional generic carriers may be shared, but their authored uses are not.
//! The resolver produces owner-to-input rows before lowering each syntax root;
//! typed dependency selection chooses which of those owners execution requires.
//! This schedules work only. Every ready expression still owes normal checked
//! invocation admission, and an admission error is never a retry signal.

use diagnostics::Diagnostic;
use syntax_trees::{SyntaxTrees, expression::ExpressionHandle, types::TypeReferenceHandle};

pub(super) struct ReadyLayer {
    pub(super) arguments: Vec<usize>,
}

pub(super) fn ready_layer(
    preparation: &crate::syntax_probes::PreparedEvaluationInputs,
    applications: &[(
        TypeReferenceHandle,
        ExpressionHandle,
        TypeReferenceHandle,
        bool,
    )],
    syntax: &SyntaxTrees,
) -> Result<ReadyLayer, Vec<Diagnostic>> {
    let mut dependencies = Vec::with_capacity(applications.len());
    for (_, expression, ..) in applications {
        let reference = syntax.expressions.source_span(*expression);
        let mut matching = preparation.typed.machines().iter().filter(|machine| {
            preparation.typed.symbols.symbol_source_span(machine.symbol) == Some(reference)
        });
        let (Some(machine), None) = (matching.next(), matching.next()) else {
            return Err(vec![
                Diagnostic::error("application probe lost its unique source owner")
                    .with_source_span(reference),
            ]);
        };
        let domain = preparation.domain_for_roots(&[machine.symbol]);
        let mut required = Vec::new();
        for &(owner, argument) in &preparation.pending_type_inputs {
            if !domain.contains(owner) {
                continue;
            }
            let dependency = applications
                .iter()
                .position(|(candidate, ..)| *candidate == argument)
                .ok_or_else(|| {
                    vec![
                        Diagnostic::error("pending type input lost its original argument")
                            .with_source_span(reference),
                    ]
                })?;
            if !required.contains(&dependency) {
                required.push(dependency);
            }
        }
        dependencies.push(required);
    }
    let arguments = dependencies
        .iter()
        .enumerate()
        .filter_map(|(ordinal, required)| required.is_empty().then_some(ordinal))
        .collect::<Vec<_>>();
    // Prove acyclicity independently of external provider readiness. A provider
    // leaf can block its dependants without turning them into a dependency cycle.
    let mut ordered = vec![false; applications.len()];
    loop {
        let mut changed = false;
        for (ordinal, required) in dependencies.iter().enumerate() {
            if !ordered[ordinal] && required.iter().all(|dependency| ordered[*dependency]) {
                ordered[ordinal] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    if let Some(ordinal) = ordered.iter().position(|ordered| !ordered) {
        return Err(vec![
            Diagnostic::error("const-generic application dependency cycle")
                .with_source_span(syntax.expressions.source_span(applications[ordinal].1)),
        ]);
    }
    Ok(ReadyLayer { arguments })
}
