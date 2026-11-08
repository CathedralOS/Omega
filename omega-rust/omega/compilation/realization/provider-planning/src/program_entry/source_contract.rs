//! Freeze entry declaration facts before target Calling settlement.
//!
//! Captured contracts contain no Source context. Surface availability,
//! provenance and the exact immutable accepted binding remain deferred until
//! the same settlement points as before; retention grants no execution authority.

use super::SelectedProgramEntry;
use super::program_entry_semantic_binding_role;
use build_time_evaluation::product_entries::{
    ArrivalRequirementContract, arrival_requirement_contract, core_arrival_requirement_contract,
};
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::service_declarations::{
    ProviderSchemaDeclaration, RetainedServiceSurface, ServiceCatalog,
};
use diagnostics::Diagnostic;

pub(super) struct PreparedEntryCallingContract {
    pub schema: symbols::SymbolHandle,
    pub schema_name: &'static str,
    pub semantic_signature: symbols::SymbolHandle,
    pub semantic_identity: String,
    pub physical_signature: symbols::SymbolHandle,
    pub physical_identity: String,
    pub physical_requirement: &'static str,
    pub physical_convention: target::ProgramEntryCallingConvention,
    pub semantic_convention: target::ProgramEntryCallingConvention,
    pub surface: Result<PreparedEntrySurface, Vec<Diagnostic>>,
    pub parameter_type_identities: Vec<String>,
    pub result_type_identity: Option<String>,
    pub accepted_binding: Option<package_compilation::AcceptedSemanticBinding>,
}

pub(super) struct PreparedEntrySurface {
    pub declaration: RetainedServiceSurface,
    pub physical_source: Result<PreparedPhysicalContractSource, Vec<Diagnostic>>,
}

pub(super) fn prepare(
    services: ServiceCatalog<'_>,
    selected: SelectedProgramEntry<'_>,
    accepted_binding: Option<&package_compilation::AcceptedSemanticBinding>,
) -> Result<Option<PreparedEntryCallingContract>, Vec<Diagnostic>> {
    let typed = services.context().typed;
    let (
        Some(schema_name),
        Some(physical_requirement),
        Some(physical_convention),
        Some(semantic_convention),
    ) = (
        selected.slot.boundary_schema,
        selected.slot.physical_arrival_requirement,
        selected.slot.physical_calling_convention,
        selected.slot.semantic_calling_convention,
    )
    else {
        if selected.slot.boundary_schema.is_some()
            || selected.slot.physical_arrival_requirement.is_some()
            || selected.slot.physical_calling_convention.is_some()
            || selected.slot.semantic_calling_convention.is_some()
        {
            return Err(vec![Diagnostic::error(format!(
                "target root slot `{}::{}` has an incomplete two-surface entry declaration",
                selected.slot.owner.root_slot_owner_name(),
                selected.slot.slot_name,
            ))]);
        }
        return Ok(None);
    };
    let schema = selected_entry_schema(typed, selected.slot, schema_name, accepted_binding)
        .map_err(|diagnostic| vec![diagnostic])?;
    let semantic =
        core_arrival_requirement_contract(typed, selected.slot.semantic_arrival_requirement)
            .map_err(|diagnostic| vec![diagnostic])?;
    let physical = arrival_requirement_contract(typed, schema, physical_requirement)
        .map_err(|diagnostic| vec![diagnostic])?;
    if semantic.signature == physical.signature
        || semantic.requirement_identity == physical.requirement_identity
    {
        return Err(vec![Diagnostic::error(format!(
            "target root slot `{}::{}` conflates physical requirement `{physical_requirement}` with semantic requirement `{}`",
            selected.slot.owner.root_slot_owner_name(),
            selected.slot.slot_name,
            selected.slot.semantic_arrival_requirement,
        ))]);
    }
    let surface = (|| {
        let declaration = services
            .retain_declaration(ProviderSchemaDeclaration::BoundaryTrait(schema.symbol))
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "target entry schema `{schema_name}` is not a boundary service schema"
                ))]
            })?;
        let physical_source =
            capture_physical_contract_source(typed, selected.slot, schema.symbol, &physical)
                .map_err(|diagnostic| vec![diagnostic]);
        Ok(PreparedEntrySurface {
            declaration,
            physical_source,
        })
    })();
    Ok(Some(PreparedEntryCallingContract {
        schema: schema.symbol,
        schema_name,
        semantic_signature: semantic.signature,
        semantic_identity: semantic.requirement_identity,
        physical_signature: physical.signature,
        physical_identity: physical.requirement_identity,
        physical_requirement,
        physical_convention,
        semantic_convention,
        surface,
        parameter_type_identities: physical
            .parameters
            .iter()
            .map(|parameter| {
                typed
                    .type_identity_text(parameter.identity)
                    .unwrap_or_default()
            })
            .collect(),
        // None is a missing result. Some("") must still reach physical plan
        // construction and its original empty-identity refusal.
        result_type_identity: physical
            .result_type_identity
            .map(|identity| typed.type_identity_text(identity).unwrap_or_default()),
        accepted_binding: accepted_binding.cloned(),
    }))
}

/// Resolve only inside the slot's contract owner. Source custody identifies a
/// bundled source unit; its leaf name selects a declaration inside that unit,
/// never a candidate in the whole program. Package bindings already retain the
/// complete declaration path and must not fall back to the bundled contract.
fn selected_entry_schema<'program>(
    typed: &'program TypedTrees,
    slot: target::ProgramEntrySlotDeclaration,
    schema_name: &str,
    accepted_binding: Option<&package_compilation::AcceptedSemanticBinding>,
) -> Result<&'program checked_trees::typed_trees::trait_definition::TraitDefinition, Diagnostic> {
    let package = slot.physical_contract_package.ok_or_else(|| {
        Diagnostic::error("target entry slot has no physical contract package identity")
    })?;
    let context = crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    let mut selected_symbols = Vec::new();
    if let Some(binding) = accepted_binding {
        if binding.role() == program_entry_semantic_binding_role(package)
            && binding.selected_provider_plan_digest().is_none()
        {
            selected_symbols.extend(typed.traits().iter().filter_map(|definition| {
                (context.contains(definition.symbol)
                    && typed.symbols.symbol_package_identity(definition.symbol)
                        == Some(binding.package())
                    && typed.symbols.display_path(definition.symbol, "::")
                        == binding.declaration_path())
                .then_some(definition.symbol)
            }));
        }
    } else {
        let expected_digest =
            program_entry_plan::exact_physical_contract_package_source_digest(package);
        for source_file in typed.symbols.source_files().filter(|source_file| {
            source_file.dependency_scope == source::DependencyScope::Product
                && source_file.package_identity.is_none()
                && source_file.path.strip_prefix(&source_file.package_root).ok()
                    == Some(std::path::Path::new(package.package_relative_source()))
                && program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
                    package,
                    source_file.source.as_bytes(),
                ) == expected_digest
        }) {
            selected_symbols.extend(typed.traits().iter().filter_map(|definition| {
                (typed.symbols.symbol_source_span(definition.symbol)
                    .is_some_and(|span| span.source_id == source_file.source_id)
                    && definition.name.as_str() == schema_name)
                    .then_some(definition.symbol)
            }));
        }
    }
    // An unlocked compile has no accepted binding, so a dependency's copy of
    // the contract (std's, usually) is loaded but not selected. Name the
    // review that supplies the binding instead of an empty candidate count.
    let package_copy_loaded = || {
        typed.symbols.source_files().any(|source_file| {
            source_file.dependency_scope == source::DependencyScope::Product
                && source_file.package_identity.is_some()
                && source_file
                    .path
                    .strip_prefix(&source_file.package_root)
                    .ok()
                    == Some(std::path::Path::new(package.package_relative_source()))
        })
    };
    if selected_symbols.is_empty() && accepted_binding.is_none() && package_copy_loaded() {
        return Err(Diagnostic::error(format!(
            "target root slot `{}::{}` takes boundary schema `{schema_name}` from a dependency's `{}`, which needs an accepted program-entry binding; run `omega update` to review and accept it",
            slot.owner.root_slot_owner_name(),
            slot.slot_name,
            package.package_relative_source(),
        )));
    }
    let [symbol] = selected_symbols.as_slice() else {
        return Err(Diagnostic::error(format!(
            "target root slot `{}::{}` requires one exact contract-owned boundary schema `{schema_name}`, but found {}",
            slot.owner.root_slot_owner_name(),
            slot.slot_name,
            selected_symbols.len(),
        )));
    };
    typed
        .traits()
        .iter()
        .find(|definition| definition.symbol == *symbol && definition.is_boundary)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "target entry contract declaration `{schema_name}` is not a boundary schema"
            ))
        })
}

pub(super) struct TargetOwnedPhysicalContractSource {
    pub package: target::ProgramEntryPhysicalContractPackage,
    pub package_source_digest: program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest,
    pub non_authoritative_package_source_report_fingerprint: u64,
}

/// Source-unit facts, not an accepted-binding verdict. The unchanged late
/// binding check consumes these alongside the attached target service schema.
pub(super) struct PreparedPhysicalContractSource {
    package: target::ProgramEntryPhysicalContractPackage,
    package_source_digest: program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest,
    package_source_report_fingerprint: u64,
    source_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    schema_source_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    schema_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    schema_path: String,
    package_relative_source: Option<std::path::PathBuf>,
    source_path: std::path::PathBuf,
}

fn capture_physical_contract_source(
    typed: &TypedTrees,
    slot: target::ProgramEntrySlotDeclaration,
    schema: symbols::SymbolHandle,
    contract: &ArrivalRequirementContract,
) -> Result<PreparedPhysicalContractSource, Diagnostic> {
    let expected_package = slot.physical_contract_package.ok_or_else(|| {
        Diagnostic::error("target physical entry requirement has no owning package identity")
    })?;
    let source_span = typed
        .symbols
        .symbol_source_span(contract.signature)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "target physical entry requirement `{}` has no authored source provenance",
                contract.requirement_identity
            ))
        })?;
    let source_file = typed.symbols.source_file(source_span).ok_or_else(|| {
        Diagnostic::error("target physical entry requirement lost its source-file provenance")
    })?;
    let schema_source_span = typed.symbols.symbol_source_span(schema).ok_or_else(|| {
        Diagnostic::error("target physical entry schema has no authored source provenance")
    })?;
    let schema_source_file = typed
        .symbols
        .source_file(schema_source_span)
        .ok_or_else(|| {
            Diagnostic::error("target physical entry schema lost its source-file provenance")
        })?;
    if schema_source_file.source_id != source_file.source_id {
        return Err(Diagnostic::error(format!(
            "target physical entry requirement `{}` and its selected schema must come from one exact source unit",
            contract.requirement_identity,
        )));
    }
    Ok(PreparedPhysicalContractSource {
        package: expected_package,
        package_source_digest:
            program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
                expected_package,
                source_file.source.as_bytes(),
            ),
        package_source_report_fingerprint: physical_contract_package_source_report_fingerprint(
            expected_package.manifest_identity().as_bytes(),
            source_file.source.as_bytes(),
        ),
        source_package: source_file.package_identity,
        schema_source_package: schema_source_file.package_identity,
        schema_package: typed.symbols.symbol_package_identity(schema),
        schema_path: typed.symbols.display_path(schema, "::"),
        package_relative_source: source_file
            .path
            .strip_prefix(&source_file.package_root)
            .ok()
            .map(std::path::Path::to_owned),
        source_path: source_file.path.clone(),
    })
}

impl PreparedPhysicalContractSource {
    pub(super) fn validate(
        self,
        service_schema: &effects::provider_plan::ServiceSchema,
        requirement_identity: &str,
        accepted_binding: Option<&package_compilation::AcceptedSemanticBinding>,
    ) -> Result<TargetOwnedPhysicalContractSource, Diagnostic> {
        let expected_package = self.package;
        let expected_role = program_entry_semantic_binding_role(expected_package);
        match accepted_binding {
            Some(binding) => {
                let exact_package_binding = binding.role() == expected_role
                    && binding.selected_provider_plan_digest().is_none()
                    && self.source_package == Some(binding.package())
                    && self.schema_source_package == Some(binding.package())
                    && self.schema_package == Some(binding.package())
                    && self.schema_path == binding.declaration_path()
                    && service_schema.declaration.trait_package_identity == Some(binding.package())
                    && package_compilation::accepted_service_schema_digest(
                        binding.role(),
                        service_schema,
                    ) == binding.normalized_schema_digest();
                if !exact_package_binding {
                    return Err(Diagnostic::error(format!(
                        "target physical entry schema `{}` does not match the exact accepted {expected_role:?} package binding",
                        self.schema_path,
                    )));
                }
            }
            None => {
                let exact_bundled_source = self.source_package.is_none()
                    && self.schema_source_package.is_none()
                    && self.package_relative_source.as_deref()
                        == Some(std::path::Path::new(
                            expected_package.package_relative_source(),
                        ))
                    && self.package_source_digest
                        == program_entry_plan::exact_physical_contract_package_source_digest(
                            expected_package,
                        );
                if !exact_bundled_source {
                    return Err(Diagnostic::error(format!(
                        "target physical entry requirement and schema `{}` require either the exact bundled {} contract or one accepted package-owned {} binding, not `{}`",
                        requirement_identity,
                        expected_package.contract_name(),
                        expected_package.contract_name(),
                        self.source_path.display(),
                    )));
                }
            }
        }
        Ok(TargetOwnedPhysicalContractSource {
            package: expected_package,
            package_source_digest: self.package_source_digest,
            non_authoritative_package_source_report_fingerprint: self
                .package_source_report_fingerprint,
        })
    }
}

fn physical_contract_package_source_report_fingerprint(identity: &[u8], source: &[u8]) -> u64 {
    let mut hash = semantic_vocabulary::Fnv1a::new();
    for bytes in [
        b"omega.program-entry-physical-package.v1".as_slice(),
        identity,
        source,
    ] {
        hash.bytes(bytes);
    }
    hash.finish()
}
