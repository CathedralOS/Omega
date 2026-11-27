use terminal_psi::{
    ClosedConformanceSchema, ClosedConformanceSchemaKind, ClosedReachArgument,
    ClosedReachParameter, TargetMachineCallable, TerminalIdentityIndex, TerminalModule,
};

struct HostedProcessExitIdentities {
    primitive: String,
    primitive_entry: String,
    primitive_semantic: String,
    primitive_callable: String,
    requirement_owner: String,
    requirement_member: String,
    requirement_semantic: String,
    requirement_callable: String,
    parameter: String,
    result: String,
}

fn hosted_process_exit_identities() -> &'static HostedProcessExitIdentities {
    use application_identity::type_identity::encoding::CoordinateTag;
    use application_identity::type_identity::encoding::atom;
    use application_identity::type_identity::encoding::bare;
    use application_identity::type_identity::encoding::byte_atom;
    use application_identity::type_identity::encoding::callable_declaration_path;
    use application_identity::type_identity::encoding::compound;
    use application_identity::type_identity::vocabulary::Label;
    use application_identity::type_identity::vocabulary::NameForm;
    use application_identity::type_identity::vocabulary::OwnerForm;
    use application_identity::type_identity::vocabulary::TypeForm;
    use language_core::bundled_files::core_revision;
    static IDENTITIES: std::sync::OnceLock<HostedProcessExitIdentities> =
        std::sync::OnceLock::new();
    IDENTITIES.get_or_init(|| {
        let primitive = "ProcessExitPrimitive";
        let requirement = "ProcessExit";
        let requirement_member = "ProcessExit::exit_process";
        let exact_nominal = |digest: &[u8; 32], declaration: &str| {
            compound(
                NameForm::Nominal,
                [
                    byte_atom(OwnerForm::ToolchainSource, digest),
                    atom(Label::Path, declaration),
                ],
            )
        };
        let signature_nominal = |declaration| {
            compound(
                NameForm::Nominal,
                [bare(OwnerForm::Toolchain), atom(Label::Path, declaration)],
            )
        };
        // These are the known catalog's ordinary declared signature types.
        // Physical i32 correspondence and actual emptiness are still checked
        // independently below; an identity string proves neither fact.
        let integer = numerics::integer_carrier::FixedInteger::signed(32).to_string();
        let argument = compound(TypeForm::Named, [atom(Label::Name, &integer)]);
        let parameter = compound(
            TypeForm::Named,
            [atom(
                Label::Name,
                &compound(NameForm::CompilerType, [atom(Label::Atom, &integer)]),
            )],
        );
        let result_type = compound(
            TypeForm::Named,
            [atom(Label::Name, &signature_nominal("Never"))],
        );
        let parameters = compound(
            CoordinateTag::Parameter,
            [
                atom(CoordinateTag::Mutable, "no"),
                atom(CoordinateTag::Const, "no"),
                argument,
            ],
        );
        let callable = |declaration| {
            compound(
                CoordinateTag::NamedCallable,
                [
                    atom(CoordinateTag::Path, &signature_nominal(declaration)),
                    atom(CoordinateTag::Parameters, &parameters),
                    atom(CoordinateTag::ResultDispatch, ""),
                ],
            )
        };
        let primitive_callable = callable(primitive);
        let requirement_callable = callable(requirement_member);
        HostedProcessExitIdentities {
            primitive: exact_nominal(&core_revision::PROCESS_EXIT_NATIVE, primitive),
            primitive_entry: exact_nominal(
                &core_revision::PROCESS_EXIT_NATIVE,
                &format!("{}::entry", primitive),
            ),
            primitive_semantic: format!(
                "toolchain::{}",
                callable_declaration_path(primitive, &primitive_callable)
            ),
            primitive_callable,
            requirement_owner: exact_nominal(&core_revision::PROCESS_EXIT, requirement),
            requirement_member: exact_nominal(&core_revision::PROCESS_EXIT, requirement_member),
            requirement_semantic: format!(
                "toolchain::{}",
                callable_declaration_path(requirement_member, &requirement_callable)
            ),
            requirement_callable,
            parameter,
            result: result_type,
        }
    })
}

/// Resolve the public contract of an authenticated primitive. This is
/// declaration admission only; execution still requires target and authority
/// settlement. The primitive never borrows the requirement's callable identity.
pub fn hosted_primitive_requirement(
    module: &TerminalModule,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
) -> Result<Option<semantic_vocabulary::BoundaryMachineId>, &'static str> {
    use terminal_psi::{
        BoundaryCompletion, BoundaryMachineResult, BoundaryRequirementKind,
        CheckedBoundaryExecution,
    };
    let Some(checked) = &boundary.declaration else {
        return Ok(None);
    };
    let CheckedBoundaryExecution::HostedPrimitive {
        operation,
        requirement,
    } = checked.execution
    else {
        return Ok(None);
    };
    if operation != language_core::sealed_intrinsics::HostedOperation::ProcessExit {
        return Err("hosted primitive operation has no admitted declaration");
    }
    let expected = hosted_process_exit_identities();
    let canonical = module
        .boundary_machine(requirement)
        .ok_or("hosted primitive references an absent requirement")?;
    let canonical_source = canonical
        .declaration
        .as_ref()
        .ok_or("hosted primitive requirement has no checked declaration")?;
    let actual_owner = module.identities.text(boundary.owner.identity());
    if boundary.id == requirement
        || boundary.kind != BoundaryRequirementKind::Machine
        || actual_owner != Some(expected.primitive.as_str())
        || module.identities.text(canonical.owner.identity())
            != Some(expected.requirement_owner.as_str())
        || module.identities.text(checked.source.state.identity())
            != Some(expected.primitive_entry.as_str())
        || module.identities.text(checked.source.semantic_declaration)
            != Some(expected.primitive_semantic.as_str())
        || module.identities.text(checked.source.callable)
            != Some(expected.primitive_callable.as_str())
        || boundary.identity != checked.source.callable
        || module
            .identities
            .text(canonical_source.source.state.identity())
            != Some(expected.requirement_member.as_str())
        || module
            .identities
            .text(canonical_source.source.semantic_declaration)
            != Some(expected.requirement_semantic.as_str())
        || module.identities.text(canonical_source.source.callable)
            != Some(expected.requirement_callable.as_str())
        || canonical.identity != canonical_source.source.callable
        || checked.source.parameter_types.len() != 1
        || checked
            .source
            .parameter_types
            .first()
            .and_then(|identity| module.identities.text(*identity))
            != Some(expected.parameter.as_str())
        || module.identities.text(checked.source.result_type) != Some(expected.result.as_str())
        || canonical.kind != BoundaryRequirementKind::Trait
        || canonical_source.execution != CheckedBoundaryExecution::Provider
        || checked.source.callable == canonical_source.source.callable
        || checked.source.semantic_declaration == canonical_source.source.semantic_declaration
        || canonical_source.source.published_may_suspend
        || canonical_source.source.published_may_block
        || checked.source.specialization.is_some()
        || !checked.method_arguments.is_empty()
        || boundary.attachment.is_some()
        || boundary.owning_service.is_some()
        || boundary.completion
            != BoundaryCompletion::External(
                semantic_vocabulary::ExternalCompletionKind::ProcessExit,
            )
        || boundary.completion != canonical.completion
        || !boundary.unconditional_termination
        || !canonical.unconditional_termination
        || checked.source.published_may_suspend
        || checked.source.published_may_block
        || checked.source.published_may_suspend != canonical_source.source.published_may_suspend
        || checked.source.published_may_block != canonical_source.source.published_may_block
        || boundary.scalar_parameters != canonical.scalar_parameters
        || boundary.scalar_parameters
            != [semantic_vocabulary::ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Signed,
                    32,
                )
                .map_err(|_| "invalid i32 type")?,
            )]
        || !boundary.structural_parameters.is_empty()
        || boundary.parameter_order != canonical.parameter_order
        || boundary.result != canonical.result
        || boundary.result_reference_regions != canonical.result_reference_regions
        || boundary.result_lifetimes != canonical.result_lifetimes
        || checked.source.parameter_types != canonical_source.source.parameter_types
        || checked.source.result_type != canonical_source.source.result_type
        || boundary.crash_routes != canonical.crash_routes
        || boundary.scalar_requires != canonical.scalar_requires
        || boundary.scalar_ensures != canonical.scalar_ensures
        || boundary.membership_requires != canonical.membership_requires
        || boundary.membership_ensures != canonical.membership_ensures
        || boundary.requires != canonical.requires
        || boundary.normal_qualifications != canonical.normal_qualifications
        || boundary.scalar_domain_requirements != canonical.scalar_domain_requirements
        || boundary.reference_outputs != canonical.reference_outputs
        || boundary.content_guarantees != canonical.content_guarantees
        || boundary.program_local_root_introductions
            != canonical.program_local_root_introductions
        || canonical.owning_service.is_none_or(|service| {
            canonical.fixed_service_reach != [service]
                || canonical.published_service_ceiling != [service]
        })
        || !boundary.published_synchronous_entries.is_empty()
        || !boundary.crash_routes.is_empty()
        || !boundary.scalar_requires.is_empty()
        || !boundary.scalar_ensures.is_empty()
        || !boundary.membership_requires.is_empty()
        || !boundary.membership_ensures.is_empty()
        || !boundary.requires.is_empty()
        || !boundary.normal_qualifications.is_empty()
        || !boundary.scalar_domain_requirements.is_empty()
        || !boundary.reference_outputs.is_empty()
        || !boundary.content_guarantees.is_empty()
        || !boundary.program_local_root_introductions.is_empty()
        || boundary.fixed_service_reach != canonical.fixed_service_reach
        || boundary.published_service_ceiling != canonical.published_service_ceiling
        || boundary.published_synchronous_entries != canonical.published_synchronous_entries
    {
        return Err("hosted primitive differs from its exact Core declaration and requirement");
    }
    let BoundaryMachineResult::Structural(result) = &boundary.result else {
        return Err("hosted primitive has no declared empty result");
    };
    let contracts = crate::inhabitation::TerminalContracts::new(module.types.structural_types.iter());
    let inhabitation = crate::inhabitation::Inhabitation::prepare(&contracts)
        .map_err(|_| "hosted primitive has malformed type contracts")?;
    if inhabitation.status(result.structural_type) != crate::inhabitation::Status::ProvedEmpty {
        return Err("hosted primitive result is not proved empty");
    }
    Ok(Some(requirement))
}

pub fn validate_boundary_callable(
    module: &TerminalModule,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
) -> Result<(), &'static str> {
    let Some(checked) = &boundary.declaration else {
        return Ok(());
    };
    if boundary.role != terminal_psi::BoundaryDeclarationRole::Callable {
        return Err("contract evidence cannot carry an executable boundary declaration");
    }
    hosted_primitive_requirement(module, boundary)?;
    if checked.execution == terminal_psi::CheckedBoundaryExecution::LayoutTargetFacts {
        super::layout_target::snapshot_fields(module, boundary)?;
    }
    if checked.execution == terminal_psi::CheckedBoundaryExecution::NumericPrimitive {
        crate::numeric_primitives::validate_boundary(module, boundary)?;
    }
    let declaration = &checked.source;
    for coordinate in [
        declaration.machine.identity(),
        declaration.state.identity(),
        declaration.semantic_declaration,
        declaration.callable,
        declaration.result_type,
    ]
    .into_iter()
    .chain(declaration.parameter_types.iter().copied())
    {
        identity(module, coordinate)?;
    }
    if declaration.contract == [0; 32]
        || !declaration.machine.same_identity_as(
            &module.identities,
            &boundary.owner,
            &module.identities,
        )
    {
        return Err("checked boundary declaration differs from its exact declaration owner");
    }
    match &declaration.specialization {
        Some(specialization) => {
            identity(module, specialization.template.identity())?;
            identity(module, specialization.template_entry.identity())?;
            if let Some(capture) = &specialization.capture {
                identity(module, capture.template_identity)?;
                if capture.template_commitment == [0; 32]
                    || capture.specialization_commitment == [0; 32]
                {
                    return Err("checked boundary capture has an empty commitment");
                }
            }
            if specialization.commitment == [0; 32] {
                return Err("checked boundary specialization has an empty commitment");
            }
        }
        None if !checked.method_arguments.is_empty() => {
            return Err("unspecialized boundary carries method arguments");
        }
        None => {}
    }
    for argument in &checked.method_arguments {
        let coordinate = match argument {
            ClosedReachArgument::Type(coordinate)
            | ClosedReachArgument::Const(coordinate)
            | ClosedReachArgument::Proposition(coordinate) => *coordinate,
            ClosedReachArgument::Machine {
                identity,
                contract_commitment,
            } => {
                if *contract_commitment == [0; 32] {
                    return Err("boundary method argument has an empty contract commitment");
                }
                *identity
            }
        };
        identity(module, coordinate)?;
    }
    Ok(())
}

fn callable_declaration(
    module: &TerminalModule,
    callable: TargetMachineCallable,
) -> Result<&terminal_psi::CheckedMachineDeclaration, &'static str> {
    match callable {
        TargetMachineCallable::Machine(machine) => module
            .machine(machine)
            .and_then(|machine| machine.declaration.as_ref()),
        TargetMachineCallable::Boundary(boundary) => module
            .boundary_machine(boundary)
            .filter(|boundary| boundary.role == terminal_psi::BoundaryDeclarationRole::Callable)
            .and_then(|boundary| boundary.declaration.as_ref())
            .map(|declaration| &declaration.source),
    }
    .ok_or("static conformance target has no checked declaration")
}

fn callable_result(
    module: &TerminalModule,
    callable: TargetMachineCallable,
) -> Result<terminal_psi::ClosedConformanceCallableResult, &'static str> {
    use terminal_psi::{
        BoundaryMachineResult, ClosedConformanceCallableResult as ResultClass,
        TerminalMachineResult,
    };
    match callable {
        TargetMachineCallable::Machine(machine) => match &module
            .machine(machine)
            .ok_or("conformance callable registry names an absent machine")?
            .result
        {
            TerminalMachineResult::Unit => Ok(ResultClass::Unit),
            TerminalMachineResult::Scalar(result) => {
                Ok(ResultClass::Scalar(result.scalar_type))
            }
            TerminalMachineResult::Structural(result) => {
                Ok(ResultClass::Structural(result.structural_type))
            }
        },
        TargetMachineCallable::Boundary(boundary) => match &module
            .boundary_machine(boundary)
            .ok_or("conformance callable registry names an absent boundary")?
            .result
        {
            BoundaryMachineResult::Unit => Ok(ResultClass::Unit),
            BoundaryMachineResult::Scalar(result) => {
                Ok(ResultClass::Scalar(result.scalar_type))
            }
            BoundaryMachineResult::Structural(result) => {
                Ok(ResultClass::Structural(result.structural_type))
            }
        },
    }
}

fn declared_callable_identity(
    module: &TerminalModule,
    declaration: &terminal_psi::CheckedMachineDeclaration,
) -> Result<String, &'static str> {
    let callable = identity(module, declaration.callable)?;
    Ok(match &declaration.specialization {
        None => callable.to_owned(),
        Some(specialization) => {
            let commitment = specialization
                .commitment
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            format!(
                "specialized-machine|callable={}:{}|application={commitment}",
                callable.len(),
                callable
            )
        }
    })
}

fn identity(
    module: &TerminalModule,
    identity: TerminalIdentityIndex,
) -> Result<&str, &'static str> {
    module
        .identities
        .text(identity)
        .filter(|text| !text.is_empty())
        .ok_or("closed conformance contains an invalid or empty identity")
}

fn take<'a>(bytes: &mut &'a [u8], count: usize) -> Result<&'a [u8], &'static str> {
    let value = bytes
        .get(..count)
        .ok_or("conformance schema definition is truncated")?;
    *bytes = &bytes[count..];
    Ok(value)
}

fn length(bytes: &mut &[u8]) -> Result<usize, &'static str> {
    let raw: [u8; 8] = take(bytes, 8)?
        .try_into()
        .map_err(|_| "invalid schema length")?;
    u64::from_le_bytes(raw)
        .try_into()
        .map_err(|_| "schema length exceeds host positions")
}

fn definition_type<'a>(bytes: &mut &'a [u8]) -> Result<&'a str, &'static str> {
    let count = length(bytes)?;
    if count == 0 {
        return Err("original signature has an empty declared type");
    }
    std::str::from_utf8(take(bytes, count)?)
        .map_err(|_| "original signature type is not canonical text")
}

fn validate_definition(schema: &ClosedConformanceSchema) -> Result<(), &'static str> {
    definition_types(schema).map(|_| ())
}

pub(crate) fn definition_types(
    schema: &ClosedConformanceSchema,
) -> Result<Option<(u32, Vec<&str>, &str)>, &'static str> {
    let mut bytes = schema.definition.as_slice();
    let domain = b"omega.requirement-schema.v5\0";
    if take(&mut bytes, domain.len())? != domain {
        return Err("conformance schema has an unknown definition domain");
    }
    let signature = length(&mut bytes)?;
    if signature == 0 {
        return Err("conformance schema has no original signature");
    }
    take(&mut bytes, signature)?;
    if length(&mut bytes)? != schema.lifetime_parameter_count as usize
        || length(&mut bytes)? != schema.parameter_kinds.len()
    {
        return Err("conformance schema telescope differs from its original definition");
    }
    for expected in &schema.parameter_kinds {
        let kind = take(&mut bytes, 1)?[0];
        if !(1..=6).contains(&kind) || kind != *expected {
            return Err("conformance schema binder category differs from its definition");
        }
        if !(1..=3).contains(&take(&mut bytes, 1)?[0]) {
            return Err("conformance schema has an invalid multiplicity");
        }
        let carry = bytes
            .iter()
            .position(|byte| *byte == 0)
            .ok_or("schema carry demand is unterminated")?;
        std::str::from_utf8(take(&mut bytes, carry)?)
            .map_err(|_| "schema carry demand is not canonical text")?;
        take(&mut bytes, 1)?;
        let contract = length(&mut bytes)?;
        take(&mut bytes, contract)?;
    }
    let types = match take(&mut bytes, 1)?[0] {
        0 => None,
        1 => {
            let outer = u32::try_from(length(&mut bytes)?)
                .map_err(|_| "original signature lifetime roster exceeds u32")?;
            // Elided signature positions take fresh lifetimes after
            // every declared one.
            let elided = u32::try_from(length(&mut bytes)?)
                .map_err(|_| "original signature elided lifetimes exceed u32")?;
            let lifetimes = outer
                .checked_add(schema.lifetime_parameter_count)
                .and_then(|lifetimes| lifetimes.checked_add(elided))
                .ok_or("original signature lifetime roster overflows")?;
            let count = length(&mut bytes)?;
            if count > bytes.len() / 8 {
                return Err("original signature parameter types are truncated");
            }
            let mut parameters = Vec::new();
            parameters
                .try_reserve_exact(count)
                .map_err(|_| "original signature parameter allocation failed")?;
            for _ in 0..count {
                parameters.push(definition_type(&mut bytes)?);
            }
            let result = definition_type(&mut bytes)?;
            Some((lifetimes, parameters, result))
        }
        _ => return Err("original signature has an invalid type-presence flag"),
    };
    if !bytes.is_empty() {
        return Err("conformance schema definition has trailing bytes");
    }
    let generic = schema.lifetime_parameter_count != 0 || !schema.parameter_kinds.is_empty();
    match (schema.kind, generic, &schema.finite_tuples) {
        (ClosedConformanceSchemaKind::Nongeneric, false, None)
        | (ClosedConformanceSchemaKind::Open, true, None) => Ok(types),
        (ClosedConformanceSchemaKind::Finite, true, Some(tuples))
            if schema.lifetime_parameter_count == 0
                && schema
                    .parameter_kinds
                    .iter()
                    .all(|kind| matches!(kind, 2 | 3))
                && tuples
                    .iter()
                    .all(|tuple| tuple.len() == schema.parameter_kinds.len()) =>
        {
            Ok(types)
        }
        _ => Err(
            "conformance schema classification contradicts its retained telescope or roster",
        ),
    }
}

pub(crate) fn argument_matches(
    module: &TerminalModule,
    expected: &ClosedReachArgument,
    actual: &ClosedReachParameter,
) -> bool {
    match (expected, actual) {
        (ClosedReachArgument::Type(identity), ClosedReachParameter::Type { argument })
        | (
            ClosedReachArgument::Proposition(identity),
            ClosedReachParameter::Proposition { argument },
        ) => module.identities.text(*identity) == Some(argument.as_str()),
        (ClosedReachArgument::Const(identity), ClosedReachParameter::Const { argument }) => {
            argument.index_identity().as_deref() == module.identities.text(*identity)
        }
        (
            ClosedReachArgument::Machine {
                identity,
                contract_commitment,
            },
            ClosedReachParameter::Machine(binding),
        ) => {
            *identity == binding.selected_identity
                && *contract_commitment == binding.selected_contract_commitment
        }
        _ => false,
    }
}

fn application_key(
    application: &terminal_psi::ClosedConformanceApplication,
) -> (
    semantic_vocabulary::MachineId,
    terminal_psi::ClosedConformanceApplicationCommitment,
    u64,
) {
    (
        application.owner,
        application.commitment,
        application.report_fingerprint,
    )
}

/// Match one actual public call to its complete closed selection. This
/// correspondence does not admit any package edge or grant issuer rights.
pub fn dispatch_application<'module>(
    applications: &[&'module terminal_psi::ClosedConformanceApplication],
    caller: semantic_vocabulary::MachineId,
    dispatch: &terminal_psi::StaticRequirementDispatch,
    callee: semantic_vocabulary::MachineId,
) -> Result<&'module terminal_psi::ClosedConformanceApplication, &'static str> {
    if callee != dispatch.realization {
        return Err("static requirement dispatch differs from its ordinary runtime call");
    }
    let key = (
        caller,
        dispatch.conformance_application_commitment,
        dispatch.conformance_application_report_fingerprint,
    );
    let start = applications.partition_point(|application| application_key(application) < key);
    let end = applications.partition_point(|application| application_key(application) <= key);
    let [application] = &applications[start..end] else {
        return Err("static requirement dispatch has no unique closed conformance application");
    };
    if application
        .schemas
        .iter()
        .filter(|schema| {
            schema.declaring_trait_identity == dispatch.declaring_trait_identity
                && schema.public_requirement_identity == dispatch.public_requirement_identity
                && schema.requirement_identity == dispatch.requirement_identity
                && schema.realization_identity == dispatch.realization_identity
        })
        .count()
        != 1
    {
        return Err("static requirement dispatch has no exact original schema");
    }
    if application
        .static_applications
        .iter()
        .filter(|applied| {
            applied.public_requirement_identity == dispatch.public_requirement_identity
                && applied.realization_identity == dispatch.realization_identity
                && applied.source_callable_identity == dispatch.realization_callable_identity
                && applied.callable == TargetMachineCallable::Machine(dispatch.realization)
        })
        .count()
        != 1
    {
        return Err("static requirement dispatch has no exact executable method application");
    }
    Ok(*application)
}

/// Prepare the existing application lookup once for a receiving operation.
/// Equal keys remain visible so a demanded ambiguous selection refuses.
pub fn dispatch_applications(
    module: &TerminalModule,
) -> Vec<&terminal_psi::ClosedConformanceApplication> {
    let mut applications = module
        .proofs.closed_conformance_applications
        .iter()
        .collect::<Vec<_>>();
    applications.sort_unstable_by_key(|application| application_key(application));
    applications
}

/// Recover the package coordinate of a canonical declaration identity so
/// its actual source request can be matched. The coordinate is not authority.
pub fn declaration_package(
    module: &TerminalModule,
    identity: TerminalIdentityIndex,
) -> Result<Option<semantic_vocabulary::PackageKeyIdentity>, &'static str> {
    identity_package(&module.identities, identity)
}

/// Read the same canonical declaration coordinate after a consumer has
/// retained the identity table without the source module. This lookup
/// alone does not authorize an operation on a foreign declaration.
pub(crate) fn identity_package(
    identities: &terminal_psi::TerminalIdentityTable,
    identity: TerminalIdentityIndex,
) -> Result<Option<semantic_vocabulary::PackageKeyIdentity>, &'static str> {
    use application_identity::type_identity::TypeIdentityPackageOwnerVisitor;
    use application_identity::type_identity::TypeIdentityVisitError;
    use application_identity::type_identity::visit_type_identity_package_owners;
    struct Owner {
        package: Option<semantic_vocabulary::PackageKeyIdentity>,
        depth: usize,
        remaining: usize,
    }
    impl TypeIdentityPackageOwnerVisitor for Owner {
        fn enter(&mut self) -> Result<(), TypeIdentityVisitError> {
            if self.depth == 128 {
                return Err(TypeIdentityVisitError::ResourceLimitExceeded);
            }
            self.depth += 1;
            Ok(())
        }
        fn leave(&mut self) {
            self.depth -= 1;
        }
        fn reserve(&mut self, bytes: usize) -> Result<(), TypeIdentityVisitError> {
            self.remaining = self
                .remaining
                .checked_sub(bytes)
                .ok_or(TypeIdentityVisitError::ResourceLimitExceeded)?;
            Ok(())
        }
        fn package_owner(&mut self, digest: [u8; 32]) -> Result<(), TypeIdentityVisitError> {
            let package = semantic_vocabulary::PackageKeyIdentity::from_digest(digest)
                .ok_or(TypeIdentityVisitError::MalformedIdentity)?;
            if self.package.replace(package).is_some() {
                return Err(TypeIdentityVisitError::MalformedIdentity);
            }
            Ok(())
        }
        fn embedded_name(&mut self, _: &str) -> Result<(), TypeIdentityVisitError> {
            Err(TypeIdentityVisitError::UnsupportedEmbeddedName)
        }
    }
    let text = identities
        .text(identity)
        .ok_or("selected declaration has no canonical identity")?;
    let mut owner = Owner {
        package: None,
        depth: 0,
        remaining: text.len(),
    };
    visit_type_identity_package_owners(text, &mut owner)
        .map_err(|_| "selected declaration lost its exact canonical package owner")?;
    Ok(owner.package)
}

pub fn validate_closed_conformances(module: &TerminalModule) -> Result<(), &'static str> {
    for boundary in &module.interfaces.boundary_machines {
        validate_boundary_callable(module, boundary)?;
    }
    for application in &module.proofs.closed_conformance_applications {
        identity(module, application.declaration_identity)?;
        identity(module, application.trait_identity)?;
        if module.machine(application.owner).is_none() {
            return Err("closed conformance owner is absent");
        }
        let mut previous = None;
        for schema in &application.schemas {
            let key = (
                identity(module, schema.declaring_trait_identity)?,
                identity(module, schema.public_requirement_identity)?,
            );
            identity(module, schema.requirement_identity)?;
            identity(module, schema.requirement_declaration.identity())?;
            identity(module, schema.realization_identity)?;
            if previous.is_some_and(|previous| previous >= key) {
                return Err("conformance schemas are repeated or not canonical");
            }
            previous = Some(key);
            validate_definition(schema)?;
            let rows = application
                .rows
                .iter()
                .filter(|row| {
                    row.declaring_trait_identity == schema.declaring_trait_identity
                        && row.public_requirement_identity == schema.public_requirement_identity
                        && row.requirement_identity == schema.requirement_identity
                        && row.requirement_declaration == schema.requirement_declaration
                })
                .collect::<Vec<_>>();
            match schema.kind {
                ClosedConformanceSchemaKind::Open if !rows.is_empty() => {
                    return Err("open conformance schema advertises finite execution coverage");
                }
                ClosedConformanceSchemaKind::Nongeneric
                    if rows.len() != 1
                        || !rows[0].family_tuple.is_empty()
                        || rows[0].realization_identity != schema.realization_identity =>
                {
                    return Err("nongeneric conformance schema lost its original row");
                }
                ClosedConformanceSchemaKind::Finite => {
                    let tuples = schema
                        .finite_tuples
                        .as_ref()
                        .ok_or("finite schema lost its declared roster")?;
                    for tuple in tuples {
                        for argument in tuple {
                            identity(module, *argument)?;
                        }
                    }
                    let tuple_keys = tuples
                        .iter()
                        .map(|tuple| {
                            tuple
                                .iter()
                                .map(|argument| identity(module, *argument))
                                .collect::<Result<Vec<_>, _>>()
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    if tuple_keys.windows(2).any(|pair| pair[0] >= pair[1]) {
                        return Err(
                            "finite conformance schema tuples are repeated or not canonical",
                        );
                    }
                    if tuples
                        .iter()
                        .enumerate()
                        .any(|(index, tuple)| tuples[..index].contains(tuple))
                        || rows.len() != tuples.len()
                        || tuples.iter().any(|tuple| {
                            rows.iter().filter(|row| &row.family_tuple == tuple).count() != 1
                        })
                    {
                        return Err(
                            "finite conformance schema has partial or duplicate tuple coverage",
                        );
                    }
                }
                _ => {}
            }
        }
        for row in &application.rows {
            identity(module, row.realization_identity)?;
            if application
                .schemas
                .iter()
                .filter(|schema| {
                    schema.declaring_trait_identity == row.declaring_trait_identity
                        && schema.public_requirement_identity == row.public_requirement_identity
                        && schema.requirement_identity == row.requirement_identity
                        && schema.requirement_declaration == row.requirement_declaration
                })
                .count()
                != 1
            {
                return Err("conformance row has no unique original schema");
            }
        }
        let mut registry_identities = Vec::new();
        let mut registry_machines = Vec::new();
        for callable in &application.realization_callables {
            let callable_identity = identity(module, callable.source_callable_identity)?;
            if registry_identities
                .last()
                .is_some_and(|previous| *previous >= callable_identity)
                || registry_machines.contains(&callable.callable)
            {
                return Err(
                    "conformance callable registry has duplicate identities or machine bindings, or is not canonical",
                );
            }
            registry_identities.push(callable_identity);
            registry_machines.push(callable.callable);
            if callable.result != callable_result(module, callable.callable)? {
                return Err(
                    "conformance callable registry result differs from its emitted declaration",
                );
            }
            let declaration = callable_declaration(module, callable.callable)?;
            if declared_callable_identity(module, declaration)? != callable_identity {
                return Err(
                    "conformance callable registry differs from its checked declaration identity",
                );
            }
        }
        let application_keys = application
            .static_applications
            .iter()
            .map(|row| static_application_order_key(&module.identities, row))
            .collect::<Result<Vec<_>, _>>()?;
        if application_keys.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("conformance static applications are repeated or not canonical");
        }
        for applied in &application.static_applications {
            identity(module, applied.public_requirement_identity)?;
            identity(module, applied.realization_identity)?;
            identity(module, applied.source_callable_identity)?;
            let schemas = application
                .schemas
                .iter()
                .filter(|schema| {
                    schema.public_requirement_identity == applied.public_requirement_identity
                        && schema.realization_identity == applied.realization_identity
                })
                .collect::<Vec<_>>();
            let [schema] = schemas.as_slice() else {
                return Err("static conformance application has no unique original schema");
            };
            if applied.method_arguments.len() != schema.parameter_kinds.len() {
                return Err(
                    "static conformance method tuple differs from its independent telescope",
                );
            }
            for (kind, argument) in schema.parameter_kinds.iter().zip(&applied.method_arguments)
            {
                let (actual, argument_identity) = match argument {
                    ClosedReachArgument::Type(identity) => (1, *identity),
                    ClosedReachArgument::Const(identity) => (2, *identity),
                    ClosedReachArgument::Proposition(identity) => (6, *identity),
                    ClosedReachArgument::Machine { identity, .. } => (4, *identity),
                };
                identity(module, argument_identity)?;
                if actual != *kind && !(actual == 2 && *kind == 3) {
                    return Err(
                        "static conformance method argument has the wrong binder category",
                    );
                }
            }
            if let Some(tuples) = &schema.finite_tuples {
                let values = applied
                    .method_arguments
                    .iter()
                    .map(|argument| match argument {
                        ClosedReachArgument::Const(identity) => Some(*identity),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or(
                        "finite conformance method application contains a non-value argument",
                    )?;
                if !tuples.contains(&values) {
                    return Err(
                        "static conformance method tuple is outside its original finite roster",
                    );
                }
            }
            let declaration = callable_declaration(module, applied.callable)?;
            if declaration
                .specialization
                .as_ref()
                .map(|specialization| specialization.commitment)
                != applied.application_commitment
            {
                return Err(
                    "static conformance application differs from its checked declaration commitment",
                );
            }
            if application
                .realization_callables
                .iter()
                .filter(|callable| {
                    callable.source_callable_identity == applied.source_callable_identity
                        && callable.callable == applied.callable
                })
                .count()
                != 1
            {
                return Err(
                    "static conformance application has no unique callable registry entry",
                );
            }
            match applied.callable {
                TargetMachineCallable::Boundary(boundary) => {
                    let arguments = module
                        .boundary_machine(boundary)
                        .and_then(|boundary| boundary.declaration.as_ref())
                        .ok_or("static conformance boundary has no independent invocation")?;
                    if arguments.method_arguments != applied.method_arguments {
                        return Err(
                            "static conformance method tuple differs from its retained boundary invocation",
                        );
                    }
                }
                TargetMachineCallable::Machine(machine)
                    if !applied.method_arguments.is_empty() =>
                {
                    let machine = module
                        .machine(machine)
                        .ok_or("static conformance application target is absent")?;
                    let invocation = machine.closed_reach_application.as_ref().ok_or(
                        "static conformance method tuple has no independent retained invocation",
                    )?;
                    if Some(invocation.specialization_commitment)
                        != applied.application_commitment
                        || invocation.telescope.len() != applied.method_arguments.len()
                        || !applied
                            .method_arguments
                            .iter()
                            .zip(&invocation.telescope)
                            .all(|(expected, actual)| {
                                argument_matches(module, expected, actual)
                            })
                    {
                        return Err(
                            "static conformance method tuple differs from its retained executable invocation",
                        );
                    }
                }
                TargetMachineCallable::Machine(_) => {}
            }
        }
        if terminal_psi::closed_conformance_application_commitment(
            &module.identities,
            application,
        ) != application.commitment
            || terminal_psi::closed_conformance_application_report_fingerprint(
                &module.identities,
                application,
            ) != application.report_fingerprint
        {
            return Err(
                "closed conformance application commitment differs from its semantic contents",
            );
        }
    }
    // Every selected call authenticates the same retained application key.
    // Sort borrowed rows once so ordinary calls do not each scan and
    // allocate a copy of the complete module roster. Equal keys remain in
    // the slice so a demanded ambiguous application still refuses.
    let applications = dispatch_applications(module);
    let mut proof_calls = module
        .proofs.proof_output_calls
        .iter()
        .filter_map(|call| {
            call.runtime_call
                .map(|runtime| ((call.caller, runtime.site), call, runtime.target))
        })
        .collect::<Vec<_>>();
    proof_calls.sort_unstable_by_key(|(site, _, _)| *site);
    for invocation in &module.proofs.proof_output_calls {
        let Some(dispatch) = &invocation.static_requirement_dispatch else {
            continue;
        };
        let callee = invocation
            .runtime_call
            .as_ref()
            .and_then(|runtime| runtime.target.machine())
            .ok_or("static requirement dispatch has no ordinary named runtime call")?;
        dispatch_application(&applications, invocation.caller, dispatch, callee)?;
    }
    for machine in &module.machines {
        for invocation in machine.module_invocations() {
            let Some(required) = invocation.inputs.required_contract else {
                continue;
            };
            let contract = module
                .invocation_machine_contract(invocation.inputs.callee, Some(required))
                .ok_or("selected call has no exact public contract and body association")?;
            let body = module
                .machine(invocation.inputs.callee)
                .ok_or("selected public contract lost its executable body")?;
            let public_envelope = module
                .execution.callables
                .declaration_envelope(required)
                .ok_or("selected public contract has no complete operational envelope")?;
            let body_envelope = module
                .execution.callables
                .machine_envelope(body.id)
                .ok_or("selected public contract has no complete selected-body envelope")?;
            if (contract.declared_service_reach.is_none()
                && contract.published_service_ceiling != body.published_service_ceiling)
                || (contract.declared_synchronous_entries.is_none()
                    && contract.published_synchronous_entries
                        != body.published_synchronous_entries)
                || (contract.declared_crash_routes.is_none()
                    && contract.crash_routes != body.crash_routes)
            {
                return Err("selected public effects differ from their exact implementation");
            }
            if contract.declared_synchronous_entries.is_none()
                && public_envelope.invocations != body_envelope.invocations
            {
                return Err(
                    "deferred public invocations differ from their exact implementation",
                );
            }
            let dispatch = contract
                .selected_application
                .as_ref()
                .ok_or("selected call public contract has no closed application")?;
            let application = dispatch_application(
                &applications,
                machine.id,
                dispatch,
                invocation.inputs.callee,
            )?;
            // A proof sidecar on this ordinary call keeps the same public
            // selection. It cannot describe the physical body as a direct
            // proof call while the operation names a public requirement.
            let site = (machine.id, invocation.site);
            let start = proof_calls.partition_point(|(key, _, _)| *key < site);
            let end = proof_calls.partition_point(|(key, _, _)| *key <= site);
            if end - start > 1 {
                return Err("selected public call has ambiguous proof-call correspondence");
            }
            for (_, proof, callee) in &proof_calls[start..end] {
                if *callee
                    != terminal_psi::EvidenceCallableTarget::Machine(invocation.inputs.callee)
                    || proof.static_requirement_dispatch.as_ref() != Some(dispatch)
                {
                    return Err(
                        "selected proof call differs from its operation's public contract",
                    );
                }
            }
            let declaration = contract
                .declaration
                .as_ref()
                .ok_or("selected call public contract has no source declaration")?;
            let schema = application
                .schemas
                .iter()
                .find(|schema| {
                    schema.declaring_trait_identity == dispatch.declaring_trait_identity
                        && schema.public_requirement_identity
                            == dispatch.public_requirement_identity
                        && schema.requirement_identity == dispatch.requirement_identity
                        && schema.realization_identity == dispatch.realization_identity
                })
                .ok_or("selected public contract has no original requirement schema")?;
            match &declaration.specialization {
                None => {
                    if declaration.state != schema.requirement_declaration
                        || declaration.callable != schema.requirement_identity
                    {
                        return Err(
                            "selected public contract differs from its complete requirement application",
                        );
                    }
                }
                Some(specialization) => {
                    if specialization.capture.is_some()
                        || specialization.template != schema.requirement_declaration
                        || specialization.template_entry != schema.requirement_declaration
                        || specialization.template_callable
                            != schema.public_requirement_identity
                    {
                        return Err("selected public member has another source template");
                    }
                    let mut methods = application.static_applications.iter().filter(|method| {
                        method.public_requirement_identity
                            == dispatch.public_requirement_identity
                            && method.realization_identity == dispatch.realization_identity
                            && method.source_callable_identity
                                == dispatch.realization_callable_identity
                            && method.callable
                                == TargetMachineCallable::Machine(dispatch.realization)
                    });
                    let method = methods
                        .next()
                        .ok_or("selected public member lost its exact method application")?;
                    if methods.next().is_some() {
                        return Err(
                            "selected public member has no unique complete method tuple",
                        );
                    }
                    let arguments = method.method_arguments.iter().map(|argument| {
                        use application_identity::conformance_identity::MachineOperandSelectionIdentity;
                        match argument {
                            ClosedReachArgument::Type(index) => {
                                Ok(MachineOperandSelectionIdentity::Type(identity(module, *index)?.to_owned()))
                            }
                            ClosedReachArgument::Const(index) => {
                                Ok(MachineOperandSelectionIdentity::Const(identity(module, *index)?.to_owned()))
                            }
                            _ => Err("selected public member needs a complete type or value tuple"),
                        }
                    }).collect::<Result<Vec<_>, _>>()?;
                    let commitment = application_identity::conformance_identity::requirement_application_commitment(
                        identity(module, specialization.template_callable)?, &arguments,
                    )?;
                    if commitment != specialization.commitment {
                        return Err(
                            "selected public member differs from the invoked method tuple",
                        );
                    }
                }
            }
            if contract.result.structural().is_some_and(|result| {
                terminal_psi::structural_type_contains_reference_by(
                    |id| module.structural_type(id),
                    result.structural_type,
                )
            }) {
                let public = crate::selected_reference_result(
                    module,
                    machine.id,
                    invocation.inputs.callee,
                    required,
                )?;
                let actual = body
                    .result
                    .structural()
                    .ok_or("selected reference body has no structural result")?;
                public.validate_actual_sources(
                    &body.structural_parameters,
                    &actual.reference_sources,
                )?;
            }
        }
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum MethodArgumentOrderKey<'a> {
    Type(&'a str),
    Const(&'a str),
    Proposition(&'a str),
    Machine(&'a str, [u8; 32]),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct StaticApplicationOrderKey<'a> {
    requirement: &'a str,
    realization: &'a str,
    callable: &'a str,
    arguments: Vec<MethodArgumentOrderKey<'a>>,
    commitment: Option<[u8; 32]>,
}

pub fn static_application_order_key<'a>(
    identities: &'a terminal_psi::TerminalIdentityTable,
    application: &terminal_psi::ClosedConformanceStaticApplication,
) -> Result<StaticApplicationOrderKey<'a>, &'static str> {
    let text = |identity| {
        identities
            .text(identity)
            .filter(|value| !value.is_empty())
            .ok_or("static application order contains an invalid identity")
    };
    let arguments = application
        .method_arguments
        .iter()
        .map(|argument| {
            Ok(match argument {
                ClosedReachArgument::Type(identity) => {
                    MethodArgumentOrderKey::Type(text(*identity)?)
                }
                ClosedReachArgument::Const(identity) => {
                    MethodArgumentOrderKey::Const(text(*identity)?)
                }
                ClosedReachArgument::Proposition(identity) => {
                    MethodArgumentOrderKey::Proposition(text(*identity)?)
                }
                ClosedReachArgument::Machine {
                    identity,
                    contract_commitment,
                } => MethodArgumentOrderKey::Machine(text(*identity)?, *contract_commitment),
            })
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    Ok(StaticApplicationOrderKey {
        requirement: text(application.public_requirement_identity)?,
        realization: text(application.realization_identity)?,
        callable: text(application.source_callable_identity)?,
        arguments,
        commitment: application.application_commitment,
    })
}
