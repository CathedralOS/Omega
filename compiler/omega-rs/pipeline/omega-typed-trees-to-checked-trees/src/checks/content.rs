//! Content-bearing signature conservation gates.
//!
//! A borrow lends access; it never supplies an owned claim that can survive
//! the call. This first P1c consumer rejects the exact retained-custody shape
//! where a content-bearing result has compatible content-bearing inputs, but
//! every compatible source is borrowed. It deliberately keys compatibility by
//! the retained compiler-owned algebra identity, never carrier or operation
//! names.

use omega_checked_trees::{CheckFacts, ContentIdentityReshuffleFact, FlowClaimOutcomeSource};
use omega_core::content::{
    ContentCaseSegment, ContentConservationEquation, ContentConservationOwnerKind,
    ContentConservationPlan, ContentConservationTerm, ContentFieldSegment, ContentPlaceRoot,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionPlan, ContentStructuralPlace,
    conservation_fingerprint, content_conservation_plan_bytes,
};
use omega_core::diagnostics::Diagnostic;
use omega_core::semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource,
};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::domain::ProofFact;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use omega_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// Derive the content equality attached to every exact input-relative claim
/// outcome. These are deliberately individual rewrite rows: distinct linear
/// claims do not imply that their projected content is disjoint, so this pass
/// never manufactures a `separate(...)` term. The later frontier theorem may
/// compose rows only when it also has the required partition evidence.
pub(crate) fn infer_identity_preserving_reshuffles(program: &TypedTrees, facts: &mut CheckFacts) {
    let outcomes = facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .flat_map(|(_, map)| {
            facts
                .flow
                .ownership
                .claim_outcome_entries
                .span_or_empty(map.entries)
                .iter()
                .map(|entry| {
                    (
                        map.machine_symbol,
                        map.state_symbol,
                        entry.output_segments,
                        entry.source,
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut reshuffles = Vec::new();

    for (machine_symbol, state_symbol, output_segments, source) in outcomes {
        let FlowClaimOutcomeSource::Input {
            parameter_symbol,
            segments: input_segments,
        } = source
        else {
            continue;
        };
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)
        else {
            continue;
        };
        let Some(state) = crate::find_state(program, state_symbol) else {
            continue;
        };
        let Some((parameter_position, parameter)) = program
            .state_parameters(state)
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == parameter_symbol)
        else {
            continue;
        };
        let input_path = facts.flow.ownership.segments.span_or_empty(input_segments);
        let output_path = facts.flow.ownership.segments.span_or_empty(output_segments);
        let Some(input_claim) =
            super::multiplicity::linear_claim_frontier(program, parameter.type_reference)
                .into_iter()
                .find(|claim| claim.path == input_path)
        else {
            continue;
        };
        let Some(output_claim) =
            super::multiplicity::linear_claim_frontier(program, state.return_type)
                .into_iter()
                .find(|claim| claim.path == output_path)
        else {
            continue;
        };
        let Some(input_content_path) = content_path(program, input_path) else {
            continue;
        };
        let Some(output_content_path) = content_path(program, output_path) else {
            continue;
        };
        let input_subject = ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: ContentPlaceRoot::Parameter {
                position: u32::try_from(parameter_position)
                    .expect("state parameter position fits in u32"),
                symbol: parameter.symbol,
                name: parameter.name.as_str().to_owned(),
                is_self: parameter.is_self,
            },
            segments: input_content_path,
        };
        let output_subject = ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root: ContentPlaceRoot::Result,
            segments: output_content_path,
        };
        let input_plans = applicable_projection_plans(
            program,
            facts,
            machine,
            state,
            input_claim.type_reference,
            &input_subject,
        );
        let output_plans = applicable_projection_plans(
            program,
            facts,
            machine,
            state,
            output_claim.type_reference,
            &output_subject,
        );
        let Some(claim_identity) =
            unique_entry_claim_identity(facts, state_symbol, parameter_symbol, input_path)
        else {
            continue;
        };

        for input_plan in input_plans {
            for output_plan in output_plans.iter().copied().filter(|output_plan| {
                output_plan.semantic_domain == input_plan.semantic_domain
                    && output_plan.fingerprint == input_plan.fingerprint
                    && output_plan.algebra == input_plan.algebra
            }) {
                let left = projection_term(input_plan, input_subject.clone());
                let right = projection_term(output_plan, output_subject.clone());
                let equation = ContentConservationEquation::new(left, right);
                let fingerprint = conservation_fingerprint(&input_plan.algebra, &equation);
                reshuffles.push(ContentIdentityReshuffleFact {
                    machine_symbol,
                    state_symbol,
                    claim_identity,
                    input_parameter_symbol: parameter_symbol,
                    input_segments,
                    output_segments,
                    plan: ContentConservationPlan {
                        owner_kind: ContentConservationOwnerKind::Machine,
                        owner: machine_symbol,
                        callable: state_symbol,
                        algebra: input_plan.algebra.clone(),
                        equation,
                        fingerprint,
                    },
                });
            }
        }
    }

    reshuffles.sort_by_key(|fact| {
        (
            fact.machine_symbol.arena_index(),
            fact.state_symbol.arena_index(),
            content_conservation_plan_bytes(&fact.plan),
        )
    });
    reshuffles.dedup();
    facts.qualifications.content.identity_reshuffles = reshuffles;
}

fn projection_term(
    plan: &ContentProjectionPlan,
    subject: ContentStructuralPlace,
) -> ContentConservationTerm {
    ContentConservationTerm::Projection {
        domain: plan.domain,
        semantic_domain: plan.semantic_domain,
        projection_machine: plan.machine,
        projection_fingerprint: plan.fingerprint,
        subject,
    }
}

fn unique_entry_claim_identity(
    facts: &CheckFacts,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    input_path: &[omega_facts::PlaceSegment],
) -> Option<PermissionClaimIdentity> {
    let identities = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.state_symbol == state_symbol
                && event.source == PermissionEventSource::StateEntry
                && event.kind == PermissionEventKind::Establish
                && event.access == PermissionAccess::Owned
                && event.obligation_live
                && event.root == omega_facts::PlaceRoot::Symbol(parameter_symbol)
                && facts.flow.ownership.segments.span_or_empty(event.segments) == input_path
                && event.claim_identity != PermissionClaimIdentity::Unknown
        })
        .map(|(_, event)| event.claim_identity)
        .fold(Vec::new(), |mut identities, identity| {
            if !identities.contains(&identity) {
                identities.push(identity);
            }
            identities
        });
    let [identity] = identities.as_slice() else {
        return None;
    };
    Some(*identity)
}

fn applicable_projection_plans<'facts>(
    program: &TypedTrees,
    facts: &'facts CheckFacts,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    type_reference: TypeReferenceHandle,
    subject: &ContentStructuralPlace,
) -> Vec<&'facts ContentProjectionPlan> {
    let Some(carrier) = unwrapped_type_reference(program, type_reference) else {
        return Vec::new();
    };
    let carrier_identity = program.normalized_type_identity(carrier).into_string();
    facts
        .qualifications
        .content
        .plans
        .iter()
        .filter(|plan| {
            plan.carrier_identity == carrier_identity
                && (type_has_domain(program, type_reference, plan.domain)
                    || contracts_establish_domain(program, machine, state, subject, plan.domain))
        })
        .collect()
}

fn unwrapped_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    if !type_reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => unwrapped_type_reference(program, *referee),
        _ => Some(type_reference),
    }
}

fn type_has_domain(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    domain: SymbolHandle,
) -> bool {
    if !type_reference.is_valid() {
        return false;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            type_has_domain(program, *referee, domain)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .any(|constraint| {
                    matches!(constraint, TypeConstraintNode::Domain(candidate) if candidate.symbol == domain)
                })
                || type_has_domain(program, *base_type, domain)
        }
        _ => false,
    }
}

fn contracts_establish_domain(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    subject: &ContentStructuralPlace,
    domain: SymbolHandle,
) -> bool {
    let mut contracts = program.state_contracts(state).iter().collect::<Vec<_>>();
    if program
        .machine_states(machine)
        .first()
        .is_some_and(|entry| entry.symbol == state.symbol)
    {
        contracts.extend(program.machine_contracts(machine));
    }
    contracts.into_iter().any(|contract| {
        let allowed = match (&subject.root, subject.version) {
            (ContentPlaceRoot::Parameter { .. }, ContentPlaceVersion::Entry) => {
                contract.kind == SignatureContractKind::Requires
            }
            (ContentPlaceRoot::Result, ContentPlaceVersion::Current) => {
                contract.kind == SignatureContractKind::Ensures
            }
            _ => false,
        };
        allowed
            && program
                .proof_facts
                .span_or_empty(contract.facts)
                .iter()
                .any(|fact| {
                    let ProofFact::Membership(membership) = fact else {
                        return false;
                    };
                    membership.domain_symbol == domain
                        && contract_place_matches(program, membership.value, subject)
                })
    })
}

fn contract_place_matches(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: &ContentStructuralPlace,
) -> bool {
    let Some((root_name, root_symbol, segments)) = contract_structural_place(program, expression)
    else {
        return false;
    };
    let root_matches = match &expected.root {
        ContentPlaceRoot::Result => root_name == "result",
        ContentPlaceRoot::Parameter { symbol, name, .. } => {
            (*symbol == root_symbol && symbol.is_valid()) || *name == root_name
        }
    };
    root_matches && content_paths_match(&expected.segments, &segments)
}

fn contract_structural_place(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(String, SymbolHandle, Vec<ContentPlaceSegment>)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let names = program.expression_table.name_path_members(path.members);
            let root = names.first()?.as_str().to_owned();
            let symbols = program
                .expression_table
                .name_path_member_symbols(path.member_symbols);
            let root_symbol = symbols.first().copied().unwrap_or(path.head_symbol);
            let segments = names
                .iter()
                .enumerate()
                .skip(1)
                .map(|(index, name)| {
                    ContentPlaceSegment::Field(ContentFieldSegment {
                        symbol: symbols
                            .get(index)
                            .copied()
                            .unwrap_or(SymbolHandle::invalid()),
                        name: name.as_str().to_owned(),
                    })
                })
                .collect();
            Some((root, root_symbol, segments))
        }
        ExpressionNode::Member(member) => {
            let (root, root_symbol, mut segments) =
                contract_structural_place(program, member.receiver)?;
            segments.push(ContentPlaceSegment::Field(ContentFieldSegment {
                symbol: member.member_symbol,
                name: member.member.as_str().to_owned(),
            }));
            Some((root, root_symbol, segments))
        }
        ExpressionNode::Indexed(indexed) => {
            let (root, root_symbol, mut segments) =
                contract_structural_place(program, indexed.collection)?;
            let ExpressionNode::Integer(index) = program.expression_table.expression(indexed.index)
            else {
                return None;
            };
            segments.push(ContentPlaceSegment::FixedIndex(index.value_u64()?));
            Some((root, root_symbol, segments))
        }
        ExpressionNode::Mutable(inner) => contract_structural_place(program, *inner),
        _ => None,
    }
}

fn content_paths_match(left: &[ContentPlaceSegment], right: &[ContentPlaceSegment]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| match (left, right) {
                (ContentPlaceSegment::Case(left), ContentPlaceSegment::Case(right)) => {
                    left.name == right.name
                        && (!left.symbol.is_valid()
                            || !right.symbol.is_valid()
                            || left.symbol == right.symbol)
                }
                (ContentPlaceSegment::FixedIndex(left), ContentPlaceSegment::FixedIndex(right)) => {
                    left == right
                }
                (ContentPlaceSegment::Field(left), ContentPlaceSegment::Field(right)) => {
                    left.name == right.name
                        && (!left.symbol.is_valid()
                            || !right.symbol.is_valid()
                            || left.symbol == right.symbol)
                }
                _ => false,
            })
}

fn content_path(
    program: &TypedTrees,
    path: &[omega_facts::PlaceSegment],
) -> Option<Vec<ContentPlaceSegment>> {
    path.iter()
        .map(|segment| match segment {
            omega_facts::PlaceSegment::Case { variant } => {
                Some(ContentPlaceSegment::Case(ContentCaseSegment {
                    symbol: *variant,
                    name: data_variant_name(program, *variant)?.to_owned(),
                }))
            }
            omega_facts::PlaceSegment::Field { symbol } => {
                Some(ContentPlaceSegment::Field(ContentFieldSegment {
                    symbol: *symbol,
                    name: data_field_name(program, *symbol)?.to_owned(),
                }))
            }
            omega_facts::PlaceSegment::FixedIndex { index } => {
                Some(ContentPlaceSegment::FixedIndex(
                    u64::try_from(*index).expect("fixed index fits u64"),
                ))
            }
            omega_facts::PlaceSegment::Index { .. } => None,
        })
        .collect()
}

fn data_variant_name(program: &TypedTrees, variant_symbol: SymbolHandle) -> Option<&str> {
    program.data_definitions().iter().find_map(|definition| {
        program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                omega_typed_trees::data::DataMember::Variant(variant) => {
                    (variant.symbol == variant_symbol).then_some(variant.name.as_str())
                }
                omega_typed_trees::data::DataMember::Field(_) => None,
            })
    })
}

fn data_field_name(program: &TypedTrees, field_symbol: SymbolHandle) -> Option<&str> {
    program.data_definitions().iter().find_map(|definition| {
        program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                omega_typed_trees::data::DataMember::Field(field) => {
                    (field.symbol == field_symbol).then_some(field.name.as_str())
                }
                omega_typed_trees::data::DataMember::Variant(variant) => program
                    .data_payload_fields(variant)
                    .iter()
                    .find_map(|field| {
                        (field.symbol == field_symbol).then_some(field.name.as_str())
                    }),
            })
    })
}

pub(crate) fn check_retained_content_custody(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    if facts.qualifications.content.plans.is_empty() {
        return Ok(());
    }

    let mut diagnostics = Vec::new();

    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            let contracts = program
                .state_signature_contracts(signature)
                .iter()
                .collect::<Vec<_>>();
            check_callable(
                program,
                facts,
                &format!("{}::{}", trait_definition.name, signature.name),
                program.state_signature_parameters(signature),
                signature.return_type,
                &contracts,
                &mut diagnostics,
            );
        }
    }

    for machine in program.machines() {
        for (state_index, state) in program.machine_states(machine).iter().enumerate() {
            let mut contracts = program.state_contracts(state).iter().collect::<Vec<_>>();
            if state_index == 0 {
                contracts.extend(program.machine_contracts(machine));
            }
            let label = if state_index == 0 {
                machine.name.to_string()
            } else {
                format!("{}::{}", machine.name, state.name)
            };
            check_callable(
                program,
                facts,
                &label,
                program.state_parameters(state),
                state.return_type,
                &contracts,
                &mut diagnostics,
            );
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

#[allow(clippy::too_many_arguments)]
fn check_callable(
    program: &TypedTrees,
    facts: &CheckFacts,
    label: &str,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
    contracts: &[&SignatureContract],
    diagnostics: &mut Vec<Diagnostic>,
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
            if expression_is_bare_result(program, membership.value) {
                push_unique(&mut result_domains, membership.domain_symbol);
            }
        }
    }

    for result_domain in result_domains {
        let Some(result_plan) = facts.qualifications.content.for_domain(result_domain) else {
            continue;
        };
        let mut borrowed_sources = Vec::new();
        let mut has_owned_source = false;

        for parameter in parameters {
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
                    if expression_names_parameter(program, membership.value, parameter) {
                        push_unique(&mut parameter_domains, membership.domain_symbol);
                    }
                }
            }

            let compatible = parameter_domains.iter().any(|domain| {
                facts
                    .qualifications
                    .content
                    .for_domain(*domain)
                    .is_some_and(|input_plan| compatible_content(input_plan, result_plan))
            });
            if !compatible {
                continue;
            }

            if type_contains_reference(program, parameter.type_reference) {
                borrowed_sources.push(parameter.name.as_str());
            } else if program.type_multiplicity(parameter.type_reference) == Multiplicity::Linear {
                has_owned_source = true;
            }
        }

        if borrowed_sources.is_empty() || has_owned_source {
            continue;
        }

        let result_name = domain_name(program, result_domain);
        let borrowed = borrowed_sources
            .iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ");
        diagnostics.push(Diagnostic::error(format!(
            "callable `{label}` returns content-bearing custody `{result_name}` sourced only from borrowed parameter{} {borrowed}; retained-after-return authority requires a consumed owned input",
            if borrowed_sources.len() == 1 { "" } else { "s" },
        )));
    }
}

fn compatible_content(left: &ContentProjectionPlan, right: &ContentProjectionPlan) -> bool {
    left.algebra == right.algebra
}

fn append_type_domains(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    domains: &mut Vec<SymbolHandle>,
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
                    push_unique(domains, domain.symbol);
                }
            }
        }
        _ => {}
    }
}

fn type_contains_reference(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    if !type_reference.is_valid() {
        return false;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. } => true,
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_contains_reference(program, *base_type)
        }
        _ => false,
    }
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

fn push_unique(domains: &mut Vec<SymbolHandle>, domain: SymbolHandle) {
    if domain.is_valid() && !domains.contains(&domain) {
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
