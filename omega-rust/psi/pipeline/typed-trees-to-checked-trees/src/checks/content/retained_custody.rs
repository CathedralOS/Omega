//! Lifetime-bound borrow custody of retained content and the domain
//! applications it admits.

mod borrowed_fields;

use checked_trees::{CheckFacts, RetainedBorrowCustodyFact};
use diagnostics::Diagnostic;
use language_semantics::content::{
    ContentConservationTerm, ContentPlaceRoot, ContentPlaceVersion, ContentProjectionPlan,
    ContentStructuralPlace,
};
use language_semantics::{Multiplicity, ReferenceAccess, SemanticDomainId};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::name::Identifier;
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

#[allow(clippy::too_many_arguments)]
pub(crate) fn check_callable(
    program: &TypedTrees,
    facts: &CheckFacts,
    label: &str,
    callable: SymbolHandle,
    callable_lifetime_parameters: &[Identifier],
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
    contracts: &[&SignatureContract],
    diagnostics: &mut Vec<Diagnostic>,
    retained_borrow_custodies: &mut Vec<RetainedBorrowCustodyFact>,
) {
    let mut result_domains = Vec::new();
    append_type_domains(program, return_type, &mut result_domains);
    for contract in contracts
        .iter()
        .filter(|contract| contract.kind == SignatureContractKind::Ensures)
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Membership(membership) = fact else {
                continue;
            };
            if expression_is_bare_result(program, membership.value)
                && let Some(domain) = nominal_domain_application(program, membership.domain_symbol)
            {
                push_unique_domain_application(&mut result_domains, domain);
            }
        }
    }

    let content_result_domain_count = result_domains
        .iter()
        .filter(|domain| {
            facts
                .qualifications
                .content
                .for_semantic_domain(domain.semantic_domain)
                .is_some()
        })
        .count();

    for result_domain in &result_domains {
        let Some(result_plan) = facts
            .qualifications
            .content
            .for_semantic_domain(result_domain.semantic_domain)
        else {
            continue;
        };
        let mut borrowed_sources = Vec::new();
        let mut owned_sources = Vec::new();

        for (parameter_position, parameter) in parameters.iter().enumerate() {
            let mut parameter_domains = Vec::new();
            append_type_domains(program, parameter.type_reference, &mut parameter_domains);
            for contract in contracts
                .iter()
                .filter(|contract| contract.kind == SignatureContractKind::Requires)
            {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    let ProofFact::Membership(membership) = fact else {
                        continue;
                    };
                    if expression_names_parameter(program, membership.value, parameter)
                        && let Some(domain) =
                            nominal_domain_application(program, membership.domain_symbol)
                    {
                        push_unique_domain_application(&mut parameter_domains, domain);
                    }
                }
            }

            let mut compatible_plans = Vec::new();
            for domain in parameter_domains {
                let Some(input_plan) = facts
                    .qualifications
                    .content
                    .for_semantic_domain(domain.semantic_domain)
                else {
                    continue;
                };
                if compatible_content(input_plan, result_plan)
                    && !compatible_plans.contains(input_plan)
                {
                    compatible_plans.push(input_plan.clone());
                }
            }
            if compatible_plans.is_empty() {
                continue;
            }

            if type_contains_reference(program, parameter.type_reference) {
                borrowed_sources.push(BorrowedContentSource {
                    parameter_position,
                    parameter,
                    compatible_plans,
                });
            } else if program.type_multiplicity(parameter.type_reference) == Multiplicity::Linear {
                owned_sources.push(parameter.name.as_str());
            }
        }

        if owned_sources.len() == 1 || (owned_sources.is_empty() && borrowed_sources.is_empty()) {
            continue;
        }

        if let Some(selected) = authored_retention_source(
            facts,
            callable,
            result_domain.semantic_domain,
            result_plan,
            parameters,
        ) && owned_sources.contains(&selected)
        {
            continue;
        }

        if owned_sources.is_empty() && !borrowed_sources.is_empty() {
            match lifetime_bound_borrow_custody(
                program,
                label,
                callable,
                callable_lifetime_parameters,
                return_type,
                result_domain.semantic_domain,
                result_plan,
                content_result_domain_count,
                &borrowed_sources,
            ) {
                Ok(fact) => {
                    retained_borrow_custodies.push(fact);
                    continue;
                }
                Err(BorrowCustodyFailure::Directed(reason)) => {
                    diagnostics.push(Diagnostic::error(format!(
                        "callable `{label}` cannot retain borrowed content-bearing custody: {reason}",
                    )));
                    continue;
                }
                Err(BorrowCustodyFailure::RequiresOwnedCustody) => {}
            }
        }

        let result_name = domain_name(program, result_domain.symbol);
        if owned_sources.len() > 1 {
            let owned = owned_sources
                .iter()
                .map(|name| format!("`{name}`"))
                .collect::<Vec<_>>()
                .join(", ");
            diagnostics.push(Diagnostic::error(format!(
                "callable `{label}` returns content-bearing custody `{result_name}` with ambiguous compatible consumed inputs {owned}; retained-after-return authority requires one unambiguous owned source or an exact postcondition correspondence",
            )));
            continue;
        }
        let borrowed = borrowed_sources
            .iter()
            .map(|source| format!("`{}`", source.parameter.name))
            .collect::<Vec<_>>()
            .join(", ");
        diagnostics.push(Diagnostic::error(format!(
            "callable `{label}` returns content-bearing custody `{result_name}` sourced only from borrowed parameter{} {borrowed}; retained-after-return authority requires a consumed owned input",
            if borrowed_sources.len() == 1 { "" } else { "s" },
        )));
    }

    borrowed_fields::check_structural_borrow_sources(
        program,
        facts,
        label,
        callable,
        parameters,
        return_type,
        contracts,
        &result_domains,
        retained_borrow_custodies,
        diagnostics,
    );
}

struct BorrowedContentSource<'a> {
    parameter_position: usize,
    parameter: &'a StateParameter,
    compatible_plans: Vec<ContentProjectionPlan>,
}

enum BorrowCustodyFailure {
    RequiresOwnedCustody,
    Directed(String),
}

#[allow(clippy::too_many_arguments)]
fn lifetime_bound_borrow_custody(
    program: &TypedTrees,
    label: &str,
    callable: SymbolHandle,
    callable_lifetime_parameters: &[Identifier],
    return_type: TypeReferenceHandle,
    retained_semantic_domain: SemanticDomainId,
    result_plan: &ContentProjectionPlan,
    content_result_domain_count: usize,
    borrowed_sources: &[BorrowedContentSource<'_>],
) -> Result<RetainedBorrowCustodyFact, BorrowCustodyFailure> {
    let Some((result_data, result_lifetime)) = lifetime_bound_result(program, return_type) else {
        return Err(BorrowCustodyFailure::RequiresOwnedCustody);
    };
    if content_result_domain_count != 1 {
        return Err(BorrowCustodyFailure::Directed(format!(
            "the bounded shared-loan rung requires exactly one content-bearing result domain, but `{label}` has {content_result_domain_count}",
        )));
    }
    let Some(callable_lifetime_parameter_ordinal) = callable_lifetime_parameters
        .iter()
        .position(|candidate| candidate == &result_lifetime)
    else {
        return Err(BorrowCustodyFailure::Directed(format!(
            "result lifetime `'{}'` does not name an explicit lifetime parameter of `{label}`",
            result_lifetime.as_str(),
        )));
    };
    if borrowed_sources.len() != 1 {
        let names = borrowed_sources
            .iter()
            .map(|source| format!("`{}`", source.parameter.name))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(BorrowCustodyFailure::Directed(format!(
            "the result lifetime has ambiguous compatible borrowed inputs {names}; exactly one whole direct shared source is required",
        )));
    }
    let source = &borrowed_sources[0];
    if source.parameter.is_self {
        return Err(BorrowCustodyFailure::Directed(
            "retained `self` loans are outside the bounded whole-parameter rung".to_owned(),
        ));
    }
    let Some((access, source_lifetime)) =
        direct_reference(program, source.parameter.type_reference)
    else {
        return Err(BorrowCustodyFailure::Directed(format!(
            "borrowed parameter `{}` carries its loan through a nested or indirect shape; only a whole direct reference is admitted",
            source.parameter.name,
        )));
    };
    if access != ReferenceAccess::Shared {
        return Err(BorrowCustodyFailure::Directed(format!(
            "borrowed parameter `{}` uses {access:?} access; retained lifetime-bound custody currently admits shared access only",
            source.parameter.name,
        )));
    }
    let Some(source_lifetime) = source_lifetime else {
        return Err(BorrowCustodyFailure::Directed(format!(
            "borrowed parameter `{}` elides its lifetime; retained custody requires the same explicit callable lifetime on source and result",
            source.parameter.name,
        )));
    };
    if source_lifetime != result_lifetime {
        return Err(BorrowCustodyFailure::Directed(format!(
            "borrowed parameter `{}` uses lifetime `'{}'`, which does not match result lifetime `'{}'`",
            source.parameter.name,
            source_lifetime.as_str(),
            result_lifetime.as_str(),
        )));
    }
    let [source_projection] = source.compatible_plans.as_slice() else {
        return Err(BorrowCustodyFailure::Directed(format!(
            "borrowed parameter `{}` has {} compatible content projections; exactly one exact projection is required",
            source.parameter.name,
            source.compatible_plans.len(),
        )));
    };

    Ok(RetainedBorrowCustodyFact {
        callable,
        source: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: ContentPlaceRoot::Parameter {
                position: u32::try_from(source.parameter_position)
                    .expect("state parameter position fits in u32"),
                symbol: source.parameter.symbol,
                name: source.parameter.name.as_str().to_owned(),
                is_self: false,
            },
            segments: Vec::new(),
        },
        result: ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root: ContentPlaceRoot::Result,
            segments: Vec::new(),
        },
        access,
        lifetime: result_lifetime,
        callable_lifetime_parameter_ordinal: u32::try_from(callable_lifetime_parameter_ordinal)
            .expect("callable lifetime parameter ordinal fits in u32"),
        result_data,
        result_lifetime_argument_ordinal: 0,
        retained_semantic_domain,
        source_projection: source_projection.clone(),
        result_projection: result_plan.clone(),
    })
}

fn lifetime_bound_result(
    program: &TypedTrees,
    return_type: TypeReferenceHandle,
) -> Option<(SymbolHandle, Identifier)> {
    let return_type = strip_constraints(program, return_type);
    let TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = program.type_reference_table.type_reference(return_type)
    else {
        return None;
    };
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == *base_symbol)?;
    if program.type_multiplicity(return_type) != Multiplicity::Linear
        || definition.lifetime_parameters.len() != 1
        || lifetime_arguments.len() != 1
        || !program
            .type_reference_table
            .type_reference_handles(*arguments)
            .is_empty()
    {
        return None;
    }
    Some((*base_symbol, lifetime_arguments[0].clone()))
}

fn direct_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<(ReferenceAccess, Option<Identifier>)> {
    let type_reference = strip_constraints(program, type_reference);
    let TypeReferenceNode::Reference {
        access, lifetime, ..
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    Some((*access, lifetime.clone()))
}

fn strip_constraints(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
) -> TypeReferenceHandle {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(type_reference)
    {
        type_reference = *base_type;
    }
    type_reference
}

/// Return the one parameter selected by an exact authored custody equality.
/// The equation must relate the whole current result projection directly to
/// the whole entry projection of one parameter in the same algebra. Partition
/// terms and structural subplaces describe transformations rather than the
/// one-to-one correspondence needed to resolve retained ownership.
fn authored_retention_source<'a>(
    facts: &CheckFacts,
    callable: SymbolHandle,
    result_domain: SemanticDomainId,
    result_plan: &ContentProjectionPlan,
    parameters: &'a [StateParameter],
) -> Option<&'a str> {
    facts
        .qualifications
        .content
        .conservation_plans
        .iter()
        .filter(|plan| plan.callable == callable && plan.algebra == result_plan.algebra)
        .find_map(|plan| {
            let left = exact_projection_subject(plan.equation.left())?;
            let right = exact_projection_subject(plan.equation.right())?;
            let parameter = match (&left.root, left.version, &right.root, right.version) {
                (
                    ContentPlaceRoot::Result,
                    ContentPlaceVersion::Current,
                    ContentPlaceRoot::Parameter { symbol, .. },
                    ContentPlaceVersion::Entry,
                ) if projection_domain(plan.equation.left()) == Some(result_domain) => *symbol,
                (
                    ContentPlaceRoot::Parameter { symbol, .. },
                    ContentPlaceVersion::Entry,
                    ContentPlaceRoot::Result,
                    ContentPlaceVersion::Current,
                ) if projection_domain(plan.equation.right()) == Some(result_domain) => *symbol,
                _ => return None,
            };
            parameters
                .iter()
                .find(|candidate| candidate.symbol == parameter)
                .map(|candidate| candidate.name.as_str())
        })
}

fn exact_projection_subject(term: &ContentConservationTerm) -> Option<&ContentStructuralPlace> {
    let ContentConservationTerm::Projection { subject, .. } = term else {
        return None;
    };
    subject.segments.is_empty().then_some(subject)
}

fn projection_domain(term: &ContentConservationTerm) -> Option<SemanticDomainId> {
    let ContentConservationTerm::Projection {
        semantic_domain, ..
    } = term
    else {
        return None;
    };
    Some(*semantic_domain)
}

fn compatible_content(left: &ContentProjectionPlan, right: &ContentProjectionPlan) -> bool {
    left.algebra == right.algebra
}

fn append_type_domains(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    domains: &mut Vec<DomainApplication>,
) {
    if !type_reference.is_valid() {
        return;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            append_type_domains(program, *referee, domains);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            append_type_domains(program, *base_type, domains);
            for constraint in program.type_reference_table.constraints(*constraints) {
                if let TypeConstraintNode::Domain(domain) = constraint {
                    push_unique_domain_application(
                        domains,
                        DomainApplication {
                            symbol: domain.symbol,
                            semantic_domain: domain.semantic_id,
                        },
                    );
                }
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DomainApplication {
    symbol: SymbolHandle,
    semantic_domain: SemanticDomainId,
}

fn nominal_domain_application(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<DomainApplication> {
    let definition = program
        .domain_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol)?;
    definition
        .semantic_id
        .is_valid()
        .then_some(DomainApplication {
            symbol,
            semantic_domain: definition.semantic_id,
        })
}

fn type_contains_reference(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    type_reference.is_valid() && crate::borrow::view_link::returns_borrow(program, type_reference)
}

fn expression_is_bare_result(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    matches!(program.expression_table.name_path_members(path.members), [name] if name.as_str() == "result")
}

fn expression_names_parameter(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameter: &StateParameter,
) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    matches!(program.expression_table.name_path_members(path.members), [name]
        if path.symbol == parameter.symbol || name.as_str() == parameter.name.as_str())
}

fn push_unique_domain_application(domains: &mut Vec<DomainApplication>, domain: DomainApplication) {
    if domain.symbol.is_valid() && domain.semantic_domain.is_valid() && !domains.contains(&domain) {
        domains.push(domain);
    }
}

fn domain_name(program: &TypedTrees, symbol: SymbolHandle) -> &str {
    program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == symbol)
        .map(|domain| domain.name.as_str())
        .unwrap_or("<unknown domain>")
}
