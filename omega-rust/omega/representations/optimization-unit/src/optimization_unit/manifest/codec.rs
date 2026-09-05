use super::*;

const PRE_PHYSICAL_MANIFEST_MAGIC: &[u8; 8] = b"OMGPPM\0\0";
const PRE_PHYSICAL_MANIFEST_VERSION: u32 = 6;

impl PrePhysicalOptimizationManifest {
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_manifest_artifact_content(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(PRE_PHYSICAL_MANIFEST_MAGIC);
        encoded.extend_from_slice(&PRE_PHYSICAL_MANIFEST_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, PrePhysicalOptimizationManifestDecodeError> {
        let mut cursor = ManifestCursor::new(encoded);
        if cursor.take(8)? != PRE_PHYSICAL_MANIFEST_MAGIC {
            return Err(PrePhysicalOptimizationManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != PRE_PHYSICAL_MANIFEST_VERSION {
            return Err(PrePhysicalOptimizationManifestDecodeError::UnsupportedVersion(version));
        }
        let identity = PrePhysicalOptimizationManifestIdentity::from_bytes(cursor.array()?);
        let stage = match cursor.byte()? {
            1 => OptimizationManifestStage::PrePhysicalAbstractPlan,
            tag => {
                return Err(PrePhysicalOptimizationManifestDecodeError::UnknownStage(
                    tag,
                ));
            }
        };
        let physical_data = match cursor.byte()? {
            1 => PhysicalOptimizationDataStatus::UnavailableBeforePhysicalRealization,
            tag => {
                return Err(PrePhysicalOptimizationManifestDecodeError::UnknownPhysicalStatus(tag));
            }
        };
        let vocabulary = u16::from_le_bytes(cursor.array()?);
        let vocabulary_marker = terminal_psi::VocabularyMarker::new(vocabulary)
            .ok_or(PrePhysicalOptimizationManifestDecodeError::UnsupportedVocabulary(vocabulary))?;
        let program_fingerprint = terminal_psi::SemanticFingerprint::from_bytes(cursor.array()?);
        let fuel_schedule = FuelScheduleIdentity::new(u32::from_le_bytes(cursor.array()?))
            .ok_or(PrePhysicalOptimizationManifestDecodeError::InvalidFuelSchedule)?;
        let initial_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let final_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let projection = OptimizedAbstractPlanProjectionIdentity::from_bytes(cursor.array()?);
        let selections = OptimizationSelections::decode(cursor.length_prefixed()?)
            .map_err(|_| PrePhysicalOptimizationManifestDecodeError::InvalidSelections)?;
        let psi_selections = OptimizationSelections::decode(cursor.length_prefixed()?)
            .map_err(|_| PrePhysicalOptimizationManifestDecodeError::InvalidSelections)?;
        let budget_per_pass = OptimizationWorkBudget::decode(cursor.take(40)?)
            .map_err(|_| PrePhysicalOptimizationManifestDecodeError::InvalidWorkBudget)?;
        let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
            .map_err(|_| PrePhysicalOptimizationManifestDecodeError::InvalidWorkUsage)?;
        let decision_log = BaselineDecisionLog::decode(cursor.length_prefixed()?)
            .map_err(|_| PrePhysicalOptimizationManifestDecodeError::InvalidDecisionLog)?;
        let pass_count = cursor.length()?;
        let mut pass_manifests = Vec::with_capacity(pass_count.min(cursor.remaining()));
        for _ in 0..pass_count {
            pass_manifests.push(
                OptimizationPassManifestRecord::decode(cursor.length_prefixed()?)
                    .map_err(|_| PrePhysicalOptimizationManifestDecodeError::InvalidPassManifest)?,
            );
        }
        let transformation_ledger = PsiTransformationLedger::decode(cursor.length_prefixed()?)
            .map_err(|_| PrePhysicalOptimizationManifestDecodeError::InvalidTransformationLedger)?;
        let identity_bundle = OptimizationIdentityBundle::decode(cursor.length_prefixed()?)
            .map_err(|_| PrePhysicalOptimizationManifestDecodeError::InvalidIdentityBundle)?;
        let source_statistics = decode_statistics(&mut cursor)?;
        let optimized_statistics = decode_statistics(&mut cursor)?;
        if cursor.remaining() != 0 {
            return Err(PrePhysicalOptimizationManifestDecodeError::TrailingBytes);
        }
        let manifest = Self {
            identity,
            stage,
            physical_data,
            psi: TerminalPsiIdentity {
                vocabulary_marker,
                program_fingerprint,
            },
            fuel_schedule,
            initial_unit,
            final_unit,
            projection,
            selections,
            psi_selections,
            budget_per_pass,
            usage,
            decision_log,
            pass_manifests,
            transformation_ledger,
            identity_bundle,
            source_statistics,
            optimized_statistics,
        };
        if manifest.identity != manifest.recomputed_identity() {
            return Err(PrePhysicalOptimizationManifestDecodeError::IdentityMismatch);
        }
        Ok(manifest)
    }
}

pub(super) fn pre_physical_manifest_identity(
    manifest: &PrePhysicalOptimizationManifest,
) -> PrePhysicalOptimizationManifestIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.pre-physical-optimization-manifest.v32\0");
    canonical.extend_from_slice(&encode_manifest_content(manifest));
    PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(&canonical)
}

fn encode_manifest_content(manifest: &PrePhysicalOptimizationManifest) -> Vec<u8> {
    let mut canonical = Vec::new();
    canonical.push(match manifest.stage {
        OptimizationManifestStage::PrePhysicalAbstractPlan => 1,
    });
    canonical.push(match manifest.physical_data {
        PhysicalOptimizationDataStatus::UnavailableBeforePhysicalRealization => 1,
    });
    canonical.extend_from_slice(&manifest.psi.vocabulary_marker.get().to_le_bytes());
    canonical.extend_from_slice(manifest.psi.program_fingerprint.as_bytes());
    canonical.extend_from_slice(&manifest.fuel_schedule.marker().to_le_bytes());
    canonical.extend_from_slice(&manifest.initial_unit.bytes());
    canonical.extend_from_slice(&manifest.final_unit.bytes());
    canonical.extend_from_slice(&manifest.projection.bytes());
    encode_bytes(&mut canonical, &manifest.selections.encode());
    encode_bytes(&mut canonical, &manifest.psi_selections.encode());
    canonical.extend_from_slice(&manifest.budget_per_pass.encode());
    canonical.extend_from_slice(&manifest.usage.encode());
    encode_bytes(&mut canonical, &manifest.decision_log.encode());
    encode_len(&mut canonical, manifest.pass_manifests.len());
    for pass in &manifest.pass_manifests {
        encode_bytes(&mut canonical, &pass.encode());
    }
    canonical.extend_from_slice(&manifest.transformation_ledger.identity().bytes());
    encode_bytes(&mut canonical, &manifest.identity_bundle.encode());
    encode_statistics(&mut canonical, manifest.source_statistics);
    encode_statistics(&mut canonical, manifest.optimized_statistics);
    canonical
}

/// Artifact encoding contains the ledger itself so decoding can reconstruct the
/// complete report. The manifest identity intentionally continues to bind the
/// ledger by its semantic identity, keeping artifact framing independent from
/// the optimization result's established identity domain.
fn encode_manifest_artifact_content(manifest: &PrePhysicalOptimizationManifest) -> Vec<u8> {
    let mut canonical = Vec::new();
    canonical.push(match manifest.stage {
        OptimizationManifestStage::PrePhysicalAbstractPlan => 1,
    });
    canonical.push(match manifest.physical_data {
        PhysicalOptimizationDataStatus::UnavailableBeforePhysicalRealization => 1,
    });
    canonical.extend_from_slice(&manifest.psi.vocabulary_marker.get().to_le_bytes());
    canonical.extend_from_slice(manifest.psi.program_fingerprint.as_bytes());
    canonical.extend_from_slice(&manifest.fuel_schedule.marker().to_le_bytes());
    canonical.extend_from_slice(&manifest.initial_unit.bytes());
    canonical.extend_from_slice(&manifest.final_unit.bytes());
    canonical.extend_from_slice(&manifest.projection.bytes());
    encode_bytes(&mut canonical, &manifest.selections.encode());
    encode_bytes(&mut canonical, &manifest.psi_selections.encode());
    canonical.extend_from_slice(&manifest.budget_per_pass.encode());
    canonical.extend_from_slice(&manifest.usage.encode());
    encode_bytes(&mut canonical, &manifest.decision_log.encode());
    encode_len(&mut canonical, manifest.pass_manifests.len());
    for pass in &manifest.pass_manifests {
        encode_bytes(&mut canonical, &pass.encode());
    }
    encode_bytes(&mut canonical, &manifest.transformation_ledger.encode());
    encode_bytes(&mut canonical, &manifest.identity_bundle.encode());
    encode_statistics(&mut canonical, manifest.source_statistics);
    encode_statistics(&mut canonical, manifest.optimized_statistics);
    canonical
}

fn encode_statistics(bytes: &mut Vec<u8>, statistics: OptimizationStructuralStatistics) {
    for value in [
        statistics.functions,
        statistics.blocks,
        statistics.nodes,
        statistics.scalar_definitions,
        statistics.scalar_uses,
        statistics.optimization_facts,
        statistics.ownership_frontier_facts,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

fn decode_statistics(
    cursor: &mut ManifestCursor<'_>,
) -> Result<OptimizationStructuralStatistics, PrePhysicalOptimizationManifestDecodeError> {
    Ok(OptimizationStructuralStatistics {
        functions: u64::from_le_bytes(cursor.array()?),
        blocks: u64::from_le_bytes(cursor.array()?),
        nodes: u64::from_le_bytes(cursor.array()?),
        scalar_definitions: u64::from_le_bytes(cursor.array()?),
        scalar_uses: u64::from_le_bytes(cursor.array()?),
        optimization_facts: u64::from_le_bytes(cursor.array()?),
        ownership_frontier_facts: u64::from_le_bytes(cursor.array()?),
    })
}

fn encode_bytes(bytes: &mut Vec<u8>, value: &[u8]) {
    encode_len(bytes, value.len());
    bytes.extend_from_slice(value);
}

fn encode_len(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("canonical manifest length fits u64")
            .to_le_bytes(),
    );
}

struct ManifestCursor<'encoded> {
    encoded: &'encoded [u8],
    offset: usize,
}

impl<'encoded> ManifestCursor<'encoded> {
    const fn new(encoded: &'encoded [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    fn take(
        &mut self,
        length: usize,
    ) -> Result<&'encoded [u8], PrePhysicalOptimizationManifestDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(PrePhysicalOptimizationManifestDecodeError::Truncated)?;
        let value = self
            .encoded
            .get(self.offset..end)
            .ok_or(PrePhysicalOptimizationManifestDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], PrePhysicalOptimizationManifestDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| PrePhysicalOptimizationManifestDecodeError::Truncated)
    }

    fn byte(&mut self) -> Result<u8, PrePhysicalOptimizationManifestDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn length(&mut self) -> Result<usize, PrePhysicalOptimizationManifestDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| PrePhysicalOptimizationManifestDecodeError::LengthOverflow)
    }

    fn length_prefixed(
        &mut self,
    ) -> Result<&'encoded [u8], PrePhysicalOptimizationManifestDecodeError> {
        let length = self.length()?;
        self.take(length)
    }

    fn remaining(&self) -> usize {
        self.encoded.len() - self.offset
    }
}
