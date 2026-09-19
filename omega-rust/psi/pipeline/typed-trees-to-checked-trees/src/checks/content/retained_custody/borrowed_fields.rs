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

use super::{
    DomainApplication, domain_name, expression_names_parameter, nominal_domain_application,
};
use checked_trees::{CheckFacts, RetainedBorrowCustodyFact};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataMember, TypeParameterKind};
use typed_trees::domain::ProofFact;
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use typed_trees::types::{
    FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

#[derive(Clone, Copy, PartialEq, Eq)]
struct ContentSource {
    domain: DomainApplication,
    borrowed: bool,
    nested: bool,
}

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
            let (mut sources, complete) = domain_sources(program, parameter.type_reference);
            if !complete {
                diagnostics.push(unsupported_structure(label, parameter.name.as_str()));
            }
            for contract in contracts
                .iter()
                .filter(|contract| contract.kind == SignatureContractKind::Requires)
            {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    if let ProofFact::Membership(membership) = fact
                        && expression_names_parameter(program, membership.value, parameter)
                        && let Some(domain) =
                            nominal_domain_application(program, membership.domain_symbol)
                    {
                        let source = ContentSource {
                            domain,
                            borrowed: super::direct_reference(program, parameter.type_reference)
                                .is_some(),
                            nested: false,
                        };
                        if !sources.contains(&source) {
                            sources.push(source);
                        }
                    }
                }
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

// Each actual retains the lexical environment in which it was supplied. A
// reused generic binder must not capture its own outer argument.
#[derive(Clone, Copy)]
struct TypeBinding {
    parameter: SymbolHandle,
    argument: TypeReferenceHandle,
    outer_scope: usize,
}

fn domain_sources(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> (Vec<ContentSource>, bool) {
    let mut sources = Vec::new();
    let complete = append_sources(
        program,
        reference,
        &[],
        false,
        false,
        &mut Vec::new(),
        &mut sources,
    );
    (sources, complete)
}

#[allow(clippy::too_many_arguments)]
fn append_sources(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    substitutions: &[TypeBinding],
    borrowed: bool,
    nested: bool,
    visiting: &mut Vec<(SymbolHandle, usize)>,
    sources: &mut Vec<ContentSource>,
) -> bool {
    let (reference, substitutions) = substituted_head(program, reference, substitutions);
    if !reference.is_valid() {
        return true;
    }
    let borrowed = borrowed || reference_shell(program, reference, substitutions);
    let definition_symbol = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            for constraint in program.type_reference_table.constraints(*constraints) {
                if let TypeConstraintNode::Domain(domain) = constraint {
                    let source = ContentSource {
                        domain: DomainApplication {
                            symbol: domain.symbol,
                            semantic_domain: domain.semantic_id,
                        },
                        borrowed,
                        nested,
                    };
                    if domain.semantic_id.is_valid() && !sources.contains(&source) {
                        sources.push(source);
                    }
                }
            }
            return append_sources(
                program,
                *base_type,
                substitutions,
                borrowed,
                nested,
                visiting,
                sources,
            );
        }
        TypeReferenceNode::Reference { referee, .. } => {
            return append_sources(
                program,
                *referee,
                substitutions,
                true,
                nested,
                visiting,
                sources,
            );
        }
        TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Literal(0),
            ..
        } => return true,
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(_),
        }
        | TypeReferenceNode::Slice { element_type } => {
            return append_sources(
                program,
                *element_type,
                substitutions,
                borrowed,
                true,
                visiting,
                sources,
            );
        }
        TypeReferenceNode::FixedArray { .. } => return false,
        TypeReferenceNode::Named { symbol, .. } => *symbol,
        TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
        _ => return true,
    };
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == definition_symbol)
    else {
        return true;
    };
    // Finite Wrapper<Wrapper<T>> consumes type-argument structure. Recursive
    // Node<Wrapper<T>> does not. This is a termination measure over the type
    // tree, not a resource capacity or an arbitrary recursion/fuel limit.
    let size = type_size(program, reference, substitutions);
    if visiting
        .iter()
        .any(|(owner, ancestor_size)| *owner == definition_symbol && size >= *ancestor_size)
    {
        return false;
    }
    let mut instantiated = substitutions.to_vec();
    if let TypeReferenceNode::Generic { arguments, .. } =
        program.type_reference_table.type_reference(reference)
    {
        instantiated.extend(
            program
                .data_type_parameters(definition)
                .iter()
                .zip(
                    program
                        .type_reference_table
                        .type_reference_handles(*arguments),
                )
                .filter_map(|(parameter, argument)| {
                    matches!(parameter.kind, TypeParameterKind::Type).then_some(TypeBinding {
                        parameter: parameter.symbol,
                        argument: *argument,
                        outer_scope: substitutions.len(),
                    })
                }),
        );
    }
    visiting.push((definition_symbol, size));
    let mut complete = true;
    for member in program.data_members(definition) {
        match member {
            DataMember::Field(field) => {
                complete &= append_sources(
                    program,
                    field.type_reference,
                    &instantiated,
                    borrowed,
                    true,
                    visiting,
                    sources,
                );
            }
            DataMember::Variant(variant) => {
                for field in program.data_payload_fields(variant) {
                    complete &= append_sources(
                        program,
                        field.type_reference,
                        &instantiated,
                        borrowed,
                        true,
                        visiting,
                        sources,
                    );
                }
            }
        }
    }
    visiting.pop();
    complete
}

fn substituted_head<'scope>(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    mut substitutions: &'scope [TypeBinding],
) -> (TypeReferenceHandle, &'scope [TypeBinding]) {
    while let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    {
        let Some(binding) = substitutions
            .iter()
            .rev()
            .find(|binding| binding.parameter == *symbol)
        else {
            break;
        };
        reference = binding.argument;
        substitutions = &substitutions[..binding.outer_scope];
    }
    (reference, substitutions)
}

fn reference_shell(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    mut substitutions: &[TypeBinding],
) -> bool {
    loop {
        (reference, substitutions) = substituted_head(program, reference, substitutions);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { .. } => return true,
            _ => return false,
        }
    }
}

fn type_size(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    substitutions: &[TypeBinding],
) -> usize {
    let (reference, substitutions) = substituted_head(program, reference, substitutions);
    let children = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_size(program, *base_type, substitutions)
        }
        TypeReferenceNode::Reference { referee, .. } => type_size(program, *referee, substitutions),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            type_size(program, *element_type, substitutions)
        }
        TypeReferenceNode::Generic { arguments, .. } => program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .fold(0usize, |size, argument| {
                size.saturating_add(type_size(program, *argument, substitutions))
            }),
        _ => 0,
    };
    children.saturating_add(1)
}
