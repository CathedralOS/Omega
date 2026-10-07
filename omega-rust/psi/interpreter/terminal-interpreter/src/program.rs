//! Admitted programs: the immutable half of interpretation, shared verbatim
//! across instances.
//!
//! `AdmittedTerminalProgram` is what `TerminalExecution::start_artifact` used
//! to build privately per execution: the verified module's executable tables
//! plus the catalog authority instance startup still consults. `TerminalProgram`
//! is the public handle: decode, verify and admit once, then instantiate any
//! number of independent executions without re-reading bytes or rechecking
//! proof. Admission, not construction, is the trust boundary — the struct's
//! fields are crate-private so a caller cannot mint program content that did
//! not pass verification.
//!
//! Per the embedding contract (wiki/spec/build/embedding.md), a Program is
//! immutable admitted Psi plus descriptions and execution metadata; instances
//! and their bindings live in `instance.rs`. Loading accepts canonical
//! semantic/proof bytes only: it runs no source compiler, touches no
//! filesystem, and creates no global runtime or thread — compilation remains
//! the separate upstream authority that produced the bytes.

use crate::errors::{TerminalArtifactInterpretError, TerminalInterpretError, malformed};
use crate::execution::ExecutableMachine;
use crate::instance::TerminalInstance;
use crate::structural_inputs::TerminalStructuralInputs;
use crate::values::TerminalScalarValue;
use semantic_vocabulary::{BoundaryMachineId, MachineId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    BoundaryMachineDeclaration, StructuralTypeDeclaration, TerminalModule, TerminalPsiIdentity,
};

/// The immutable content of one verified Terminal-Psi module, shared verbatim
/// by every instance of the same admitted program.
///
/// Admission splits the module into the execution-ready projections the
/// interpreter loop reads — `machines` and the indexed declaration maps —
/// plus `catalogs`, the subset of roster rows a
/// fresh instance still consults: the entry selector, the direct-entry
/// placed-view input rows, and the structural type and domain rosters that
/// placed-view referent validation runs against. Verification-only payloads
/// (proof vocabularies, conformance applications) are dropped at admission rather than retained per program.
#[derive(Clone)]
pub(crate) struct AdmittedTerminalProgram {
    /// Canonical Terminal-Psi identity of the verified module, computed once
    /// at admission; provider installations bind to this exact identity.
    pub(crate) identity: TerminalPsiIdentity,
    /// Slim catalog: entry, placed-view inputs and structural types/domains.
    /// These descriptions grant no host
    /// provisions; drivers name required ingress without rebuilding source.
    /// Other rosters are empty after admission consumes their authority.
    pub(crate) catalogs: TerminalModule,
    pub(crate) machines: BTreeMap<MachineId, ExecutableMachine>,
    pub(crate) structural_types: BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    pub(crate) boundary_machines: BTreeMap<BoundaryMachineId, BoundaryMachineDeclaration>,
    /// Availability, not deployment choice. Without explicit receiving-host
    /// selection, a demanded candidate-bearing slot requires installation.
    /// Selecting a host import does not remove its retained checked alternatives.
    pub(crate) provider_candidates: BTreeSet<BoundaryMachineId>,
}

impl AdmittedTerminalProgram {
    /// Move one verified module into shared immutable form. The module arrives
    /// already verified by the caller — admission converts representation,
    /// never re-checks semantics.
    pub(crate) fn admit(mut module: TerminalModule) -> Result<Self, TerminalInterpretError> {
        let terminal_psi =
            terminal_codec::terminal_psi_identity(&module).map_err(|_| malformed())?;
        let machines = std::mem::take(&mut module.machines)
            .into_iter()
            .map(|machine| {
                (
                    machine.id,
                    ExecutableMachine {
                        attachment: machine.attachment,
                        nominal_cleanup_subject: machine.nominal_cleanup_subject,
                        parameters: machine.parameters,
                        structural_parameters: machine.structural_parameters,
                        structural_places: machine.structural_places,
                        entry_claims: machine.entry_claims,
                        content_entry_claims: machine.content_entry_claims,
                        result: machine.result,
                        entry: machine.entry,
                        blocks: machine
                            .blocks
                            .into_iter()
                            .map(|block| (block.id, block))
                            .collect(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let boundary_machines = module
            .boundary_machines
            .iter()
            .cloned()
            .map(|boundary| (boundary.id, boundary))
            .collect::<BTreeMap<_, _>>();
        let structural_types = module
            .structural_types
            .iter()
            .cloned()
            .map(|declaration| (declaration.id, declaration))
            .collect();
        let provider_candidates = module
            .provider_candidates
            .iter()
            .map(|candidate| candidate.boundary)
            .collect();
        let mut catalogs = TerminalModule::for_entry(module.entry);
        catalogs.placed_view_inputs = std::mem::take(&mut module.placed_view_inputs);
        catalogs.structural_types = std::mem::take(&mut module.structural_types);
        catalogs.structural_domains = std::mem::take(&mut module.structural_domains);
        // Selectors name tables of this catalog; dynamic calls read it.
        catalogs.dynamic_dispatch = std::mem::take(&mut module.dynamic_dispatch);
        Ok(Self {
            identity: terminal_psi,
            catalogs,
            machines,
            structural_types,
            boundary_machines,
            provider_candidates,
        })
    }

    /// The declared boundary a demanded import slot names, when `slot` is a
    /// host-bindable import of this program.
    pub(crate) fn import_declaration(
        &self,
        slot: BoundaryMachineId,
    ) -> Option<&BoundaryMachineDeclaration> {
        if self.provider_candidates.contains(&slot) {
            return None;
        }
        self.boundary_machines.get(&slot)
    }
}

/// One admitted Terminal-Psi program: decoded, verified, immutable and
/// shareable across every independent instance it instantiates.
///
/// `load` is the whole authority here — admission replays the canonical
/// decode and the interpreter's verification once, then keeps the result.
/// Cloning shares the same admitted program; it does not re-verify.
/// `imports` describes the default catalog; `execution_requirements` selects
/// demanded slots for each admitted supply selection. Each instance describes
/// its selected imports and establishes adapters through `TerminalInstance::bind_import`.
#[derive(Clone)]
pub struct TerminalProgram {
    pub(crate) program: std::sync::Arc<AdmittedTerminalProgram>,
}

impl std::fmt::Debug for TerminalProgram {
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

impl TerminalProgram {
    /// Describe the exact admitted entry instead of reconstructing it from
    /// source or provisioning values by a source-name convention.
    pub fn entry(&self) -> TerminalProgramEntry<'_> {
        let machine = &self.program.machines[&self.program.catalogs.entry];
        TerminalProgramEntry {
            scalar_parameters: &machine.parameters,
            structural_parameters: &machine.structural_parameters,
            placed_view_inputs: &self.program.catalogs.placed_view_inputs,
            result: &machine.result,
        }
    }

    /// Exact admitted payload description for typed host adapters. Describing
    /// a type establishes neither runtime contents nor result custody.
    pub fn structural_type(
        &self,
        identity: StructuralTypeId,
    ) -> Option<&StructuralTypeDeclaration> {
        self.program.structural_types.get(&identity)
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
    /// Decode canonical semantic and proof sections, verify them under the
    /// interpreter's admission profile, and retain the admitted program.
    ///
    /// Loading consumes bytes only — no source compiler, filesystem, package
    /// authority, global runtime or thread. Producing those bytes from source
    /// is a separate compilation authority that stays upstream of this API.
    pub fn load(semantic_bytes: &[u8]) -> Result<Self, TerminalArtifactInterpretError> {
        let module = terminal_codec::decode_module(semantic_bytes)
            .map_err(TerminalArtifactInterpretError::SemanticDecode)?;
        Ok(Self {
            program: std::sync::Arc::new(
                AdmittedTerminalProgram::admit(module)
                    .map_err(TerminalArtifactInterpretError::Execution)?,
            ),
        })
    }

    /// Decode one complete portable Terminal-Psi envelope and admit it as a
    /// reusable program. The envelope carries no checked-tree or build object.
    pub fn load_envelope(artifact_bytes: &[u8]) -> Result<Self, TerminalArtifactInterpretError> {
        let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(artifact_bytes)
            .map_err(TerminalArtifactInterpretError::ArtifactDecode)?;
        Self::load(artifact.semantic_bytes())
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
                .boundary_machines
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
        let candidates = &self.program.provider_candidates;
        self.program
            .boundary_machines
            .values()
            .filter(move |declaration| !candidates.contains(&declaration.id))
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
            .boundary_machines
            .get(&slot)
            .filter(|declaration| {
                matches!(
                    declaration.completion,
                    terminal_psi::BoundaryCompletion::External(_)
                )
            })
    }

    /// Resolve an import's display name to its exact slot. A matching name
    /// identifies the slot; it establishes neither interface compatibility
    /// nor call authority — binding still checks the full declared contract.
    pub fn import_by_identity(&self, identity: &str) -> Option<BoundaryMachineId> {
        self.imports()
            .find(|declaration| declaration.identity == identity)
            .map(|declaration| declaration.id)
    }

    /// Start one instance of this program with its scalar arguments and every
    /// structural entry input in one record. No demanded import is invoked
    /// until `invoke`; establish each through
    /// [`TerminalInstance::bind_import`] first.
    ///
    /// The instance shares this program's admitted content verbatim — the
    /// second and later instances of one program decode and verify nothing.
    pub fn instantiate<'host>(
        &self,
        scalar_arguments: &[TerminalScalarValue],
        structural_inputs: TerminalStructuralInputs<'_>,
    ) -> Result<TerminalInstance<'host>, TerminalArtifactInterpretError> {
        self.instantiate_with(scalar_arguments, structural_inputs, None)
    }

    /// Start one instance under a supply selection previously admitted
    /// against this exact program identity.
    pub fn instantiate_installed<'host>(
        &self,
        scalar_arguments: &[TerminalScalarValue],
        structural_inputs: TerminalStructuralInputs<'_>,
        installation: &crate::effects::AdmittedProviderInstallation,
    ) -> Result<TerminalInstance<'host>, TerminalArtifactInterpretError> {
        self.instantiate_with(scalar_arguments, structural_inputs, Some(installation))
    }

    /// Wrap an already-admitted program as the public handle — the route the
    /// bare-artifact start entries take after their own decode and verify.
    pub(crate) fn from_admitted(program: AdmittedTerminalProgram) -> Self {
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
    ) -> Result<crate::execution::TerminalExecution, TerminalArtifactInterpretError> {
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
            crate::execution::TerminalExecution,
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
    ) -> Result<TerminalInstance<'host>, TerminalArtifactInterpretError> {
        let (execution, requirements) =
            self.prepare_execution(scalar_arguments, structural_inputs, installation)?;
        Ok(TerminalInstance::new(
            self.clone(),
            execution,
            requirements.imports,
        ))
    }
}
