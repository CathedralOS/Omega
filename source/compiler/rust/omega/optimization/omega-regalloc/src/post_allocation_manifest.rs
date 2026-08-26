use std::collections::BTreeSet;
use std::fmt::Write;

use omega_optimization_core::{
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};
use omega_register_model::TargetRegisterEnvironmentIdentity;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity;

use crate::{
    TerminalAllocationLegalityIdentity, TerminalAllocatorAvailabilityIdentity,
    TerminalFixedViewCopyIdentity, TerminalLiteralFoldIdentity, TerminalLiveRangeIdentity,
    TerminalLivenessIdentity, TerminalRegisterHomeIdentity, ValidatedTerminalAllocationLegality,
    ValidatedTerminalLiveRanges, ValidatedTerminalRegisterHomes,
};

const POST_ALLOCATION_MANIFEST_MAGIC: &[u8; 8] = b"OMGPAO\0\0";
const POST_ALLOCATION_MANIFEST_VERSION: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationManifestStage {
    ValidatedRegisterHomes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationSpillStatus {
    NotRequiredForValidatedHomePlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PostAllocationStatistics {
    pub functions: u64,
    pub assignments: u64,
    pub distinct_physical_views: u64,
    pub virtual_interferences: u64,
    pub fixed_view_transitions: u64,
}

/// Ordered physical-form rewrites applied to the selected CFG before the
/// rooted liveness/range/legality/home chain. Order is semantic custody; this
/// is not an unordered feature or optimization-level set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostAllocationSelectedTransformation {
    FixedViewCopy(TerminalFixedViewCopyIdentity),
    LiteralFold(TerminalLiteralFoldIdentity),
}

/// Structured report at the first independently validated physical-home
/// boundary. This record is not machine-emission or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationOptimizationManifest {
    pub identity: PostAllocationOptimizationManifestIdentity,
    pub stage: PostAllocationManifestStage,
    pub pre_physical: PrePhysicalOptimizationManifestIdentity,
    pub target: NativeTarget,
    pub selected: TerminalSelectedInstructionPlanIdentity,
    pub selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>,
    pub selected_transformations: Vec<PostAllocationSelectedTransformation>,
    pub liveness: TerminalLivenessIdentity,
    pub ranges: TerminalLiveRangeIdentity,
    pub legality: TerminalAllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: TerminalAllocatorAvailabilityIdentity,
    pub homes: TerminalRegisterHomeIdentity,
    pub spills: PostAllocationSpillStatus,
    pub frame: PostAllocationUnavailableData,
    pub emission: PostAllocationUnavailableData,
    pub publication: PostAllocationUnavailableData,
    pub statistics: PostAllocationStatistics,
}

impl PostAllocationOptimizationManifest {
    pub fn recomputed_identity(&self) -> PostAllocationOptimizationManifestIdentity {
        let mut canonical = Vec::new();
        canonical.extend_from_slice(b"omega.post-allocation-optimization-manifest.v4\0");
        canonical.extend_from_slice(&encode_manifest_content(self));
        PostAllocationOptimizationManifestIdentity::from_canonical_bytes(&canonical)
    }

    /// Canonical artifact form. Decoding returns a plain record; independent
    /// post-allocation validation remains mandatory before custody accepts it.
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_manifest_content(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(POST_ALLOCATION_MANIFEST_MAGIC);
        encoded.extend_from_slice(&POST_ALLOCATION_MANIFEST_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, PostAllocationOptimizationManifestDecodeError> {
        let mut cursor = PostAllocationManifestCursor::new(encoded);
        if cursor.take(POST_ALLOCATION_MANIFEST_MAGIC.len())? != POST_ALLOCATION_MANIFEST_MAGIC {
            return Err(PostAllocationOptimizationManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != POST_ALLOCATION_MANIFEST_VERSION {
            return Err(PostAllocationOptimizationManifestDecodeError::UnsupportedVersion(version));
        }
        let identity = PostAllocationOptimizationManifestIdentity::from_bytes(cursor.array()?);
        let stage = match cursor.byte()? {
            1 => PostAllocationManifestStage::ValidatedRegisterHomes,
            tag => {
                return Err(PostAllocationOptimizationManifestDecodeError::UnknownStage(
                    tag,
                ));
            }
        };
        let pre_physical = PrePhysicalOptimizationManifestIdentity::from_bytes(cursor.array()?);
        let target = decode_target(&mut cursor)?;
        let selected = TerminalSelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let selected_lowering_completion = match cursor.byte()? {
            0 => None,
            1 => Some(SelectedLoweringOptimizationCompletionIdentity::from_bytes(
                cursor.array()?,
            )),
            tag => {
                return Err(
                    PostAllocationOptimizationManifestDecodeError::UnknownCompletionStatus(tag),
                );
            }
        };
        let transformation_count = cursor.length()?;
        let mut selected_transformations =
            Vec::with_capacity(transformation_count.min(cursor.remaining()));
        for _ in 0..transformation_count {
            selected_transformations.push(match cursor.byte()? {
                1 => PostAllocationSelectedTransformation::FixedViewCopy(
                    TerminalFixedViewCopyIdentity::from_bytes(cursor.array()?),
                ),
                2 => PostAllocationSelectedTransformation::LiteralFold(
                    TerminalLiteralFoldIdentity::from_bytes(cursor.array()?),
                ),
                tag => {
                    return Err(
                        PostAllocationOptimizationManifestDecodeError::UnknownTransformationTag(
                            tag,
                        ),
                    );
                }
            });
        }
        let liveness = TerminalLivenessIdentity::from_bytes(cursor.array()?);
        let ranges = TerminalLiveRangeIdentity::from_bytes(cursor.array()?);
        let legality = TerminalAllocationLegalityIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let allocator_availability =
            TerminalAllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
        let homes = TerminalRegisterHomeIdentity::from_bytes(cursor.array()?);
        let spills = match cursor.byte()? {
            1 => PostAllocationSpillStatus::NotRequiredForValidatedHomePlan,
            tag => {
                return Err(PostAllocationOptimizationManifestDecodeError::UnknownSpillStatus(tag));
            }
        };
        let frame = decode_unavailable(&mut cursor)?;
        let emission = decode_unavailable(&mut cursor)?;
        let publication = decode_unavailable(&mut cursor)?;
        let statistics = PostAllocationStatistics {
            functions: u64::from_le_bytes(cursor.array()?),
            assignments: u64::from_le_bytes(cursor.array()?),
            distinct_physical_views: u64::from_le_bytes(cursor.array()?),
            virtual_interferences: u64::from_le_bytes(cursor.array()?),
            fixed_view_transitions: u64::from_le_bytes(cursor.array()?),
        };
        if cursor.remaining() != 0 {
            return Err(PostAllocationOptimizationManifestDecodeError::TrailingBytes);
        }
        let manifest = Self {
            identity,
            stage,
            pre_physical,
            target,
            selected,
            selected_lowering_completion,
            selected_transformations,
            liveness,
            ranges,
            legality,
            register_environment,
            allocator_availability,
            homes,
            spills,
            frame,
            emission,
            publication,
            statistics,
        };
        if manifest.identity != manifest.recomputed_identity() {
            return Err(PostAllocationOptimizationManifestDecodeError::IdentityMismatch);
        }
        Ok(manifest)
    }

    pub fn render_text(&self) -> String {
        let mut output = String::new();
        writeln!(output, "Omega post-allocation optimization manifest").unwrap();
        writeln!(output, "stage: validated register homes").unwrap();
        writeln!(output, "manifest identity: {}", hex(&self.identity.bytes())).unwrap();
        writeln!(
            output,
            "pre-physical manifest: {}",
            hex(&self.pre_physical.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "target: {:?}/{:?}",
            self.target.architecture, self.target.object_format
        )
        .unwrap();
        writeln!(output, "selected plan: {}", hex(&self.selected.bytes())).unwrap();
        match self.selected_lowering_completion {
            Some(identity) => writeln!(
                output,
                "selected-lowering completion: {}",
                hex(&identity.bytes())
            )
            .unwrap(),
            None => writeln!(output, "selected-lowering completion: not run").unwrap(),
        }
        writeln!(
            output,
            "selected transformations: {}",
            self.selected_transformations.len()
        )
        .unwrap();
        for (index, transformation) in self.selected_transformations.iter().enumerate() {
            let (kind, identity) = match transformation {
                PostAllocationSelectedTransformation::FixedViewCopy(identity) => {
                    ("fixed-view-copy", identity.bytes())
                }
                PostAllocationSelectedTransformation::LiteralFold(identity) => {
                    ("literal-fold", identity.bytes())
                }
            };
            writeln!(
                output,
                "selected transformation {index}: {kind} {}",
                hex(&identity)
            )
            .unwrap();
        }
        writeln!(output, "register homes: {}", hex(&self.homes.bytes())).unwrap();
        writeln!(
            output,
            "allocator availability: {}",
            hex(&self.allocator_availability.bytes())
        )
        .unwrap();
        writeln!(output, "spills: not required for validated home plan").unwrap();
        writeln!(output, "frame: unavailable").unwrap();
        writeln!(output, "emission: unavailable").unwrap();
        writeln!(output, "publication: unavailable").unwrap();
        writeln!(output, "functions: {}", self.statistics.functions).unwrap();
        writeln!(output, "assignments: {}", self.statistics.assignments).unwrap();
        writeln!(
            output,
            "distinct physical views: {}",
            self.statistics.distinct_physical_views
        )
        .unwrap();
        writeln!(
            output,
            "virtual interferences: {}",
            self.statistics.virtual_interferences
        )
        .unwrap();
        writeln!(
            output,
            "fixed-view transitions: {}",
            self.statistics.fixed_view_transitions
        )
        .unwrap();
        output
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPostAllocationOptimizationManifest {
    record: PostAllocationOptimizationManifest,
}

impl ValidatedPostAllocationOptimizationManifest {
    pub const fn record(&self) -> &PostAllocationOptimizationManifest {
        &self.record
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationOptimizationManifestError {
    RootMismatch,
    UnresolvedFixedViewTransitions,
    StatisticsOverflow,
    NonCanonicalTransformationLedger,
    IdentityMismatch,
    ContentMismatch,
}

impl std::fmt::Display for PostAllocationOptimizationManifestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid post-allocation optimization manifest: {self:?}"
        )
    }
}

impl std::error::Error for PostAllocationOptimizationManifestError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationOptimizationManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownArchitecture(u8),
    UnknownObjectFormat(u8),
    TargetLayoutOverflow,
    LengthOverflow,
    UnknownTransformationTag(u8),
    UnknownCompletionStatus(u8),
    UnknownSpillStatus(u8),
    UnknownUnavailableStatus(u8),
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for PostAllocationOptimizationManifestDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid post-allocation manifest encoding: {self:?}"
        )
    }
}

impl std::error::Error for PostAllocationOptimizationManifestDecodeError {}

pub fn project_post_allocation_optimization_manifest(
    pre_physical: PrePhysicalOptimizationManifestIdentity,
    selected_transformations: &[PostAllocationSelectedTransformation],
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    homes: &ValidatedTerminalRegisterHomes,
) -> Result<ValidatedPostAllocationOptimizationManifest, PostAllocationOptimizationManifestError> {
    let record = expected_record(
        pre_physical,
        None,
        selected_transformations,
        ranges,
        legality,
        homes,
    )?;
    Ok(ValidatedPostAllocationOptimizationManifest { record })
}

pub fn project_post_allocation_optimization_manifest_after_selected_lowering(
    pre_physical: PrePhysicalOptimizationManifestIdentity,
    selected_lowering_completion: SelectedLoweringOptimizationCompletionIdentity,
    selected_transformations: &[PostAllocationSelectedTransformation],
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    homes: &ValidatedTerminalRegisterHomes,
) -> Result<ValidatedPostAllocationOptimizationManifest, PostAllocationOptimizationManifestError> {
    let record = expected_record(
        pre_physical,
        Some(selected_lowering_completion),
        selected_transformations,
        ranges,
        legality,
        homes,
    )?;
    Ok(ValidatedPostAllocationOptimizationManifest { record })
}

pub fn validate_post_allocation_optimization_manifest(
    candidate: &PostAllocationOptimizationManifest,
    pre_physical: PrePhysicalOptimizationManifestIdentity,
    selected_transformations: &[PostAllocationSelectedTransformation],
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    homes: &ValidatedTerminalRegisterHomes,
) -> Result<ValidatedPostAllocationOptimizationManifest, PostAllocationOptimizationManifestError> {
    if candidate.identity != candidate.recomputed_identity() {
        return Err(PostAllocationOptimizationManifestError::IdentityMismatch);
    }
    let expected = expected_record(
        pre_physical,
        None,
        selected_transformations,
        ranges,
        legality,
        homes,
    )?;
    if candidate != &expected {
        return Err(PostAllocationOptimizationManifestError::ContentMismatch);
    }
    Ok(ValidatedPostAllocationOptimizationManifest {
        record: candidate.clone(),
    })
}

pub fn validate_post_allocation_optimization_manifest_after_selected_lowering(
    candidate: &PostAllocationOptimizationManifest,
    pre_physical: PrePhysicalOptimizationManifestIdentity,
    selected_lowering_completion: SelectedLoweringOptimizationCompletionIdentity,
    selected_transformations: &[PostAllocationSelectedTransformation],
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    homes: &ValidatedTerminalRegisterHomes,
) -> Result<ValidatedPostAllocationOptimizationManifest, PostAllocationOptimizationManifestError> {
    if candidate.identity != candidate.recomputed_identity() {
        return Err(PostAllocationOptimizationManifestError::IdentityMismatch);
    }
    let expected = expected_record(
        pre_physical,
        Some(selected_lowering_completion),
        selected_transformations,
        ranges,
        legality,
        homes,
    )?;
    if candidate != &expected {
        return Err(PostAllocationOptimizationManifestError::ContentMismatch);
    }
    Ok(ValidatedPostAllocationOptimizationManifest {
        record: candidate.clone(),
    })
}

fn expected_record(
    pre_physical: PrePhysicalOptimizationManifestIdentity,
    selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>,
    selected_transformations: &[PostAllocationSelectedTransformation],
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    homes: &ValidatedTerminalRegisterHomes,
) -> Result<PostAllocationOptimizationManifest, PostAllocationOptimizationManifestError> {
    let mut unique_transformations = BTreeSet::new();
    if selected_transformations.iter().any(|transformation| {
        let key = match transformation {
            PostAllocationSelectedTransformation::FixedViewCopy(identity) => {
                (1_u8, identity.bytes())
            }
            PostAllocationSelectedTransformation::LiteralFold(identity) => (2_u8, identity.bytes()),
        };
        !unique_transformations.insert(key)
    }) {
        return Err(PostAllocationOptimizationManifestError::NonCanonicalTransformationLedger);
    }
    if legality.receipt().ranges() != ranges.receipt().identity()
        || homes.receipt().legality() != legality.receipt().identity()
        || homes.receipt().ranges() != ranges.receipt().identity()
        || homes.receipt().register_environment() != legality.receipt().register_environment()
        || homes.receipt().allocator_availability() != legality.receipt().allocator_availability()
        || homes.plan().functions.len() != ranges.plan().functions.len()
        || homes.plan().functions.len() != legality.plan().functions.len()
    {
        return Err(PostAllocationOptimizationManifestError::RootMismatch);
    }
    let transition_count = legality
        .plan()
        .functions
        .iter()
        .flat_map(|function| &function.virtual_registers)
        .map(|register| register.entry_transitions.len())
        .sum::<usize>();
    if transition_count != 0 {
        return Err(PostAllocationOptimizationManifestError::UnresolvedFixedViewTransitions);
    }
    let distinct_views = homes
        .plan()
        .functions
        .iter()
        .flat_map(|function| &function.assignments)
        .map(|assignment| assignment.view)
        .collect::<BTreeSet<_>>()
        .len();
    let interference_count = ranges
        .plan()
        .functions
        .iter()
        .map(|function| function.interference.len())
        .sum::<usize>();
    let statistics = PostAllocationStatistics {
        functions: count(homes.plan().functions.len())?,
        assignments: count(homes.receipt().assignment_count())?,
        distinct_physical_views: count(distinct_views)?,
        virtual_interferences: count(interference_count)?,
        fixed_view_transitions: 0,
    };
    let mut record = PostAllocationOptimizationManifest {
        identity: PostAllocationOptimizationManifestIdentity::from_canonical_bytes(b"pending"),
        stage: PostAllocationManifestStage::ValidatedRegisterHomes,
        pre_physical,
        target: ranges.plan().target,
        selected: ranges.plan().selected,
        selected_lowering_completion,
        selected_transformations: selected_transformations.to_vec(),
        liveness: ranges.receipt().liveness(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        register_environment: legality.receipt().register_environment(),
        allocator_availability: legality.receipt().allocator_availability(),
        homes: homes.receipt().identity(),
        spills: PostAllocationSpillStatus::NotRequiredForValidatedHomePlan,
        frame: PostAllocationUnavailableData::Unavailable,
        emission: PostAllocationUnavailableData::Unavailable,
        publication: PostAllocationUnavailableData::Unavailable,
        statistics,
    };
    record.identity = record.recomputed_identity();
    Ok(record)
}

fn count(value: usize) -> Result<u64, PostAllocationOptimizationManifestError> {
    u64::try_from(value).map_err(|_| PostAllocationOptimizationManifestError::StatisticsOverflow)
}

fn statistics_values(statistics: PostAllocationStatistics) -> [u64; 5] {
    [
        statistics.functions,
        statistics.assignments,
        statistics.distinct_physical_views,
        statistics.virtual_interferences,
        statistics.fixed_view_transitions,
    ]
}

fn encode_manifest_content(manifest: &PostAllocationOptimizationManifest) -> Vec<u8> {
    let mut canonical = Vec::new();
    canonical.push(match manifest.stage {
        PostAllocationManifestStage::ValidatedRegisterHomes => 1,
    });
    canonical.extend_from_slice(&manifest.pre_physical.bytes());
    encode_target(&mut canonical, manifest.target);
    canonical.extend_from_slice(&manifest.selected.bytes());
    match manifest.selected_lowering_completion {
        Some(identity) => {
            canonical.push(1);
            canonical.extend_from_slice(&identity.bytes());
        }
        None => canonical.push(0),
    }
    canonical.extend_from_slice(
        &u64::try_from(manifest.selected_transformations.len())
            .expect("post-allocation transformation length fits u64")
            .to_le_bytes(),
    );
    for transformation in &manifest.selected_transformations {
        match transformation {
            PostAllocationSelectedTransformation::FixedViewCopy(identity) => {
                canonical.push(1);
                canonical.extend_from_slice(&identity.bytes());
            }
            PostAllocationSelectedTransformation::LiteralFold(identity) => {
                canonical.push(2);
                canonical.extend_from_slice(&identity.bytes());
            }
        }
    }
    canonical.extend_from_slice(&manifest.liveness.bytes());
    canonical.extend_from_slice(&manifest.ranges.bytes());
    canonical.extend_from_slice(&manifest.legality.bytes());
    canonical.extend_from_slice(&manifest.register_environment.bytes());
    canonical.extend_from_slice(&manifest.allocator_availability.bytes());
    canonical.extend_from_slice(&manifest.homes.bytes());
    canonical.push(match manifest.spills {
        PostAllocationSpillStatus::NotRequiredForValidatedHomePlan => 1,
    });
    for unavailable in [manifest.frame, manifest.emission, manifest.publication] {
        canonical.push(match unavailable {
            PostAllocationUnavailableData::Unavailable => 1,
        });
    }
    for value in statistics_values(manifest.statistics) {
        canonical.extend_from_slice(&value.to_le_bytes());
    }
    canonical
}

fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    bytes.push(match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    });
    bytes.push(match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    });
    bytes.extend_from_slice(
        &u64::try_from(target.pointer_size)
            .expect("target pointer size fits u64")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &u64::try_from(target.pointer_alignment)
            .expect("target pointer alignment fits u64")
            .to_le_bytes(),
    );
}

fn decode_target(
    cursor: &mut PostAllocationManifestCursor<'_>,
) -> Result<NativeTarget, PostAllocationOptimizationManifestDecodeError> {
    let architecture = match cursor.byte()? {
        1 => Architecture::Aarch64,
        2 => Architecture::X86_64,
        tag => {
            return Err(PostAllocationOptimizationManifestDecodeError::UnknownArchitecture(tag));
        }
    };
    let object_format = match cursor.byte()? {
        1 => ObjectFormat::Elf,
        2 => ObjectFormat::MachO,
        3 => ObjectFormat::Coff,
        tag => {
            return Err(PostAllocationOptimizationManifestDecodeError::UnknownObjectFormat(tag));
        }
    };
    let pointer_size = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| PostAllocationOptimizationManifestDecodeError::TargetLayoutOverflow)?;
    let pointer_alignment = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| PostAllocationOptimizationManifestDecodeError::TargetLayoutOverflow)?;
    Ok(NativeTarget {
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
    })
}

fn decode_unavailable(
    cursor: &mut PostAllocationManifestCursor<'_>,
) -> Result<PostAllocationUnavailableData, PostAllocationOptimizationManifestDecodeError> {
    match cursor.byte()? {
        1 => Ok(PostAllocationUnavailableData::Unavailable),
        tag => Err(PostAllocationOptimizationManifestDecodeError::UnknownUnavailableStatus(tag)),
    }
}

struct PostAllocationManifestCursor<'encoded> {
    encoded: &'encoded [u8],
    offset: usize,
}

impl<'encoded> PostAllocationManifestCursor<'encoded> {
    const fn new(encoded: &'encoded [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    fn take(
        &mut self,
        length: usize,
    ) -> Result<&'encoded [u8], PostAllocationOptimizationManifestDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(PostAllocationOptimizationManifestDecodeError::Truncated)?;
        let value = self
            .encoded
            .get(self.offset..end)
            .ok_or(PostAllocationOptimizationManifestDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], PostAllocationOptimizationManifestDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| PostAllocationOptimizationManifestDecodeError::Truncated)
    }

    fn byte(&mut self) -> Result<u8, PostAllocationOptimizationManifestDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn length(&mut self) -> Result<usize, PostAllocationOptimizationManifestDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| PostAllocationOptimizationManifestDecodeError::LengthOverflow)
    }

    fn remaining(&self) -> usize {
        self.encoded.len() - self.offset
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::{
        PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
        SelectedLoweringOptimizationCompletionIdentity,
    };
    use omega_register_model::TargetRegisterEnvironmentIdentity;
    use omega_target::NativeTarget;
    use omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity;

    use super::*;

    type Mutation = fn(&mut PostAllocationOptimizationManifest);

    fn record() -> PostAllocationOptimizationManifest {
        let mut record = PostAllocationOptimizationManifest {
            identity: PostAllocationOptimizationManifestIdentity::from_canonical_bytes(b"pending"),
            stage: PostAllocationManifestStage::ValidatedRegisterHomes,
            pre_physical: PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(b"pre"),
            target: NativeTarget::linux_x64(),
            selected: TerminalSelectedInstructionPlanIdentity::from_canonical_bytes(b"selected"),
            selected_lowering_completion: None,
            selected_transformations: Vec::new(),
            liveness: TerminalLivenessIdentity([1; 32]),
            ranges: TerminalLiveRangeIdentity::from_bytes([2; 32]),
            legality: TerminalAllocationLegalityIdentity::from_bytes([3; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([4; 32]),
            allocator_availability: TerminalAllocatorAvailabilityIdentity::from_bytes([6; 32]),
            homes: TerminalRegisterHomeIdentity::from_bytes([5; 32]),
            spills: PostAllocationSpillStatus::NotRequiredForValidatedHomePlan,
            frame: PostAllocationUnavailableData::Unavailable,
            emission: PostAllocationUnavailableData::Unavailable,
            publication: PostAllocationUnavailableData::Unavailable,
            statistics: PostAllocationStatistics {
                functions: 1,
                assignments: 2,
                distinct_physical_views: 2,
                virtual_interferences: 1,
                fixed_view_transitions: 0,
            },
        };
        record.identity = record.recomputed_identity();
        record
    }

    #[test]
    fn identity_binds_every_post_allocation_domain() {
        let baseline = record();
        assert_eq!(baseline.identity, baseline.recomputed_identity());
        let mutations: Vec<Mutation> = vec![
            |record| {
                record.pre_physical =
                    PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(b"other")
            },
            |record| record.target = NativeTarget::linux_arm64(),
            |record| {
                record.selected =
                    TerminalSelectedInstructionPlanIdentity::from_canonical_bytes(b"other")
            },
            |record| {
                record.selected_lowering_completion = Some(
                    SelectedLoweringOptimizationCompletionIdentity::from_canonical_bytes(
                        b"completed",
                    ),
                )
            },
            |record| {
                record.selected_transformations.push(
                    PostAllocationSelectedTransformation::FixedViewCopy(
                        TerminalFixedViewCopyIdentity([6; 32]),
                    ),
                )
            },
            |record| {
                record.selected_transformations.push(
                    PostAllocationSelectedTransformation::LiteralFold(
                        TerminalLiteralFoldIdentity::from_bytes([13; 32]),
                    ),
                )
            },
            |record| record.liveness = TerminalLivenessIdentity([7; 32]),
            |record| record.ranges = TerminalLiveRangeIdentity::from_bytes([8; 32]),
            |record| record.legality = TerminalAllocationLegalityIdentity::from_bytes([9; 32]),
            |record| {
                record.register_environment =
                    TargetRegisterEnvironmentIdentity::from_bytes([10; 32])
            },
            |record| {
                record.allocator_availability =
                    TerminalAllocatorAvailabilityIdentity::from_bytes([12; 32])
            },
            |record| record.homes = TerminalRegisterHomeIdentity::from_bytes([11; 32]),
            |record| record.statistics.functions += 1,
            |record| record.statistics.assignments += 1,
            |record| record.statistics.distinct_physical_views += 1,
            |record| record.statistics.virtual_interferences += 1,
            |record| record.statistics.fixed_view_transitions += 1,
        ];
        for mutate in mutations {
            let mut changed = baseline.clone();
            mutate(&mut changed);
            assert_ne!(baseline.identity, changed.recomputed_identity());
        }
        let text = baseline.render_text();
        assert!(text.contains("spills: not required"));
        assert!(text.contains("publication: unavailable"));
    }

    #[test]
    fn canonical_codec_round_trips_both_routes_and_rejects_corruption() {
        let direct = record();
        let encoded = direct.encode();
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&encoded),
            Ok(direct)
        );

        let mut transformed = record();
        transformed.selected_lowering_completion = Some(
            SelectedLoweringOptimizationCompletionIdentity::from_canonical_bytes(b"completed"),
        );
        transformed.selected_transformations = vec![
            PostAllocationSelectedTransformation::FixedViewCopy(TerminalFixedViewCopyIdentity(
                [12; 32],
            )),
            PostAllocationSelectedTransformation::LiteralFold(
                TerminalLiteralFoldIdentity::from_bytes([13; 32]),
            ),
        ];
        transformed.identity = transformed.recomputed_identity();
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&transformed.encode()),
            Ok(transformed)
        );

        let mut identity_tamper = encoded.clone();
        identity_tamper[12] ^= 1;
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&identity_tamper),
            Err(PostAllocationOptimizationManifestDecodeError::IdentityMismatch)
        );
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&trailing),
            Err(PostAllocationOptimizationManifestDecodeError::TrailingBytes)
        );
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&encoded[..encoded.len() - 1]),
            Err(PostAllocationOptimizationManifestDecodeError::Truncated)
        );
        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 1;
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&wrong_magic),
            Err(PostAllocationOptimizationManifestDecodeError::WrongMagic)
        );
        let mut wrong_version = encoded.clone();
        wrong_version[8..12].copy_from_slice(&3_u32.to_le_bytes());
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&wrong_version),
            Err(PostAllocationOptimizationManifestDecodeError::UnsupportedVersion(3))
        );
        let content_offset = 8 + 4 + 32;
        let mut unknown_architecture = encoded.clone();
        unknown_architecture[content_offset + 1 + 32] = 9;
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&unknown_architecture),
            Err(PostAllocationOptimizationManifestDecodeError::UnknownArchitecture(9))
        );
        let mut one_transformation = record();
        one_transformation.selected_transformations =
            vec![PostAllocationSelectedTransformation::FixedViewCopy(
                TerminalFixedViewCopyIdentity([12; 32]),
            )];
        one_transformation.identity = one_transformation.recomputed_identity();
        let mut unknown_transformation = one_transformation.encode();
        let transformation_tag_offset = content_offset + 1 + 32 + 18 + 32 + 1 + 8;
        unknown_transformation[transformation_tag_offset] = 9;
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&unknown_transformation),
            Err(PostAllocationOptimizationManifestDecodeError::UnknownTransformationTag(9))
        );
    }
}
