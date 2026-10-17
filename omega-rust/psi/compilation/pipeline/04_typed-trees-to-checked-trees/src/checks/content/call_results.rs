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
    DomainEstablishmentRoute, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, QualificationEvidenceOrigin,
};
use symbols::SymbolHandle;
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
    program: &dyn ::validation::CheckingInput,
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
        let Some(domain) =
            typed_trees::domain::domain_by_symbol(program, domain_symbol).filter(|_| {
                validation::field_domains::domain_requires_provenance(program, domain_symbol)
            })
        else {
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
            let site = validation::find_call_site(
                program,
                machine_symbol,
                state_symbol,
                statement_index,
                call_ordinal,
            )?;
            let is_result_place = matches!(
                (place.root, site),
                (
                    PlaceRoot::Expression(expression),
                    validation::CallSite::Expression {
                        expression: actual,
                        ..
                    }
                ) if expression == actual
            );
            if !is_result_place {
                return call_parameter_qualification_join(
                    program,
                    facts,
                    &fact.evidence,
                    place.root,
                    path,
                    domain_symbol,
                    semantic_domain,
                    site,
                    fact.point,
                );
            }
            let (PlaceRoot::Expression(expression), validation::CallSite::Expression { call, .. }) =
                (place.root, site)
            else {
                return None;
            };
            if fact.evidence.origin != QualificationEvidenceOrigin::Propagated
                || fact.evidence.source_symbol != call.target_symbol
                || fact.evidence.requirement_symbol.is_valid()
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
            if validation::find_machine_head(program, call.target_symbol)
                .is_some_and(|(machine, _)| machine.body_is_present)
            {
                return Some(());
            }

            // A requirement publishes an obligation, not an issuer grant.
            // Keep its exact declared subject until installation: the selected
            // checked provider must return this membership, or the crossing
            // must independently authorize its introduction. Terminal retains
            // the domain's route catalog and the requirement's result paths,
            // so imported supply cannot replace private issuer provenance with
            // an annotation. Do not choose a possible satisfier here: Psi
            // checks the same caller before any target/provider is selected.
            // Content carries an additional conservation obligation, kept in
            // the routes below: promising provenance cannot create capacity.
            // Authorized issuance likewise keeps its existing freshness join.
            if projection.is_none()
                && boundary_requirement_signature(program, call.target_symbol, domain).is_none()
                && program.traits().iter().any(|owner| {
                    owner.is_boundary
                        && program
                            .trait_machine_signature_by_symbol(owner, call.target_symbol)
                            .is_some()
                })
            {
                return Some(());
            }

            let call_parameters = validation::call_target_parameters(program, call.target_symbol);

            // Preserve the existing bare-result issuance route, including its
            // exact carrier/subject checks. Fresh supply must be fresh: a
            // call that consumes a live owned claim of the same content
            // family transfers that input, and transferred input is not
            // fresh supply. Such results must be justified through the
            // conservation or unique-forwarding routes below; counting them
            // as issuance would record the same custody twice. The route
            // does not require a receiving local, nor authorize minting
            // nested fields of another carrier.
            // Provider selection/admission remains an independent gate.
            if path.is_empty()
                && match (call_parameters, projection) {
                    (Some(parameters), Some(projection)) => !consumes_qualified_input(
                        program,
                        facts,
                        &invocation,
                        parameters,
                        projection,
                    ),
                    // Non-content provenance carries no owned custody to
                    // transfer; keep its contract-only route.
                    (Some(_), None) => true,
                    (None, _) => false,
                }
                && (facts.proof.contract_facts.iter().any(|(_, contract)| {
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
                    let Some(membership) = program.proof_facts.get(contract.fact).as_membership()
                    else {
                        return false;
                    };
                    if membership.domain_symbol != domain_symbol
                        || membership.semantic_domain != semantic_domain
                    {
                        return false;
                    }
                    authorized_ensures_signature(program, owner_symbol, state_symbol, contract.fact)
                }) || result_type_issuance_route(program, call.target_symbol, domain))
            {
                return Some(());
            }

            // A case-scoped grant: the requirement is an `established by`
            // route for the domain and the subject path selects one case's
            // payload of the owned result carrier, where that payload's
            // declared type carries this domain on the domain's own target
            // carrier. The payload annotation is the provider's issuance
            // assertion for that case — the same authority as `-> T in D` on
            // a bare result, scoped to the selected case. The declared
            // identity gate above already pinned path, domain, and algebra
            // identity; a call that consumed a live owned claim of this
            // family transfers it and must justify itself below.
            if path
                .iter()
                .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
                && case_payload_issuance_route(program, call.target_symbol, domain, path)
                && match (call_parameters, projection) {
                    (Some(parameters), Some(projection)) => !consumes_qualified_input(
                        program,
                        facts,
                        &invocation,
                        parameters,
                        projection,
                    ),
                    (Some(_), None) => true,
                    (None, _) => false,
                }
            {
                return Some(());
            }

            // Non-content theories retain the existing boundary authorization
            // rule. In particular, a routed alias without an exact direct
            // route is not silently accepted as a predicate-only theory.
            let projection = projection?;
            result_claim(program, facts, &invocation, path)?;
            let parameters = call_parameters?;
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

            // One input may return through mutually exclusive cases of one
            // result sum. These are alternative destinations, not simultaneous
            // supply. Independent record fields still require conservation.
            let outputs = declared
                .iter()
                .filter(|(_, _, identity)| *identity == semantic_domain)
                .collect::<Vec<_>>();
            if outputs.is_empty()
                || outputs.iter().enumerate().any(|(position, output)| {
                    outputs[..position].iter().any(|previous| {
                        !crate::checks::multiplicity::claim_paths_are_case_alternatives(
                            &previous.0,
                            &output.0,
                        )
                    })
                })
            {
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

/// The out-parameter direction of the call-ensures join: a routed
/// `DomainMembership` claim rooted at an exact argument place rather than the
/// result expression. The claim joins when the callable declares
/// `ensures <parameter> in <domain>` for the exact non-self parameter whose
/// completed argument denotes this place and the claim is either owed by the
/// callee's checked body or minted through an `established by` boundary
/// requirement on an exact mutable parameter — immutable parameters stay
/// caller premises, so no boundary route may establish into them.
#[allow(clippy::too_many_arguments)]
fn call_parameter_qualification_join(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    evidence: &facts::QualificationEvidence,
    root: PlaceRoot,
    path: &[PlaceSegment],
    domain_symbol: SymbolHandle,
    semantic_domain: language_semantics::SemanticDomainId,
    site: validation::CallSite<'_>,
    point: ProgramPoint,
) -> Option<()> {
    let ProgramPoint::CallEnsures {
        machine_symbol,
        state_symbol,
        statement_index,
        call_ordinal,
    } = point
    else {
        return None;
    };
    let call_target = match site {
        validation::CallSite::Expression { call, .. } => call.target_symbol,
        validation::CallSite::Statement(call) => call.target_symbol,
        validation::CallSite::TransitionNamed { path, .. } => path.symbol,
    };
    if evidence.origin != QualificationEvidenceOrigin::Propagated
        || evidence.source_symbol != call_target
        || evidence.requirement_symbol.is_valid()
    {
        return None;
    }
    let parameters = validation::call_target_parameters(program, call_target)?;
    let arguments = validation::call_site_argument_expressions(program, &site);
    let state_flow = facts.flow.control.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == machine_symbol && state.state_symbol == state_symbol)
            .then_some(state)
    })?;
    let call_flow = facts
        .flow
        .control
        .calls
        .span(state_flow.calls)?
        .iter()
        .find(|call| {
            call.statement_index == statement_index
                && call.call_ordinal == call_ordinal
                && call.target_symbol == call_target
        })?;
    let operands = facts.flow.control.operands.span(call_flow.operands)?;
    let receiver = match site {
        validation::CallSite::Expression { call, .. } if call.receiver.is_valid() => {
            Some(call.receiver)
        }
        _ => None,
    };
    if parameters.len() != arguments.len()
        || !operands
            .iter()
            .map(|operand| operand.expression)
            .eq(receiver.into_iter().chain(arguments.iter().copied()))
    {
        return None;
    }
    let identities = crate::flow::call_parameter_qualification_identities(program, call_target);
    let position = parameters
        .iter()
        .enumerate()
        .find_map(|(position, parameter)| {
            // Declared mutable domains are validity obligations, not new routed
            // authority. Only an authored ensures can rejoin content provenance.
            if !identities
                .iter()
                .any(|(declared, symbol, identity, from_declared_type)| {
                    !*from_declared_type
                        && *declared == position
                        && *symbol == domain_symbol
                        && *identity == semantic_domain
                })
            {
                return None;
            }
            let argument = arguments.get(position)?;
            if crate::flow::operand_binds_live_view(program, parameter.type_reference) {
                // Join the same completed reference used by flow publication.
                // Its descriptor spelling may name different storage, or may
                // already have been replaced by a later operand's evaluation.
                let operand = operands.get(position + usize::from(receiver.is_some()))?;
                return facts
                    .flow
                    .control
                    .operand_referents
                    .span(operand.referents)?
                    .iter()
                    .any(|referent| {
                        referent.root == root
                            && facts
                                .flow
                                .control
                                .operand_referent_segments
                                .span(referent.segments)
                                .is_some_and(|segments| segments == path)
                    })
                    .then_some(position);
            }
            let canonical = validation::canonical_place_from_expression_in_state(
                program,
                state_symbol,
                statement_index,
                *argument,
            )?;
            (canonical.root == root && canonical.segments.as_slice() == path).then_some(position)
        })?;
    // Checked bodies owe their authored parameter claims at every exit.
    if validation::find_state(program, call_target)
        .is_some_and(|(machine, _)| machine.body_is_present)
    {
        return Some(());
    }
    let parameter = parameters.iter().nth(position)?;
    if !crate::checks::contracts::is_readable_mutable_reference(program, parameter.type_reference) {
        return None;
    }
    facts
        .proof
        .contract_facts
        .iter()
        .any(|(_, contract)| {
            let checked_trees::ContractProofFactOwner::StateSignature {
                owner_symbol,
                state_symbol,
            } = contract.owner
            else {
                return false;
            };
            if state_symbol != call_target
                || contract.kind != checked_trees::ContractProofFactKind::Ensures
            {
                return false;
            }
            let Some(membership) = program.proof_facts.get(contract.fact).as_membership() else {
                return false;
            };
            if membership.domain_symbol != domain_symbol
                || membership.semantic_domain != semantic_domain
                || crate::flow::ensured_parameter_position(program, parameters, membership.value)
                    != Some(position)
            {
                return false;
            }
            authorized_ensures_signature(program, owner_symbol, state_symbol, contract.fact)
        })
        .then_some(())
}

#[allow(clippy::too_many_arguments)]
fn replay_equation(
    program: &dyn ::validation::CheckingInput,
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
            position, symbol, ..
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
        if subject.version != ContentPlaceVersion::Entry || parameter.symbol != symbol {
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

/// The signature `state_symbol` on the `owner_symbol` trait carries `ensures`
/// authorization for `fact`, when the owner and its requirement exist.
fn authorized_ensures_signature(
    program: &dyn ::validation::CheckingInput,
    owner_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    fact: arena::Handle<typed_trees::domain::ProofFact>,
) -> bool {
    program
        .trait_definition_by_symbol(owner_symbol)
        .and_then(|owner| program.trait_machine_signature_by_symbol(owner, state_symbol))
        .is_some_and(|signature| {
            ::validation::boundary_qualification_authorization(
                program,
                owner_symbol,
                signature,
                typed_trees::signature::SignatureContractKind::Ensures,
                program.proof_facts.get(fact),
            )
            .is_some()
        })
}

/// The signature a boundary trait declares under `target_symbol`, present
/// only when one of `domain`'s `established by` routes names that trait and
/// requirement as the issuance witness.
fn boundary_requirement_signature<'a>(
    program: &'a dyn ::validation::CheckingInput,
    target_symbol: SymbolHandle,
    domain: &typed_trees::domain::DomainDefinition,
) -> Option<&'a typed_trees::signature::StateSignature> {
    let owner = program.traits().iter().find(|owner| {
        owner.is_boundary
            && program
                .trait_machine_signature_by_symbol(owner, target_symbol)
                .is_some()
    })?;
    let signature = program.trait_machine_signature_by_symbol(owner, target_symbol)?;
    domain
        .establishment_routes
        .iter()
        .any(|route| {
            matches!(
                route,
                DomainEstablishmentRoute::BoundaryRequirement {
                    boundary_trait,
                    requirement,
                } if *boundary_trait == owner.symbol && *requirement == signature.symbol
            )
        })
        .then_some(signature)
}

/// The other authorized issuance spelling: a boundary requirement carries
/// its grant on the constrained result type (`-> T in D`) instead of an
/// `ensures` clause. When the domain's `established by` route names that
/// exact requirement, the signature's own declared constraint is the
/// issuance witness — the bare-result and freshness gates above still
/// apply, so transferred custody cannot mint supply here either.
fn result_type_issuance_route(
    program: &dyn ::validation::CheckingInput,
    target_symbol: SymbolHandle,
    domain: &typed_trees::domain::DomainDefinition,
) -> bool {
    let Some(signature) = boundary_requirement_signature(program, target_symbol, domain) else {
        return false;
    };
    crate::facts::qualification_evidence::unwrapped_type_references_match(
        program,
        signature.return_type,
        domain.target_type,
    )
}

/// The case-scoped spelling of `result_type_issuance_route`: instead of the
/// result itself being `T in D`, one case of the owned result carrier
/// declares its payload `T in D` and the granted subject path selects exactly
/// that payload. The `established by` route must name the called requirement,
/// and the leaf field's declared carrier must be the domain's own target
/// type — a payload that wears the constraint on an unrelated carrier cannot
/// mint here.
fn case_payload_issuance_route(
    program: &dyn ::validation::CheckingInput,
    target_symbol: SymbolHandle,
    domain: &typed_trees::domain::DomainDefinition,
    path: &[PlaceSegment],
) -> bool {
    let Some(signature) = boundary_requirement_signature(program, target_symbol, domain) else {
        return false;
    };
    qualified_leaf_carrier(program, signature.return_type, path).is_some_and(|carrier| {
        crate::facts::qualification_evidence::unwrapped_type_references_match(
            program,
            carrier,
            domain.target_type,
        )
    })
}

/// The declared type of the field at `path` inside an owned result carrier:
/// `Case` segments select the variant whose payload the following `Field`
/// resolves in, `Field` descends record storage, `FixedIndex` descends an
/// element. Anything else has no declared carrier to compare.
fn qualified_leaf_carrier(
    program: &dyn ::validation::CheckingInput,
    return_type: typed_trees::types::TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let mut carrier = return_type;
    let mut variant_scope: Option<SymbolHandle> = None;
    for segment in path {
        match segment {
            PlaceSegment::Case { variant } => {
                variant_scope = Some(*variant);
            }
            PlaceSegment::Field { symbol } => {
                let data =
                    validation::field_domains::owned_nominal_data_definition(program, carrier)?;
                let field = match variant_scope.take() {
                    Some(variant_symbol) => program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            typed_trees::data::DataMember::Variant(variant)
                                if variant.symbol == variant_symbol =>
                            {
                                Some(variant)
                            }
                            _ => None,
                        })
                        .and_then(|variant| {
                            program.data_payload_field_by_symbol(variant, *symbol)
                        })?,
                    None => program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            typed_trees::data::DataMember::Field(field)
                                if field.symbol == *symbol =>
                            {
                                Some(field)
                            }
                            _ => None,
                        })?,
                };
                carrier = field.type_reference;
            }
            PlaceSegment::FixedIndex { .. } => {
                let mut reference = carrier;
                while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
                    program.type_reference_table.type_reference(reference)
                {
                    reference = *base_type;
                }
                let typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
                    program.type_reference_table.type_reference(reference)
                else {
                    return None;
                };
                carrier = *element_type;
                variant_scope = None;
            }
            _ => return None,
        }
    }
    Some(carrier)
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

/// The actual argument expression bound to one authored formal position.
fn input_argument(
    program: &dyn ::validation::CheckingInput,
    invocation: &Invocation<'_>,
    parameters: &[StateParameter],
    position: usize,
) -> Option<ExpressionHandle> {
    let arguments = program
        .expression_table
        .expression_handles(invocation.call.arguments);
    parameters.get(position)?;
    arguments.get(position).copied()
}

/// Whether the call consumes a live owned claim carrying this exact content
/// family: a pre-call membership fact on a place rooted at an actual
/// argument, transferred into the call. Issuance must not be credited for
/// such a call — its result is at most a transfer.
fn consumes_qualified_input(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    invocation: &Invocation<'_>,
    parameters: &[StateParameter],
    projection: &ContentProjectionPlan,
) -> bool {
    parameters.iter().enumerate().any(|(position, parameter)| {
        crate::checks::multiplicity::linear_claim_frontier(program, parameter.type_reference)
            .iter()
            .any(|claim| {
                input_claim(
                    program,
                    facts,
                    invocation,
                    parameters,
                    position,
                    &claim.path,
                    projection,
                )
                .is_some()
            })
    })
}

fn input_claim(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    invocation: &Invocation<'_>,
    parameters: &[StateParameter],
    position: usize,
    path: &[PlaceSegment],
    projection: &ContentProjectionPlan,
) -> Option<PermissionClaimIdentity> {
    let parameter = parameters.get(position)?;
    let argument = input_argument(program, invocation, parameters, position)?;
    let projections = crate::flow::literal_expression_projections(
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
        let mut actual = validation::canonical_place_from_expression_in_state(
            program,
            invocation.state,
            invocation.statement,
            projection.expression,
        )?;
        actual.extend_segments(&projection.remaining);
        actual
    } else {
        let mut actual = validation::canonical_place_from_expression_in_state(
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
                facts::domain_membership_symbol_and_instance_pattern!(domain_symbol, semantic_domain)
                if domain_symbol == projection.domain && semantic_domain == projection.semantic_domain)
                && matches!(fact.place, FactPlace::Place(place)
                    if facts.semantic.places.get(place).root == actual.root
                        && facts.semantic.place_segments.span_or_empty(facts.semantic.places.get(place).segments) == actual.segments)
        })
    });
    if !qualified {
        return None;
    }
    if let Some(identity) = unique_claim(
        facts,
        invocation,
        PermissionEventKind::Transfer,
        actual.root,
        &actual.segments,
        false,
    ) {
        return Some(identity);
    }
    // A constructed argument has completed before the invocation. Its operands
    // carry the membership premises, but ownership transfers their identities
    // into the anonymous owner's field frontier before the call consumes it.
    // Join both moves by exact claim identity: an annotation, matching content,
    // or a source move without this invocation's owner transfer proves nothing.
    let transferred = unique_claim(
        facts,
        invocation,
        PermissionEventKind::Transfer,
        PlaceRoot::Expression(argument),
        path,
        false,
    )?;
    let operand = unique_claim(
        facts,
        invocation,
        PermissionEventKind::Transfer,
        actual.root,
        &actual.segments,
        true,
    )?;
    (operand == transferred).then_some(transferred)
}

fn result_claim(
    program: &dyn ::validation::CheckingInput,
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
    let state = program.state_by_symbol(invocation.state)?;
    let statement = program
        .statement_table
        .statement_at(state, invocation.statement)?;
    let mut destination = match statement {
        StatementNode::LocalData(local) if local.initial_value == invocation.expression => {
            validation::CanonicalPlace {
                root: PlaceRoot::Symbol(local.symbol),
                segments: Vec::new(),
            }
        }
        StatementNode::Assignment(assignment) if assignment.value == invocation.expression => {
            validation::canonical_place_from_expression_in_state(
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
    allow_statement_source: bool,
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
                || (allow_statement_source
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
