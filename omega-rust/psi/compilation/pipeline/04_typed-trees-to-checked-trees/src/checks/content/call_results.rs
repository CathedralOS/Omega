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

mod parameter_qualifications;
use parameter_qualifications::ParameterQualificationJoin;

pub(super) struct Invocation<'program> {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement: usize,
    pub ordinal: usize,
    pub expression: ExpressionHandle,
    pub call: &'program TableCallExpression,
    /// The selected ownership walk's completed prefix, or its published ledger.
    /// A previous permission pass cannot supply evidence for this completion.
    pub events: InvocationEvents<'program>,
}

pub(super) enum InvocationEvents<'events> {
    Published(&'events CheckFacts),
    Completed(&'events [checked_trees::FlowPermissionEventFact]),
}

impl InvocationEvents<'_> {
    fn iter(&self) -> impl Iterator<Item = &checked_trees::FlowPermissionEventFact> {
        let (published, completed) = match self {
            Self::Published(facts) => (Some(*facts), None),
            Self::Completed(events) => (None, Some(*events)),
        };
        published
            .into_iter()
            .flat_map(|facts| {
                facts
                    .flow
                    .ownership
                    .permissions
                    .iter()
                    .map(|(_, event)| event)
            })
            .chain(completed.into_iter().flatten())
    }
}

impl Invocation<'_> {
    pub(super) fn evaluation(
        &self,
        program: &dyn ::validation::CheckingInput,
    ) -> Option<checked_trees::FlowPermissionEvaluation> {
        let evaluation = crate::flow::call_permission_evaluation(
            program,
            program.machine_by_symbol(self.machine)?,
            program.state_by_symbol(self.state)?,
            self.statement,
            self.ordinal,
        );
        (evaluation != checked_trees::FlowPermissionEvaluation::Unknown).then_some(evaluation)
    }
}

pub(super) fn check_call_result_qualifications(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Only immutable subject preparation is reused. Each promise still joins
    // its own evidence and, for bodyless calls, its exact entry membership.
    let mut parameter_join = None;
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
        let mut primitive_error = None;
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
                if !matches!(&parameter_join, Some((point, _)) if *point == fact.point) {
                    parameter_join = Some((
                        fact.point,
                        ParameterQualificationJoin::prepare(program, facts, site, fact.point),
                    ));
                }
                return parameter_join.as_ref()?.1.as_ref()?.check(
                    &fact.evidence,
                    place.root,
                    path,
                    domain_symbol,
                    semantic_domain,
                );
            }
            let (PlaceRoot::Expression(expression), validation::CallSite::Expression { call, .. }) =
                (place.root, site)
            else {
                return None;
            };
            let public_contract =
                validation::requirement_contract_signature(program, machine_symbol, &site);
            // Rewriting may replace an evidence child with an executable body
            // while flow retains its public requirement as the fact's source.
            // Only the validated invocation contract joins those identities.
            let exact_source = fact.evidence.source_symbol == call.target_symbol
                || public_contract
                    .is_some_and(|(_, signature)| fact.evidence.source_symbol == signature.symbol);
            if fact.evidence.origin != QualificationEvidenceOrigin::Propagated
                || !exact_source
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
                events: InvocationEvents::Published(facts),
            };
            let selected = public_contract
                .map(|(_, signature)| signature)
                .or_else(|| program.requirement_call_signature(machine_symbol, call));
            let declared = crate::flow::call_result_qualification_identities(
                program,
                call.target_symbol,
                selected,
            );
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
            if validation::call_target_has_checked_body_contract(program, call.target_symbol) {
                return Some(());
            }
            // Every ordinary selected conformance owes its full public result
            // contract, including an open evidence binder's eventual selection.
            // The body independently proves carrier, predicates and custody.
            // Boundary requirements retain their separate admission and
            // conservation obligations below.
            if public_contract.is_some_and(|(declaration, _)| !declaration.is_boundary) {
                return Some(());
            }

            // This is a conditional normal-return contract, not an admitted
            // provider receipt. The exact sealed operation remains an
            // unresolved compiler execution obligation in Terminal. Both
            // execution engines refuse generic provider/handler fallback.
            // In particular Granted still follows conservation below: these
            // private routes authorize content transition, not fresh storage.
            match private_placement_result_contract(program, &invocation, domain, path) {
                Ok(true) => {
                    if result_claim(program, facts, &invocation, path).is_none() {
                        primitive_error = Some(Diagnostic::error(
                            "placement primitive result has no exact owned result claim",
                        ));
                        return None;
                    }
                    return Some(());
                }
                Ok(false) => {}
                Err(error) => {
                    primitive_error = Some(error);
                    return None;
                }
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

            let call_parameters = selected
                .map(|signature| program.state_signature_parameters(signature))
                .or_else(|| validation::call_target_parameters(program, call.target_symbol));

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
                    let exact_call = facts.proof.contract_call_at(&(
                        invocation.machine,
                        invocation.state,
                        invocation.statement,
                        invocation.ordinal,
                    ));
                    let (owner_symbol, state_symbol, applied) = match contract.owner {
                        checked_trees::ContractProofFactOwner::CallSignature {
                            owner_symbol,
                            state_symbol,
                            ..
                        } => {
                            let Some(exact_call) = exact_call else {
                                return false;
                            };
                            if !crate::proof::contracts::call_signatures::owns_contract(
                                program,
                                &facts.proof,
                                exact_call,
                                contract,
                            ) {
                                return false;
                            }
                            (owner_symbol, state_symbol, true)
                        }
                        checked_trees::ContractProofFactOwner::StateSignature {
                            owner_symbol,
                            state_symbol,
                        } if !exact_call.is_some_and(|call| call.signature.is_valid()) => {
                            (owner_symbol, state_symbol, false)
                        }
                        _ => return false,
                    };
                    if state_symbol != call.target_symbol
                        || contract.kind != checked_trees::ContractProofFactKind::Ensures
                    {
                        return false;
                    }
                    let selected_fact = if applied {
                        Some(program.proof_facts.get(contract.fact))
                    } else {
                        invocation_contract_fact(
                            program,
                            selected,
                            owner_symbol,
                            state_symbol,
                            contract.fact,
                        )
                    };
                    let Some(membership) = selected_fact.and_then(|fact| fact.as_membership())
                    else {
                        return false;
                    };
                    if membership.domain_symbol != domain_symbol
                        || membership.semantic_domain != semantic_domain
                    {
                        return false;
                    }
                    authorized_ensures_signature(program, owner_symbol, state_symbol, contract.fact)
                }) || result_type_issuance_route(program, call.target_symbol, selected, domain))
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
                && case_payload_issuance_route(program, call.target_symbol, selected, domain, path)
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
            if let Some(error) = primitive_error {
                diagnostics.push(error);
                continue;
            }
            let location = match fact.point {
                ProgramPoint::CallEnsures {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    call_ordinal,
                } => format!(
                    " in `{}`, state `{}` (statement {statement_index}, call {call_ordinal})",
                    crate::labels::symbol_name(program, machine_symbol),
                    crate::labels::symbol_name(program, state_symbol),
                ),
                _ => String::new(),
            };
            diagnostics.push(Diagnostic::error(format!(
                "cannot establish call-result qualification `{}`{location}: the exact invocation, authorized route or consumed qualified claims, and result correspondence are not proved",
                domain.name,
            )));
        }
    }
}

/// Recognize only an authenticated closed lifecycle operation whose exact
/// result subject and evaluated P/T plan survive preparation. An arbitrary
/// bodyless exact-machine route is not a compiler implementation.
fn private_placement_result_contract(
    program: &dyn ::validation::CheckingInput,
    invocation: &Invocation<'_>,
    domain: &typed_trees::domain::DomainDefinition,
    path: &[PlaceSegment],
) -> Result<bool, Diagnostic> {
    let callable = invocation.call.target_symbol;
    let Some(declaration) =
        typed_trees::typed_trees::PlacementExecutionDeclaration::authenticate(program, callable)?
    else {
        return Ok(false);
    };
    if !domain.establishment_routes.iter().any(|route| {
        matches!(route, DomainEstablishmentRoute::ExactMachine { machine }
            if *machine == declaration.template())
    }) {
        return Ok(false);
    }
    let (policy, schema) = declaration.invocation_operands(
        program,
        invocation.machine,
        invocation.call.authored_machine_application,
    )?;
    let nominal = |reference| match program.type_reference_table.type_reference(reference) {
        typed_trees::types::TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
        _ => None,
    };
    let mut plans = program
        .placement_lifecycle_plans
        .iter()
        .filter_map(|(_, plan)| match plan {
            typed_trees::typed_trees::PlacementLifecyclePlan::Evaluated {
                callable: candidate,
                operation,
                policy,
                schema,
                ..
            } if *candidate == declaration.machine() => Some((*operation, *policy, *schema)),
            _ => None,
        });
    let expected = nominal(policy)
        .zip(nominal(schema))
        .map(|(policy, schema)| (declaration.operation(), policy, schema));
    if expected.is_none() || plans.next() != expected || plans.next().is_some() {
        return Err(Diagnostic::error(
            "placement primitive result lost its unique evaluated operation contract",
        ));
    }
    let state = program
        .state_by_symbol(declaration.state())
        .ok_or_else(|| {
            Diagnostic::error("placement primitive result lost its authenticated entry")
        })?;
    let matches = path
        .iter()
        .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
        && qualified_leaf_carrier(program, state.return_type, path).is_some_and(|carrier| {
            crate::facts::qualification_evidence::unwrapped_type_references_match(
                program,
                carrier,
                domain.target_type,
            )
        });
    if !matches {
        return Err(Diagnostic::error(
            "placement primitive result does not name the exact declared case payload",
        ));
    }
    Ok(true)
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
/// Match the invocation's payload while retaining the declaration's original
/// clause as the authorization witness. Applying a telescope cannot turn a
/// provider's stronger guarantee into a public issuer grant.
fn invocation_contract_fact<'program>(
    program: &'program dyn ::validation::CheckingInput,
    selected: Option<&typed_trees::signature::StateSignature>,
    owner: SymbolHandle,
    requirement: SymbolHandle,
    fact: arena::Handle<typed_trees::domain::ProofFact>,
) -> Option<&'program typed_trees::domain::ProofFact> {
    let Some(selected) = selected else {
        return Some(program.proof_facts.get(fact));
    };
    let owner = program.trait_definition_by_symbol(owner)?;
    let declared = program.trait_machine_signature_by_symbol(owner, requirement)?;
    program.applied_signature_contract_fact(declared, selected, fact)
}

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
    let authority_signature = program
        .closed_requirement_application(target_symbol)
        .filter(|application| application.declaring_trait == owner.symbol)
        .map_or(signature.symbol, |application| application.template);
    domain
        .establishment_routes
        .iter()
        .any(|route| {
            matches!(
                route,
                DomainEstablishmentRoute::BoundaryRequirement {
                    boundary_trait,
                    requirement,
                } if *boundary_trait == owner.symbol && *requirement == authority_signature
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
    selected: Option<&typed_trees::signature::StateSignature>,
    domain: &typed_trees::domain::DomainDefinition,
) -> bool {
    let Some(signature) = boundary_requirement_signature(program, target_symbol, domain) else {
        return false;
    };
    crate::facts::qualification_evidence::unwrapped_type_references_match(
        program,
        selected.unwrap_or(signature).return_type,
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
    selected: Option<&typed_trees::signature::StateSignature>,
    domain: &typed_trees::domain::DomainDefinition,
    path: &[PlaceSegment],
) -> bool {
    let Some(signature) = boundary_requirement_signature(program, target_symbol, domain) else {
        return false;
    };
    qualified_leaf_carrier(program, selected.unwrap_or(signature).return_type, path).is_some_and(
        |carrier| {
            crate::facts::qualification_evidence::unwrapped_type_references_match(
                program,
                carrier,
                domain.target_type,
            )
        },
    )
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
                // The path belongs to the selected closed application, not
                // its unspecialized template. Do not peel a reference: that
                // would turn borrowed storage into an owned issuance subject.
                let data = program.closed_data_definition(
                    program
                        .type_reference_table
                        .unconstrained_type_reference(carrier),
                )?;
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

pub(super) fn projection_places<'term>(
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

pub(super) fn input_claim(
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
    let state_flow = facts
        .flow
        .control
        .state(invocation.machine, invocation.state)?;
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
    let (operands, receiver_offset) =
        crate::flow::captured_call_operands(program, &facts.flow, state_flow, call_flow)?;
    let operand = operands.get(position + receiver_offset)?;
    let values = crate::flow::captured_operand_values(
        program,
        &facts.flow,
        &facts.semantic,
        state_flow,
        operand,
        parameter.type_reference,
        path,
    )?;
    let [value] = values.as_slice() else {
        return None;
    };
    let places = crate::flow::captured_observation_places(program, &facts.flow, value)?;
    let [place] = places.as_slice() else {
        return None;
    };
    let actual = &place.projected;
    // A detached owned child was evaluated before its aggregate consumed it.
    // Use that exact capture's membership epoch, as ordinary call obligations
    // do. Views retain invocation-live evidence through the same shared rule.
    // The transfer joins below still owe current custody of this exact claim.
    let contexts = crate::flow::operand_contexts(
        program,
        &facts.flow.contexts,
        &facts.flow.control,
        &facts.semantic,
        value.observation,
        value.observation_type,
        call_flow.entry_constraints,
    );
    let qualified = contexts.iter().any(|context| {
        facts.semantic.context_view(facts.semantic.contexts.get(*context)).facts().any(|fact| {
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
    let evaluation = invocation.evaluation(program)?;
    let mut current = validation::canonical_place_from_expression_in_state(
        program,
        invocation.state,
        invocation.statement,
        argument,
    )?;
    current.extend_segments(path);
    let transferred = unique_claim(
        facts,
        invocation,
        evaluation,
        PermissionEventKind::Transfer,
        current.root,
        &current.segments,
    )?;
    if current == *actual {
        return Some(transferred);
    }
    // A constructed child can be established directly in its destination,
    // without an intermediate owner to move out of. Join its exact receiving
    // establishment to the captured producer and the call's current transfer.
    // A receiving type or an unrelated historical claim supplies neither join.
    invocation
        .events
        .iter()
        .any(|event| {
            event.machine_symbol == invocation.machine
                && event.state_symbol == invocation.state
                && event.kind == PermissionEventKind::Establish
                && event.access == PermissionAccess::Owned
                && event.obligation_live
                && event.evaluation == evaluation
                && event.root == current.root
                && facts.flow.ownership.segments.span_or_empty(event.segments) == current.segments
                && event.claim_identity == transferred
                && validation::receiving_value(program, &facts.flow.ownership.segments, event)
                    == Some((
                        invocation.statement,
                        value.observation_projection.expression,
                        value.observation_projection.remaining.as_slice(),
                    ))
        })
        .then_some(transferred)
}

pub(super) fn result_claim(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    invocation: &Invocation<'_>,
    path: &[PlaceSegment],
) -> Option<PermissionClaimIdentity> {
    // Ownership names the actual receiving place, which can be a temporary,
    // a returned expression, or a child of constructed storage. Join that
    // establishment back to its producer through the same operation used by
    // claim reconciliation, not a list of receiving statement spellings.
    let evaluation = invocation.evaluation(program)?;
    let call_source = PermissionEventSource::Call {
        statement_index: invocation.statement,
        call_ordinal: invocation.ordinal,
        target_symbol: invocation.call.target_symbol,
    };
    let statement_source = PermissionEventSource::Statement {
        statement_index: invocation.statement,
    };
    let mut identity = None;
    for event in invocation.events.iter() {
        if event.machine_symbol != invocation.machine
            || event.state_symbol != invocation.state
            || event.kind != PermissionEventKind::Establish
            || event.evaluation != evaluation
            || event.access != PermissionAccess::Owned
            || !event.obligation_live
            || event.claim_identity == PermissionClaimIdentity::Unknown
            || (event.source != call_source && event.source != statement_source)
        {
            continue;
        }
        if validation::receiving_value(program, &facts.flow.ownership.segments, event)
            != Some((invocation.statement, invocation.expression, path))
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

fn unique_claim(
    facts: &CheckFacts,
    invocation: &Invocation<'_>,
    evaluation: checked_trees::FlowPermissionEvaluation,
    kind: PermissionEventKind,
    root: PlaceRoot,
    path: &[PlaceSegment],
) -> Option<PermissionClaimIdentity> {
    let source = PermissionEventSource::Call {
        statement_index: invocation.statement,
        call_ordinal: invocation.ordinal,
        target_symbol: invocation.call.target_symbol,
    };
    let mut identity = None;
    for event in invocation.events.iter() {
        if event.machine_symbol != invocation.machine
            || event.state_symbol != invocation.state
            || event.kind != kind
            || event.evaluation != evaluation
            || event.access != PermissionAccess::Owned
            || !event.obligation_live
            || event.root != root
            || facts.flow.ownership.segments.span_or_empty(event.segments) != path
            || event.source != source
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
