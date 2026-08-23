use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;

use omega_executable_installation::{ArtifactId, InstalledCodeId};
use psi_core::{
    ContentAlgebra, ProgramLocalCapacityExpression, StructuralDomainId, StructuralTypeId,
};
use psi_terminal::{
    ProgramLocalRootIntroductionSchema, TerminalModule, TerminalPsiIdentity,
    program_local_root_introduction_identity,
};

use super::{
    ExternalRootDiagnostic, ExternalRootId, InstalledExternalRoot, ProviderExecutionId,
    RootAdmissionId, RootSlotId, RootSlotOwnerId, TerminalObjectEvidence, bind_terminal_function,
};

/// Non-authoritative prebinding of one portable producer schema to one exact
/// installed environment-to-program slot occurrence.
///
/// This record deliberately does not carry a lifecycle epoch and cannot mint
/// content. It closes the installation facts already available today so a
/// later lifecycle join can consume one typed occurrence instead of repeating
/// requirement, provider, artifact, and slot matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramLocalRootInstalledOccurrence {
    pub occurrence_identity: u64,
    pub terminal_psi: TerminalPsiIdentity,
    pub root: ExternalRootId,
    pub slot: RootSlotId,
    pub owner: RootSlotOwnerId,
    pub installed_code: InstalledCodeId,
    pub artifact: ArtifactId,
    pub admission: RootAdmissionId,
    pub provider_execution: ProviderExecutionId,
    pub requirement_identity: String,
    pub source_parameter_position: u32,
    pub qualification_identity: String,
    pub carrier_identity: String,
    pub schema_identity: u64,
    pub projection: psi_core::ContentProjectionIdentity,
    pub algebra: ContentAlgebra,
    pub per_occurrence_capacity: ProgramLocalCapacityExpression,
}

/// Exact installed-slot count derived for one schema and one installed
/// artifact occurrence. It remains prebinding evidence, not an aggregate root:
/// lifecycle epoch and authority introduction are intentionally absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramLocalRootInstalledCount {
    pub terminal_psi: TerminalPsiIdentity,
    pub installed_code: InstalledCodeId,
    pub artifact: ArtifactId,
    pub requirement_identity: String,
    pub source_parameter_position: u32,
    pub qualification_identity: String,
    pub carrier_identity: String,
    pub schema_identity: u64,
    pub algebra: ContentAlgebra,
    pub per_occurrence_capacity: ProgramLocalCapacityExpression,
    pub installed_slot_count: NonZeroU64,
    pub occurrence_identities: Vec<u64>,
}

type OccurrenceKey = (InstalledCodeId, RootSlotId, u64);
type CountKey = (u16, [u8; 32], InstalledCodeId, u64);

/// Installation-owned ledger for the non-minting occurrence prebinding.
#[derive(Debug, Default)]
pub struct ProgramLocalRootInstalledOccurrenceLedger {
    occurrences: BTreeMap<OccurrenceKey, ProgramLocalRootInstalledOccurrence>,
}

impl ProgramLocalRootInstalledOccurrenceLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Join every authorized schema on the installed root's exact requirement
    /// to the live slot, provider execution, admission, and artifact
    /// occurrence. The join is transactional and replay rejecting.
    pub fn prebind<TerminalArtifact: TerminalObjectEvidence>(
        &mut self,
        module: &TerminalModule,
        terminal_artifact: &TerminalArtifact,
        root: &InstalledExternalRoot<'_>,
    ) -> Result<Vec<ProgramLocalRootInstalledOccurrence>, ExternalRootDiagnostic> {
        let terminal_psi = psi_terminal_codec::terminal_psi_identity(module).map_err(|error| {
            ExternalRootDiagnostic(format!(
                "cannot identify terminal program-local root catalog: {error}"
            ))
        })?;
        if terminal_artifact.terminal_psi() != terminal_psi {
            return Err(ExternalRootDiagnostic(
                "program-local root catalog does not match the terminal artifact identity".into(),
            ));
        }
        let text_offset = terminal_artifact
            .function_text_offset(module.entry)
            .ok_or_else(|| {
                ExternalRootDiagnostic(
                    "terminal artifact has no installed entry for the program-local root catalog"
                        .into(),
                )
            })?;
        bind_terminal_function(
            terminal_artifact,
            root.installed_code,
            root.evidence.root.candidate.entry,
            text_offset,
        )?;
        let requirement_identity = &root.evidence.root.candidate.requirement_identity;
        let mut matching = module
            .boundary_machines
            .iter()
            .filter(|boundary| boundary.identity == *requirement_identity);
        let boundary = matching.next().ok_or_else(|| {
            ExternalRootDiagnostic(format!(
                "installed root requirement `{requirement_identity}` is absent from terminal Psi"
            ))
        })?;
        if matching.next().is_some() {
            return Err(ExternalRootDiagnostic(format!(
                "installed root requirement `{requirement_identity}` is not unique in terminal Psi"
            )));
        }

        let mut pending = Vec::with_capacity(boundary.program_local_root_introductions.len());
        let mut local_schema_keys = BTreeSet::new();
        for schema in &boundary.program_local_root_introductions {
            let resolved = resolve_schema(module, boundary, schema)?;
            if root
                .evidence
                .root
                .boundary
                .plan()
                .call
                .parameters
                .get(schema.argument_index as usize)
                .is_none()
                || !root
                    .evidence
                    .root
                    .candidate
                    .entry_claims
                    .iter()
                    .any(|claim| {
                        claim.parameter_index == schema.argument_index as usize
                            && claim.domain == resolved.qualification_identity
                    })
            {
                return Err(ExternalRootDiagnostic(
                    "program-local root schema does not match an exact installed entry claim and ABI position"
                        .into(),
                ));
            }
            if !local_schema_keys.insert((schema.source_parameter_position, schema.identity)) {
                return Err(ExternalRootDiagnostic(
                    "program-local root producer schemas repeat one semantic occurrence".into(),
                ));
            }
            let key = (root.installed_code.identity(), root.slot, schema.identity);
            if self.occurrences.contains_key(&key) {
                return Err(ExternalRootDiagnostic(
                    "program-local root installed occurrence was already prebound".into(),
                ));
            }
            let occurrence_identity = occurrence_identity(root, terminal_psi, schema.identity);
            pending.push((
                key,
                ProgramLocalRootInstalledOccurrence {
                    occurrence_identity,
                    terminal_psi,
                    root: root.root,
                    slot: root.slot,
                    owner: root.owner,
                    installed_code: root.installed_code.identity(),
                    artifact: root.installed_code.artifact(),
                    admission: root.evidence.admission,
                    provider_execution: root.evidence.provider_execution.identity,
                    requirement_identity: requirement_identity.clone(),
                    source_parameter_position: schema.source_parameter_position,
                    qualification_identity: resolved.qualification_identity,
                    carrier_identity: resolved.carrier_identity,
                    schema_identity: schema.identity,
                    projection: schema.projection,
                    algebra: schema.algebra.clone(),
                    per_occurrence_capacity: schema.capacity.clone(),
                },
            ));
        }

        let joined = pending
            .iter()
            .map(|(_, occurrence)| occurrence.clone())
            .collect();
        for (key, occurrence) in pending {
            self.occurrences.insert(key, occurrence);
        }
        Ok(joined)
    }

    pub fn occurrences(&self) -> impl Iterator<Item = &ProgramLocalRootInstalledOccurrence> {
        self.occurrences.values()
    }

    /// Count distinct prebound slots from ledger state. No producer-authored
    /// cardinality or aggregate is accepted.
    pub fn counts(&self) -> Vec<ProgramLocalRootInstalledCount> {
        let mut groups: BTreeMap<CountKey, (ProgramLocalRootInstalledOccurrence, Vec<u64>)> =
            BTreeMap::new();
        for occurrence in self.occurrences.values() {
            let key = (
                occurrence.terminal_psi.vocabulary_marker.get(),
                *occurrence.terminal_psi.program_fingerprint.as_bytes(),
                occurrence.installed_code,
                occurrence.schema_identity,
            );
            groups
                .entry(key)
                .and_modify(|(_, identities)| identities.push(occurrence.occurrence_identity))
                .or_insert_with(|| (occurrence.clone(), vec![occurrence.occurrence_identity]));
        }
        groups
            .into_values()
            .map(|(occurrence, mut identities)| {
                identities.sort_unstable();
                ProgramLocalRootInstalledCount {
                    terminal_psi: occurrence.terminal_psi,
                    installed_code: occurrence.installed_code,
                    artifact: occurrence.artifact,
                    requirement_identity: occurrence.requirement_identity,
                    source_parameter_position: occurrence.source_parameter_position,
                    qualification_identity: occurrence.qualification_identity,
                    carrier_identity: occurrence.carrier_identity,
                    schema_identity: occurrence.schema_identity,
                    algebra: occurrence.algebra,
                    per_occurrence_capacity: occurrence.per_occurrence_capacity,
                    installed_slot_count: NonZeroU64::new(
                        u64::try_from(identities.len())
                            .expect("installed occurrence count fits u64"),
                    )
                    .expect("count group is nonempty"),
                    occurrence_identities: identities,
                }
            })
            .collect()
    }
}

struct ResolvedSchema {
    qualification_identity: String,
    carrier_identity: String,
}

fn resolve_schema(
    module: &TerminalModule,
    boundary: &psi_terminal::BoundaryMachineDeclaration,
    schema: &ProgramLocalRootIntroductionSchema,
) -> Result<ResolvedSchema, ExternalRootDiagnostic> {
    let parameter = boundary
        .structural_parameters
        .get(schema.argument_index as usize)
        .ok_or_else(|| {
            ExternalRootDiagnostic(
                "program-local root schema names an absent structural parameter".into(),
            )
        })?;
    if parameter.position != schema.source_parameter_position
        || parameter.structural_type != schema.carrier
        || !parameter.qualifications.contains(&schema.qualification)
        || !boundary.requires.iter().any(|requirement| {
            requirement.argument_index == schema.argument_index
                && requirement.domain == schema.qualification
        })
    {
        return Err(ExternalRootDiagnostic(
            "program-local root schema does not match the requirement's exact qualified semantic parameter"
                .into(),
        ));
    }
    let qualification = unique_domain(module, schema.qualification)?;
    let carrier = unique_type(module, schema.carrier)?;
    if qualification.carrier != schema.carrier {
        return Err(ExternalRootDiagnostic(
            "program-local root qualification and carrier do not agree".into(),
        ));
    }
    let expected = program_local_root_introduction_identity(
        &boundary.identity,
        &qualification.identity,
        &carrier.identity,
        schema,
    );
    if schema.identity != expected {
        return Err(ExternalRootDiagnostic(
            "program-local root schema identity does not replay from its exact requirement and content facts"
                .into(),
        ));
    }
    Ok(ResolvedSchema {
        qualification_identity: qualification.identity.clone(),
        carrier_identity: carrier.identity.clone(),
    })
}

fn unique_domain(
    module: &TerminalModule,
    id: StructuralDomainId,
) -> Result<&psi_terminal::StructuralDomainDeclaration, ExternalRootDiagnostic> {
    let mut rows = module
        .structural_domains
        .iter()
        .filter(|declaration| declaration.id == id);
    let row = rows.next().ok_or_else(|| {
        ExternalRootDiagnostic("program-local root schema names an absent qualification".into())
    })?;
    if rows.next().is_some() {
        return Err(ExternalRootDiagnostic(
            "program-local root qualification identity is not unique".into(),
        ));
    }
    Ok(row)
}

fn unique_type(
    module: &TerminalModule,
    id: StructuralTypeId,
) -> Result<&psi_terminal::StructuralTypeDeclaration, ExternalRootDiagnostic> {
    let mut rows = module
        .structural_types
        .iter()
        .filter(|declaration| declaration.id == id);
    let row = rows.next().ok_or_else(|| {
        ExternalRootDiagnostic("program-local root schema names an absent carrier".into())
    })?;
    if rows.next().is_some() {
        return Err(ExternalRootDiagnostic(
            "program-local root carrier identity is not unique".into(),
        ));
    }
    Ok(row)
}

fn occurrence_identity(
    root: &InstalledExternalRoot<'_>,
    terminal_psi: TerminalPsiIdentity,
    schema_identity: u64,
) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut mix = |byte: u8| {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x1000_0000_01b3);
    };
    for byte in b"omega.program-local-root.installed-prebinding.v1" {
        mix(*byte);
    }
    for byte in terminal_psi.vocabulary_marker.get().to_le_bytes() {
        mix(byte);
    }
    for byte in terminal_psi.program_fingerprint.as_bytes() {
        mix(*byte);
    }
    for value in [
        root.root.normalized_identity(),
        root.slot.normalized_identity(),
        root.owner.normalized_identity(),
        root.evidence.admission.normalized_identity(),
        root.evidence
            .provider_execution
            .identity
            .normalized_identity(),
        root.installed_code.identity().normalized_identity(),
        root.installed_code.artifact().normalized_identity(),
        schema_identity,
    ] {
        for byte in value.to_le_bytes() {
            mix(byte);
        }
    }
    if hash == 0 { 1 } else { hash }
}
