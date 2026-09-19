//! Replay provisional owned result qualifications against the exact invocation.
//!
//! Flow publishes normal-return promises before ownership checking. This gate
//! joins their structural subjects to the completed ownership ledger; a result
//! annotation or a compatible content algebra cannot substitute for that join.

use super::content_paths::content_segments_to_fact_path;
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use facts::{FactOrigin, FactPayload, FactPlace, PlaceRoot, PlaceSegment, ProgramPoint};
use language_semantics::content::{
    ContentConservationTerm, ContentPlaceRoot, ContentPlaceVersion, ContentProjectionPlan,
    ContentStructuralPlace,
};
use language_semantics::{
    PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    QualificationEvidenceOrigin,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, TableCallExpression};
use typed_trees::signature::StateParameter;
use typed_trees::statement::StatementNode;

struct Invocation<'program> {
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    ordinal: usize,
    expression: ExpressionHandle,
    call: &'program TableCallExpression,
}

pub(super) fn check_call_result_qualifications(
    program: &TypedTrees,
    facts: &CheckFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (_, fact) in facts.semantic.facts.iter() {
        let (FactOrigin::CallEnsures, FactPlace::Place(place)) = (fact.origin, fact.place) else {
            continue;
        };
        let FactPayload::DomainMembership {
            domain_symbol,
            semantic_domain,
            ..
        } = fact.payload
        else {
            continue;
        };
        let Some(domain) = program.domain_definitions().iter().find(|domain| {
            domain.symbol == domain_symbol
                && crate::facts::field_domain::domain_requires_provenance(program, domain_symbol)
        }) else {
            continue;
        };
        let place = facts.semantic.places.get(place);
        let path = facts.semantic.place_segments.span_or_empty(place.segments);
        let valid = (|| {
            let ProgramPoint::CallEnsures {
                machine_symbol,
                state_symbol,
                statement_index,
                call_ordinal,
            } = fact.point
            else {
                return None;
            };
            let PlaceRoot::Expression(expression) = place.root else {
                return None;
            };
            let Some(crate::semantic_calls::CallSite::Expression {
                expression: actual,
                call,
            }) = crate::semantic_calls::find_call_site(
                program,
                machine_symbol,
                state_symbol,
                statement_index,
                call_ordinal,
            )
            else {
                return None;
            };
            if actual != expression
                || fact.evidence.origin != QualificationEvidenceOrigin::Propagated
                || fact.evidence.source_symbol != call.target_symbol
                || fact.evidence.requirement_symbol.is_valid()
                || fact.evidence.receipt_identity != 0
            {
                return None;
            }
            let invocation = Invocation {
                machine: machine_symbol,
                state: state_symbol,
                statement: statement_index,
                ordinal: call_ordinal,
                expression,
                call,
            };
            let declared =
                crate::flow::call_result_qualification_identities(program, call.target_symbol);
            if !declared.iter().any(|(declared_path, symbol, identity)| {
                declared_path == path && *symbol == domain_symbol && *identity == semantic_domain
            }) {
                return None;
            }
            let projection = facts
                .qualifications
                .content
                .for_semantic_domain(semantic_domain);
            // Checked bodies owe routed field membership at every exit. The
            // multiplicity checker independently replays their returned claim
            // maps, including identity-forwarding wrappers without a theorem.
            if program.machines().iter().any(|machine| {
                machine.body_is_present
                    && program
                        .machine_states(machine)
                        .iter()
                        .any(|state| state.symbol == call.target_symbol)
            }) {
                return Some(());
            }

            // Preserve the existing bare-result issuance route, including its
            // exact carrier/subject checks. It does not require a receiving
            // local, nor authorize minting nested fields of another carrier.
            // Provider selection/admission remains an independent gate.
            if path.is_empty()
                && facts.proof.contract_facts.iter().any(|(_, contract)| {
                    let checked_trees::ContractProofFactOwner::StateSignature {
                        owner_symbol,
                        state_symbol,
                    } = contract.owner
                    else {
                        return false;
                    };
                    if state_symbol != call.target_symbol
                        || contract.kind != checked_trees::ContractProofFactKind::Ensures
                    {
                        return false;
                    }
                    let typed_trees::domain::ProofFact::Membership(membership) =
                        program.proof_facts.get(contract.fact)
                    else {
                        return false;
                    };
                    if membership.domain_symbol != domain_symbol
                        || membership.semantic_domain != semantic_domain
                    {
                        return false;
                    }
                    program
                    .traits()
                    .iter()
                    .find(|owner| owner.symbol == owner_symbol)
                    .and_then(|owner| {
                        program
                            .trait_machine_signatures(owner)
                            .iter()
                            .find(|signature| signature.symbol == state_symbol)
                    })
                    .is_some_and(|signature| {
                        crate::facts::qualification_evidence::boundary_qualification_authorization(
                            program,
                            owner_symbol,
                            signature,
                            typed_trees::signature::SignatureContractKind::Ensures,
                            contract.fact,
                        )
                        .is_some()
                    })
                })
            {
                return Some(());
            }

            // Non-content theories retain the existing boundary authorization
            // rule. In particular, a routed alias without an exact direct
            // route is not silently accepted as a predicate-only theory.
            let projection = projection?;
            result_claim(program, facts, &invocation, path)?;
            let parameters =
                crate::semantic_calls::call_target_parameters(program, call.target_symbol)?;
            if facts
                .qualifications
                .content
                .conservation_plans
                .iter()
                .any(|plan| {
                    plan.callable == call.target_symbol
                        && plan.algebra == projection.algebra
                        && [
                            (plan.equation.left(), plan.equation.right()),
                            (plan.equation.right(), plan.equation.left()),
                        ]
                        .into_iter()
                        .any(|(input, output)| {
                            replay_equation(
                                program,
                                facts,
                                &invocation,
                                parameters,
                                projection,
                                path,
                                input,
                                output,
                            )
                        })
                })
            {
                return Some(());
            }

            let outputs = declared
                .iter()
                .filter(|(_, _, identity)| *identity == semantic_domain)
                .count();
            if outputs != 1 {
                return None;
            }
            let mut inputs = Vec::new();
            for (position, parameter) in parameters.iter().enumerate() {
                for claim in crate::checks::multiplicity::linear_claim_frontier(
                    program,
                    parameter.type_reference,
                ) {
                    if let Some(identity) = input_claim(
                        program,
                        facts,
                        &invocation,
                        parameters,
                        position,
                        &claim.path,
                        projection,
                    ) {
                        inputs.push(identity);
                    }
                }
            }
            // The only implicit bodyless relationship is unique identity
            // forwarding of this same qualification, not same-algebra minting.
            (inputs.len() == 1).then_some(())
        })()
        .is_some();
        if !valid {
            diagnostics.push(Diagnostic::error(format!(
                "cannot establish call-result qualification `{}`: the exact invocation, authorized route or consumed qualified claims, and result correspondence are not proved",
                domain.name,
            )));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn replay_equation(
    program: &TypedTrees,
    facts: &CheckFacts,
    invocation: &Invocation<'_>,
    parameters: &[StateParameter],
    projection: &ContentProjectionPlan,
    required_path: &[PlaceSegment],
    input: &ContentConservationTerm,
    output: &ContentConservationTerm,
) -> bool {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    if !projection_places(input, projection, &mut inputs)
        || !projection_places(output, projection, &mut outputs)
        || inputs.is_empty()
        || outputs.is_empty()
    {
        return false;
    }
    let mut input_identities = Vec::new();
    for subject in inputs {
        let ContentPlaceRoot::Parameter {
            position,
            symbol,
            is_self,
            ..
        } = subject.root
        else {
            return false;
        };
        let Ok(position) = usize::try_from(position) else {
            return false;
        };
        let Some(parameter) = parameters.get(position) else {
            return false;
        };
        let Some(path) = content_segments_to_fact_path(&subject.segments) else {
            return false;
        };
        if subject.version != ContentPlaceVersion::Entry
            || parameter.symbol != symbol
            || parameter.is_self != is_self
        {
            return false;
        }
        let Some(identity) = input_claim(
            program, facts, invocation, parameters, position, &path, projection,
        ) else {
            return false;
        };
        if input_identities.contains(&identity) {
            return false;
        }
        input_identities.push(identity);
    }
    let mut output_identities = Vec::new();
    let mut includes_required = false;
    for subject in outputs {
        if subject.root != ContentPlaceRoot::Result
            || subject.version != ContentPlaceVersion::Current
        {
            return false;
        }
        let Some(path) = content_segments_to_fact_path(&subject.segments) else {
            return false;
        };
        includes_required |= path == required_path;
        let Some(identity) = result_claim(program, facts, invocation, &path) else {
            return false;
        };
        if output_identities.contains(&identity) {
            return false;
        }
        output_identities.push(identity);
    }
    includes_required
}

fn projection_places<'term>(
    term: &'term ContentConservationTerm,
    projection: &ContentProjectionPlan,
    places: &mut Vec<&'term ContentStructuralPlace>,
) -> bool {
    match term {
        ContentConservationTerm::Projection {
            domain,
            semantic_domain,
            projection_machine,
            projection_report_fingerprint,
            subject,
        } => {
            if *domain != projection.domain
                || *semantic_domain != projection.semantic_domain
                || *projection_machine != projection.machine
                || *projection_report_fingerprint != projection.report_fingerprint
            {
                return false;
            }
            places.push(subject);
            true
        }
        ContentConservationTerm::Separate(children) => children
            .iter()
            .all(|child| projection_places(child, projection, places)),
    }
}

fn input_claim(
    program: &TypedTrees,
    facts: &CheckFacts,
    invocation: &Invocation<'_>,
    parameters: &[StateParameter],
    position: usize,
    path: &[PlaceSegment],
    projection: &ContentProjectionPlan,
) -> Option<PermissionClaimIdentity> {
    let arguments = program
        .expression_table
        .expression_handles(invocation.call.arguments);
    let explicit_self =
        parameters.iter().any(|parameter| parameter.is_self) && arguments.len() == parameters.len();
    let parameter = parameters.get(position)?;
    let argument = if parameter.is_self && !explicit_self {
        invocation.call.receiver
    } else {
        let argument_position = parameters[..position]
            .iter()
            .filter(|parameter| explicit_self || !parameter.is_self)
            .count();
        *arguments.get(argument_position)?
    };
    let projections = crate::flow::literal_value_projections(
        program,
        argument,
        parameter.type_reference,
        path,
        false,
    );
    let actual = if let Some(projections) = projections {
        let [projection] = projections.as_slice() else {
            return None;
        };
        let mut actual = crate::flow::canonical_place_from_expression_in_state(
            program,
            invocation.state,
            invocation.statement,
            projection.expression,
        )?;
        actual.extend_segments(&projection.remaining);
        actual
    } else {
        let mut actual = crate::flow::canonical_place_from_expression_in_state(
            program,
            invocation.state,
            invocation.statement,
            argument,
        )?;
        actual.extend_segments(path);
        actual
    };
    // Membership is a live pre-call fact about the actual claim, not a
    // declaration annotation. This also admits qualification expressed solely
    // by a checked `requires` contract without treating it as new authority.
    let state_flow = facts.flow.control.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == invocation.machine && state.state_symbol == invocation.state)
            .then_some(state)
    })?;
    let call_flow = facts
        .flow
        .control
        .calls
        .span_or_empty(state_flow.calls)
        .iter()
        .find(|call| {
            call.statement_index == invocation.statement
                && call.call_ordinal == invocation.ordinal
                && call.target_symbol == invocation.call.target_symbol
        })?;
    let qualified = facts.flow.contexts.semantic_context_refs.span_or_empty(call_flow.entry_semantic_contexts).iter().any(|reference| {
        facts.semantic.context_view(facts.semantic.contexts.get(reference.context)).facts().any(|fact| {
            matches!(fact.payload,
                FactPayload::DomainMembership { domain_symbol, semantic_domain, .. }
                | FactPayload::ContractDomainMembership { domain_symbol, semantic_domain, .. }
                if domain_symbol == projection.domain && semantic_domain == projection.semantic_domain)
                && matches!(fact.place, FactPlace::Place(place)
                    if facts.semantic.places.get(place).root == actual.root
                        && facts.semantic.place_segments.span_or_empty(facts.semantic.places.get(place).segments) == actual.segments)
        })
    });
    if !qualified {
        return None;
    }
    unique_claim(
        facts,
        invocation,
        PermissionEventKind::Transfer,
        actual.root,
        &actual.segments,
        false,
    )
}

fn result_claim(
    program: &TypedTrees,
    facts: &CheckFacts,
    invocation: &Invocation<'_>,
    path: &[PlaceSegment],
) -> Option<PermissionClaimIdentity> {
    if let Some(identity) = unique_claim(
        facts,
        invocation,
        PermissionEventKind::Establish,
        PlaceRoot::Expression(invocation.expression),
        path,
        false,
    ) {
        return Some(identity);
    }
    let state = crate::semantic_calls::find_state(program, invocation.state)?;
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(invocation.statement)?;
    let mut destination = match statement {
        StatementNode::LocalData(local) if local.initial_value == invocation.expression => {
            crate::flow::CanonicalPlace {
                root: PlaceRoot::Symbol(local.symbol),
                segments: Vec::new(),
            }
        }
        StatementNode::Assignment(assignment) if assignment.value == invocation.expression => {
            crate::flow::canonical_place_from_expression_in_state(
                program,
                invocation.state,
                invocation.statement,
                assignment.target,
            )?
        }
        _ => return None,
    };
    destination.extend_segments(path);
    unique_claim(
        facts,
        invocation,
        PermissionEventKind::Establish,
        destination.root,
        &destination.segments,
        true,
    )
}

fn unique_claim(
    facts: &CheckFacts,
    invocation: &Invocation<'_>,
    kind: PermissionEventKind,
    root: PlaceRoot,
    path: &[PlaceSegment],
    direct_local: bool,
) -> Option<PermissionClaimIdentity> {
    let source = PermissionEventSource::Call {
        statement_index: invocation.statement,
        call_ordinal: invocation.ordinal,
        target_symbol: invocation.call.target_symbol,
    };
    let mut identity = None;
    for (_, event) in facts.flow.ownership.permissions.iter() {
        if event.machine_symbol != invocation.machine
            || event.state_symbol != invocation.state
            || event.kind != kind
            || event.access != PermissionAccess::Owned
            || !event.obligation_live
            || event.root != root
            || facts.flow.ownership.segments.span_or_empty(event.segments) != path
            || !(event.source == source
                || (direct_local
                    && event.source
                        == PermissionEventSource::Statement {
                            statement_index: invocation.statement,
                        }))
            || event.claim_identity == PermissionClaimIdentity::Unknown
        {
            continue;
        }
        if identity.is_some_and(|previous| previous != event.claim_identity) {
            return None;
        }
        identity = Some(event.claim_identity);
    }
    identity
}

#[cfg(test)]
mod tests {
    use super::check_call_result_qualifications;
    use checked_trees::CheckedTrees;
    use facts::{FactOrigin, FactPayload, FactPlace, PlaceRoot, PlaceSegment, ProgramPoint};
    use language_semantics::{PermissionAccess, PermissionEventKind, PermissionEventSource};
    use source_files_to_tokens::Lexer;
    use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};

    fn fixture(requires_only: bool) -> CheckedTrees {
        let source = r#"
            data ByteUnit {}
            data CountedQuantity<Unit> { magnitude: u64; }
            trait Content<A> { machine project(subject: &Self) -> A; }
            data Region [linear] { length: u64; }
            domain Region::Granted established by Provider::grant;
            machine Granted::content(region: &Region) -> CountedQuantity<ByteUnit>
            satisfies Content<CountedQuantity<ByteUnit>>::project
            { CountedQuantity { magnitude: region.length } }
            boundary trait Provider {
                machine grant(raw: Region) -> Region ensures result in Granted;
            }
            data Parts { left: Region in Granted; right: Region in Granted; }
            boundary trait Partition {
                machine rearrange(PARAMETERS) -> Parts
                REQUIREMENTS
                ensures separate(Granted::content(old(&first)), Granted::content(old(&second)))
                    == separate(Granted::content(&result.left), Granted::content(&result.right));
            }
            machine forward(partition: &Partition, PARAMETERS) -> Parts
            REQUIREMENTS
            reaches Partition
            {
                let parts: Parts = partition.rearrange(first, second);
                Parts { left: parts.left, right: parts.right }
            }
        "#
        .replace(
            "PARAMETERS",
            if requires_only {
                "first: Region, second: Region"
            } else {
                "first: Region in Granted, second: Region in Granted"
            },
        )
        .replace(
            "REQUIREMENTS",
            if requires_only {
                "requires first in Granted, second in Granted"
            } else {
                ""
            },
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize replay fixture");
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse replay fixture");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve replay fixture");
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type replay fixture");
        let checked = crate::lower_typed_trees(typed).expect("check original replay fixture");
        assert_replay(&checked, true);
        checked
    }

    fn assert_replay(checked: &CheckedTrees, accepted: bool) {
        let mut diagnostics = Vec::new();
        check_call_result_qualifications(&checked.typed, &checked.facts, &mut diagnostics);
        assert_eq!(
            diagnostics.is_empty(),
            accepted,
            "replay diagnostics: {diagnostics:#?}"
        );
        if !accepted {
            assert!(
                diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic.message.contains("exact invocation"))
            );
        }
    }

    fn result_fact(checked: &CheckedTrees) -> facts::FactHandle {
        checked.facts.semantic.facts.iter().find_map(|(handle, fact)| {
            (fact.origin == FactOrigin::CallEnsures
                && matches!(fact.payload, FactPayload::DomainMembership { .. })
                && matches!(fact.place, FactPlace::Place(place)
                    if matches!(checked.facts.semantic.places.get(place).root, PlaceRoot::Expression(_))
                        && !checked.facts.semantic.places.get(place).segments.is_empty()))
            .then_some(handle)
        }).expect("provisional result field fact")
    }

    #[test]
    fn replay_accepts_live_requires_only_input_membership() {
        fixture(true);
    }

    #[test]
    fn replay_rejects_wrong_call_ordinal() {
        let mut checked = fixture(false);
        let handle = result_fact(&checked);
        let ProgramPoint::CallEnsures { call_ordinal, .. } =
            &mut checked.facts.semantic.facts.get_mut(handle).point
        else {
            panic!("result fact must retain its invocation");
        };
        *call_ordinal += 100;
        assert_replay(&checked, false);
    }

    #[test]
    fn replay_rejects_wrong_call_target() {
        let mut checked = fixture(false);
        let handle = result_fact(&checked);
        let unrelated = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "forward")
            .expect("wrapper")
            .symbol;
        checked
            .facts
            .semantic
            .facts
            .get_mut(handle)
            .evidence
            .source_symbol = unrelated;
        assert_replay(&checked, false);
    }

    #[test]
    fn replay_rejects_undeclared_result_field() {
        let mut checked = fixture(false);
        let handle = result_fact(&checked);
        let fact = *checked.facts.semantic.facts.get(handle);
        let FactPlace::Place(original) = fact.place else {
            panic!("result place");
        };
        let root = checked.facts.semantic.places.get(original).root;
        let wrong_field = checked
            .domain_definitions()
            .iter()
            .find(|domain| domain.name.as_str().ends_with("Granted"))
            .expect("qualification")
            .symbol;
        let replacement = checked.facts.semantic.append_place(facts::Place {
            root,
            segments: arena::HandleSpan::empty(),
        });
        checked.facts.semantic.push_place_segment(
            replacement,
            PlaceSegment::Field {
                symbol: wrong_field,
            },
        );
        checked.facts.semantic.facts.get_mut(handle).place = FactPlace::Place(replacement);
        assert_replay(&checked, false);
    }

    #[test]
    fn replay_rejects_duplicate_consumed_claim_identity() {
        let mut checked = fixture(false);
        let target = checked
            .facts
            .semantic
            .facts
            .get(result_fact(&checked))
            .evidence
            .source_symbol;
        let transfers = checked.facts.flow.ownership.permissions.iter().filter_map(|(handle, event)| {
            (event.kind == PermissionEventKind::Transfer
                && event.access == PermissionAccess::Owned
                && event.obligation_live
                && matches!(event.source, PermissionEventSource::Call { target_symbol, .. } if target_symbol == target))
                .then_some((handle, event.claim_identity))
        }).collect::<Vec<_>>();
        let [first, second] = transfers.as_slice() else {
            panic!("two independently consumed inputs: {transfers:#?}");
        };
        assert_ne!(first.1, second.1, "original claims are distinct");
        checked
            .facts
            .flow
            .ownership
            .permissions
            .get_mut(second.0)
            .claim_identity = first.1;
        assert_replay(&checked, false);
    }

    #[test]
    fn unrelated_boundary_cannot_publish_noncontent_qualification() {
        let source = r#"
            domain u32::Secret established by Allowed::issue;
            boundary trait Allowed {
                machine issue() -> u32 ensures result in Secret;
            }
            boundary trait Other {
                machine issue() -> u32 ensures result in Secret;
            }
            machine receive(provider: &Other) -> u32 in Secret
            reaches Other
            {
                let value: u32 in Secret = provider.issue();
                value
            }
        "#;
        let check = |source: &str| {
            let tokens = Lexer::new(source)
                .tokenize()
                .expect("tokenize authority control");
            let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
                .expect("parse authority control");
            let resolved =
                resolve(ResolutionRequest::new(&syntax)).expect("resolve authority control");
            let typed =
                symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                    .expect("type authority control");
            crate::lower_typed_trees(typed)
        };
        let authorized = source
            .replace("provider: &Other", "provider: &Allowed")
            .replace("reaches Other", "reaches Allowed");
        check(&authorized)
            .expect("the exact allowed route establishes non-content scalar membership");
        let diagnostics =
            check(source).expect_err("the unrelated boundary cannot establish Secret");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("exact invocation")),
            "{diagnostics:#?}"
        );
    }
}
