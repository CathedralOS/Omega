//! Content segments, contract places and projection plans as fact paths.

use checked_trees::CheckFacts;
use checked_trees::facts::ContentBodyClaimBinding;
use language_semantics::content::{
    ContentCaseSegment, ContentConservationTerm, ContentFieldSegment, ContentPlaceRoot,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionPlan, ContentStructuralPlace,
};
use language_semantics::{
    PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    SemanticDomainId,
};
use symbols::SymbolHandle;
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

/// Pending body-claim bindings: one row per exact projection subject, refused
/// when one claim identity would stand for two separated subjects, published
/// only after its subject segments are interned. The joins that decide a claim
/// identity stay with their owners; this owner records rows, never authority.
#[derive(Default)]
pub(super) struct ClaimBindings {
    rows: Vec<PendingBinding>,
}

struct PendingBinding {
    binding: ContentBodyClaimBinding,
    path: Vec<facts::PlaceSegment>,
}

impl ClaimBindings {
    /// Record one projection subject. An unknown identity names no claim, and
    /// one claim identity cannot stand for two separated subjects.
    pub(super) fn push(
        &mut self,
        domain: SymbolHandle,
        semantic_domain: SemanticDomainId,
        machine: SymbolHandle,
        subject: &ContentStructuralPlace,
        path: Vec<facts::PlaceSegment>,
        claim_identity: PermissionClaimIdentity,
        observation: arena::Handle<checked_trees::FlowOperandFact>,
    ) -> Option<()> {
        if claim_identity == PermissionClaimIdentity::Unknown
            || self
                .rows
                .iter()
                .any(|pending| pending.binding.claim_identity == claim_identity)
        {
            return None;
        }
        self.rows.push(PendingBinding {
            binding: ContentBodyClaimBinding {
                claim_identity,
                domain,
                semantic_domain,
                projection_machine: machine,
                parameter_symbol: match subject.root {
                    ContentPlaceRoot::Parameter { symbol, .. } => symbol,
                    ContentPlaceRoot::Result => SymbolHandle::invalid(),
                },
                segments: Default::default(),
                observation,
            },
            path,
        });
        Some(())
    }

    /// Walk one conservation term, binding every projection subject through
    /// `claim_at`, which resolves that subject's claim from its own evidence.
    /// A missing or ambiguous claim refuses the whole walk.
    pub(super) fn extend_term(
        &mut self,
        term: &ContentConservationTerm,
        mut claim_at: impl FnMut(&ContentStructuralPlace) -> Option<PermissionClaimIdentity>,
    ) -> Option<()> {
        // One instantiation per resolver, and the reversed push keeps the
        // term's own left-to-right order on the stack.
        let mut pending = vec![term];
        while let Some(term) = pending.pop() {
            match term {
                ContentConservationTerm::Projection {
                    domain,
                    semantic_domain,
                    projection_machine,
                    subject,
                    ..
                } => {
                    let claim_identity = claim_at(subject)?;
                    self.push(
                        *domain,
                        *semantic_domain,
                        *projection_machine,
                        subject,
                        content_segments_to_fact_path(&subject.segments)?,
                        claim_identity,
                        arena::Handle::invalid(),
                    )?;
                }
                ContentConservationTerm::Separate(children) => {
                    pending.extend(children.iter().rev());
                }
            }
        }
        Some(())
    }

    pub(super) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The recorded rows, still holding their uninterred subject segments.
    pub(super) fn rows(
        &self,
    ) -> impl Iterator<Item = (&ContentBodyClaimBinding, &[facts::PlaceSegment])> {
        self.rows
            .iter()
            .map(|pending| (&pending.binding, pending.path.as_slice()))
    }

    /// Publish the recorded rows, interning each subject's segments once.
    pub(super) fn publish(
        self,
        facts: &mut CheckFacts,
    ) -> arena::HandleSpan<ContentBodyClaimBinding> {
        let rows = self
            .rows
            .into_iter()
            .map(|mut pending| {
                pending.binding.segments = facts.flow.ownership.segments.insert_many(pending.path);
                pending.binding
            })
            .collect::<Vec<_>>();
        facts
            .qualifications
            .content
            .body_claim_bindings
            .insert_many(rows)
    }
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
    entry_claim_identity(
        facts,
        state_symbol,
        parameter_symbol,
        input_path,
        facts
            .flow
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event),
    )
}

/// Recording and final consumers share the exact entry-claim join. Recording
/// supplies its event prefix because the final permission arena is not yet
/// published; neither consumer reconstructs identity from carrier geometry.
pub(super) fn entry_claim_identity<'event>(
    facts: &CheckFacts,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    input_path: &[facts::PlaceSegment],
    events: impl Iterator<Item = &'event checked_trees::FlowPermissionEventFact>,
) -> Option<PermissionClaimIdentity> {
    let identities = crate::collections::unique_in_order(
        events
            .filter(|event| {
                event.state_symbol == state_symbol
                    && event.source == PermissionEventSource::StateEntry
                    && event.kind == PermissionEventKind::Establish
                    && event.access == PermissionAccess::Owned
                    && event.obligation_live
                    && event.root == facts::PlaceRoot::Symbol(parameter_symbol)
                    && facts.flow.ownership.segments.span_or_empty(event.segments) == input_path
                    && event.claim_identity != PermissionClaimIdentity::Unknown
            })
            .map(|event| event.claim_identity),
    );
    let [identity] = identities.as_slice() else {
        return None;
    };
    Some(*identity)
}

pub(crate) fn applicable_projection_plans<'facts>(
    program: &dyn ::validation::CheckingInput,
    facts: &'facts CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    type_reference: TypeReferenceHandle,
    subject: &ContentStructuralPlace,
) -> Vec<&'facts ContentProjectionPlan> {
    let Some(carrier) = program.unwrapped_type_reference(type_reference) else {
        return Vec::new();
    };
    let carrier_identity = program.normalized_type_identity_text(carrier);
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

fn type_has_domain(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    subject: &ContentStructuralPlace,
    domain: SymbolHandle,
    semantic_domain: SemanticDomainId,
) -> bool {
    if program
        .domain_definition(domain)
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
                    let Some(membership) = fact.as_membership() else {
                        return false;
                    };
                    membership.domain_symbol == domain
                        && contract_place_matches(program, membership.value, subject)
                })
    })
}

fn contract_place_matches(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    expected: &ContentStructuralPlace,
) -> bool {
    let Some((root_name, root_symbol, segments)) = contract_structural_place(program, expression)
    else {
        return false;
    };
    let root_matches = match &expected.root {
        ContentPlaceRoot::Result => language_core::is_reserved_result(&root_name),
        ContentPlaceRoot::Parameter { symbol, name, .. } => {
            (*symbol == root_symbol && symbol.is_valid()) || *name == root_name
        }
    };
    root_matches && content_paths_match(&expected.segments, &segments)
}

fn contract_structural_place(
    program: &dyn ::validation::CheckingInput,
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
                        ::validation::push_structural_field(
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
            ::validation::push_structural_field(
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
            let index = program.expression_table.integer_node(indexed.index)?;
            segments.push(ContentPlaceSegment::FixedIndex(index.value_u64()?));
            Some((root, root_symbol, segments))
        }
        ExpressionNode::Borrow(inner) => contract_structural_place(program, inner.target),
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

pub(crate) fn content_path(
    program: &dyn ::validation::CheckingInput,
    path: &[facts::PlaceSegment],
) -> Option<Vec<ContentPlaceSegment>> {
    path.iter()
        .map(|segment| match segment {
            facts::PlaceSegment::Case { variant } => {
                Some(ContentPlaceSegment::Case(ContentCaseSegment {
                    symbol: *variant,
                    name: ::validation::data_variant_name(program, *variant)?.to_owned(),
                }))
            }
            facts::PlaceSegment::Field { symbol } => {
                Some(ContentPlaceSegment::Field(ContentFieldSegment {
                    symbol: *symbol,
                    name: crate::checks::multiplicity::data_field_name(program, *symbol)?
                        .to_owned(),
                }))
            }
            facts::PlaceSegment::FixedIndex { index } => Some(ContentPlaceSegment::FixedIndex(
                u64::try_from(*index).expect("fixed index fits u64"),
            )),
            facts::PlaceSegment::FixedRange { .. }
            | facts::PlaceSegment::OpaqueRange
            | facts::PlaceSegment::Index { .. } => None,
        })
        .collect()
}

/// Carrier fields read by a normalized content projection. Content equality
/// requires these observations, not every qualification or unrelated field in
/// the enclosing owner. Constants have no storage dependency.
pub(crate) fn projection_observations(
    expression: &language_semantics::content::ContentProjectionExpression,
) -> Option<Vec<Vec<facts::PlaceSegment>>> {
    use language_semantics::content::{ContentProjectionExpression, ContentScalarExpression};
    fn scalar(
        expression: &ContentScalarExpression,
        paths: &mut Vec<Vec<facts::PlaceSegment>>,
    ) -> Option<()> {
        match expression {
            ContentScalarExpression::SubjectField(fields)
            | ContentScalarExpression::RuntimeScalarEmbedding(fields) => {
                let path = fields
                    .iter()
                    .map(|field| {
                        field
                            .symbol
                            .is_valid()
                            .then_some(facts::PlaceSegment::Field {
                                symbol: field.symbol,
                            })
                    })
                    .collect::<Option<Vec<_>>>()?;
                crate::collections::push_unique(&mut *paths, path);
            }
            ContentScalarExpression::Natural(_) => {}
            ContentScalarExpression::Successor(inner) => scalar(inner, paths)?,
            ContentScalarExpression::Arithmetic { left, right, .. } => {
                scalar(left, paths)?;
                scalar(right, paths)?;
            }
        }
        Some(())
    }
    let mut paths = Vec::new();
    match expression {
        ContentProjectionExpression::CountedQuantity { magnitude } => {
            scalar(magnitude, &mut paths)?
        }
        ContentProjectionExpression::IntervalSet { members } => {
            for member in members {
                scalar(member.start(), &mut paths)?;
                scalar(member.end(), &mut paths)?;
            }
        }
    }
    Some(paths)
}

/// Compare one content dependency with its exact entry source. The ordinary
/// origin query covers preceding statements; the capture's write prefix covers
/// effects already performed in the current statement. Neither alone suffices.
pub(crate) fn preserves_entry_observation(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &checked_trees::FlowStateFact,
    operand: &checked_trees::FlowOperandFact,
    observed: validation::CanonicalPlace,
    expected: &validation::CanonicalPlace,
) -> bool {
    crate::flow::operand_entry_origin_is_stable(program, &facts.flow, operand, &observed)
        && crate::flow::value_origin_before_statement(
            program,
            &facts.flow,
            machine,
            state,
            operand.statement_index,
            observed,
            None,
            crate::flow::ValueObservation::Contents,
        )
        .is_some_and(|origin| origin == *expected)
}
