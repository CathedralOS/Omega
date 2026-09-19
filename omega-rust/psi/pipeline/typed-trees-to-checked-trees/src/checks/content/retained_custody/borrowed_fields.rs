//! A record shell cannot turn a loan into owned retained content.
//!
//! The root checker owns exact lifetime-bound retention and one-to-one source
//! selection. This additional rejection checks beneath structural shells on
//! both sides of the signature, including explicitly linear records. A loan
//! applies only to its descendants: an unrelated reference field must not
//! classify an owned sibling as borrowed.
//!
//! These rows are a source-availability check, not claim correspondences or
//! conservation evidence. Finding an owned source grants nothing; partition
//! geometry, exact outcomes, issuance and qualification transport still owe
//! their independent checks. Consequently fixed arrays need one element-type
//! visit, not one row per runtime element or an invented capacity count.
//! Recursive expansion and unresolved extents remain explicit analysis limits,
//! never evidence of an owned source.

use super::structural_sources::{ContentSource, domain_sources, parameter_sources};
use super::{DomainApplication, domain_name};
use checked_trees::{CheckFacts, RetainedBorrowCustodyFact};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::signature::{SignatureContract, StateParameter};
use typed_trees::types::TypeReferenceHandle;

#[allow(clippy::too_many_arguments)]
pub(super) fn check_structural_borrow_sources(
    program: &TypedTrees,
    facts: &CheckFacts,
    label: &str,
    callable: SymbolHandle,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
    contracts: &[&SignatureContract],
    root_result_domains: &[DomainApplication],
    retained_borrow_custodies: &[RetainedBorrowCustodyFact],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (mut results, result_complete) = domain_sources(program, return_type);
    for domain in root_result_domains {
        let source = ContentSource {
            domain: *domain,
            borrowed: super::direct_reference(program, return_type).is_some(),
            nested: false,
            path: Vec::new(),
        };
        if !results.contains(&source) {
            results.push(source);
        }
    }
    results.retain(|result| {
        !result.borrowed
            && facts
                .qualifications
                .content
                .for_semantic_domain(result.domain.semantic_domain)
                .is_some()
            && (result.nested
                || !retained_borrow_custodies.iter().any(|custody| {
                    custody.callable == callable
                        && custody.retained_semantic_domain == result.domain.semantic_domain
                }))
    });
    if results.is_empty() {
        return;
    }
    if !result_complete {
        diagnostics.push(unsupported_structure(label, "result"));
        return;
    }
    let sources = parameters
        .iter()
        .map(|parameter| {
            let (sources, complete) = parameter_sources(program, parameter, contracts);
            if !complete {
                diagnostics.push(unsupported_structure(label, parameter.name.as_str()));
            }
            (parameter, sources)
        })
        .collect::<Vec<_>>();

    for result in &results {
        let Some(result_plan) = facts
            .qualifications
            .content
            .for_semantic_domain(result.domain.semantic_domain)
        else {
            continue;
        };
        let mut owned_source = false;
        let mut nested_source = false;
        let mut borrowed_parameters = Vec::new();
        for (parameter, sources) in &sources {
            for source in sources {
                let Some(source_plan) = facts
                    .qualifications
                    .content
                    .for_semantic_domain(source.domain.semantic_domain)
                else {
                    continue;
                };
                if source_plan.algebra != result_plan.algebra {
                    continue;
                }
                if source.borrowed {
                    nested_source |= source.nested;
                    if !borrowed_parameters.contains(&parameter.name.as_str()) {
                        borrowed_parameters.push(parameter.name.as_str());
                    }
                } else {
                    owned_source = true;
                }
            }
        }
        // Whole direct inputs/results retain the existing lifetime-bound
        // exception. Nested retention needs an exact subplace/lifetime join;
        // the root-only receipt cannot authorize a different returned field.
        if owned_source || borrowed_parameters.is_empty() || (!result.nested && !nested_source) {
            continue;
        }
        let borrowed = borrowed_parameters
            .iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ");
        diagnostics.push(Diagnostic::error(format!(
            "callable `{label}` returns content-bearing custody `{}` sourced only from borrowed parameter{} {borrowed} through a structural field; retained-after-return authority requires a consumed owned input",
            domain_name(program, result.domain.symbol),
            if borrowed_parameters.len() == 1 { "" } else { "s" },
        )));
    }
}

fn unsupported_structure(label: &str, subject: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "callable `{label}` cannot check structural content custody for `{subject}`: recursive expansion or an unresolved array extent requires a finite structural source analysis",
    ))
}
