//! Compiler run operations must retain their exact bundled declaration and
//! complete contract after receiving. Descriptor geometry identifies fields;
//! it does not authorize another callable to acquire compiler custody.

use language_core::sealed_intrinsics::{
    CompilerPrimitive, RunCustodyOperation, SealedDeclarationSite, SealedIntrinsic,
    SealedIntrinsicCatalog,
};
use language_semantics::type_identity::encoding::{CoordinateTag, atom, bare, byte_atom, compound};
use language_semantics::type_identity::vocabulary::{
    ConstraintForm, IndexForm, Label, NameForm, OwnerForm, TypeForm,
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, IntegerMathTerm as Math, IntegerSign, IntegerType,
    IntegerValue, PlaceId, Proposition, ScalarTerm, ScalarType, StructuralDomainId,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BoundaryMachineDeclaration, CheckedBoundaryCallable, CheckedBoundaryExecution,
    CheckedMachineDeclaration, ClosedReachArgument, NominalDeclarationIdentity,
    StructuralDomainDeclaration, StructuralFieldType, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalIdentityIndex, TerminalIdentityTable,
};

use crate::{RunCustodyContract, RunDescriptorRoles};

struct SourceCoordinates {
    owner: String,
    entry: String,
    semantic: String,
    callable: String,
    parameters: Vec<String>,
    result: String,
}

fn path(operation: RunCustodyOperation) -> &'static str {
    match operation {
        RunCustodyOperation::ReleaseEmpty => "Run::retire_empty",
        RunCustodyOperation::Split => "Run::split",
        RunCustodyOperation::Merge => "Run::merge",
    }
}

fn core_name(path: &str) -> String {
    compound(
        NameForm::Nominal,
        [bare(OwnerForm::Toolchain), atom(Label::Path, path)],
    )
}
fn named(name: &str) -> String {
    compound(TypeForm::Named, [atom(Label::Name, name)])
}
fn scalar(name: &str) -> String {
    named(&compound(NameForm::CompilerType, [atom(Label::Atom, name)]))
}
fn generic(path: &str, arguments: &[&str]) -> String {
    compound(
        TypeForm::Generic,
        std::iter::once(atom(Label::Name, &core_name(path)))
            .chain(arguments.iter().map(|argument| (*argument).to_owned())),
    )
}
fn qualified(base: String, domains: &[(&str, &[&str])]) -> String {
    // The ordinary canonical normalizer orders the domain identities before
    // wrapping their declared-domain constraint, and retains both layers.
    let mut domains = domains
        .iter()
        .map(|(path, arguments)| {
            compound(
                ConstraintForm::DeclaredDomain,
                std::iter::once(atom(Label::Name, &core_name(path)))
                    .chain(arguments.iter().map(|argument| (*argument).to_owned())),
            )
        })
        .collect::<Vec<_>>();
    domains.sort_unstable();
    compound(
        TypeForm::Constrained,
        std::iter::once(base).chain(
            domains.into_iter().map(|domain| {
                compound(ConstraintForm::DeclaredDomain, [atom(Label::Name, &domain)])
            }),
        ),
    )
}
fn live_run(policy: &str, element: &str) -> String {
    let arguments = [policy, element];
    qualified(generic("Run", &arguments), &[("Live", &arguments)])
}
fn source_digest() -> Result<[u8; 32], &'static str> {
    static DIGEST: std::sync::OnceLock<Result<[u8; 32], &'static str>> = std::sync::OnceLock::new();
    *DIGEST.get_or_init(|| {
        let (file, bytes) = language_core::sealed_intrinsics::run_source();
        source::source_identity::toolchain_source_identity_digest(&source::SourceFile {
            source_id: source::SourceId(0),
            path: std::path::Path::new("/compiler/core").join(file),
            package_root: "/compiler/core".into(),
            package_identity: None,
            dependency_scope: source::DependencyScope::Product,
            origin: source::SourceOrigin::Toolchain,
            resolution_stratum: source::SourceResolutionStratum::Base,
            source: std::str::from_utf8(bytes)
                .map_err(|_| "run catalog source is not UTF-8")?
                .into(),
        })
        .map_err(|_| "run catalog source has no exact identity")
    })
}

fn extent_digest() -> Result<[u8; 32], &'static str> {
    static DIGEST: std::sync::OnceLock<Result<[u8; 32], &'static str>> = std::sync::OnceLock::new();
    *DIGEST.get_or_init(|| {
        let (file, bytes) = language_core::sealed_intrinsics::extent_source();
        source::source_identity::toolchain_source_identity_digest(&source::SourceFile {
            source_id: source::SourceId(0),
            path: std::path::Path::new("/compiler/core").join(file),
            package_root: "/compiler/core".into(),
            package_identity: None,
            dependency_scope: source::DependencyScope::Product,
            origin: source::SourceOrigin::Toolchain,
            resolution_stratum: source::SourceResolutionStratum::Base,
            source: std::str::from_utf8(bytes)
                .map_err(|_| "extent catalog source is not UTF-8")?
                .into(),
        })
        .map_err(|_| "extent catalog source has no exact identity")
    })
}

fn source_nominal(digest: &[u8; 32], path: &str) -> String {
    compound(
        NameForm::Nominal,
        [
            byte_atom(OwnerForm::ToolchainSource, digest),
            atom(Label::Path, path),
        ],
    )
}

fn live_domain_identity(policy: &str, element: &str) -> Result<String, &'static str> {
    Ok(compound(
        TypeForm::Generic,
        [
            atom(Label::Name, &source_nominal(&source_digest()?, "Live")),
            policy.to_owned(),
            element.to_owned(),
        ],
    ))
}
fn coordinates(
    operation: RunCustodyOperation,
    policy: &str,
    element: &str,
) -> Result<SourceCoordinates, &'static str> {
    let declaration = path(operation);
    let (file, bytes) = language_core::sealed_intrinsics::run_source();
    if SealedIntrinsicCatalog::classify(SealedDeclarationSite {
        file: std::path::Path::new(file),
        declaration,
        source: bytes,
    }) != Some(SealedIntrinsic::CompilerPrimitive(
        CompilerPrimitive::RunCustody(operation),
    )) {
        return Err("run operation has no exact admitted compiler declaration");
    }
    let digest = source_digest()?;
    let exact = |path: &str| {
        compound(
            NameForm::Nominal,
            [
                byte_atom(OwnerForm::ToolchainSource, &digest),
                atom(Label::Path, path),
            ],
        )
    };
    let binder_p = atom(IndexForm::ConstName, "$T0");
    let binder_t = named("$T1");
    let formal = live_run(&binder_p, &binder_t);
    let template_parameters = match operation {
        RunCustodyOperation::ReleaseEmpty => vec![formal],
        RunCustodyOperation::Split => vec![formal, named("u64")],
        RunCustodyOperation::Merge => vec![formal.clone(), formal],
    };
    let parameters = template_parameters
        .iter()
        .map(|ty| {
            compound(
                CoordinateTag::Parameter,
                [
                    atom(CoordinateTag::Mutable, "no"),
                    atom(CoordinateTag::Const, "no"),
                    ty.clone(),
                ],
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let callable = compound(
        CoordinateTag::NamedCallable,
        [
            atom(CoordinateTag::Path, &core_name(declaration)),
            atom(CoordinateTag::Parameters, &parameters),
            atom(CoordinateTag::ResultDispatch, ""),
        ],
    );
    let run = live_run(policy, element);
    let parameters = match operation {
        RunCustodyOperation::ReleaseEmpty => vec![run.clone()],
        RunCustodyOperation::Split => vec![run.clone(), scalar("u64")],
        RunCustodyOperation::Merge => vec![run.clone(), run.clone()],
    };
    let result = match operation {
        RunCustodyOperation::ReleaseEmpty => qualified(
            named(&core_name("Extent")),
            &[("Extent::Granted", &[]), ("Extent::Vacant", &[])],
        ),
        RunCustodyOperation::Split => generic("RunParts", &[policy, element]),
        RunCustodyOperation::Merge => run,
    };
    Ok(SourceCoordinates {
        owner: exact(declaration),
        entry: exact(&format!(
            "{declaration}::{}",
            declaration
                .rsplit("::")
                .next()
                .ok_or("run catalog declaration has no entry")?
        )),
        semantic: format!("toolchain::{declaration}"),
        callable,
        parameters,
        result,
    })
}

struct ActualTypes<'a> {
    operation: RunCustodyOperation,
    policy: &'a str,
    element: &'a str,
}

fn static_source<'a>(
    identities: &'a TerminalIdentityTable,
    owner: &NominalDeclarationIdentity,
    identity: TerminalIdentityIndex,
    checked: &'a CheckedBoundaryCallable,
) -> Result<ActualTypes<'a>, &'static str> {
    let CheckedBoundaryExecution::RunCustody {
        operation, policy, ..
    } = &checked.execution
    else {
        return Err("run source authentication needs its actual compiler execution");
    };
    let [ClosedReachArgument::Const(p), ClosedReachArgument::Type(t)] =
        checked.method_arguments.as_slice()
    else {
        return Err("run compiler source lost its static AccessPolicy value and T application");
    };
    let text = |identity| {
        identities
            .text(identity)
            .filter(|text| !text.is_empty())
            .ok_or("run compiler source has an unresolved identity coordinate")
    };
    if policy != p {
        return Err("run execution policy differs from its actual P type operand");
    }
    let actual = ActualTypes {
        operation: *operation,
        policy: text(*p)?,
        element: text(*t)?,
    };
    let expected = coordinates(*operation, actual.policy, actual.element)?;
    let source = &checked.source;
    let specialization = source
        .specialization
        .as_ref()
        .ok_or("run compiler source has no exact closed specialization")?;
    if text(owner.identity())? != expected.owner
        || text(source.machine.identity())? != expected.owner
        || text(source.state.identity())? != expected.entry
        || text(source.semantic_declaration)? != expected.semantic
        || text(specialization.template.identity())? != expected.owner
        || text(specialization.template_entry.identity())? != expected.entry
        || text(specialization.template_semantic_declaration)? != expected.semantic
        || text(specialization.template_callable)? != expected.callable
        || specialization.capture.is_some()
    {
        return Err("run compiler source differs from its exact bundled owner or template");
    }
    let mut commitment =
        language_semantics::conformance_identity::MachineApplicationCommitment::new(
            &expected.callable,
        )?;
    commitment.conformance_argument(actual.policy)?;
    commitment.type_argument(actual.element)?;
    let commitment = commitment.finish();
    let callable = CheckedMachineDeclaration::callable_evidence_identity(
        &expected.callable,
        Some(&commitment),
    );
    if specialization.commitment != commitment
        || text(source.callable)? != callable
        || identity != source.callable
    {
        return Err("run compiler source changed its complete static specialization");
    }
    if source.parameter_types.len() != expected.parameters.len()
        || source
            .parameter_types
            .iter()
            .zip(&expected.parameters)
            .any(|(actual, expected)| identities.text(*actual) != Some(expected.as_str()))
        || text(source.result_type)? != expected.result
    {
        return Err("run compiler source changed its actual parameter or result types");
    }
    if source.contract == [0; 32]
        || checked.service_declaration != source.contract
        || source.published_may_suspend
        || source.published_may_block
    {
        return Err("run compiler source lost its complete total contract commitment");
    }
    Ok(actual)
}

fn scalar_name(scalar: ScalarType) -> Result<String, &'static str> {
    Ok(match scalar {
        ScalarType::Boolean => "bool".to_owned(),
        ScalarType::Integer(integer) if integer.is_address() => "addr".to_owned(),
        ScalarType::Integer(integer) => format!(
            "{}{}",
            if integer.sign() == IntegerSign::Unsigned {
                "u"
            } else {
                "i"
            },
            integer
                .bits()
                .ok_or("run element has an open integer width")?
        ),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => "f32".to_owned(),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => "f64".to_owned(),
        ScalarType::Callable(_) => {
            return Err("run callable element needs its exact source contract correspondence");
        }
    })
}

// Keep the original qualified identity in the source and specialization. This
// conversion serves only the exact ordinary storage identity comparison. An
// unserved qualification/generic normalization refuses rather than erasing it.
fn validate_element_storage<'a>(
    identities: &TerminalIdentityTable,
    actual: &str,
    element: &StructuralFieldType,
    mut lookup: impl FnMut(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration>,
) -> Result<(), &'static str> {
    match element {
        StructuralFieldType::Scalar(kind) => {
            let name = scalar_name(*kind)?;
            if actual != scalar(&name) {
                return Err("run scalar element needs its exact qualified storage correspondence");
            }
            Ok(())
        }
        StructuralFieldType::Structural(id) => {
            let declaration =
                lookup(*id).ok_or("run element lost its actual structural declaration")?;
            let identity = identities
                .text(declaration.identity)
                .ok_or("run element lost its actual structural identity")?;
            if actual != identity {
                return Err(
                    "run structural element needs its exact qualified storage correspondence",
                );
            }
            Ok(())
        }
        _ => Err("run element has no admitted ordinary storage correspondence"),
    }
}

fn require_record_owner(
    declaration: &StructuralTypeDeclaration,
    nominal: &str,
    identities: &TerminalIdentityTable,
) -> Result<(), &'static str> {
    if identities
        .text(declaration.identity)
        .is_none_or(str::is_empty)
        || identities.text(declaration.declaration_identity) != Some(nominal)
    {
        return Err("run descriptor differs from its complete nominal P and T application");
    }
    Ok(())
}

fn release_roles<'module>(
    boundary: &BoundaryMachineDeclaration,
    contract: &crate::EmptyRunReleaseContract<'module>,
    mut lookup: impl FnMut(StructuralTypeId) -> Option<&'module StructuralTypeDeclaration>,
) -> Result<RunDescriptorRoles<'module>, &'static str> {
    let parameter = boundary
        .structural_parameters
        .first()
        .ok_or("empty run lost its exact structural input")?;
    let carrier = lookup(parameter.structural_type).ok_or("empty run lost its actual carrier")?;
    let StructuralTypeShape::Record { fields, .. } = &carrier.shape else {
        return Err("empty run is not a descriptor record");
    };
    let [storage, count] = fields.as_slice() else {
        return Err("empty run lost its checked descriptor fields");
    };
    let extent = storage
        .field_type
        .as_structural()
        .ok_or("empty run backing is not structural")?;
    let backing = lookup(extent).ok_or("empty run lost its checked backing declaration")?;
    let StructuralTypeShape::Record { fields, .. } = &backing.shape else {
        return Err("empty run backing is not a descriptor record");
    };
    let [base, length] = fields.as_slice() else {
        return Err("empty run lost its checked geometry fields");
    };
    if storage.identity != contract.storage
        || count.id != contract.count
        || length.id != contract.length
        || extent != contract.result.structural_type
    {
        return Err("empty run scalar contract differs from its projected descriptor roles");
    }
    Ok(RunDescriptorRoles {
        carrier: carrier.id,
        extent,
        storage,
        count,
        base,
        length,
    })
}

/// The caller prepares type lookup once for the containing declaration family.
/// These checks authenticate source meaning only; the actual ordered live-cell
/// custody, placement, loans and constant-cost execution remain independent.
pub(crate) fn validate<'module>(
    module: &terminal_psi::TerminalModule,
    boundary: &BoundaryMachineDeclaration,
    contract: &RunCustodyContract<'module>,
    mut lookup: impl FnMut(StructuralTypeId) -> Option<&'module StructuralTypeDeclaration>,
    mut domains: impl FnMut(StructuralDomainId) -> Option<&'module StructuralDomainDeclaration>,
) -> Result<(), &'static str> {
    let checked = boundary
        .declaration
        .as_ref()
        .ok_or("run compiler operation has no checked source")?;
    let actual = static_source(
        &module.identities,
        &boundary.owner,
        boundary.identity,
        checked,
    )?;
    let (run, parts) = match (actual.operation, contract) {
        (RunCustodyOperation::ReleaseEmpty, RunCustodyContract::ReleaseEmpty(contract)) => {
            (release_roles(boundary, contract, &mut lookup)?, None)
        }
        (
            RunCustodyOperation::Split,
            RunCustodyContract::Split {
                run,
                left,
                right,
                result,
            },
        ) => (
            *run,
            Some((
                left.id,
                right.id,
                result.structural_type,
                left.identity,
                right.identity,
            )),
        ),
        (RunCustodyOperation::Merge, RunCustodyContract::Merge { run, .. }) => (*run, None),
        _ => return Err("run compiler source changed its projected operation"),
    };
    let CheckedBoundaryExecution::RunCustody {
        policy, element, ..
    } = &checked.execution
    else {
        unreachable!("static source authenticated the same execution")
    };
    validate_element_storage(&module.identities, actual.element, element, &mut lookup)?;
    let run_declaration = lookup(run.carrier).ok_or("run lost its actual carrier declaration")?;
    require_record_owner(run_declaration, "toolchain::Run", &module.identities)?;
    let sequence = run_declaration
        .retained_sequence
        .as_ref()
        .ok_or("run declaration lost its complete retained element custody type")?;
    let [_, ClosedReachArgument::Type(element_identity)] = checked.method_arguments.as_slice()
    else {
        unreachable!("static source authenticated the closed application")
    };
    if sequence.policy != *policy
        || sequence.element_identity != *element_identity
        || sequence.element != *element
        || sequence.storage != run.storage.id
        || sequence.count != run.count.id
    {
        return Err(
            "run retained sequence differs from its actual policy, element or storage roles",
        );
    }
    let extent = lookup(run.extent).ok_or("run lost its actual backing declaration")?;
    require_record_owner(extent, "toolchain::Extent", &module.identities)?;
    if extent.retained_sequence.is_some() {
        return Err("run backing cannot invent a retained element sequence");
    }
    if let Some((_, _, id, _, _)) = parts {
        let parts_declaration = lookup(id).ok_or("run split lost its actual parts declaration")?;
        require_record_owner(parts_declaration, "toolchain::RunParts", &module.identities)?;
        if parts_declaration.retained_sequence.is_some() {
            return Err(
                "run parts retain their two child sequences rather than inventing a parent sequence",
            );
        }
    }
    for (ordinal, parameter) in boundary.structural_parameters.iter().enumerate() {
        if parameter.position as usize != ordinal
            || parameter.place
                != PlaceId::new(ordinal as u64 + 1).ok_or("run formal coordinate overflow")?
        {
            return Err("run compiler source changed its exact formal storage coordinates");
        }
    }
    let fields = FieldRoles {
        storage: run.storage.id,
        count: run.count.id,
        base: run.base.id,
        length: run.length.id,
        address: run
            .base
            .field_type
            .scalar_type()
            .and_then(|ty| ty.as_integer())
            .ok_or("run base has no checked address scalar")?,
    };
    let (requires, ensures) = scalar_contract(
        actual.operation,
        fields,
        parts.map(|(left, right, _, _, _)| (left, right)),
    )?;
    if !same_clauses(&boundary.scalar_requires, requires)
        || !same_clauses(&boundary.scalar_ensures, ensures)
    {
        return Err("run compiler source changed its complete scalar requires or ensures");
    }
    validate_qualified_roles(
        module,
        boundary,
        &actual,
        run,
        parts.map(|(_, _, _, left, right)| (left, right)),
        &mut domains,
    )?;
    Ok(())
}

fn qualified_domain<'a>(
    identities: &TerminalIdentityTable,
    domain: &'a StructuralDomainDeclaration,
    carrier: StructuralTypeId,
    identity: &str,
    subject: &str,
) -> Result<&'a StructuralDomainDeclaration, &'static str> {
    if domain.carrier != carrier
        || identities.text(domain.identity) != Some(identity)
        || identities.text(domain.subject_identity) != Some(subject)
        || !domain.complete_predicate_theory
        || domain.required_memberships.as_deref() != Some(&[])
        || domain
            .establishment_routes
            .as_ref()
            .is_none_or(Vec::is_empty)
    {
        return Err("run contract qualification lost its exact protected carrier or declaration");
    }
    Ok(domain)
}

/// The same protected Live family accompanies access and custody transitions.
/// Matching a Run-shaped descriptor never substitutes for this exact source,
/// applied type and complete issuer correspondence.
pub(crate) fn validate_live_domain(
    module: &terminal_psi::TerminalModule,
    carrier: StructuralTypeId,
    policy: &str,
    element: &str,
    domain: &StructuralDomainDeclaration,
) -> Result<(), &'static str> {
    let domain = qualified_domain(
        &module.identities,
        domain,
        carrier,
        &live_domain_identity(policy, element)?,
        &generic("Run", &[policy, element]),
    )?;
    if domain.predicate.is_some()
        || domain.access_policy.is_some()
        || domain.byte_predicate.is_some()
        || domain.content_projection.is_some()
    {
        return Err("run Live declaration changed its opaque custody qualification");
    }
    let permitted = [
        "Run::empty",
        "Run::initialize_from_profile",
        "Run::relocate_from_profiles",
        "Run::split",
        "Run::merge",
    ];
    let mut seen = [false; 5];
    for route in domain.establishment_routes.as_ref().unwrap() {
        let terminal_psi::StructuralEstablishmentRoute::ExactMachine {
            declaration,
            machine_identity,
        } = route
        else {
            return Err("run Live declaration changed its authorized compiler routes");
        };
        let position = permitted
            .iter()
            .position(|path| {
                module.identities.text(declaration.identity())
                    == Some(format!("toolchain::{path}").as_str())
            })
            .ok_or("run Live declaration admits another issuer")?;
        if std::mem::replace(&mut seen[position], true)
            || module
                .identities
                .text(*machine_identity)
                .is_none_or(str::is_empty)
        {
            return Err("run Live declaration repeats or loses an issuer callable");
        }
        let operation = match position {
            3 => Some(RunCustodyOperation::Split),
            4 => Some(RunCustodyOperation::Merge),
            _ => None,
        };
        if let Some(operation) = operation
            && module.identities.text(*machine_identity)
                != Some(coordinates(operation, policy, element)?.callable.as_str())
        {
            return Err("run Live declaration changes a compiler issuer signature");
        }
    }
    if seen.contains(&false) {
        return Err("run Live declaration lost an authorized route");
    }
    Ok(())
}

fn validate_qualified_roles<'a>(
    module: &terminal_psi::TerminalModule,
    boundary: &BoundaryMachineDeclaration,
    actual: &ActualTypes<'_>,
    run: RunDescriptorRoles<'_>,
    parts: Option<(TerminalIdentityIndex, TerminalIdentityIndex)>,
    domains: &mut impl FnMut(StructuralDomainId) -> Option<&'a StructuralDomainDeclaration>,
) -> Result<(), &'static str> {
    use terminal_psi::{
        StructuralDomainRequirement, StructuralPathQualification, StructuralPathSegment,
    };
    let Some(parameter) = boundary.structural_parameters.first() else {
        return Err("run contract lost its qualified input");
    };
    let [live] = parameter.qualifications.as_slice() else {
        return Err("run contract needs exactly its closed Live membership");
    };
    let domain = domains(*live).ok_or("run contract lost its Live declaration")?;
    validate_live_domain(module, run.carrier, actual.policy, actual.element, domain)?;
    let [granted] = parameter.projected_qualifications.as_slice() else {
        return Err("run contract lost its exact backing qualification");
    };
    if granted.path != [StructuralPathSegment::Field(run.storage.identity)] {
        return Err("run contract moves its backing qualification to another subject");
    }
    let granted_id = granted.domain;
    qualified_domain(
        &module.identities,
        domains(granted_id).ok_or("run contract lost its backing grant declaration")?,
        run.extent,
        &source_nominal(&extent_digest()?, "Extent::Granted"),
        &named(&core_name("Extent")),
    )?;
    let expected_input = vec![*live];
    for parameter in &boundary.structural_parameters {
        if parameter.qualifications != expected_input
            || parameter.projected_qualifications.as_slice() != [granted.clone()]
            || !parameter.additional_input_qualifications.is_empty()
        {
            return Err("run contract changes the Live or backing premise of an input");
        }
    }
    let expected_requires = boundary
        .structural_parameters
        .iter()
        .map(|parameter| StructuralDomainRequirement {
            argument_index: parameter.position,
            path: Vec::new(),
            domain: *live,
        })
        .collect::<Vec<_>>();
    if boundary.requires != expected_requires
        || !boundary.normal_qualifications.is_empty()
        || !boundary.scalar_domain_requirements.is_empty()
        || !boundary.program_local_root_introductions.is_empty()
        || !boundary.content_guarantees.is_empty()
    {
        return Err("run contract changes qualified premises or introduces unrelated authority");
    }
    let terminal_psi::BoundaryMachineResult::Structural(result) = &boundary.result else {
        unreachable!("descriptor projection checked the result")
    };
    let mut projected = Vec::new();
    match actual.operation {
        RunCustodyOperation::ReleaseEmpty => {
            if result.qualifications.len() != 2
                || !result.qualifications.contains(&granted_id)
                || !result.projected_qualifications.is_empty()
            {
                return Err("empty run release lost its granted vacant result");
            }
            let vacant = result
                .qualifications
                .iter()
                .find(|id| **id != granted_id)
                .ok_or("empty run release repeats its grant instead of vacancy")?;
            qualified_domain(
                &module.identities,
                domains(*vacant).ok_or("empty run release lost its vacancy declaration")?,
                run.extent,
                &source_nominal(&extent_digest()?, "Extent::Vacant"),
                &named(&core_name("Extent")),
            )?;
            return Ok(());
        }
        RunCustodyOperation::Merge => {
            if result.qualifications != expected_input {
                return Err("run merge lost its Live result");
            }
            projected.push(granted.clone());
        }
        RunCustodyOperation::Split => {
            if !result.qualifications.is_empty() {
                return Err("run split changes its unqualified parts result");
            }
            let (left, right) = parts.ok_or("run split lost its qualified field roles")?;
            for field in [left, right] {
                let path = vec![StructuralPathSegment::Field(field)];
                projected.push(StructuralPathQualification {
                    path: path.clone(),
                    domain: *live,
                });
                projected.push(StructuralPathQualification {
                    path: [path, granted.path.clone()].concat(),
                    domain: granted_id,
                });
            }
        }
    }
    projected.sort_unstable();
    if result.projected_qualifications != projected {
        return Err("run result changed its exact qualified field frontier");
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct FieldRoles {
    storage: StructuralFieldId,
    count: StructuralFieldId,
    base: StructuralFieldId,
    length: StructuralFieldId,
    address: IntegerType,
}

fn canonical(mut proposition: Proposition) -> Proposition {
    match &mut proposition {
        Proposition::Equal(left, right) => {
            if left > right {
                std::mem::swap(left, right);
            }
        }
        Proposition::IntegerMathEqual(left, right) => {
            if left > right {
                std::mem::swap(left, right);
            }
        }
        Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
            for child in children.iter_mut() {
                *child = canonical(child.clone());
            }
            children.sort_unstable();
        }
        _ => {}
    }
    proposition
}
fn same_clauses(actual: &[Proposition], expected: Vec<Proposition>) -> bool {
    // The source reader canonicalizes equality and alternatives inside each
    // proposition. The authored clause roster itself stays ordered: a call's
    // requirement obligation names that exact row, including any duplicates.
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| canonical(actual.clone()) == canonical(expected))
}
fn or(clauses: Vec<Proposition>) -> Proposition {
    let mut flat = Vec::new();
    for clause in clauses {
        match clause {
            Proposition::Disjunction(inner) => flat.extend(inner),
            other => flat.push(other),
        }
    }
    Proposition::Disjunction(flat)
}
fn ne(left: ScalarTerm, right: ScalarTerm) -> Proposition {
    or(vec![
        Proposition::LessThan(left.clone(), right.clone()),
        Proposition::LessThan(right, left),
    ])
}
fn equal(left: ScalarTerm, right: ScalarTerm) -> Proposition {
    Proposition::Equal(left, right)
}
fn math_equal(left: Math, right: Math) -> Proposition {
    Proposition::IntegerMathEqual(left, right)
}
fn embed(term: ScalarTerm) -> Result<Math, &'static str> {
    Math::embed(term).ok_or("run scalar contract embeds a non-integer")
}
fn sum(left: Math, right: Math) -> Math {
    Math::Add(Box::new(left), Box::new(right))
}
fn product(left: Math, right: Math) -> Math {
    Math::Multiply(Box::new(left), Box::new(right))
}

fn scalar_contract(
    operation: RunCustodyOperation,
    fields: FieldRoles,
    parts: Option<(StructuralFieldId, StructuralFieldId)>,
) -> Result<(Vec<Proposition>, Vec<Proposition>), &'static str> {
    let count_type =
        IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| "run count carrier is invalid")?;
    let place = |id| PlaceId::new(id).ok_or("run scalar contract has an invalid place coordinate");
    let field = |root, path, scalar_type| {
        Ok::<_, &'static str>(ScalarTerm::integer_field_path(
            place(root)?,
            path,
            scalar_type,
        ))
    };
    let zero = || ScalarTerm::Integer {
        scalar_type: count_type,
        value: IntegerValue::Unsigned(0),
    };
    let count = |root, prefix: &[Path]| {
        field(
            root,
            prefix
                .iter()
                .cloned()
                .chain([Path::Field(fields.count)])
                .collect(),
            count_type,
        )
    };
    let length = |root, prefix: &[Path]| {
        field(
            root,
            prefix
                .iter()
                .cloned()
                .chain([Path::Field(fields.storage), Path::Field(fields.length)])
                .collect(),
            count_type,
        )
    };
    let base = |root, prefix: &[Path]| {
        field(
            root,
            prefix
                .iter()
                .cloned()
                .chain([Path::Field(fields.storage), Path::Field(fields.base)])
                .collect(),
            fields.address,
        )
    };
    let extent_length = |root| field(root, vec![Path::Field(fields.length)], count_type);
    let extent_base = |root| field(root, vec![Path::Field(fields.base)], fields.address);
    match operation {
        RunCustodyOperation::ReleaseEmpty => Ok((
            vec![
                equal(count(1, &[])?, zero()),
                equal(length(1, &[])?, zero()),
            ],
            vec![
                equal(extent_base(2)?, base(1, &[])?),
                equal(extent_length(2)?, length(1, &[])?),
            ],
        )),
        RunCustodyOperation::Split => {
            let (left, right) = parts.ok_or("run split lost its actual result field roles")?;
            let left = [Path::Field(left)];
            let right = [Path::Field(right)];
            let cut = ScalarTerm::Value {
                id: ValueId::new(1).ok_or("run cut value coordinate is invalid")?,
                scalar_type: ScalarType::Integer(count_type),
            };
            let run_count = count(1, &[])?;
            let run_length = length(1, &[])?;
            let run_base = base(1, &[])?;
            let left_count = count(2, &left)?;
            let right_count = count(2, &right)?;
            let left_length = length(2, &left)?;
            let right_length = length(2, &right)?;
            let left_base = base(2, &left)?;
            let right_base = base(2, &right)?;
            Ok((
                vec![Proposition::LessOrEqual(cut.clone(), run_count.clone())],
                vec![
                    equal(left_count, cut.clone()),
                    equal(
                        right_count,
                        ScalarTerm::exact_integer_subtract(
                            count_type,
                            run_count.clone(),
                            cut.clone(),
                        )
                        .map_err(|_| "run split count subtraction is malformed")?,
                    ),
                    equal(left_base, run_base.clone()),
                    math_equal(
                        sum(embed(left_length.clone())?, embed(right_length.clone())?),
                        embed(run_length.clone())?,
                    ),
                    math_equal(
                        product(embed(left_length.clone())?, embed(run_count.clone())?),
                        product(embed(run_length)?, embed(cut.clone())?),
                    ),
                    or(vec![
                        ne(cut.clone(), zero()),
                        equal(left_length.clone(), zero()),
                    ]),
                    or(vec![
                        ne(cut, run_count),
                        equal(right_length.clone(), zero()),
                    ]),
                    or(vec![
                        ne(left_length.clone(), zero()),
                        equal(right_base.clone(), run_base.clone()),
                    ]),
                    or(vec![
                        equal(right_length.clone(), zero()),
                        math_equal(
                            embed(right_base.clone())?,
                            sum(embed(run_base.clone())?, embed(left_length)?),
                        ),
                    ]),
                    or(vec![ne(right_length, zero()), equal(right_base, run_base)]),
                ],
            ))
        }
        RunCustodyOperation::Merge => {
            let left_count = count(1, &[])?;
            let right_count = count(2, &[])?;
            let left_length = length(1, &[])?;
            let right_length = length(2, &[])?;
            let left_base = base(1, &[])?;
            let right_base = base(2, &[])?;
            let counts = sum(embed(left_count)?, embed(right_count)?);
            let lengths = sum(embed(left_length.clone())?, embed(right_length.clone())?);
            let maximum = Math::literal(IntegerValue::Unsigned(u64::MAX.into()));
            Ok((
                vec![
                    Proposition::IntegerMathLessOrEqual(counts.clone(), maximum.clone()),
                    Proposition::IntegerMathLessOrEqual(lengths.clone(), maximum),
                    or(vec![
                        equal(left_length.clone(), zero()),
                        equal(right_length, zero()),
                        math_equal(
                            embed(right_base.clone())?,
                            sum(embed(left_base.clone())?, embed(left_length.clone())?),
                        ),
                    ]),
                ],
                vec![
                    math_equal(embed(count(3, &[])?)?, counts),
                    math_equal(embed(length(3, &[])?)?, lengths),
                    or(vec![
                        equal(left_length.clone(), zero()),
                        equal(base(3, &[])?, left_base),
                    ]),
                    or(vec![
                        ne(left_length, zero()),
                        equal(base(3, &[])?, right_base),
                    ]),
                ],
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_psi::{CheckedMachineSpecialization, TerminalIdentityTableBuilder};

    fn checked(
        operation: RunCustodyOperation,
    ) -> (
        TerminalIdentityTable,
        NominalDeclarationIdentity,
        TerminalIdentityIndex,
        CheckedBoundaryCallable,
    ) {
        let p = named(&compound(
            NameForm::Nominal,
            [
                byte_atom(OwnerForm::Package, &[7; 32]),
                atom(Label::Path, "Policy"),
            ],
        ));
        let t = scalar("u64");
        let expected = coordinates(operation, &p, &t).unwrap();
        let mut identities = TerminalIdentityTableBuilder::default();
        let owner = NominalDeclarationIdentity::new(identities.intern(&expected.owner));
        let state = NominalDeclarationIdentity::new(identities.intern(&expected.entry));
        let semantic = identities.intern(&expected.semantic);
        let template_callable = identities.intern(&expected.callable);
        let mut commitment =
            language_semantics::conformance_identity::MachineApplicationCommitment::new(
                &expected.callable,
            )
            .unwrap();
        commitment.conformance_argument(&p).unwrap();
        commitment.type_argument(&t).unwrap();
        let commitment = commitment.finish();
        let callable = identities.intern(&CheckedMachineDeclaration::callable_evidence_identity(
            &expected.callable,
            Some(&commitment),
        ));
        let policy = identities.intern(&p);
        let element = identities.intern(&t);
        let checked = CheckedBoundaryCallable {
            service_declaration: [9; 32],
            source: CheckedMachineDeclaration {
                machine: owner.clone(),
                state: state.clone(),
                semantic_declaration: semantic,
                parameter_types: expected
                    .parameters
                    .iter()
                    .map(|ty| identities.intern(ty))
                    .collect(),
                callable,
                result_type: identities.intern(&expected.result),
                contract: [9; 32],
                published_may_suspend: false,
                published_may_block: false,
                specialization: Some(CheckedMachineSpecialization {
                    capture: None,
                    template: owner.clone(),
                    template_entry: state,
                    template_callable,
                    template_semantic_declaration: semantic,
                    commitment,
                }),
            },
            execution: CheckedBoundaryExecution::RunCustody {
                operation,
                policy,
                element: StructuralFieldType::Scalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                )),
            },
            method_arguments: vec![
                ClosedReachArgument::Const(policy),
                ClosedReachArgument::Type(element),
            ],
        };
        (identities.snapshot(), owner, callable, checked)
    }

    fn received_module(operation: RunCustodyOperation) -> terminal_psi::TerminalModule {
        let (identities, owner, identity, checked) = checked(operation);
        let [ClosedReachArgument::Const(policy), _] = checked.method_arguments.as_slice() else {
            unreachable!()
        };
        let policy = identities.text(*policy).unwrap().to_owned();
        let mut identities = identities.into_builder();
        let mut types = crate::storage::run_custody::tests::types();
        types[0].declaration_identity = identities.intern("toolchain::Extent");
        types[0].identity = identities.intern(&named(&core_name("Extent")));
        for (ordinal, name) in [(1, "Run"), (2, "RunParts")] {
            types[ordinal].declaration_identity = identities.intern(&format!("toolchain::{name}"));
            types[ordinal].identity = identities.intern(&generic(name, &[&policy, &named("u64")]));
        }
        let [
            ClosedReachArgument::Const(policy_id),
            ClosedReachArgument::Type(element_id),
        ] = checked.method_arguments.as_slice()
        else {
            unreachable!()
        };
        let CheckedBoundaryExecution::RunCustody { element, .. } = &checked.execution else {
            unreachable!()
        };
        types[1].retained_sequence = Some(terminal_psi::StructuralRetainedSequence {
            policy: *policy_id,
            element_identity: *element_id,
            element: element.clone(),
            storage: StructuralFieldId::new(3).unwrap(),
            count: StructuralFieldId::new(4).unwrap(),
        });
        let mut boundary = crate::storage::run_custody::tests::boundary(operation);
        boundary.owner = owner;
        boundary.identity = identity;
        boundary.declaration = Some(checked);
        let field = |id| StructuralFieldId::new(id).unwrap();
        let (requires, ensures) = scalar_contract(
            operation,
            FieldRoles {
                storage: field(3),
                count: field(4),
                base: field(1),
                length: field(2),
                address: IntegerType::address(64).unwrap(),
            },
            Some((field(5), field(6))),
        )
        .unwrap();
        boundary.scalar_requires = requires;
        boundary.scalar_ensures = ensures;
        use terminal_psi::{
            StructuralDomainDeclaration, StructuralDomainRequirement, StructuralEstablishmentRoute,
            StructuralPathQualification, StructuralPathSegment,
        };
        let [
            ClosedReachArgument::Const(_),
            ClosedReachArgument::Type(element),
        ] = boundary
            .declaration
            .as_ref()
            .unwrap()
            .method_arguments
            .as_slice()
        else {
            unreachable!()
        };
        let element = identities.text(*element).unwrap().to_owned();
        let domain_id = |raw| StructuralDomainId::new(raw).unwrap();
        let mut routes = Vec::new();
        for path in [
            "Run::empty",
            "Run::initialize_from_profile",
            "Run::relocate_from_profiles",
            "Run::split",
            "Run::merge",
        ] {
            let declaration =
                NominalDeclarationIdentity::new(identities.intern(&format!("toolchain::{path}")));
            let callable = match path {
                "Run::split" => {
                    coordinates(RunCustodyOperation::Split, &policy, &element)
                        .unwrap()
                        .callable
                }
                "Run::merge" => {
                    coordinates(RunCustodyOperation::Merge, &policy, &element)
                        .unwrap()
                        .callable
                }
                _ => format!("catalog:{path}"),
            };
            routes.push(StructuralEstablishmentRoute::ExactMachine {
                declaration,
                machine_identity: identities.intern(&callable),
            });
        }
        let mut domains = Vec::new();
        for (raw, identity, subject, carrier, routes) in [
            (
                1,
                live_domain_identity(&policy, &element).unwrap(),
                generic("Run", &[&policy, &element]),
                types[1].id,
                routes.clone(),
            ),
            (
                2,
                source_nominal(&extent_digest().unwrap(), "Extent::Granted"),
                named(&core_name("Extent")),
                types[0].id,
                vec![routes[0].clone()],
            ),
            (
                3,
                source_nominal(&extent_digest().unwrap(), "Extent::Vacant"),
                named(&core_name("Extent")),
                types[0].id,
                vec![routes[0].clone()],
            ),
        ] {
            domains.push(StructuralDomainDeclaration {
                required_memberships: Some(Vec::new()),
                complete_predicate_theory: true,
                predicate: None,
                id: domain_id(raw),
                semantic_domain: semantic_vocabulary::DomainSemanticId::new(raw).unwrap(),
                identity: identities.intern(&identity),
                carrier,
                subject_identity: identities.intern(&subject),
                content_projection: None,
                byte_predicate: None,
                establishment_routes: Some(routes),
                access_policy: None,
            });
        }
        let granted = StructuralPathQualification {
            path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(3))],
            domain: domain_id(2),
        };
        for parameter in &mut boundary.structural_parameters {
            parameter.qualifications = vec![domain_id(1)];
            parameter.projected_qualifications = vec![granted.clone()];
        }
        boundary.requires = boundary
            .structural_parameters
            .iter()
            .map(|parameter| StructuralDomainRequirement {
                argument_index: parameter.position,
                path: Vec::new(),
                domain: domain_id(1),
            })
            .collect();
        let terminal_psi::BoundaryMachineResult::Structural(result) = &mut boundary.result else {
            unreachable!()
        };
        match operation {
            RunCustodyOperation::ReleaseEmpty => {
                result.qualifications = vec![domain_id(2), domain_id(3)]
            }
            RunCustodyOperation::Merge => {
                result.qualifications = vec![domain_id(1)];
                result.projected_qualifications = vec![granted];
            }
            RunCustodyOperation::Split => {
                for field in [TerminalIdentityIndex(5), TerminalIdentityIndex(6)] {
                    result
                        .projected_qualifications
                        .push(StructuralPathQualification {
                            path: vec![StructuralPathSegment::Field(field)],
                            domain: domain_id(1),
                        });
                    result
                        .projected_qualifications
                        .push(StructuralPathQualification {
                            path: vec![
                                StructuralPathSegment::Field(field),
                                StructuralPathSegment::Field(TerminalIdentityIndex(3)),
                            ],
                            domain: domain_id(2),
                        });
                }
                result.projected_qualifications.sort_unstable();
            }
        }
        let mut module = terminal_psi::TerminalModule::declarations();
        module.identities = identities.snapshot();
        module.structural_types = types;
        module.structural_domains = domains;
        module.boundary_machines = vec![boundary];
        module
    }

    #[test]
    fn run_receiving_checks_the_saved_declaration_and_scalar_contract_together() {
        for operation in [
            RunCustodyOperation::ReleaseEmpty,
            RunCustodyOperation::Split,
            RunCustodyOperation::Merge,
        ] {
            let original = received_module(operation);
            crate::validate_run_custody_declarations(&original).unwrap();
            for fault in 0..27 {
                let mut changed = original.clone();
                let boundary = &mut changed.boundary_machines[0];
                match fault {
                    0 => {
                        boundary.owner = boundary.declaration.as_ref().unwrap().source.state.clone()
                    }
                    1 => boundary.scalar_requires.clear(),
                    2 => boundary.scalar_ensures[0] = Proposition::Truth,
                    3 => {
                        boundary
                            .declaration
                            .as_mut()
                            .unwrap()
                            .source
                            .parameter_types[0] = boundary.identity
                    }
                    4 => boundary
                        .declaration
                        .as_mut()
                        .unwrap()
                        .method_arguments
                        .swap(0, 1),
                    5 => {
                        let CheckedBoundaryExecution::RunCustody { element, .. } =
                            &mut boundary.declaration.as_mut().unwrap().execution
                        else {
                            unreachable!()
                        };
                        *element = StructuralFieldType::Scalar(ScalarType::Boolean);
                    }
                    6 => {
                        changed.structural_types[1]
                            .retained_sequence
                            .as_mut()
                            .unwrap()
                            .element_identity = changed.structural_types[2].identity;
                    }
                    7 => {
                        changed.structural_types[0].declaration_identity =
                            changed.structural_types[1].declaration_identity
                    }
                    8 => {
                        boundary
                            .declaration
                            .as_mut()
                            .unwrap()
                            .source
                            .specialization
                            .as_mut()
                            .unwrap()
                            .commitment[0] ^= 1
                    }
                    9 => boundary.scalar_ensures.swap(0, 1),
                    10 => boundary.structural_parameters[0].qualifications.clear(),
                    11 => boundary.structural_parameters[0]
                        .projected_qualifications
                        .clear(),
                    12 => boundary.requires.clear(),
                    13 => {
                        let terminal_psi::BoundaryMachineResult::Structural(result) =
                            &mut boundary.result
                        else {
                            unreachable!()
                        };
                        result.qualifications.clear();
                        result.projected_qualifications.clear();
                    }
                    14 => changed.structural_domains[0].establishment_routes = None,
                    15 => {
                        changed.structural_domains[0].identity =
                            changed.structural_domains[1].identity
                    }
                    16 => {
                        changed.structural_domains[1].subject_identity =
                            changed.structural_domains[0].subject_identity
                    }
                    17 => {
                        changed.structural_domains[0]
                            .establishment_routes
                            .as_mut()
                            .unwrap()
                            .pop()
                            .unwrap();
                    }
                    18 => changed.structural_types[1].retained_sequence = None,
                    19 => {
                        changed.structural_types[1]
                            .retained_sequence
                            .as_mut()
                            .unwrap()
                            .policy = changed.structural_types[2].identity
                    }
                    20 => {
                        changed.structural_types[1]
                            .retained_sequence
                            .as_mut()
                            .unwrap()
                            .element_identity = changed.structural_types[0].identity
                    }
                    21 => {
                        changed.structural_types[1]
                            .retained_sequence
                            .as_mut()
                            .unwrap()
                            .element = StructuralFieldType::Scalar(ScalarType::Boolean)
                    }
                    22 => {
                        changed.structural_types[1]
                            .retained_sequence
                            .as_mut()
                            .unwrap()
                            .storage = StructuralFieldId::new(4).unwrap()
                    }
                    23 => {
                        changed.structural_types[1]
                            .retained_sequence
                            .as_mut()
                            .unwrap()
                            .count = StructuralFieldId::new(3).unwrap()
                    }
                    24 => {
                        let issuer = changed.structural_domains[0]
                            .establishment_routes
                            .as_ref()
                            .unwrap()[0]
                            .clone();
                        changed.structural_domains[0]
                            .establishment_routes
                            .as_mut()
                            .unwrap()[1] = issuer;
                    }
                    25 => {
                        boundary.structural_parameters[0].projected_qualifications[0].path =
                            Vec::new()
                    }
                    26 => {
                        let policy =
                            match boundary.declaration.as_ref().unwrap().method_arguments[0] {
                                ClosedReachArgument::Const(policy) => policy,
                                _ => unreachable!(),
                            };
                        boundary.declaration.as_mut().unwrap().method_arguments[0] =
                            ClosedReachArgument::Type(policy);
                    }
                    _ => unreachable!(),
                }
                assert!(
                    crate::validate_run_custody_declarations(&changed).is_err(),
                    "{operation:?} received fault {fault}"
                );
            }
        }
    }

    #[test]
    fn run_receiving_source_refuses_changed_owner_operation_policy_and_specialization() {
        for operation in [
            RunCustodyOperation::ReleaseEmpty,
            RunCustodyOperation::Split,
            RunCustodyOperation::Merge,
        ] {
            let (identities, owner, identity, source) = checked(operation);
            static_source(&identities, &owner, identity, &source).unwrap();
            for fault in 0..7 {
                let mut changed = source.clone();
                match fault {
                    0 => changed.source.state = changed.source.machine.clone(),
                    1 => changed.source.specialization.as_mut().unwrap().commitment[0] ^= 1,
                    2 => changed.method_arguments.swap(0, 1),
                    3 => {
                        if let CheckedBoundaryExecution::RunCustody { policy, .. } =
                            &mut changed.execution
                        {
                            let ClosedReachArgument::Type(element) = changed.method_arguments[1]
                            else {
                                unreachable!()
                            };
                            *policy = element;
                        }
                    }
                    4 => changed.service_declaration[0] ^= 1,
                    5 => changed.source.result_type = changed.source.callable,
                    6 => {
                        if let CheckedBoundaryExecution::RunCustody { operation, .. } =
                            &mut changed.execution
                        {
                            *operation = match *operation {
                                RunCustodyOperation::Split => RunCustodyOperation::Merge,
                                _ => RunCustodyOperation::Split,
                            };
                        }
                    }
                    _ => unreachable!(),
                }
                assert!(
                    static_source(&identities, &owner, identity, &changed).is_err(),
                    "{operation:?} fault {fault}"
                );
            }
        }
    }

    #[test]
    fn run_receiving_requires_the_complete_scalar_contract_including_empty_edges() {
        let field = |id| StructuralFieldId::new(id).unwrap();
        let fields = FieldRoles {
            storage: field(1),
            count: field(2),
            base: field(3),
            length: field(4),
            address: IntegerType::address(64).unwrap(),
        };
        for operation in [
            RunCustodyOperation::ReleaseEmpty,
            RunCustodyOperation::Split,
            RunCustodyOperation::Merge,
        ] {
            let (requires, ensures) =
                scalar_contract(operation, fields, Some((field(5), field(6)))).unwrap();
            assert!(same_clauses(&requires, requires.clone()));
            assert!(same_clauses(&ensures, ensures.clone()));
            if requires.len() > 1 {
                let mut swapped = requires.clone();
                swapped.swap(0, 1);
                assert!(
                    !same_clauses(&swapped, requires.clone()),
                    "{operation:?} swapped requirement rows"
                );
            }
            let mut swapped = ensures.clone();
            swapped.swap(0, 1);
            assert!(
                !same_clauses(&swapped, ensures.clone()),
                "{operation:?} swapped guarantee rows"
            );
            let mut missing_requirement = requires.clone();
            missing_requirement.pop();
            assert!(!same_clauses(&missing_requirement, requires));
            for ordinal in 0..ensures.len() {
                let mut forged = ensures.clone();
                forged[ordinal] = Proposition::Truth;
                assert!(
                    !same_clauses(&forged, ensures.clone()),
                    "{operation:?} guarantee {ordinal}"
                );
            }
        }
    }
}
