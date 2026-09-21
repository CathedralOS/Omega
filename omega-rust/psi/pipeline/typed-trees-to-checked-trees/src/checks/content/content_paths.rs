//! Content segments, contract places and projection plans as fact paths.

use checked_trees::CheckFacts;
use language_semantics::content::{
    ContentCaseSegment, ContentConservationTerm, ContentFieldSegment, ContentPlaceRoot,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionPlan, ContentStructuralPlace,
};
use language_semantics::{
    PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    SemanticDomainId,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::SignatureContractKind;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn content_segments_to_fact_path(
    segments: &[ContentPlaceSegment],
) -> Option<Vec<facts::PlaceSegment>> {
    segments
        .iter()
        .map(|segment| match segment {
            ContentPlaceSegment::Case(case) if case.symbol.is_valid() => {
                Some(facts::PlaceSegment::Case {
                    variant: case.symbol,
                })
            }
            ContentPlaceSegment::Field(field) if field.symbol.is_valid() => {
                Some(facts::PlaceSegment::Field {
                    symbol: field.symbol,
                })
            }
            ContentPlaceSegment::FixedIndex(index) => Some(facts::PlaceSegment::FixedIndex {
                index: usize::try_from(*index).ok()?,
            }),
            ContentPlaceSegment::Case(_) | ContentPlaceSegment::Field(_) => None,
        })
        .collect()
}

pub(crate) fn projection_term(
    plan: &ContentProjectionPlan,
    subject: ContentStructuralPlace,
) -> ContentConservationTerm {
    ContentConservationTerm::Projection {
        domain: plan.domain,
        semantic_domain: plan.semantic_domain,
        projection_machine: plan.machine,
        projection_report_fingerprint: plan.report_fingerprint,
        subject,
    }
}

pub(crate) fn unique_entry_claim_identity(
    facts: &CheckFacts,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    input_path: &[facts::PlaceSegment],
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
                && event.root == facts::PlaceRoot::Symbol(parameter_symbol)
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

pub(crate) fn applicable_projection_plans<'facts>(
    program: &TypedTrees,
    facts: &'facts CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
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
                && (type_has_domain(program, type_reference, plan.semantic_domain)
                    || contracts_establish_domain(
                        program,
                        machine,
                        state,
                        subject,
                        plan.domain,
                        plan.semantic_domain,
                    ))
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
    domain: SemanticDomainId,
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
                    matches!(constraint, TypeConstraintNode::Domain(candidate) if candidate.semantic_id == domain)
                })
                || type_has_domain(program, *base_type, domain)
        }
        _ => false,
    }
}

fn contracts_establish_domain(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    subject: &ContentStructuralPlace,
    domain: SymbolHandle,
    semantic_domain: SemanticDomainId,
) -> bool {
    if program
        .domain_definitions()
        .iter()
        .find(|definition| definition.symbol == domain)
        .is_none_or(|definition| definition.semantic_id != semantic_domain)
    {
        // Contract proof facts currently retain only the nominal family. They
        // cannot establish one exact indexed application without laundering
        // another family member into it.
        return false;
    }
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
            let segments =
                names
                    .iter()
                    .enumerate()
                    .skip(1)
                    .fold(Vec::new(), |mut segments, (index, name)| {
                        push_contract_field(
                            program,
                            &mut segments,
                            symbols
                                .get(index)
                                .copied()
                                .unwrap_or(SymbolHandle::invalid()),
                            name.as_str(),
                        );
                        segments
                    });
            Some((root, root_symbol, segments))
        }
        ExpressionNode::Member(member) => {
            let (root, root_symbol, mut segments) =
                contract_structural_place(program, member.receiver)?;
            push_contract_field(
                program,
                &mut segments,
                member.member_symbol,
                member.member.as_str(),
            );
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
        ExpressionNode::Borrow(inner) => contract_structural_place(program, inner.target),
        _ => None,
    }
}

fn push_contract_field(
    program: &TypedTrees,
    segments: &mut Vec<ContentPlaceSegment>,
    field_symbol: SymbolHandle,
    field_name: &str,
) {
    if let Some(variant_symbol) = facts::payload_variant_for_field(program, field_symbol)
        && let Some(variant_name) = data_variant_name(program, variant_symbol)
    {
        segments.push(ContentPlaceSegment::Case(ContentCaseSegment {
            symbol: variant_symbol,
            name: variant_name.to_owned(),
        }));
    }
    segments.push(ContentPlaceSegment::Field(ContentFieldSegment {
        symbol: field_symbol,
        name: field_name.to_owned(),
    }));
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

pub(crate) fn content_path(
    program: &TypedTrees,
    path: &[facts::PlaceSegment],
) -> Option<Vec<ContentPlaceSegment>> {
    path.iter()
        .map(|segment| match segment {
            facts::PlaceSegment::Case { variant } => {
                Some(ContentPlaceSegment::Case(ContentCaseSegment {
                    symbol: *variant,
                    name: data_variant_name(program, *variant)?.to_owned(),
                }))
            }
            facts::PlaceSegment::Field { symbol } => {
                Some(ContentPlaceSegment::Field(ContentFieldSegment {
                    symbol: *symbol,
                    name: data_field_name(program, *symbol)?.to_owned(),
                }))
            }
            facts::PlaceSegment::FixedIndex { index } => Some(ContentPlaceSegment::FixedIndex(
                u64::try_from(*index).expect("fixed index fits u64"),
            )),
            facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::Index { .. } => None,
        })
        .collect()
}

fn data_variant_name(program: &TypedTrees, variant_symbol: SymbolHandle) -> Option<&str> {
    program.data_definitions().iter().find_map(|definition| {
        program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Variant(variant) => {
                    (variant.symbol == variant_symbol).then_some(variant.name.as_str())
                }
                typed_trees::data::DataMember::Field(_) => None,
            })
    })
}

fn data_field_name(program: &TypedTrees, field_symbol: SymbolHandle) -> Option<&str> {
    program.data_definitions().iter().find_map(|definition| {
        program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) => {
                    (field.symbol == field_symbol).then_some(field.name.as_str())
                }
                typed_trees::data::DataMember::Variant(variant) => program
                    .data_payload_fields(variant)
                    .iter()
                    .find_map(|field| {
                        (field.symbol == field_symbol).then_some(field.name.as_str())
                    }),
            })
    })
}

#[cfg(test)]
mod tests {
    use super::{content_paths_match, content_segments_to_fact_path};
    use language_semantics::content::{
        ContentCaseSegment, ContentFieldSegment, ContentPlaceSegment,
    };
    use symbols::SymbolHandle;

    fn case_segment(name: &str, arena_index: u32) -> ContentPlaceSegment {
        ContentPlaceSegment::Case(ContentCaseSegment {
            symbol: SymbolHandle::from_arena_index(arena_index),
            name: name.to_owned(),
        })
    }

    fn field_segment(name: &str, arena_index: u32) -> ContentPlaceSegment {
        ContentPlaceSegment::Field(ContentFieldSegment {
            symbol: SymbolHandle::from_arena_index(arena_index),
            name: name.to_owned(),
        })
    }

    #[test]
    fn content_segments_to_fact_path_keeps_symbol_bound_segments() {
        let segments = [
            case_segment("Some", 7),
            field_segment("payload", 9),
            ContentPlaceSegment::FixedIndex(3),
        ];
        let path = content_segments_to_fact_path(&segments)
            .expect("symbol-bound segments convert to a fact path");
        assert!(matches!(
            path.as_slice(),
            [
                facts::PlaceSegment::Case { variant }
                    , facts::PlaceSegment::Field { symbol }
                    , facts::PlaceSegment::FixedIndex { index: 3 }
            ] if variant.arena_index() == 7 && symbol.arena_index() == 9
        ));
    }

    #[test]
    fn content_segments_to_fact_path_drops_symbol_free_segments() {
        assert!(
            content_segments_to_fact_path(&[case_segment("Some", 0)]).is_none(),
            "a case segment with no variant symbol cannot become a fact path"
        );
        assert!(
            content_segments_to_fact_path(
                &[field_segment("kept", 4), field_segment("dropped", 0),]
            )
            .is_none(),
            "one symbol-free field segment sinks the whole path"
        );
    }

    #[test]
    fn content_paths_match_compares_exact_segments() {
        let left = [case_segment("Some", 7), field_segment("payload", 9)];
        let right = [case_segment("Some", 7), field_segment("payload", 9)];
        assert!(content_paths_match(&left, &right));
    }

    #[test]
    fn content_paths_match_admits_symbol_free_names() {
        // Contract-authored places may carry a name but no resolved symbol:
        // equality then falls back to the spelled name alone.
        assert!(content_paths_match(
            &[field_segment("payload", 0)],
            &[field_segment("payload", 9)],
        ));
        assert!(content_paths_match(
            &[case_segment("Some", 7)],
            &[case_segment("Some", 0)],
        ));
    }

    #[test]
    fn content_paths_match_rejects_symbol_disagreement() {
        assert!(!content_paths_match(
            &[field_segment("payload", 4)],
            &[field_segment("payload", 9)],
        ));
        assert!(!content_paths_match(
            &[case_segment("Some", 4)],
            &[case_segment("Some", 9)],
        ));
    }

    #[test]
    fn content_paths_match_rejects_name_disagreement() {
        assert!(!content_paths_match(
            &[field_segment("payload", 9)],
            &[field_segment("count", 9)],
        ));
    }

    #[test]
    fn content_paths_match_rejects_shape_disagreement() {
        assert!(!content_paths_match(
            &[case_segment("Some", 7)],
            &[field_segment("Some", 7)],
        ));
        assert!(!content_paths_match(
            &[field_segment("payload", 9)],
            &[
                field_segment("payload", 9),
                ContentPlaceSegment::FixedIndex(0)
            ],
        ));
        assert!(!content_paths_match(
            &[ContentPlaceSegment::FixedIndex(3)],
            &[ContentPlaceSegment::FixedIndex(4)],
        ));
        assert!(content_paths_match(
            &[ContentPlaceSegment::FixedIndex(3)],
            &[ContentPlaceSegment::FixedIndex(3)],
        ));
    }
}
