//! Inert policy is projected from accepted contracts and retained source
//! coordinates. Missing projection support refuses that nonempty axis; it does
//! not ask the retired checker to supply a second judgment.

use super::nominal::nominal_identity_from_symbols;
use super::published_input::{PublishedPackageReviewInput, rejected};
use crate::record::*;
use diagnostics::Diagnostic;
use semantic_vocabulary::{PackageKeyIdentity, Proposition, ScalarTerm, ValueId};
use target::TargetProfile;
use typed_trees::{TypedTrees, machine::Machine, types::TypeReferenceHandle};

type Review<T> = Result<T, Vec<Diagnostic>>;

pub(super) fn package(
    input: PublishedPackageReviewInput<'_>,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Review<PackagePolicyBaseline> {
    let source = input.custody.source_inspection().program();
    if !input
        .custody
        .build()
        .config
        .opaque_representation_selections
        .is_empty()
        || !input.custody.build().config.service_audits.is_empty()
        || !input.custody.build().config.behavior_exclusions.is_empty()
    {
        return Err(rejected(
            "Build authority and representation selections need their complete accepted disclosures",
        ));
    }
    let public_api = public_api(input, package)?;
    let callables = callables(input, target, package)?;
    let terminal_permissions = permissions(input, target, package)?;
    let providers = input.custody.providers();
    // Each empty family below has an independently checked census. A nonempty
    // family needs its complete projection, rather than an empty replacement.
    let selected_providers = super::published_providers::selected(input, target, package)?;
    let native_bindings = super::published_providers::native_inventory(input, package)?;
    if source
        .module()
        .external_providers
        .iter()
        .any(|row| row.realization_package == Some(package))
    {
        return Err(rejected(
            "external supplies need their complete accepted declaration and binding projection",
        ));
    }
    if !input
        .custody
        .execution()
        .plans()
        .assumed_bindings()
        .is_empty()
    {
        return Err(rejected(
            "assumed bindings need their complete accepted contract projection",
        ));
    }
    if input.candidate.data_definitions().iter().any(|data| {
        data.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque
            && input.candidate.symbols.symbol_package_identity(data.symbol) == Some(package)
    }) {
        return Err(rejected(
            "opaque declarations need their complete selected representation projection",
        ));
    }
    if !source.module().boundary_application_demands.is_empty()
        || !source.module().boundary_application_candidates.is_empty()
    {
        return Err(rejected(
            "boundary applications need their complete exact demanded and selected application projection",
        ));
    }
    if !source.module().opaque_representations.is_empty() {
        return Err(rejected(
            "opaque representations need their complete admitted application projection",
        ));
    }
    let (dangerous_capabilities, slack_uses) = authority(input, &callables)?;
    let representation = PackagePolicyRepresentation {
        package,
        target: representation_target(target)?,
        declarations: Vec::new(),
        producer_availability: Vec::new(),
        selected_availability: Vec::new(),
        demands: Vec::new(),
    };
    let semantic_dependencies = super::published_dependencies::project(input, package, &callables)?;
    Ok(PackagePolicyBaseline {
        package,
        target,
        public_api,
        callables,
        selected_providers,
        terminal_permissions,
        representation,
        native_bindings,
        external_supplies: Vec::new(),
        dangerous_capabilities,
        slack_uses,
        semantic_dependencies,
        boundary_applications: PackagePolicyBoundaryApplications {
            demands: Vec::new(),
            realizations: Vec::new(),
        },
        assumed_bindings: Vec::new(),
        service_audits: Vec::new(),
        restricted_build_requests: restricted_build_requests(input)?,
        component_assumptions: PackagePolicyComponentAssumption::project(providers),
    })
}

pub(super) fn nominal(
    input: PublishedPackageReviewInput<'_>,
    symbol: symbols::SymbolHandle,
) -> Review<PackageReviewNominalIdentity> {
    nominal_identity_from_symbols(
        &input.candidate.symbols,
        symbol,
        input.custody.sources().exact_toolchain_sources(),
    )
}

fn public_api(
    input: PublishedPackageReviewInput<'_>,
    package: PackageKeyIdentity,
) -> Review<PackagePolicyPublicApi> {
    let typed = input.candidate;
    let owned = |symbol| typed.symbols.symbol_package_identity(symbol) == Some(package);
    if typed
        .traits()
        .iter()
        .any(|row| row.is_public && owned(row.symbol))
        || typed
            .conformances()
            .iter()
            .any(|row| row.is_public && owned(row.symbol))
        || typed
            .domain_definitions()
            .iter()
            .any(|row| row.is_public && owned(row.symbol))
        || typed
            .propositions()
            .iter()
            .any(|row| row.is_public && owned(row.symbol))
        || typed
            .const_declarations()
            .iter()
            .any(|row| row.is_public && owned(row.symbol))
        || typed
            .operators()
            .iter()
            .any(|row| row.is_public && owned(row.symbol))
    {
        return Err(rejected(
            "public data, trait, proposition, domain, constant or operator declarations need their complete accepted schema projection",
        ));
    }
    Ok(PackagePolicyPublicApi {
        traits: Vec::new(),
        conformances: Vec::new(),
        domains: Vec::new(),
        propositions: Vec::new(),
        consts: Vec::new(),
        operators: Vec::new(),
        data: public_data(input, package)?,
    })
}

fn public_data(
    input: PublishedPackageReviewInput<'_>,
    package: PackageKeyIdentity,
) -> Review<Vec<PackagePolicyDataShape>> {
    let typed = input.candidate;
    let mut rows = Vec::new();
    for data in typed.data_definitions().iter().filter(|data| {
        data.is_public && typed.symbols.symbol_package_identity(data.symbol) == Some(package)
    }) {
        if !typed.data_type_parameters(data).is_empty()
            || !data.lifetime_parameters.is_empty()
            || !data.conformance_bounds.is_empty()
            || data.quotient.is_some()
            || !data.requires_facts.is_empty()
            || data.cleanup_selection.is_some()
        {
            return Err(rejected(
                "public data needs its complete generic, invariant, quotient or cleanup projection",
            ));
        }
        let field = |field: &typed_trees::data::DataField| -> Review<PackageReviewDataField> {
            Ok(PackageReviewDataField {
                identity: field.identity,
                name: field.name.name.to_string(),
                relevance: field.relevance,
                type_identity: type_identity(
                    typed,
                    field.type_reference,
                    input.custody.sources().exact_toolchain_sources(),
                )?,
            })
        };
        let members = typed
            .data_members(data)
            .iter()
            .map(|member| match member {
                typed_trees::data::DataMember::Field(value) => {
                    Ok(PackageReviewDataMember::Field(field(value)?))
                }
                typed_trees::data::DataMember::Variant(variant) => {
                    if !variant.requires_facts.is_empty() {
                        return Err(rejected(
                            "public variant needs its complete accepted invariant projection",
                        ));
                    }
                    Ok(PackageReviewDataMember::Variant {
                        identity: variant.identity,
                        name: variant.name.name.to_string(),
                        payload: typed
                            .data_payload_fields(variant)
                            .iter()
                            .map(&field)
                            .collect::<Review<Vec<_>>>()?,
                        retired_payload_identities: variant.retired_payload_identities.clone(),
                    })
                }
            })
            .collect::<Review<Vec<_>>>()?;
        rows.push(PackagePolicyDataShape {
            identity: nominal(input, data.symbol)?,
            kind: PackageReviewDataKind::Ordinary,
            supply: data.supply_mode,
            lifetime_parameter_count: 0,
            type_parameters: Vec::new(),
            properties: PackageReviewDataProperties {
                multiplicity: data.properties.multiplicity,
                carry: data.properties.carry,
            },
            zero_gated: data.zero_gated,
            invariants: Vec::new(),
            retired_identities: data.retired_identities.clone(),
            members,
        });
    }
    rows.sort_by(|a, b| a.identity.cmp(&b.identity));
    Ok(rows)
}

pub(super) fn permissions(
    input: PublishedPackageReviewInput<'_>,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Review<PackagePolicyTerminalPermissions> {
    super::published_services::permissions(input, target, package)
}

fn callables(
    input: PublishedPackageReviewInput<'_>,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Review<PackagePolicyCallables> {
    let product = input.custody.source_inspection().program();
    require_no_unprojected_capability_flows(product.module())?;
    let body_reaches = terminal_semantics::body_services::body_service_reaches(product.module())
        .map_err(rejected)?;
    let mut rows = Vec::new();
    for machine in input.candidate.machines() {
        if input
            .candidate
            .symbols
            .symbol_package_identity(machine.symbol)
            != Some(package)
        {
            continue;
        }
        let role = if !machine.is_public
            && machine.supply_mode == language_semantics::MachineSupplyMode::AdmissionClaim
        {
            PackagePolicyCallableRole::PrivateAssumption
        } else if !machine.is_public
            && matches!(
                machine.supply_mode,
                language_semantics::MachineSupplyMode::ExternalRealization { .. }
            )
        {
            PackagePolicyCallableRole::PrivateExternal
        } else if machine.supply_mode.is_boundary_declaration() {
            PackagePolicyCallableRole::Boundary
        } else if machine.is_public {
            PackagePolicyCallableRole::Public
        } else {
            continue;
        };
        rows.push(callable(input, machine, role, &body_reaches)?);
    }
    if let Some(selected) = input.custody.build().selected_build_machine_symbol {
        let accepted = input
            .custody
            .build()
            .accepted_build_source
            .as_ref()
            .ok_or_else(|| {
                rejected("selected Build has no retained accepted source publication")
            })?;
        let build_typed = &***accepted.typed();
        let machine = build_typed.machine_by_symbol(selected).ok_or_else(|| {
            rejected("selected Build has no exact typed declaration in its accepted source")
        })?;
        if build_typed.symbols.symbol_package_identity(selected) == Some(package) {
            // Build uses its own accepted code and formal identities. It must
            // never borrow a Product machine with a coincident symbol slot.
            require_no_unprojected_capability_flows(
                accepted.publication().artifact.semantic_module(),
            )?;
            let body_reaches = terminal_semantics::body_services::body_service_reaches(
                accepted.publication().artifact.semantic_module(),
            )
            .map_err(rejected)?;
            rows.push(build_callable(input, accepted, machine, &body_reaches)?);
        }
    }
    rows.sort_by(|a, b| a.identity.cmp(&b.identity).then(a.role.cmp(&b.role)));
    rows.dedup();
    Ok(PackagePolicyCallables {
        package,
        target,
        callables: rows,
    })
}

fn callable(
    input: PublishedPackageReviewInput<'_>,
    machine: &Machine,
    role: PackagePolicyCallableRole,
    body_reaches: &terminal_semantics::body_services::BodyServiceReaches,
) -> Review<PackagePolicyCallable> {
    if machine.supply_mode.is_boundary_declaration()
        || matches!(
            machine.supply_mode,
            language_semantics::MachineSupplyMode::ExternalRealization { .. }
        )
    {
        return Err(rejected(
            "boundary callable policy needs the complete accepted boundary signature",
        ));
    }
    let source = input.custody.source_inspection().program();
    let accepted = source
        .accepted_machine(machine.symbol)
        .map_err(|reason| rejected(&reason))?;
    let envelope = source
        .module()
        .callables
        .machine_envelope(accepted.id)
        .ok_or_else(|| rejected("accepted callable has no authenticated operational envelope"))?;
    callable_surface(
        input.candidate,
        input.custody.sources().exact_toolchain_sources(),
        machine,
        role,
        accepted,
        envelope,
        body_reaches.get(accepted.id).map_err(rejected)?,
        |service| {
            let symbol = source
                .service_symbol(service)
                .map_err(|reason| rejected(&reason))?;
            nominal(input, symbol)
        },
    )
}

fn build_callable(
    input: PublishedPackageReviewInput<'_>,
    source: &psi_terminal_production::CheckedInvocationSource,
    machine: &Machine,
    body_reaches: &terminal_semantics::body_services::BodyServiceReaches,
) -> Review<PackagePolicyCallable> {
    let typed = &***source.typed();
    let publication = source.publication();
    let mut ids = publication
        .machines
        .iter()
        .filter(|(_, symbol)| **symbol == machine.symbol);
    let (Some((id, _)), None) = (ids.next(), ids.next()) else {
        return Err(rejected("Build has no unique accepted machine coordinate"));
    };
    let module = publication.artifact.semantic_module();
    let accepted = module
        .machine(id)
        .ok_or_else(|| rejected("Build accepted machine disappeared"))?;
    let envelope = module
        .callables
        .machine_envelope(id)
        .ok_or_else(|| rejected("Build has no authenticated operational envelope"))?;
    callable_surface(
        typed,
        input.custody.sources().exact_toolchain_sources(),
        machine,
        PackagePolicyCallableRole::Build,
        accepted,
        envelope,
        body_reaches.get(id).map_err(rejected)?,
        |service| {
            let mut rows = publication
                .services
                .iter()
                .filter(|row| row.service == service);
            let (Some(row), None) = (rows.next(), rows.next()) else {
                return Err(rejected(
                    "Build service has no unique retained source coordinate",
                ));
            };
            nominal_identity_from_symbols(
                &typed.symbols,
                row.declaration,
                input.custody.sources().exact_toolchain_sources(),
            )
        },
    )
}

fn callable_surface(
    typed: &TypedTrees,
    exact_sources: &[(source::SourceId, [u8; 32])],
    machine: &Machine,
    role: PackagePolicyCallableRole,
    accepted: &terminal_psi::TerminalMachine,
    envelope: &terminal_psi::TerminalCallableEnvelope,
    body_reach: &[semantic_vocabulary::ServiceId],
    service: impl Fn(semantic_vocabulary::ServiceId) -> Review<PackageReviewNominalIdentity>,
) -> Review<PackagePolicyCallable> {
    if !typed.machine_type_parameters(machine).is_empty()
        || !machine.lifetime_parameters.is_empty()
        || !machine.conformance_bounds.is_empty()
        || !typed.machine_trait_conformances(machine).is_empty()
    {
        return Err(rejected(
            "generic or conforming callable policy needs its complete accepted telescope projection",
        ));
    }
    if !envelope.invocations.is_empty() {
        return Err(rejected(
            "callable invocation effects need their exact body occurrence and formal-substitution projection",
        ));
    }
    if envelope.suspends || envelope.blocks {
        return Err(rejected(
            "callable suspension and blocking effects need their retained body occurrence projection",
        ));
    }
    let [entry] = typed.machine_states(machine) else {
        return Err(rejected("callable source has no unique entry signature"));
    };
    let parameters = typed
        .state_parameters(entry)
        .iter()
        .map(|parameter| {
            Ok(PackageReviewSignatureParameter {
                name: parameter.name.name.to_string(),
                type_identity: signature_type_identity(
                    typed,
                    parameter.type_reference,
                    exact_sources,
                )?,
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
            })
        })
        .collect::<Review<Vec<_>>>()?;
    if !accepted.contract.erased_scalar_formals.is_empty()
        || !accepted.contract.erased_proof_formals.is_empty()
        || !accepted.contract.outcome_specific_ensures.is_empty()
    {
        return Err(rejected(
            "callable policy needs its retained erased and outcome-specific contract lanes",
        ));
    }
    let mut contracts = Vec::new();
    let mut scalar_parameters = accepted.parameters.iter();
    let mut formal_subjects = Vec::new();
    for (position, kind) in accepted.parameter_order.iter().enumerate() {
        if *kind == terminal_psi::BoundaryParameterKind::Scalar {
            let formal = scalar_parameters
                .next()
                .ok_or_else(|| rejected("accepted scalar formal order exceeds its signature"))?;
            formal_subjects.push((
                formal.id,
                u32::try_from(position).map_err(|_| rejected("formal ordinal overflows"))?,
            ));
        }
    }
    if scalar_parameters.next().is_some() {
        return Err(rejected(
            "accepted signature omits a scalar formal position",
        ));
    }
    let context = ContractSubjects {
        parameters: &formal_subjects,
        result: accepted.result.scalar().map(|row| row.id),
    };
    for fact in &accepted.contract.source_checked_requires {
        contracts.push(contract(
            PackageReviewContractKind::Requires,
            fact,
            &context,
        )?);
    }
    for fact in &accepted.contract.requires {
        contracts.push(contract(
            PackageReviewContractKind::Requires,
            fact,
            &context,
        )?);
    }
    for fact in &accepted.contract.ensures {
        contracts.push(contract(
            PackageReviewContractKind::Ensures,
            &fact.proposition,
            &context,
        )?);
    }
    contracts.sort();
    contracts.dedup();
    let mut reach = envelope
        .service_reach
        .iter()
        .map(|id| service(*id))
        .collect::<Review<Vec<_>>>()?;
    reach.sort();
    reach.dedup();
    let mut realized = body_reach
        .iter()
        .map(|id| service(*id))
        .collect::<Review<Vec<_>>>()?;
    realized.sort();
    realized.dedup();
    let mut invokes = envelope
        .invocations
        .iter()
        .map(|target| match target {
            terminal_psi::TerminalCallableInvocation::Parameter(ordinal) => {
                Ok(PackageReviewSynchronousInvocation::Parameter(*ordinal))
            }
            terminal_psi::TerminalCallableInvocation::Service(id) => {
                Ok(PackageReviewSynchronousInvocation::Service(service(*id)?))
            }
        })
        .collect::<Review<Vec<_>>>()?;
    invokes.sort();
    invokes.dedup();
    let termination = match &envelope.termination {
        terminal_psi::TerminalCallableTermination::NotGuaranteed => {
            PackagePolicyTermination::NoGuarantee
        }
        terminal_psi::TerminalCallableTermination::Terminates { premises }
            if premises.is_empty() =>
        {
            PackagePolicyTermination::Terminates {
                premises: Vec::new(),
            }
        }
        _ => {
            return Err(rejected(
                "callable progress premises need their exact accepted source domain projection",
            ));
        }
    };
    if !accepted.contract.crash_routes.is_empty() {
        return Err(rejected(
            "guarded callable crashes need their accepted guard and source subject projection",
        ));
    }
    let identity = nominal_identity_from_symbols(&typed.symbols, machine.symbol, exact_sources)?;
    let return_type = (!matches!(accepted.result, terminal_psi::TerminalMachineResult::Unit))
        .then(|| signature_type_identity(typed, entry.return_type, exact_sources))
        .transpose()?;
    Ok(PackagePolicyCallable {
        target_context: machine.authored_target.as_ref().map_or(
            PackagePolicyTargetContext::TargetIndependent,
            |target| PackagePolicyTargetContext::TargetMember {
                target: target.name.to_string(),
            },
        ),
        role,
        identity,
        supply: PackageReviewCallableSupply::CheckedBody,
        spelling: None,
        lifetime_parameter_count: 0,
        type_parameters: Vec::new(),
        conformance_bounds: Vec::new(),
        parameters,
        return_type,
        conformances: Vec::new(),
        operator_realizations: Vec::new(),
        requirement_realizations: Vec::new(),
        contracts,
        declared_service_reach: (machine.service_reach_row
            != language_semantics::ServiceReachRowId::NULL)
            .then(|| reach.clone()),
        service_reach_dependency: PackagePolicyServiceReachDependency {
            concrete: realized.clone(),
            parameters: Vec::new(),
        },
        checked_service_reach: PackageReviewCheckedServiceReach::CheckedBody {
            realized: realized.clone(),
            concrete: realized,
        },
        unresolved_installation_reaches: Vec::new(),
        declared_synchronous_invocations: (!machine.invokes.is_empty()).then(|| invokes.clone()),
        realized_synchronous_invocations: invokes,
        capability_flows: Vec::new(),
        reachable_capability_flows: Vec::new(),
        checked_may_suspend: envelope.suspends,
        checked_may_block: envelope.blocks,
        declared_may_suspend: (!machine.suspends_keyword_source_spans.is_empty())
            .then_some(machine.suspends),
        declared_may_block: (!machine.blocks_keyword_source_spans.is_empty())
            .then_some(machine.blocks),
        declared_termination: (!matches!(termination, PackagePolicyTermination::NoGuarantee))
            .then(|| termination.clone()),
        checked_termination: termination,
        checked_crash: PackagePolicyCrash {
            interface: PackageReviewCrashInterface::PublishedCeiling,
            published: Vec::new(),
            structural_runtime_requirements: None,
            inferred: PackagePolicyInferredCrash::Unknown,
        },
        mutation: PackagePolicyMutation {
            completeness: PackageReviewWriteFrameCompleteness::Opaque,
            paths: Vec::new(),
        },
    })
}

pub(super) fn type_identity(
    typed: &TypedTrees,
    reference: TypeReferenceHandle,
    exact_sources: &[(source::SourceId, [u8; 32])],
) -> Review<PackageReviewTypeIdentity> {
    if !typed
        .type_reference_table
        .contains_type_reference(reference)
    {
        return Err(rejected("type has a stale or missing source coordinate"));
    }
    let identity = typed
        .exact_owner_type_identity(typed_trees::type_identity::ExactOwnerTypeIdentityRequest {
            type_reference: reference,
            binders: &[],
            substitutions: &[],
            exact_toolchain_sources: exact_sources,
        })
        .ok_or_else(|| rejected("type has no complete source-owned canonical identity"))?;
    let canonical = typed
        .type_identity_text(identity)
        .ok_or_else(|| rejected("normalized type identity is unresolved"))?;
    struct AdmittedIdentity;
    impl language_semantics::type_identity::TypeIdentityPackageOwnerVisitor for AdmittedIdentity {
        fn enter(
            &mut self,
        ) -> Result<(), language_semantics::type_identity::TypeIdentityVisitError> {
            Ok(())
        }
        fn leave(&mut self) {}
        fn reserve(
            &mut self,
            _: usize,
        ) -> Result<(), language_semantics::type_identity::TypeIdentityVisitError> {
            Ok(())
        }
        fn package_owner(
            &mut self,
            _: [u8; 32],
        ) -> Result<(), language_semantics::type_identity::TypeIdentityVisitError> {
            Ok(())
        }
        fn embedded_name(
            &mut self,
            _: &str,
        ) -> Result<(), language_semantics::type_identity::TypeIdentityVisitError> {
            Ok(())
        }
    }
    language_semantics::type_identity::visit_type_identity_package_owners(
        &canonical,
        &mut AdmittedIdentity,
    )
    .map_err(|reason| {
        rejected(&format!(
            "unresolved or unsupported type identity: {reason}"
        ))
    })?;
    Ok(PackageReviewTypeIdentity { canonical })
}

pub(super) fn signature_type_identity(
    typed: &TypedTrees,
    reference: TypeReferenceHandle,
    exact_sources: &[(source::SourceId, [u8; 32])],
) -> Review<PackageReviewTypeIdentity> {
    let runtime = type_identity(typed, reference, exact_sources)?;
    let lifetime = super::lifetime_topology::review_lifetime_topology_with_substitutions(
        typed,
        exact_sources,
        reference,
        &[],
        &[],
        &[],
        &[],
        &mut Vec::new(),
    )?;
    Ok(PackageReviewTypeIdentity {
        canonical: super::framing::framed_identity(
            "signature-type",
            &[runtime.canonical, lifetime],
        ),
    })
}

struct ContractSubjects<'a> {
    parameters: &'a [(ValueId, u32)],
    result: Option<ValueId>,
}
fn contract(
    kind: PackageReviewContractKind,
    fact: &Proposition,
    subjects: &ContractSubjects<'_>,
) -> Review<PackageReviewCallableContract> {
    Ok(PackageReviewCallableContract {
        kind,
        result_case: None,
        binding: None,
        evidence_lane_position: None,
        fact: PackageReviewContractFact::Expression(proposition(fact, subjects)?),
    })
}
fn binary(
    operator: PackageReviewContractBinaryOperator,
    left: PackageReviewContractExpression,
    right: PackageReviewContractExpression,
) -> PackageReviewContractExpression {
    PackageReviewContractExpression::Binary {
        meaning: PackageReviewContractOperatorMeaning::Builtin,
        operator,
        left: Box::new(left),
        right: Box::new(right),
    }
}
fn proposition(
    fact: &Proposition,
    subjects: &ContractSubjects<'_>,
) -> Review<PackageReviewContractExpression> {
    use PackageReviewContractBinaryOperator as B;
    use PackageReviewContractExpression as E;
    Ok(match fact {
        Proposition::Truth => E::Boolean(true),
        Proposition::Falsehood => E::Boolean(false),
        Proposition::Equal(a, b) => binary(B::Equal, scalar(a, subjects)?, scalar(b, subjects)?),
        Proposition::LessThan(a, b) => binary(B::Less, scalar(a, subjects)?, scalar(b, subjects)?),
        Proposition::LessOrEqual(a, b) => {
            binary(B::LessOrEqual, scalar(a, subjects)?, scalar(b, subjects)?)
        }
        Proposition::Conjunction(rows) | Proposition::Disjunction(rows) => {
            let conjunction = matches!(fact, Proposition::Conjunction(_));
            let mut expressions = rows.iter().map(|row| proposition(row, subjects));
            let mut expression = expressions
                .next()
                .transpose()?
                .unwrap_or(E::Boolean(conjunction));
            for next in expressions {
                expression = binary(if conjunction { B::And } else { B::Or }, expression, next?);
            }
            expression
        }
        _ => {
            return Err(rejected(
                "contract proposition needs its complete accepted semantic subject projection",
            ));
        }
    })
}
fn scalar(
    term: &ScalarTerm,
    subjects: &ContractSubjects<'_>,
) -> Review<PackageReviewContractExpression> {
    use PackageReviewContractExpression as E;
    Ok(match term {
        ScalarTerm::Boolean(value) => E::Boolean(*value),
        ScalarTerm::Integer { value, .. } => E::Integer(value.to_string()),
        ScalarTerm::Value { id, .. } => {
            if Some(*id) == subjects.result {
                E::Result
            } else {
                let ordinal=subjects.parameters.iter().find_map(|(value,ordinal)|(*value==*id).then_some(*ordinal)).ok_or_else(||rejected("contract references a value outside its accepted formal/result telescope"))?;
                E::Parameter(ordinal)
            }
        }
        _ => {
            return Err(rejected(
                "contract scalar term needs its complete accepted arithmetic or structural projection",
            ));
        }
    })
}

fn restricted_build_requests(
    input: PublishedPackageReviewInput<'_>,
) -> Review<Vec<PackagePolicyRestrictedBuildRequest>> {
    Ok(input
        .custody
        .build_admission()
        .restricted_build_requests
        .iter()
        .map(Into::into)
        .collect())
}

pub(super) fn representation_target(
    profile: TargetProfile,
) -> Review<PackageReviewRepresentationTarget> {
    let native = profile
        .native_realization()
        .ok_or_else(|| rejected("target has no native representation"))?;
    Ok(PackageReviewRepresentationTarget {
        profile: match profile {
            TargetProfile::LinuxArm64 => PackageReviewRepresentationTargetProfile::LinuxArm64,
            TargetProfile::LinuxX64 => PackageReviewRepresentationTargetProfile::LinuxX64,
            TargetProfile::MacosArm64 => PackageReviewRepresentationTargetProfile::MacosArm64,
            TargetProfile::MacosX64 => PackageReviewRepresentationTargetProfile::MacosX64,
            TargetProfile::WindowsX64 => PackageReviewRepresentationTargetProfile::WindowsX64,
            TargetProfile::UefiX64 => PackageReviewRepresentationTargetProfile::UefiX64,
            TargetProfile::CrossPlatformCli => {
                PackageReviewRepresentationTargetProfile::CrossPlatformCli
            }
            TargetProfile::LocalUnchecked => {
                PackageReviewRepresentationTargetProfile::LocalUnchecked
            }
            TargetProfile::AlphaBootstrap => {
                return Err(rejected(
                    "bootstrap target has no native review representation",
                ));
            }
        },
        architecture: match native.architecture {
            target::Architecture::Aarch64 => PackageReviewRepresentationArchitecture::Aarch64,
            target::Architecture::X86_64 => PackageReviewRepresentationArchitecture::X86_64,
        },
        object_format: match native.object_format {
            target::ObjectFormat::Elf => PackageReviewRepresentationObjectFormat::Elf,
            target::ObjectFormat::MachO => PackageReviewRepresentationObjectFormat::MachO,
            target::ObjectFormat::Coff => PackageReviewRepresentationObjectFormat::Coff,
        },
        pointer_size: u16::try_from(native.pointer_size)
            .map_err(|_| rejected("pointer size exceeds review representation"))?,
        pointer_alignment: u16::try_from(native.pointer_alignment)
            .map_err(|_| rejected("pointer alignment exceeds review representation"))?,
    })
}

fn authority(
    input: PublishedPackageReviewInput<'_>,
    callables: &PackagePolicyCallables,
) -> Review<(
    Vec<PackageReviewDangerousAuthority>,
    Vec<PackageReviewDangerousAuthoritySlack>,
)> {
    let catalog = input
        .custody
        .source_inspection()
        .inventory()
        .retained_service_catalog();
    let services = provider_planning::service_declarations::ServiceDeclarations::prepare(
        catalog.declared_traits(),
        &input.custody.providers().boundary_calling_plan_realizations,
    );
    let mut dangerous = Vec::new();
    let mut slack = Vec::new();
    for definition in input.candidate.service_reaches.definitions() {
        let Some(class) =
            super::authority::dangerous_authority_class(&input, &services, definition)?
        else {
            continue;
        };
        let service = nominal(input, definition.symbol)?;
        if callables.callables.iter().any(|callable| {
            callable
                .declared_service_reach
                .as_ref()
                .is_some_and(|rows| rows.contains(&service))
                || callable
                    .checked_service_reach
                    .realized()
                    .is_some_and(|rows| rows.contains(&service))
        }) {
            dangerous.push(PackageReviewDangerousAuthority {
                class,
                service: service.clone(),
            });
        }
        for callable in &callables.callables {
            if callable
                .declared_service_reach
                .as_ref()
                .is_some_and(|rows| rows.contains(&service))
                && callable
                    .checked_service_reach
                    .realized()
                    .is_some_and(|rows| !rows.contains(&service))
            {
                slack.push(PackageReviewDangerousAuthoritySlack {
                    class,
                    callable: callable.identity.clone(),
                    service: service.clone(),
                });
            }
        }
    }
    dangerous.sort();
    dangerous.dedup();
    slack.sort();
    slack.dedup();
    Ok((dangerous, slack))
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerType, ScalarType};

    #[test]
    fn contract_subjects_keep_source_formal_positions_after_structural_inputs() {
        let value = ValueId::new(13).unwrap();
        let result = ValueId::new(41).unwrap();
        let formals = [(value, 1)];
        let subjects = ContractSubjects {
            parameters: &formals,
            result: Some(result),
        };
        assert_eq!(
            scalar(
                &ScalarTerm::Value {
                    id: value,
                    scalar_type: ScalarType::Integer(IntegerType::U64)
                },
                &subjects
            )
            .unwrap(),
            PackageReviewContractExpression::Parameter(1)
        );
        assert_eq!(
            scalar(
                &ScalarTerm::Value {
                    id: result,
                    scalar_type: ScalarType::Integer(IntegerType::U64)
                },
                &subjects
            )
            .unwrap(),
            PackageReviewContractExpression::Result
        );
    }

    #[test]
    fn contract_subjects_refuse_an_unrelated_accepted_value() {
        let formals = [(ValueId::new(13).unwrap(), 1)];
        let subjects = ContractSubjects {
            parameters: &formals,
            result: None,
        };
        let unrelated = ScalarTerm::Value {
            id: ValueId::new(14).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::U64),
        };
        assert!(scalar(&unrelated, &subjects).is_err());
    }

    #[test]
    fn unsupported_contract_propositions_do_not_disappear() {
        let subjects = ContractSubjects {
            parameters: &[],
            result: None,
        };
        assert!(
            proposition(
                &Proposition::Atom(semantic_vocabulary::PropositionId::new(1).unwrap()),
                &subjects
            )
            .is_err()
        );
    }
}

fn require_no_unprojected_capability_flows(module: &terminal_psi::TerminalModule) -> Review<()> {
    // Service reach is an operational ceiling, not a capability-flow verb.
    // This projection serves ordinary storage and primitive service arguments;
    // access transitions and resource-bearing contracts require their own
    // retained subject/verb receipts before they can be disclosed.
    if !module.access_view_inputs.is_empty()
        || !module.access_view_plans.is_empty()
        || !module.access_lifecycle_plans.is_empty()
        || module.boundary_machines.iter().any(|row| {
            !row.requires.is_empty()
                || !row.normal_qualifications.is_empty()
                || !row.scalar_domain_requirements.is_empty()
                || !row.content_guarantees.is_empty()
                || !row.program_local_root_introductions.is_empty()
        })
    {
        return Err(rejected(
            "capability transitions need their exact accepted subjects and flow verbs",
        ));
    }
    Ok(())
}
