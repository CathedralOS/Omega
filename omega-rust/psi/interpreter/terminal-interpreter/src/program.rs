//! Admitted programs: the immutable half of interpretation, shared verbatim
//! across instances.
//!
//! `TerminalProgram` retains one canonical module, either owned after external
//! decoding or borrowed from compiler output. Instances share it and search its
//! already-sorted declarations directly; no executable-machine projection or
//! copied lookup tables intervene. Borrowed programs cannot outlive the artifact.
//! External loads retain the whole decoded module, including non-executable
//! catalogs, rather than copying a second runtime representation. Each execution
//! may attach an admitted target-layout overlay; its small record replacement
//! table borrows the same canonical catalog owner and preserves artifact identity.
//! The portable root likewise remains canonical: every installed invocation
//! resolves its exact retained executable through artifact-bound target selections,
//! and descriptions, capability preflight, and entry custody use that same choice.
//!
//! Per the embedding contract (wiki/spec/build/embedding.md), a Program is
//! immutable admitted Psi plus descriptions and execution metadata; instances
//! and their bindings live in `instance.rs`. Loading accepts canonical
//! bytes or a sealed artifact view: it runs no source compiler, touches no
//! filesystem, and creates no global runtime or thread — compilation remains
//! the separate upstream authority that produced the bytes.

use crate::errors::{TerminalArtifactInterpretError, TerminalInterpretError};
mod declarations;
pub(crate) use declarations::ProgramDeclarations;

use crate::instance::TerminalInstance;
use crate::structural_inputs::TerminalStructuralInputs;
use crate::values::TerminalScalarValue;
use semantic_vocabulary::{BoundaryMachineId, MachineId, StructuralTypeId};
use std::borrow::Cow;
use std::collections::BTreeMap;
use terminal_psi::{
    BoundaryMachineDeclaration, StructuralTypeDeclaration, TerminalModule, TerminalPsiIdentity,
};

/// Immutable program storage shared by all instances. External ingress owns
/// its decoded module; in-process execution borrows the compiler's retained
/// module. Neither route reconstructs executable machines or declaration maps.
pub(crate) struct AdmittedTerminalProgram<'artifact> {
    /// Canonical Terminal-Psi identity of the module, computed once
    /// at admission; provider installations bind to this exact identity.
    pub(crate) identity: TerminalPsiIdentity,
    pub(crate) catalogs: std::sync::Arc<Cow<'artifact, TerminalModule>>,
    /// Selected geometry belongs to an execution view, never canonical storage.
    pub(crate) target_layouts: Option<std::sync::Arc<terminal_installation::AdmittedTargetLayouts>>,
}

impl AdmittedTerminalProgram<'_> {
    /// Resolve this invocation's executable root without changing canonical
    /// storage. Catalog selection identifies a retained body; it grants no
    /// input backing, provider authority, or result issuance.
    pub(crate) fn resolve_entry(
        &self,
        installation: Option<&crate::effects::TerminalRuntimeSupplies>,
    ) -> Result<MachineId, TerminalInterpretError> {
        if installation.is_some_and(|installation| installation.terminal_psi != self.identity) {
            return Err(TerminalInterpretError::ProviderInstallationIdentityMismatch);
        }
        let root = installation
            .and_then(|supplies| supplies.entry)
            .unwrap_or(self.catalogs.entry);
        let selected = installation.and_then(|installation| installation.target_machines.as_ref());
        let machine = match selected {
            Some(selected) => selected
                .entry(self.identity, &self.catalogs, root)
                .map_err(TerminalInterpretError::TargetMachineSelection)?,
            None => match root {
                terminal_psi::TargetMachineCallable::Machine(machine) => {
                    if self
                        .catalogs
                        .target_machine_families
                        .iter()
                        .any(|family| family.members.iter().any(|member| member.callable == root))
                    {
                        return Err(TerminalInterpretError::TargetMachineSelectionMissing(root));
                    }
                    machine
                }
                terminal_psi::TargetMachineCallable::Boundary(_) => {
                    return Err(TerminalInterpretError::TargetMachineSelectionMissing(root));
                }
            },
        };
        self.machines()
            .get(&machine)
            .ok_or(TerminalInterpretError::VerifiedEntryMachineMissing)?;
        Ok(machine)
    }

    fn validate_layout_selection(
        &self,
        installation: Option<&crate::effects::TerminalRuntimeSupplies>,
    ) -> Result<(), TerminalInterpretError> {
        let layouts = installation.and_then(|supplies| supplies.target_layouts.as_ref());
        if let Some(layouts) = layouts {
            let machines = installation.and_then(|supplies| supplies.target_machines.as_ref());
            if layouts.terminal_psi() != self.identity
                || machines.is_none_or(|machines| {
                    machines.terminal_psi() != self.identity
                        || machines.target() != layouts.target()
                })
            {
                return Err(TerminalInterpretError::TargetLayoutSelectionIdentityMismatch);
            }
        } else if let Some(application) = self.catalogs.target_layout_applications.first() {
            return Err(TerminalInterpretError::TargetLayoutSelectionMissing(
                application.id,
            ));
        }
        Ok(())
    }

    pub(crate) fn machines(&self) -> ProgramDeclarations<'_, terminal_psi::TerminalMachine> {
        ProgramDeclarations::new(&self.catalogs.machines)
    }

    /// The machine keyed `machine` — admission proves the catalog entry
    /// exists, so a miss reports a defect through the verified-target arm.
    pub(crate) fn verified_machine(
        &self,
        machine: MachineId,
    ) -> Result<&terminal_psi::TerminalMachine, TerminalInterpretError> {
        self.machines()
            .get(&machine)
            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(machine))
    }

    /// The boundary machine keyed `boundary` — admission proves the catalog
    /// entry exists, so a miss reports a defect through the verified arm.
    pub(crate) fn verified_boundary_machine(
        &self,
        boundary: &BoundaryMachineId,
    ) -> Result<&BoundaryMachineDeclaration, TerminalInterpretError> {
        self.boundary_machines().get(boundary).ok_or(
            TerminalInterpretError::VerifiedBoundaryMachineMissing(*boundary),
        )
    }

    pub(crate) fn structural_types(&self) -> ProgramDeclarations<'_, StructuralTypeDeclaration> {
        ProgramDeclarations::with_replacements(
            &self.catalogs.structural_types,
            self.target_layouts
                .as_ref()
                .map_or(&[], |layouts| layouts.resolved_records()),
        )
    }

    pub(crate) fn boundary_machines(&self) -> ProgramDeclarations<'_, BoundaryMachineDeclaration> {
        ProgramDeclarations::new(&self.catalogs.boundary_machines)
    }

    pub(crate) fn has_provider(&self, boundary: &BoundaryMachineId) -> bool {
        self.catalogs
            .provider_candidates
            .binary_search_by_key(boundary, |candidate| candidate.boundary)
            .is_ok()
    }

    /// Independent heads are contract-only declarations. Legacy rows whose
    /// canonical callable is an actual member retain ordinary call identity.
    pub(crate) fn is_target_family_head(&self, boundary: BoundaryMachineId) -> bool {
        let head = terminal_psi::TargetMachineCallable::Boundary(boundary);
        self.catalogs
            .target_machine_families
            .binary_search_by_key(&head, |family| family.canonical)
            .ok()
            .is_some_and(|position| {
                !self.catalogs.target_machine_families[position]
                    .members
                    .iter()
                    .any(|member| member.callable == head)
            })
    }

    /// The declared boundary a demanded import slot names, when `slot` is a
    /// host-bindable import of this program.
    pub(crate) fn import_declaration(
        &self,
        slot: BoundaryMachineId,
    ) -> Option<&BoundaryMachineDeclaration> {
        if self.has_provider(&slot) || self.is_target_family_head(slot) {
            return None;
        }
        self.boundary_machines().get(&slot).filter(|declaration| {
            declaration.role == terminal_psi::BoundaryDeclarationRole::Callable
        })
    }
}

/// One canonical Terminal-Psi program: immutable and
/// shareable across every independent instance it instantiates.
///
/// `load` decodes external canonical bytes; `borrow` uses retained compiler
/// output without decoding. Neither performs proof verification or grants host
/// authority. Cloning shares storage; instances keep their mutable state separate.
/// `imports` describes the default catalog; `execution_requirements` selects
/// demanded slots for each admitted supply selection. Each instance describes
/// its selected imports and establishes adapters through `TerminalInstance::bind_import`.
#[derive(Clone)]
pub struct TerminalProgram<'artifact> {
    pub(crate) program: std::sync::Arc<AdmittedTerminalProgram<'artifact>>,
}

impl std::fmt::Debug for TerminalProgram<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TerminalProgram")
            .field("identity", &self.program.identity)
            .finish()
    }
}

/// The exact admitted entry input/result contract. It grants no host backing,
/// entry qualification, or effect authority; drivers must supply those inputs.
pub struct TerminalProgramEntry<'program> {
    /// The installed retained body, not the artifact's portable callable root.
    pub machine: MachineId,
    pub scalar_parameters: &'program [terminal_psi::ValueDeclaration],
    pub structural_parameters: &'program [terminal_psi::StructuralParameterDeclaration],
    placed_view_inputs: &'program [terminal_psi::TerminalPlacedViewInput],
    pub result: &'program terminal_psi::TerminalMachineResult,
}

impl TerminalProgramEntry<'_> {
    /// Borrow only this exact executable entry's placed-view custody demands.
    /// Other retained target bodies are not this invocation's input contract.
    pub fn placed_view_inputs(
        &self,
    ) -> impl Iterator<Item = &terminal_psi::TerminalPlacedViewInput> {
        self.placed_view_inputs
            .iter()
            .filter(move |row| row.machine == self.machine)
    }
}

impl<'artifact> TerminalProgram<'artifact> {
    /// The retained canonical module every public view derives from.
    pub fn module(&self) -> &TerminalModule {
        &self.program.catalogs
    }

    /// Describe an ordinary concrete root without an installation. A portable
    /// family root remains uninstalled and cannot masquerade as one member.
    pub fn entry(&self) -> Result<TerminalProgramEntry<'_>, TerminalInterpretError> {
        self.describe_entry(self.program.resolve_entry(None)?)
    }

    /// Describe the executable selected under this exact artifact's installation.
    /// This is the same root resolution used by capability preflight and startup;
    /// observing the contract supplies neither input custody nor host authority.
    pub fn entry_installed(
        &self,
        installation: &crate::effects::TerminalRuntimeSupplies,
    ) -> Result<TerminalProgramEntry<'_>, TerminalInterpretError> {
        self.program.validate_layout_selection(Some(installation))?;
        self.describe_entry(self.program.resolve_entry(Some(installation))?)
    }

    fn describe_entry(
        &self,
        entry: MachineId,
    ) -> Result<TerminalProgramEntry<'_>, TerminalInterpretError> {
        let machine = self
            .program
            .machines()
            .get(&entry)
            .ok_or(TerminalInterpretError::VerifiedEntryMachineMissing)?;
        Ok(TerminalProgramEntry {
            machine: entry,
            scalar_parameters: &machine.parameters,
            structural_parameters: &machine.structural_parameters,
            placed_view_inputs: &self.program.catalogs.placed_view_inputs,
            result: &machine.result,
        })
    }

    /// Exact admitted payload description for typed host adapters. Describing
    /// a type establishes neither runtime contents nor result custody.
    pub fn structural_type(
        &self,
        identity: StructuralTypeId,
    ) -> Option<&StructuralTypeDeclaration> {
        self.program.structural_types().get(&identity)
    }

    /// The precise import/provider roster of this entry's possible execution,
    /// after capability decisions on its admitted immutable projection. Runs
    /// no guest code; every execution start enforces this same decision before
    /// entry inputs bind or effects begin.
    pub fn execution_requirements(
        &self,
        installation: Option<&crate::effects::TerminalRuntimeSupplies>,
    ) -> Result<crate::capabilities::TerminalExecutionRequirements, TerminalInterpretError> {
        self.program.validate_layout_selection(installation)?;
        crate::capabilities::check(&self.program, installation)
    }
    pub fn execution_requirements_with_private_entry_ram(
        &self,
        installation: Option<&crate::effects::TerminalRuntimeSupplies>,
        recipes: &[terminal_installation::CheckedPrivateEntryRamRecipe],
    ) -> Result<crate::capabilities::TerminalExecutionRequirements, TerminalInterpretError> {
        self.program.validate_layout_selection(installation)?;
        crate::capabilities::check_with_private_entry_ram(
            &self.execution_program(installation),
            installation,
            recipes,
        )
    }

    /// Decode canonical semantic bytes and retain their module and identity.
    ///
    /// Loading consumes bytes only — no source compiler, filesystem, package
    /// authority, global runtime or thread. Producing those bytes from source
    /// is a separate compilation authority that stays upstream of this API.
    pub fn load(semantic_bytes: &[u8]) -> Result<Self, TerminalArtifactInterpretError> {
        let module = terminal_codec::decode_module(semantic_bytes)
            .map_err(TerminalArtifactInterpretError::SemanticDecode)?;
        let identity = terminal_codec::terminal_psi_identity(&module)
            .map_err(TerminalArtifactInterpretError::SemanticDecode)?;
        Ok(Self::from_admitted(AdmittedTerminalProgram {
            identity,
            catalogs: std::sync::Arc::new(Cow::Owned(module)),
            target_layouts: None,
        }))
    }

    /// Execute retained compiler output without serialization, decoding or
    /// copying its immutable tables. The artifact must outlive every program
    /// handle, instance and suspended execution derived from this borrow.
    pub fn borrow(artifact: terminal_codec::TerminalArtifactView<'artifact>) -> Self {
        Self::from_admitted(AdmittedTerminalProgram {
            identity: artifact.identity(),
            catalogs: std::sync::Arc::new(Cow::Borrowed(artifact.module())),
            target_layouts: None,
        })
    }

    /// Decode one complete portable Terminal-Psi envelope and admit it as a
    /// reusable program. The envelope carries no checked-tree or build object.
    pub fn load_envelope(artifact_bytes: &[u8]) -> Result<Self, TerminalArtifactInterpretError> {
        let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(artifact_bytes)
            .map_err(TerminalArtifactInterpretError::ArtifactDecode)?;
        let (module, identity) = artifact.into_semantics();
        Ok(Self::from_admitted(AdmittedTerminalProgram {
            identity,
            catalogs: std::sync::Arc::new(Cow::Owned(module)),
            target_layouts: None,
        }))
    }

    /// The exact Terminal-Psi identity this program was admitted under.
    pub fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.program.identity
    }

    /// Admit receiving-host supply choices against this immutable program.
    /// This reuses admitted content and checked installation custody, without
    /// decoding bytes or checking source again. Duplicate or competing choices,
    /// unknown slots and external-completion imports reject atomically.
    /// A selected import still needs its exact adapter and actual authority
    /// established on each fresh instance before any guest effect.
    pub fn admit_runtime_supplies(
        &self,
        request: crate::effects::TerminalRuntimeSupplyRequest<'_>,
    ) -> Result<crate::effects::TerminalRuntimeSupplies, TerminalInterpretError> {
        use crate::effects::{AdmittedBoundarySupply, TerminalRuntimeSupplies};

        if request
            .installation
            .is_some_and(|installation| installation.terminal_psi() != self.program.identity)
        {
            return Err(TerminalInterpretError::ProviderInstallationIdentityMismatch);
        }
        if request
            .target_machines
            .is_some_and(|selected| selected.terminal_psi() != self.program.identity)
        {
            return Err(TerminalInterpretError::TargetMachineSelectionIdentityMismatch);
        }
        let mut installation = request
            .installation
            .map(TerminalRuntimeSupplies::from)
            .unwrap_or_else(|| TerminalRuntimeSupplies {
                terminal_psi: self.program.identity,
                entry: None,
                supplies: BTreeMap::new(),
                assumed_bindings: Vec::new(),
                target_machines: None,
                target_layouts: None,
            });
        installation.target_machines = request.target_machines.cloned();
        installation.entry = request.entry;
        installation.target_layouts = request.target_layouts.cloned().map(std::sync::Arc::new);
        self.program
            .validate_layout_selection(Some(&installation))?;
        self.program.resolve_entry(Some(&installation))?;
        for &boundary in request.host_imports {
            let rejected = |reason| TerminalInterpretError::RuntimeSupplySelectionRejected {
                boundary,
                reason,
            };
            let declaration = self
                .program
                .boundary_machines()
                .get(&boundary)
                .ok_or_else(|| rejected("host import names no declared boundary"))?;
            if declaration.role != terminal_psi::BoundaryDeclarationRole::Callable {
                return Err(rejected(
                    "host import names contract evidence rather than a callable",
                ));
            }
            if self.program.is_target_family_head(boundary) {
                let member = crate::calls::target_families::selected_member(
                    &self.program,
                    installation.target_machines.as_ref(),
                    boundary,
                )?;
                if !matches!(member, terminal_psi::TargetMachineCallable::Boundary(_)) {
                    return Err(rejected(
                        "a checked target-family member cannot be replaced by a host import",
                    ));
                }
            }
            if declaration.completion != terminal_psi::BoundaryCompletion::NormalReturn {
                return Err(rejected(
                    "external completion requires separate domain admission",
                ));
            }
            if installation.supplies.contains_key(&boundary)
                || installation
                    .assumed_bindings
                    .iter()
                    .any(|binding| binding.required == boundary)
            {
                return Err(rejected(
                    "host import duplicates or competes with a selected supply",
                ));
            }
            installation
                .supplies
                .insert(boundary, AdmittedBoundarySupply::HostImport);
        }
        Ok(installation)
    }

    /// Default host-import descriptions without a receiving-host supply choice:
    /// candidate-bearing slots require installation by default. Explicit host
    /// choices are visible through `TerminalInstance::import` after admission,
    /// never by mutating this canonical catalog. The declaration returned is
    /// the exact contract — signature,
    /// requirements, crash routes and guarantees — not a display summary.
    pub fn imports(&self) -> impl Iterator<Item = &BoundaryMachineDeclaration> {
        self.program
            .boundary_machines()
            .values()
            .filter(move |declaration| {
                declaration.role == terminal_psi::BoundaryDeclarationRole::Callable
                    && !self.program.has_provider(&declaration.id)
                    && !self.program.is_target_family_head(declaration.id)
            })
    }

    /// The exact import description for one demanded slot, absent when `slot`
    /// is not a host-bindable import of this program.
    pub fn import(&self, slot: BoundaryMachineId) -> Option<&BoundaryMachineDeclaration> {
        self.program.import_declaration(slot)
    }

    /// Exact external requirement contract, independent of ordinary provider
    /// candidates. This is only a description: no containing-domain authority
    /// or crossing-safety premise follows from finding the slot.
    pub fn external_completion_requirement(
        &self,
        slot: BoundaryMachineId,
    ) -> Option<&BoundaryMachineDeclaration> {
        self.program
            .boundary_machines()
            .get(&slot)
            .filter(|declaration| {
                declaration.role == terminal_psi::BoundaryDeclarationRole::Callable
                    && matches!(
                        declaration.completion,
                        terminal_psi::BoundaryCompletion::External(_)
                    )
            })
    }

    /// Start one instance of this program with its scalar arguments and every
    /// structural entry input in one record. No demanded import is invoked
    /// until `invoke`; establish each through
    /// [`TerminalInstance::bind_import`] first.
    ///
    /// The instance shares this program's admitted content verbatim — the
    /// second and later instances of one program decode nothing.
    pub fn instantiate<'host>(
        &self,
        scalar_arguments: &[TerminalScalarValue],
        structural_inputs: TerminalStructuralInputs<'_>,
    ) -> Result<TerminalInstance<'host, 'artifact>, TerminalArtifactInterpretError> {
        self.instantiate_with(scalar_arguments, structural_inputs, None)
    }

    /// Start one instance under a supply selection previously admitted
    /// against this exact program identity.
    pub fn instantiate_installed<'host>(
        &self,
        scalar_arguments: &[TerminalScalarValue],
        structural_inputs: TerminalStructuralInputs<'_>,
        installation: &crate::effects::TerminalRuntimeSupplies,
    ) -> Result<TerminalInstance<'host, 'artifact>, TerminalArtifactInterpretError> {
        self.instantiate_with(scalar_arguments, structural_inputs, Some(installation))
    }

    /// Share immutable storage between program handles and their instances.
    pub(crate) fn from_admitted(program: AdmittedTerminalProgram<'artifact>) -> Self {
        Self {
            program: std::sync::Arc::new(program),
        }
    }

    /// Bind the complete host input record to one fresh execution on this
    /// admitted program — the shared tail of `instantiate` and the bare
    /// `TerminalExecution` artifact starts, so every entry accepts the same
    /// `TerminalStructuralInputs` lanes.
    ///
    /// Boolean field values convert into the scalar-field lane before
    /// binding; a conversion failure is reported back under the boolean
    /// argument shape the host actually supplied. Cases and fixed byte-array
    /// backings bind after construction and before any operation runs.
    pub(crate) fn start_execution(
        &self,
        scalar_arguments: &[TerminalScalarValue],
        structural_inputs: TerminalStructuralInputs<'_>,
        installation: Option<&crate::effects::TerminalRuntimeSupplies>,
    ) -> Result<crate::execution::TerminalExecution<'artifact>, TerminalArtifactInterpretError>
    {
        self.prepare_execution(scalar_arguments, structural_inputs, installation)
            .map(|(execution, _)| execution)
    }

    fn execution_program(
        &self,
        installation: Option<&crate::effects::TerminalRuntimeSupplies>,
    ) -> std::sync::Arc<AdmittedTerminalProgram<'artifact>> {
        let Some(layouts) = installation.and_then(|supplies| supplies.target_layouts.as_ref())
        else {
            return std::sync::Arc::clone(&self.program);
        };
        std::sync::Arc::new(AdmittedTerminalProgram {
            identity: self.program.identity,
            catalogs: std::sync::Arc::clone(&self.program.catalogs),
            target_layouts: Some(std::sync::Arc::clone(layouts)),
        })
    }

    fn prepare_execution(
        &self,
        scalar_arguments: &[TerminalScalarValue],
        structural_inputs: TerminalStructuralInputs<'_>,
        installation: Option<&crate::effects::TerminalRuntimeSupplies>,
    ) -> Result<
        (
            crate::execution::TerminalExecution<'artifact>,
            crate::capabilities::TerminalExecutionRequirements,
        ),
        TerminalArtifactInterpretError,
    > {
        let requirements = self
            .execution_requirements_with_private_entry_ram(
                installation,
                structural_inputs.private_entry_ram,
            )
            .map_err(TerminalArtifactInterpretError::Execution)?;
        if let Some(boundary) = requirements.missing_providers.first() {
            return Err(TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::ProviderInstallationMissing(*boundary),
            ));
        }
        crate::structural_inputs::zero_initialized::validate(structural_inputs)
            .map_err(TerminalArtifactInterpretError::Execution)?;
        let converted_boolean_fields;
        let scalar_fields = if structural_inputs.boolean_fields.is_empty() {
            structural_inputs.scalar_fields
        } else {
            converted_boolean_fields = structural_inputs
                .boolean_fields
                .iter()
                .map(crate::structural_inputs::scalar_fields::TerminalStructuralScalarFieldValue::from)
                .collect::<Vec<_>>();
            &converted_boolean_fields
        };
        let mut execution = crate::execution::TerminalExecution::start_with_program(
            self.execution_program(installation),
            scalar_arguments,
            structural_inputs.arguments,
            scalar_fields,
            structural_inputs.primitive_values,
            structural_inputs.placed_view_establishments,
            structural_inputs.private_entry_ram,
            structural_inputs.zero_initialized_arguments,
            structural_inputs.established_erased_fields,
            installation,
        )
        .map_err(TerminalArtifactInterpretError::Execution)
        .map_err(|error| match error {
            TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::StructuralScalarFieldArgumentInvalid {
                    argument_index,
                    field,
                },
            ) if !structural_inputs.boolean_fields.is_empty() => {
                TerminalArtifactInterpretError::Execution(
                    TerminalInterpretError::StructuralBooleanFieldArgumentInvalid {
                        argument_index,
                        field,
                    },
                )
            }
            other => other,
        })?;
        execution
            .bind_structural_cases(structural_inputs.cases)
            .map_err(TerminalArtifactInterpretError::Execution)?;
        if !structural_inputs.byte_arrays.is_empty() {
            execution
                .bind_byte_arrays(structural_inputs.byte_arrays)
                .map_err(TerminalArtifactInterpretError::Execution)?;
        }
        execution.external_demands = requirements.external_completions.clone();
        Ok((execution, requirements))
    }

    fn instantiate_with<'host>(
        &self,
        scalar_arguments: &[TerminalScalarValue],
        structural_inputs: TerminalStructuralInputs<'_>,
        installation: Option<&crate::effects::TerminalRuntimeSupplies>,
    ) -> Result<TerminalInstance<'host, 'artifact>, TerminalArtifactInterpretError> {
        let (execution, requirements) =
            self.prepare_execution(scalar_arguments, structural_inputs, installation)?;
        Ok(TerminalInstance::new(
            self.clone(),
            execution,
            requirements.imports,
        ))
    }
}

#[cfg(test)]
mod private_entry_target_tests {
    use super::*;

    #[test]
    #[ignore = "explicit qualification consumes OMEGA_ENTRY_RAM_CANONICAL_ARTIFACT"]
    fn private_entry_recipe_requires_exact_installed_geometry() {
        let path = std::env::var_os("OMEGA_ENTRY_RAM_CANONICAL_ARTIFACT")
            .expect("qualification must supply actual public CLI canonical output");
        let bytes = std::fs::read(path).expect("read actual canonical bytes");
        let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes)
            .expect("independent canonical admission");
        let program = TerminalProgram::borrow(artifact.view());
        let terminal_psi::TargetMachineCallable::Machine(entry) = artifact.semantic_module().entry
        else {
            panic!("ordinary direct entry required")
        };
        let row = artifact
            .semantic_module()
            .placed_view_inputs
            .iter()
            .find(|row| row.machine == entry)
            .expect("actual direct placed row")
            .clone();
        let machines =
            terminal_installation::admit_target_machine_selections(artifact.view(), "linux_x86_64");
        let layouts =
            terminal_installation::admit_target_layouts(artifact.view(), &machines).unwrap();
        let recipe = terminal_installation::admit_private_entry_ram(
            artifact.view(),
            &layouts,
            entry,
            &terminal_installation::PrivateEntryRamRecipe {
                input: row,
                contents: vec![41, 0, 0, 0, 3, 0, 0, 0],
            },
        )
        .expect("actual checked Linux typed initializer");
        let recipes = std::slice::from_ref(&recipe);
        assert!(
            program
                .execution_requirements_with_private_entry_ram(None, recipes)
                .is_err()
        );
        for name in ["macos_arm64"] {
            let other_machines =
                terminal_installation::admit_target_machine_selections(artifact.view(), name);
            let other_layouts =
                terminal_installation::admit_target_layouts(artifact.view(), &other_machines)
                    .unwrap();
            let installation = program
                .admit_runtime_supplies(crate::effects::TerminalRuntimeSupplyRequest {
                    entry: Some(artifact.semantic_module().entry),
                    installation: None,
                    host_imports: &[],
                    target_machines: Some(&other_machines),
                    target_layouts: Some(&other_layouts),
                })
                .expect("genuine admitted other-target installation");
            assert!(matches!(
                program.execution_requirements_with_private_entry_ram(Some(&installation), recipes),
                Err(crate::errors::TerminalInterpretError::TargetLayoutSelectionIdentityMismatch)
            ));
        }
    }
}
