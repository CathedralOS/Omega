//! Rejoin a selected fixed-image member to its revision-pinned Core declaration.

use application_identity::type_identity::compiler_type_identity;
use application_identity::type_identity::encoding::{
    CoordinateTag, atom, bare, byte_atom, callable_declaration_path, compound,
};
use application_identity::type_identity::vocabulary::{Label, NameForm, OwnerForm};
use language_core::bundled_files::{BundledFile, core_revision};
use language_core::sealed_intrinsics::{
    FixedImageOperation, SealedIntrinsic, SealedIntrinsicCatalog,
};
use semantic_vocabulary::{
    BoundaryRequirementKind, ExternalBindingIdentity, IntegerType, ScalarType,
};
use terminal_psi::{
    BoundaryMachineResult, ExternalProviderConformance, TerminalModule, TerminalProviderSelection,
    TerminalSealedNativeIntrinsic,
};

use super::text;

pub(super) fn selected<'m>(
    module: &'m TerminalModule,
    choice: &TerminalProviderSelection,
    requirement: &str,
) -> Result<Option<(usize, &'m ExternalProviderConformance)>, String> {
    let mut matched = None;
    for (position, supply) in module.interfaces.external_providers.iter().enumerate() {
        let Some(owner) = &supply.provider else {
            continue;
        };
        if owner.declaration.identity() != choice.provider_declaration
            || owner.identity != choice.provider_identity
            || owner.package != choice.provider_package
            || text(module, supply.requirement_identity)? != requirement
            || supply.sealed_intrinsic.is_none()
        {
            continue;
        }
        supply.validate(&module.identities)?;
        if choice.provider_custody
            != semantic_vocabulary::compiler_evaluation::DeclarationCustody::Toolchain
            || choice.provider_application.is_some()
            || choice.composition_mode
                != semantic_vocabulary::source_selections::CompositionMode::Fused
        {
            return Err(
                "a sealed fixed-image supply requires its exact bundled implementation".into(),
            );
        }
        if supply.sealed_intrinsic
            != Some(TerminalSealedNativeIntrinsic::FixedImage(
                FixedImageOperation::ProgramImageRetainedSuffixStart,
            ))
        {
            return Err(
                "selected external operation has no admitted logical provider realization".into(),
            );
        }
        if supply.target.as_deref() != Some(FixedImageOperation::TARGET)
            || choice.target != FixedImageOperation::TARGET
        {
            return Err("the selected fixed-image operation requires uefi_x86_64".into());
        }
        if matched.is_some() {
            return Err(
                "chosen provider has competing fixed-image conformances for the same requirement"
                    .into(),
            );
        }
        verify_image_start(module, choice, supply, owner)?;
        matched = Some((position, supply));
    }
    Ok(matched)
}

fn verify_image_start(
    module: &TerminalModule,
    choice: &TerminalProviderSelection,
    supply: &ExternalProviderConformance,
    owner: &terminal_psi::ExternalProviderOwner,
) -> Result<(), String> {
    let operation = FixedImageOperation::ProgramImageRetainedSuffixStart;
    let catalog_path = |intrinsic| {
        SealedIntrinsicCatalog::exact_declaration(intrinsic)
            .filter(|member| member.source == BundledFile::CoreProgramImage)
            .map(|member| member.declaration)
            .ok_or("fixed-image operation lost its bundled declaration")
    };
    let method = catalog_path(SealedIntrinsic::FixedImageRequirement(operation))?;
    let member = catalog_path(SealedIntrinsic::FixedImageRealization(operation))?;
    let (service, name) = method
        .rsplit_once("::")
        .ok_or("fixed-image requirement has no owner")?;
    let (provider, _) = member
        .split_once("::")
        .ok_or("fixed-image realization has no provider")?;
    let exact = |path| {
        compound(
            NameForm::Nominal,
            [
                byte_atom(OwnerForm::ToolchainSource, &core_revision::PROGRAM_IMAGE),
                atom(Label::Path, path),
            ],
        )
    };
    let callable = |path| {
        compound(
            CoordinateTag::NamedCallable,
            [
                atom(
                    CoordinateTag::Path,
                    &compound(
                        NameForm::Nominal,
                        [bare(OwnerForm::Toolchain), atom(Label::Path, path)],
                    ),
                ),
                atom(CoordinateTag::Parameters, ""),
                atom(CoordinateTag::ResultDispatch, ""),
            ],
        )
    };
    let required_callable = callable(method);
    let supplied_callable = callable(member);
    let leaf = &supply.realization;
    let boundary = module
        .interfaces
        .boundary_machines
        .iter()
        .find(|row| row.identity == supply.requirement_identity)
        .ok_or("fixed-image operation has no declared requirement")?;
    let checked = boundary
        .declaration
        .as_ref()
        .ok_or("fixed-image requirement has no checked source")?;
    let declared = &checked.source;
    let addr = IntegerType::address(64).map_err(|_| "invalid fixed-image address type")?;
    if supply.binding != ExternalBindingIdentity::CompilerIntrinsic
        || supply.numeric_intrinsic.is_some()
        || supply.requirement_schema.is_some()
        || owner.package.is_some()
        || supply.realization_package.is_some()
        || supply.requirement_package.is_some()
        || text(module, choice.schema_declaration)? != format!("toolchain::{service}")
        || !module.interfaces.services.iter().any(|row| {
            Some(row.id) == boundary.owning_service
                && row.identity == choice.schema_application
                && row.parents.is_empty()
        })
        || text(module, owner.identity)? != format!("toolchain::{provider}")
        || text(module, owner.declaration.identity())? != exact(provider)
        || text(module, supply.requirement.identity())? != exact(service)
        || declared.machine != supply.requirement
        || boundary.owner != supply.requirement
        || text(module, declared.state.identity())? != exact(method)
        || text(module, boundary.identity)? != required_callable
        || declared.callable != boundary.identity
        || text(module, declared.semantic_declaration)?
            != format!(
                "toolchain::{}",
                callable_declaration_path(method, &required_callable)
            )
        || text(module, leaf.machine.identity())? != exact(member)
        || text(module, leaf.state.identity())? != exact(&format!("{member}::{name}"))
        || text(module, leaf.callable)? != supplied_callable
        || text(module, leaf.semantic_declaration)?
            != format!(
                "toolchain::{}",
                callable_declaration_path(member, &supplied_callable)
            )
        || checked.execution != terminal_psi::CheckedBoundaryExecution::Provider
        || !checked.method_arguments.is_empty()
        || !declared.parameter_types.is_empty()
        || declared.specialization.is_some()
        || declared.published_may_suspend
        || declared.published_may_block
        || !empty_image_obligations(boundary)
        || boundary.kind != BoundaryRequirementKind::Trait
        || boundary.role != terminal_psi::BoundaryDeclarationRole::Callable
        || boundary.completion != terminal_psi::BoundaryCompletion::NormalReturn
        || !boundary.crash_routes.is_empty()
        || !boundary.parameter_order.is_empty()
        || !boundary.structural_parameters.is_empty()
        || !boundary.scalar_parameters.is_empty()
        || boundary.result != BoundaryMachineResult::Scalar(ScalarType::Integer(addr).into())
        || !leaf.parameter_types.is_empty()
        || leaf.result_type != declared.result_type
        || text(module, leaf.result_type)? != compiler_type_identity("address")
        || leaf.specialization.is_some()
        || leaf.published_may_suspend
        || leaf.published_may_block
    {
        return Err(
            "fixed-image operation differs from its checked Core member or requirement".into(),
        );
    }
    Ok(())
}

// The address is a scalar observation. No branch of this operation establishes
// provenance, storage, a proof about the result, or any other service's authority.
fn empty_image_obligations(boundary: &terminal_psi::BoundaryMachineDeclaration) -> bool {
    !boundary.unconditional_termination
        && boundary.numeric_intrinsic.is_none()
        && boundary.attachment.is_none()
        && boundary.result_lifetimes.as_deref() == Some(&[])
        && boundary.result_reference_regions.is_empty()
        && boundary.scalar_requires.is_empty()
        && boundary.scalar_ensures.is_empty()
        && boundary.requires.is_empty()
        && boundary.normal_qualifications.is_empty()
        && boundary.scalar_domain_requirements.is_empty()
        && boundary.membership_ensures.is_empty()
        && boundary.reference_outputs.is_empty()
        && boundary.content_guarantees.is_empty()
        && boundary.program_local_root_introductions.is_empty()
        && boundary.fixed_service_reach.is_empty()
        && boundary.published_service_ceiling.is_empty()
        && boundary.published_synchronous_entries.is_empty()
        && !boundary.service_reach_is_installation_bound
}
