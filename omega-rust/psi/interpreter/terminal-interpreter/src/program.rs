//! Admitted programs: the immutable half of interpretation, shared verbatim
//! across instances.
//!
//! `TerminalProgram` retains one canonical module, either owned after external
//! decoding or borrowed from compiler output. Instances share it and search its
//! already-sorted declarations directly; no executable-machine projection or
//! copied lookup tables intervene. Borrowed programs cannot outlive the artifact.
//! External loads retain the whole decoded module, including non-executable
//! catalogs, rather than copying a second runtime representation.
//!
//! Per the embedding contract (wiki/spec/build/embedding.md), a Program is
//! immutable admitted Psi plus descriptions and execution metadata; instances
//! and their bindings live in `instance.rs`. Loading accepts canonical
//! bytes or a sealed artifact view: it runs no source compiler, touches no
//! filesystem, and creates no global runtime or thread — compilation remains
//! the separate upstream authority that produced the bytes.

use crate::errors::{TerminalArtifactInterpretError, TerminalInterpretError, malformed};
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
    pub(crate) catalogs: Cow<'artifact, TerminalModule>,
}

impl AdmittedTerminalProgram<'_> {
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
        ProgramDeclarations::new(&self.catalogs.structural_types)
    }

    /// The record-or-mixed field roster of `identity`, absent when the type
    /// is missing or not field-shaped — the lookup every shape consumer
    /// opens through the declarations table.
    pub(crate) fn record_fields(
        &self,
        identity: &StructuralTypeId,
    ) -> Option<&[terminal_psi::StructuralFieldDeclaration]> {
        self.structural_types().record_fields(identity)
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

    /// The declared boundary a demanded import slot names, when `slot` is a
    /// host-bindable import of this program.
    pub(crate) fn import_declaration(
        &self,
        slot: BoundaryMachineId,
    ) -> Option<&BoundaryMachineDeclaration> {
        if self.has_provider(&slot) {
            return None;
        }
        self.boundary_machines().get(&slot)
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
    pub scalar_parameters: &'program [terminal_psi::ValueDeclaration],
    pub structural_parameters: &'program [terminal_psi::StructuralParameterDeclaration],
    pub placed_view_inputs: &'program [terminal_psi::TerminalPlacedViewInput],
    pub result: &'program terminal_psi::TerminalMachineResult,
}

impl<'artifact> TerminalProgram<'artifact> {
    /// Describe the exact admitted entry instead of reconstructing it from
    /// source or provisioning values by a source-name convention.
    pub fn entry(&self) -> Result<TerminalProgramEntry<'_>, TerminalInterpretError> {
        let machine = self
            .program
            .machines()
            .get(&self.program.catalogs.entry)
            .ok_or_else(malformed)?;
        Ok(TerminalProgramEntry {
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
        installation: Option<&crate::effects::AdmittedProviderInstallation>,
    ) -> Result<crate::capabilities::TerminalExecutionRequirements, TerminalInterpretError> {
        crate::capabilities::check(&self.program, installation)
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
            catalogs: Cow::Owned(module),
        }))
    }

    /// Execute retained compiler output without serialization, decoding or
    /// copying its immutable tables. The artifact must outlive every program
    /// handle, instance and suspended execution derived from this borrow.
    pub fn borrow(artifact: terminal_codec::TerminalArtifactView<'artifact>) -> Self {
        Self::from_admitted(AdmittedTerminalProgram {
            identity: artifact.identity(),
            catalogs: Cow::Borrowed(artifact.module()),
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
            catalogs: Cow::Owned(module),
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
    ) -> Result<crate::effects::AdmittedProviderInstallation, TerminalInterpretError> {
        use crate::effects::{AdmittedBoundarySupply, AdmittedProviderInstallation};

        if request
            .installation
            .is_some_and(|installation| installation.terminal_psi != self.program.identity)
        {
            return Err(TerminalInterpretError::ProviderInstallationIdentityMismatch);
        }
        let mut installation =
            request
                .installation
                .cloned()
                .unwrap_or_else(|| AdmittedProviderInstallation {
                    terminal_psi: self.program.identity,
                    supplies: BTreeMap::new(),
                    assumed_bindings: Vec::new(),
                });
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
            .filter(move |declaration| !self.program.has_provider(&declaration.id))
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
                matches!(
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
        installation: &crate::effects::AdmittedProviderInstallation,
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
        installation: Option<&crate::effects::AdmittedProviderInstallation>,
    ) -> Result<crate::execution::TerminalExecution<'artifact>, TerminalArtifactInterpretError>
    {
        self.prepare_execution(scalar_arguments, structural_inputs, installation)
            .map(|(execution, _)| execution)
    }

    fn prepare_execution(
        &self,
        scalar_arguments: &[TerminalScalarValue],
        structural_inputs: TerminalStructuralInputs<'_>,
        installation: Option<&crate::effects::AdmittedProviderInstallation>,
    ) -> Result<
        (
            crate::execution::TerminalExecution<'artifact>,
            crate::capabilities::TerminalExecutionRequirements,
        ),
        TerminalArtifactInterpretError,
    > {
        let requirements = self
            .execution_requirements(installation)
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
            std::sync::Arc::clone(&self.program),
            scalar_arguments,
            structural_inputs.arguments,
            scalar_fields,
            structural_inputs.primitive_values,
            structural_inputs.placed_view_establishments,
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
        installation: Option<&crate::effects::AdmittedProviderInstallation>,
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
