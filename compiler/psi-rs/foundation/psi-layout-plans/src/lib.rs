#![forbid(unsafe_code)]

//! Normalized programmable-layout plans and symbolic materialization.
//!
//! Layout policies describe geometry. A materializer consumes validated
//! geometry plus compiler-issued symbolic values; source programs never
//! receive numeric code addresses or an arbitrary byte-patching primitive.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IntegerInterpretation {
    Signed,
    Unsigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutPlacementReport {
    At {
        offset: u64,
    },
    IntegerAt {
        offset: u64,
        stored_width: u64,
        interpretation: IntegerInterpretation,
    },
    Bits {
        container: u64,
        container_width: u64,
        destination_lsb: u64,
        source_lsb: u64,
        width: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutFieldEntryReport {
    /// Normalized field name. Compiler-issued keys do not escape into artifact
    /// reports.
    pub field: String,
    /// Authored stable schema identity when this scope is numbered. Canonical
    /// plan identity uses this instead of the source-facing name, so a rename
    /// preserves identity.
    pub member_identity: Option<u64>,
    pub placement: LayoutPlacementReport,
}

/// A validated layout plan, ready for consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutPlanReport {
    /// Canonical identity of the complete reflected schema, including stable
    /// field/case identities and tombstones but excluding numbered-member
    /// source names and runtime discriminants.
    pub schema_identity: u64,
    pub entries: Vec<LayoutFieldEntryReport>,
    /// Declaration-order offsets when every field has one fixed `At`
    /// placement. Fragmented plans deliberately have no such projection.
    pub offsets: Option<Vec<u64>>,
    pub size: Option<u64>,
    pub align: u64,
}

/// Deterministic semantic identity of one validated layout plan.
///
/// Compiler-issued field keys, numbered-member source names, and authored entry
/// order are deliberately absent. Repeated fragments are sorted by stable
/// member identity (or by name for positional schemas) and complete normalized
/// placement, while schema identity, size, and alignment remain
/// identity-bearing. The derived `offsets` convenience projection is excluded
/// because it contains no fact beyond the entries.
pub fn normalized_layout_plan_fingerprint(layout: &LayoutPlanReport) -> u64 {
    let mut entries = layout.entries.iter().collect::<Vec<_>>();
    entries.sort_unstable_by(|left, right| {
        member_sort_key(left)
            .cmp(&member_sort_key(right))
            .then_with(|| {
                placement_sort_key(&left.placement).cmp(&placement_sort_key(&right.placement))
            })
    });

    let mut hash = 0xcbf29ce484222325u64;
    hash_fingerprint_bytes(&mut hash, b"omega.layout-plan.v3");
    hash_fingerprint_u64(&mut hash, layout.schema_identity);
    hash_fingerprint_byte(&mut hash, u8::from(layout.size.is_some()));
    if let Some(size) = layout.size {
        hash_fingerprint_u64(&mut hash, size);
    }
    hash_fingerprint_u64(&mut hash, layout.align);
    hash_fingerprint_u64(&mut hash, entries.len() as u64);
    for entry in entries {
        match entry.member_identity {
            Some(identity) => {
                hash_fingerprint_byte(&mut hash, 1);
                hash_fingerprint_u64(&mut hash, identity);
            }
            None => {
                hash_fingerprint_byte(&mut hash, 0);
                hash_fingerprint_u64(&mut hash, entry.field.len() as u64);
                hash_fingerprint_bytes(&mut hash, entry.field.as_bytes());
            }
        }
        match entry.placement {
            LayoutPlacementReport::At { offset } => {
                hash_fingerprint_byte(&mut hash, 0);
                hash_fingerprint_u64(&mut hash, offset);
            }
            LayoutPlacementReport::IntegerAt {
                offset,
                stored_width,
                interpretation,
            } => {
                hash_fingerprint_byte(&mut hash, 2);
                hash_fingerprint_u64(&mut hash, offset);
                hash_fingerprint_u64(&mut hash, stored_width);
                hash_fingerprint_byte(
                    &mut hash,
                    match interpretation {
                        IntegerInterpretation::Signed => 0,
                        IntegerInterpretation::Unsigned => 1,
                    },
                );
            }
            LayoutPlacementReport::Bits {
                container,
                container_width,
                destination_lsb,
                source_lsb,
                width,
            } => {
                hash_fingerprint_byte(&mut hash, 1);
                for value in [
                    container,
                    container_width,
                    destination_lsb,
                    source_lsb,
                    width,
                ] {
                    hash_fingerprint_u64(&mut hash, value);
                }
            }
        }
    }
    if hash == 0 { 1 } else { hash }
}

fn member_sort_key(entry: &LayoutFieldEntryReport) -> (u8, u64, &str) {
    match entry.member_identity {
        Some(identity) => (0, identity, ""),
        None => (1, 0, entry.field.as_str()),
    }
}

fn placement_sort_key(placement: &LayoutPlacementReport) -> (u8, u64, u64, u64, u64, u64) {
    match *placement {
        LayoutPlacementReport::At { offset } => (0, offset, 0, 0, 0, 0),
        LayoutPlacementReport::IntegerAt {
            offset,
            stored_width,
            interpretation,
        } => (
            2,
            offset,
            stored_width,
            match interpretation {
                IntegerInterpretation::Signed => 0,
                IntegerInterpretation::Unsigned => 1,
            },
            0,
            0,
        ),
        LayoutPlacementReport::Bits {
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        } => (
            1,
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        ),
    }
}

fn hash_fingerprint_u64(hash: &mut u64, value: u64) {
    hash_fingerprint_bytes(hash, &value.to_le_bytes());
}

fn hash_fingerprint_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        hash_fingerprint_byte(hash, *byte);
    }
}

fn hash_fingerprint_byte(hash: &mut u64, byte: u8) {
    *hash ^= u64::from(byte);
    *hash = hash.wrapping_mul(0x100000001b3);
}

/// One ordinary scalar supplied to a validated dictated-layout materializer.
/// The field name selects compiler-validated plan entries; callers never
/// provide a byte offset or destination bit position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarFieldValue {
    pub field: String,
    pub width_bits: u16,
    pub value: u64,
}

impl ScalarFieldValue {
    pub fn new(
        field: impl Into<String>,
        width_bits: u16,
        value: u64,
    ) -> Result<Self, MaterializationDiagnostic> {
        if width_bits == 0 || width_bits > 64 {
            return Err(MaterializationDiagnostic(format!(
                "scalar field width {width_bits} is outside 1..=64 bits"
            )));
        }
        if width_bits < 64 && value > low_mask(width_bits) {
            return Err(MaterializationDiagnostic(format!(
                "scalar value {value:#x} does not fit its {width_bits}-bit field"
            )));
        }
        Ok(Self {
            field: field.into(),
            width_bits,
            value,
        })
    }
}

/// Declared scalar shape used when decoding bytes through a validated layout.
/// The width comes from the compiler-materialized schema, not from the bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarFieldSchema {
    pub field: String,
    pub width_bits: u16,
}

impl ScalarFieldSchema {
    pub fn new(
        field: impl Into<String>,
        width_bits: u16,
    ) -> Result<Self, MaterializationDiagnostic> {
        if width_bits == 0 || width_bits > 64 {
            return Err(MaterializationDiagnostic(format!(
                "scalar field width {width_bits} is outside 1..=64 bits"
            )));
        }
        Ok(Self {
            field: field.into(),
            width_bits,
        })
    }
}

/// Compiler-issued identity of an inbound entry stub. The numeric identity is
/// never a callable address and cannot be used for arithmetic or control flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryStubId(u64);

impl EntryStubId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, MaterializationDiagnostic> {
        nonzero_identity("entry stub", identity).map(Self)
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Compiler-issued identity of statically placed data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DataSymbolId(u64);

impl DataSymbolId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, MaterializationDiagnostic> {
        nonzero_identity("data symbol", identity).map(Self)
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Closed source vocabulary for a toolchain-resolved value. Runtime-created
/// addresses remain ordinary `addr` data and do not enter this plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelocationTarget {
    Data(DataSymbolId),
    Entry(EntryStubId),
}

impl RelocationTarget {
    pub const fn normalized_identity(self) -> u64 {
        match self {
            Self::Data(identity) => identity.normalized_identity(),
            Self::Entry(identity) => identity.normalized_identity(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicFieldValue {
    pub field: String,
    pub width_bits: u16,
    pub target: RelocationTarget,
}

impl SymbolicFieldValue {
    pub fn new(
        field: impl Into<String>,
        width_bits: u16,
        target: RelocationTarget,
    ) -> Result<Self, MaterializationDiagnostic> {
        if width_bits == 0 || width_bits > 64 {
            return Err(MaterializationDiagnostic(format!(
                "symbolic field width {width_bits} is outside 1..=64 bits"
            )));
        }
        Ok(Self {
            field: field.into(),
            width_bits,
            target,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    LittleEndian,
    BigEndian,
}

/// When another party first consumes the materialized structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsumptionInstant {
    /// A loader reads the structure before the first Omega instruction. Only
    /// fixed values or relocations native to that loader are legal.
    BeforeOmegaEntry,
    /// Omega/provider code runs after the final address is known and may apply
    /// a generated writer before handing the structure to hardware/firmware.
    AfterOmegaHandoff,
}

/// Phase in which a materialized object is placed at its final address.
/// Consumption and placement are independent: a build-placed object may be
/// consumed only after handoff, while a loader-placed table may be consumed
/// before the first Omega instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlacementPhase {
    Build,
    Load,
    PostHandoff,
}

/// Closed permitted range for the complete placed object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlacementAddressRange {
    start_inclusive: u64,
    end_exclusive: u64,
}

impl PlacementAddressRange {
    pub fn new(
        start_inclusive: u64,
        end_exclusive: u64,
    ) -> Result<Self, MaterializationDiagnostic> {
        if start_inclusive >= end_exclusive {
            return Err(MaterializationDiagnostic(format!(
                "placement address range {start_inclusive:#x}..{end_exclusive:#x} is empty or reversed"
            )));
        }
        Ok(Self {
            start_inclusive,
            end_exclusive,
        })
    }

    pub const fn start_inclusive(self) -> u64 {
        self.start_inclusive
    }

    pub const fn end_exclusive(self) -> u64 {
        self.end_exclusive
    }

    fn contains(self, base_address: u64, byte_len: usize) -> bool {
        let Ok(byte_len) = u64::try_from(byte_len) else {
            return false;
        };
        base_address >= self.start_inclusive
            && base_address
                .checked_add(byte_len)
                .is_some_and(|end| end <= self.end_exclusive)
    }
}

/// Compiler-issued identity of a machine-state regime (for example, x86
/// long mode). It is a normalized policy identity, not a user-selected name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MachineRegimeId(u64);

impl MachineRegimeId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, MaterializationDiagnostic> {
        nonzero_identity("machine regime", identity).map(Self)
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Compiler-issued identity of the attenuated artifact-installation authority
/// required by a placement. This cites scope; it is not the capability value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactInstallationScopeId(u64);

impl ArtifactInstallationScopeId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, MaterializationDiagnostic> {
        nonzero_identity("artifact installation scope", identity).map(Self)
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Normalized requirements a concrete placement must satisfy. The layout's
/// own alignment is joined into this record during materialization derivation;
/// policy constraints can strengthen it but never weaken it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlacementConstraints {
    permitted_range: Option<PlacementAddressRange>,
    alignment: u64,
    phase: PlacementPhase,
    machine_regime: Option<MachineRegimeId>,
    installation_scope: Option<ArtifactInstallationScopeId>,
}

impl PlacementConstraints {
    pub fn new(
        permitted_range: Option<PlacementAddressRange>,
        alignment: u64,
        phase: PlacementPhase,
        machine_regime: Option<MachineRegimeId>,
        installation_scope: Option<ArtifactInstallationScopeId>,
    ) -> Result<Self, MaterializationDiagnostic> {
        if alignment == 0 {
            return Err(MaterializationDiagnostic(
                "placement alignment must be nonzero".into(),
            ));
        }
        Ok(Self {
            permitted_range,
            alignment,
            phase,
            machine_regime,
            installation_scope,
        })
    }

    pub const fn unconstrained(phase: PlacementPhase) -> Self {
        Self {
            permitted_range: None,
            alignment: 1,
            phase,
            machine_regime: None,
            installation_scope: None,
        }
    }

    pub const fn permitted_range(self) -> Option<PlacementAddressRange> {
        self.permitted_range
    }

    pub const fn alignment(self) -> u64 {
        self.alignment
    }

    pub const fn phase(self) -> PlacementPhase {
        self.phase
    }

    pub const fn machine_regime(self) -> Option<MachineRegimeId> {
        self.machine_regime
    }

    pub const fn installation_scope(self) -> Option<ArtifactInstallationScopeId> {
        self.installation_scope
    }

    fn joined_with_layout(
        mut self,
        layout_alignment: u64,
        byte_len: usize,
    ) -> Result<Self, MaterializationDiagnostic> {
        if layout_alignment == 0 {
            return Err(MaterializationDiagnostic(
                "layout alignment must be nonzero".into(),
            ));
        }
        self.alignment = checked_lcm(self.alignment, layout_alignment).ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "placement alignment {} and layout alignment {layout_alignment} have no representable common multiple",
                self.alignment
            ))
        })?;
        if let Some(range) = self.permitted_range {
            let range_len = range.end_exclusive - range.start_inclusive;
            let byte_len = u64::try_from(byte_len).map_err(|_| {
                MaterializationDiagnostic(
                    "materialization length cannot be represented as an address range".into(),
                )
            })?;
            if byte_len > range_len {
                return Err(MaterializationDiagnostic(format!(
                    "{}-byte materialization cannot fit in permitted range {:#x}..{:#x}",
                    byte_len, range.start_inclusive, range.end_exclusive
                )));
            }
        }
        Ok(self)
    }

    pub fn validate_site(
        self,
        byte_len: usize,
        site: PlacementSite,
    ) -> Result<(), MaterializationDiagnostic> {
        if site.phase != self.phase {
            return Err(MaterializationDiagnostic(format!(
                "placement phase {:?} does not satisfy required phase {:?}",
                site.phase, self.phase
            )));
        }
        if !site.base_address.is_multiple_of(self.alignment) {
            return Err(MaterializationDiagnostic(format!(
                "placement address {:#x} is not aligned to {} bytes",
                site.base_address, self.alignment
            )));
        }
        if let Some(range) = self.permitted_range
            && !range.contains(site.base_address, byte_len)
        {
            return Err(MaterializationDiagnostic(format!(
                "{}-byte placement at {:#x} lies outside permitted range {:#x}..{:#x}",
                byte_len, site.base_address, range.start_inclusive, range.end_exclusive
            )));
        }
        if self.machine_regime.is_some() && site.machine_regime != self.machine_regime {
            return Err(MaterializationDiagnostic(format!(
                "placement machine regime {:?} does not satisfy required regime {:?}",
                site.machine_regime, self.machine_regime
            )));
        }
        if self.installation_scope.is_some() && site.installation_scope != self.installation_scope {
            return Err(MaterializationDiagnostic(format!(
                "placement installation scope {:?} does not satisfy required scope {:?}",
                site.installation_scope, self.installation_scope
            )));
        }
        Ok(())
    }
}

/// Concrete facts known when a linker, loader, or provider chooses a final
/// address. Validation compares these facts to the normalized constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlacementSite {
    pub base_address: u64,
    pub phase: PlacementPhase,
    pub machine_regime: Option<MachineRegimeId>,
    pub installation_scope: Option<ArtifactInstallationScopeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterializationContext {
    pub consumption: ConsumptionInstant,
    pub byte_order: ByteOrder,
    /// Width accepted by the target container's native absolute relocation.
    /// `None` means no such relocation is available.
    pub native_pointer_relocation_bits: Option<u16>,
    pub placement: PlacementConstraints,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializationWrite {
    pub field: String,
    pub target: RelocationTarget,
    pub container_byte_offset: u64,
    pub container_width_bits: u16,
    pub destination_lsb: u16,
    pub source_lsb: u16,
    pub width: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterializationAction {
    /// Constant-folded when a fixed image or earlier placement pass already
    /// knows the target address.
    ResolvedWrite {
        write: MaterializationWrite,
        source_value: u64,
    },
    /// A loader-native whole-pointer relocation. Fragmented native
    /// relocations are deliberately absent from the vocabulary.
    NativePointerRelocation {
        field: String,
        target: RelocationTarget,
        destination_byte_offset: u64,
        width_bits: u16,
    },
    /// A deriver-generated post-handoff writer step. Providers resolve the
    /// target without exposing its numeric address to ordinary Omega code.
    RuntimeWriter(MaterializationWrite),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostHandoffWriterSource {
    Resolved(u64),
    Resolve(RelocationTarget),
}

/// `PHWRITR1`: target-neutral packed input ABI for a reusable post-handoff
/// fragment. Word zero is the destination address. The remaining words are
/// dense source slots assigned by the fragment plan. Numeric words stay inside
/// provider-owned invocation evidence; source code sees neither this context
/// nor an address-valued writer operation.
pub const POST_HANDOFF_WRITER_CONTEXT_ABI_V1: u64 = 0x5048_5752_4954_5231;
pub const POST_HANDOFF_WRITER_DESTINATION_OFFSET: usize = 0;
pub const POST_HANDOFF_WRITER_SOURCE_SLOTS_OFFSET: usize = 8;
pub const POST_HANDOFF_WRITER_SOURCE_SLOT_WIDTH: usize = 8;

pub fn post_handoff_writer_context_byte_len(source_slot_count: usize) -> Option<usize> {
    source_slot_count
        .checked_mul(POST_HANDOFF_WRITER_SOURCE_SLOT_WIDTH)?
        .checked_add(POST_HANDOFF_WRITER_SOURCE_SLOTS_OFFSET)
}

/// One address-free fragment in a reusable generated writer. `source_slot`
/// indexes the provider-private invocation context. Field names, symbolic
/// identities, resolved values, and concrete placement are deliberately
/// absent: none changes the emitted transfer geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratedPostHandoffWriterStep {
    pub container_byte_offset: u64,
    pub container_width_bits: u16,
    pub destination_lsb: u16,
    pub source_lsb: u16,
    pub width: u16,
    pub source_slot: usize,
}

/// Static normalized plan for one reusable post-handoff fragment.
///
/// Its fingerprint covers only facts that can change emitted code. Exact
/// relocation targets, resolved content, placement, resolver authority, and
/// roots belong to `PostHandoffWriterInvocationPlan` instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedPostHandoffWriterFragmentPlan {
    context_abi: u64,
    byte_len: usize,
    byte_order: ByteOrder,
    source_slot_count: usize,
    steps: Vec<GeneratedPostHandoffWriterStep>,
    fingerprint: u64,
}

impl GeneratedPostHandoffWriterFragmentPlan {
    pub const fn context_abi(&self) -> u64 {
        self.context_abi
    }

    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    pub const fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }

    pub const fn source_slot_count(&self) -> usize {
        self.source_slot_count
    }

    pub fn steps(&self) -> &[GeneratedPostHandoffWriterStep] {
        &self.steps
    }

    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
}

/// Exact value bound to one private source slot for one invocation. The target
/// remains present even for a pre-resolved value so fragmented writes group by
/// compiler-issued source identity rather than accidentally by equal numeric
/// content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostHandoffWriterSourceSlot {
    pub target: RelocationTarget,
    pub source: PostHandoffWriterSource,
}

/// Invocation-sensitive half of generated writer lowering. This evidence is
/// intentionally separate from the reusable fragment identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostHandoffWriterInvocationPlan {
    pub fragment: GeneratedPostHandoffWriterFragmentPlan,
    pub placement: PlacementConstraints,
    pub sources: Vec<PostHandoffWriterSourceSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostHandoffWriterStep {
    pub write: MaterializationWrite,
    pub source: PostHandoffWriterSource,
}

/// Provider-consumable writer program derived from symbolic materialization
/// actions. It contains no source-callable address operation: only the
/// provider resolver may turn a sealed relocation target into address bits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostHandoffWriterPlan {
    pub byte_len: usize,
    pub byte_order: ByteOrder,
    pub placement: PlacementConstraints,
    pub steps: Vec<PostHandoffWriterStep>,
}

impl PostHandoffWriterPlan {
    /// Derive one reusable address-free fragment plus exact invocation evidence.
    /// Source slots follow first target occurrence and are dense. Repeated
    /// fragments of one symbolic target therefore consume one once-resolved
    /// word even when the concrete address or artifact realization changes.
    pub fn lower_reusable_fragment(
        &self,
    ) -> Result<PostHandoffWriterInvocationPlan, MaterializationDiagnostic> {
        if self.steps.is_empty() {
            return Err(MaterializationDiagnostic(
                "post-handoff writer requires at least one fragment".into(),
            ));
        }

        let mut sources = Vec::<PostHandoffWriterSourceSlot>::new();
        let mut steps = Vec::with_capacity(self.steps.len());
        for step in &self.steps {
            if let PostHandoffWriterSource::Resolve(target) = step.source
                && target != step.write.target
            {
                return Err(MaterializationDiagnostic(format!(
                    "post-handoff writer source {target:?} does not match write target {:?}",
                    step.write.target
                )));
            }
            validate_write(self.byte_len, &step.write)?;

            let source_slot = if let Some(index) = sources
                .iter()
                .position(|source| source.target == step.write.target)
            {
                if sources[index].source != step.source {
                    return Err(MaterializationDiagnostic(format!(
                        "post-handoff writer target {:?} has inconsistent invocation values across fragments",
                        step.write.target
                    )));
                }
                index
            } else {
                let index = sources.len();
                sources.push(PostHandoffWriterSourceSlot {
                    target: step.write.target,
                    source: step.source,
                });
                index
            };
            steps.push(GeneratedPostHandoffWriterStep {
                container_byte_offset: step.write.container_byte_offset,
                container_width_bits: step.write.container_width_bits,
                destination_lsb: step.write.destination_lsb,
                source_lsb: step.write.source_lsb,
                width: step.write.width,
                source_slot,
            });
        }

        let fragment = GeneratedPostHandoffWriterFragmentPlan {
            context_abi: POST_HANDOFF_WRITER_CONTEXT_ABI_V1,
            byte_len: self.byte_len,
            byte_order: self.byte_order,
            source_slot_count: sources.len(),
            fingerprint: generated_post_handoff_writer_fingerprint(
                self.byte_len,
                self.byte_order,
                sources.len(),
                &steps,
            ),
            steps,
        };
        Ok(PostHandoffWriterInvocationPlan {
            fragment,
            placement: self.placement,
            sources,
        })
    }

    /// Validate the complete direct-destination program without resolving a
    /// symbolic address or mutating the destination. Provider preparation
    /// uses this pass to bind one exact checked writer before any numeric
    /// entry address exists outside the sealed resolver.
    pub fn validate(
        &self,
        destination_len: usize,
        site: PlacementSite,
    ) -> Result<(), MaterializationDiagnostic> {
        self.placement.validate_site(self.byte_len, site)?;
        if destination_len < self.byte_len {
            return Err(MaterializationDiagnostic(format!(
                "post-handoff writer needs {} bytes, destination has {}",
                self.byte_len, destination_len
            )));
        }
        let mut sources = Vec::<PostHandoffWriterSourceSlot>::new();
        for step in &self.steps {
            if let PostHandoffWriterSource::Resolve(target) = step.source
                && target != step.write.target
            {
                return Err(MaterializationDiagnostic(format!(
                    "post-handoff writer source {target:?} does not match write target {:?}",
                    step.write.target
                )));
            }
            validate_write(self.byte_len, &step.write)?;
            if let Some(source) = sources
                .iter()
                .find(|source| source.target == step.write.target)
            {
                if source.source != step.source {
                    return Err(MaterializationDiagnostic(format!(
                        "post-handoff writer target {:?} has inconsistent invocation values across fragments",
                        step.write.target
                    )));
                }
            } else {
                sources.push(PostHandoffWriterSourceSlot {
                    target: step.write.target,
                    source: step.source,
                });
            }
        }
        Ok(())
    }

    /// Validates the concrete placement and every write, resolves every target,
    /// then writes directly into the unpublished destination. Repeated
    /// fragments of one target resolve once so a provider cannot observe
    /// inconsistent address values within one materialization. Failure before
    /// the write loop leaves bytes unchanged; a provider must keep the
    /// destination unpublished on any later failure.
    pub fn execute(
        &self,
        destination: &mut [u8],
        site: PlacementSite,
        mut resolve: impl FnMut(RelocationTarget) -> Option<u64>,
    ) -> Result<(), MaterializationDiagnostic> {
        self.validate(destination.len(), site)?;

        let mut resolved_targets = std::collections::BTreeMap::new();
        let mut values = Vec::with_capacity(self.steps.len());
        for step in &self.steps {
            let value = match step.source {
                PostHandoffWriterSource::Resolved(value) => value,
                PostHandoffWriterSource::Resolve(target) => {
                    if let Some(value) = resolved_targets.get(&target) {
                        *value
                    } else {
                        let value = resolve(target).ok_or_else(|| {
                            MaterializationDiagnostic(format!(
                                "post-handoff writer could not resolve symbolic target {target:?}"
                            ))
                        })?;
                        resolved_targets.insert(target, value);
                        value
                    }
                }
            };
            values.push(value);
        }

        for (step, value) in self.steps.iter().zip(values) {
            apply_write(
                &mut destination[..self.byte_len],
                self.byte_order,
                &step.write,
                value,
            )?;
        }
        Ok(())
    }
}

fn generated_post_handoff_writer_fingerprint(
    byte_len: usize,
    byte_order: ByteOrder,
    source_slot_count: usize,
    steps: &[GeneratedPostHandoffWriterStep],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_fingerprint_bytes(&mut hash, b"omega.post-handoff-writer.v1");
    hash_fingerprint_u64(&mut hash, POST_HANDOFF_WRITER_CONTEXT_ABI_V1);
    hash_fingerprint_u64(&mut hash, byte_len as u64);
    hash_fingerprint_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_fingerprint_u64(&mut hash, source_slot_count as u64);
    hash_fingerprint_u64(&mut hash, steps.len() as u64);
    for step in steps {
        for value in [
            step.container_byte_offset,
            u64::from(step.container_width_bits),
            u64::from(step.destination_lsb),
            u64::from(step.source_lsb),
            u64::from(step.width),
            step.source_slot as u64,
        ] {
            hash_fingerprint_u64(&mut hash, value);
        }
    }
    if hash == 0 { 1 } else { hash }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicMaterializationPlan {
    pub byte_len: usize,
    pub byte_order: ByteOrder,
    pub placement: PlacementConstraints,
    pub actions: Vec<MaterializationAction>,
}

impl SymbolicMaterializationPlan {
    pub fn derive_post_handoff_writer(
        &self,
    ) -> Result<PostHandoffWriterPlan, MaterializationDiagnostic> {
        let mut steps = Vec::with_capacity(self.actions.len());
        for action in &self.actions {
            let step = match action {
                MaterializationAction::ResolvedWrite {
                    write,
                    source_value,
                } => PostHandoffWriterStep {
                    write: write.clone(),
                    source: PostHandoffWriterSource::Resolved(*source_value),
                },
                MaterializationAction::RuntimeWriter(write) => PostHandoffWriterStep {
                    write: write.clone(),
                    source: PostHandoffWriterSource::Resolve(write.target),
                },
                MaterializationAction::NativePointerRelocation { .. } => {
                    return Err(MaterializationDiagnostic(
                        "loader-native relocation cannot enter a post-handoff writer program"
                            .into(),
                    ));
                }
            };
            steps.push(step);
        }
        Ok(PostHandoffWriterPlan {
            byte_len: self.byte_len,
            byte_order: self.byte_order,
            placement: self.placement,
            steps,
        })
    }

    /// Applies a fully resolved plan atomically with respect to the destination
    /// slice: unresolved actions reject before any output byte changes.
    pub fn materialize_resolved_into(
        &self,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        if destination.len() < self.byte_len {
            return Err(MaterializationDiagnostic(format!(
                "materialization needs {} bytes, destination has {}",
                self.byte_len,
                destination.len()
            )));
        }
        if let Some(action) = self
            .actions
            .iter()
            .find(|action| !matches!(action, MaterializationAction::ResolvedWrite { .. }))
        {
            return Err(MaterializationDiagnostic(format!(
                "materialization still contains an unresolved action: {action:?}"
            )));
        }

        let mut staged = destination[..self.byte_len].to_vec();
        for action in &self.actions {
            let MaterializationAction::ResolvedWrite {
                write,
                source_value,
            } = action
            else {
                unreachable!("unresolved actions were rejected above")
            };
            apply_write(&mut staged, self.byte_order, write, *source_value)?;
        }
        destination[..self.byte_len].copy_from_slice(&staged);
        Ok(())
    }
}

/// Materializes one complete ordinary scalar value through validated layout
/// entries. Output starts from zero so padding and reserved bits stay
/// deterministic. The destination is changed only after every field and
/// fragment has validated.
///
/// This is the numeric sibling of symbolic materialization. It cannot resolve
/// code/data symbols, install hardware state, or mint authority; it only turns
/// named scalar values into bytes according to an already validated plan.
pub fn materialize_scalar_layout_into(
    layout: &LayoutPlanReport,
    values: &[ScalarFieldValue],
    byte_order: ByteOrder,
    destination: &mut [u8],
) -> Result<(), MaterializationDiagnostic> {
    let byte_len = layout
        .size
        .ok_or_else(|| {
            MaterializationDiagnostic(
                "scalar materialization requires a fixed-size layout plan".into(),
            )
        })
        .and_then(|size| {
            usize::try_from(size).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "fixed layout size {size} cannot be represented on this compiler host"
                ))
            })
        })?;
    if destination.len() < byte_len {
        return Err(MaterializationDiagnostic(format!(
            "scalar materialization needs {byte_len} bytes, destination has {}",
            destination.len()
        )));
    }

    let mut supplied = std::collections::BTreeMap::new();
    for value in values {
        if supplied.insert(value.field.as_str(), value).is_some() {
            return Err(MaterializationDiagnostic(format!(
                "scalar field `{}` is supplied more than once",
                value.field
            )));
        }
    }

    let planned = layout
        .entries
        .iter()
        .map(|entry| entry.field.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if let Some(field) = planned.iter().find(|field| !supplied.contains_key(**field)) {
        return Err(MaterializationDiagnostic(format!(
            "layout field `{field}` has no supplied scalar value"
        )));
    }
    if let Some(field) = supplied.keys().find(|field| !planned.contains(**field)) {
        return Err(MaterializationDiagnostic(format!(
            "supplied scalar field `{field}` has no entry in the validated layout plan"
        )));
    }

    let mut staged = vec![0_u8; byte_len];
    for entry in &layout.entries {
        let value = supplied
            .get(entry.field.as_str())
            .expect("complete field set validated above");
        apply_scalar_entry(&mut staged, byte_order, entry, value)?;
    }
    destination[..byte_len].copy_from_slice(&staged);
    Ok(())
}

/// Decodes one complete fixed scalar layout without establishing any semantic
/// domain or authority fact. Callers receive ordinary named values; a separate
/// validator decides whether those values establish an imported-table claim.
pub fn decode_scalar_layout(
    layout: &LayoutPlanReport,
    fields: &[ScalarFieldSchema],
    byte_order: ByteOrder,
    source: &[u8],
) -> Result<Vec<ScalarFieldValue>, MaterializationDiagnostic> {
    let byte_len = layout
        .size
        .ok_or_else(|| {
            MaterializationDiagnostic("scalar decoding requires a fixed-size layout plan".into())
        })
        .and_then(|size| {
            usize::try_from(size).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "fixed layout size {size} cannot be represented on this compiler host"
                ))
            })
        })?;
    if source.len() < byte_len {
        return Err(MaterializationDiagnostic(format!(
            "scalar decoding needs {byte_len} bytes, source has {}",
            source.len()
        )));
    }

    let mut decoded = std::collections::BTreeMap::new();
    for field in fields {
        if decoded
            .insert(field.field.as_str(), (field.width_bits, 0_u64, 0_u64))
            .is_some()
        {
            return Err(MaterializationDiagnostic(format!(
                "scalar field `{}` is declared more than once",
                field.field
            )));
        }
    }
    let planned = layout
        .entries
        .iter()
        .map(|entry| entry.field.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if let Some(field) = planned.iter().find(|field| !decoded.contains_key(**field)) {
        return Err(MaterializationDiagnostic(format!(
            "layout field `{field}` has no scalar decode schema"
        )));
    }
    if let Some(field) = decoded.keys().find(|field| !planned.contains(**field)) {
        return Err(MaterializationDiagnostic(format!(
            "scalar decode field `{field}` has no entry in the validated layout plan"
        )));
    }

    for entry in &layout.entries {
        let (width_bits, value, covered) = decoded
            .get_mut(entry.field.as_str())
            .expect("complete field set validated above");
        let fragment = scalar_fragment(entry, *width_bits)?;
        validate_fragment(
            byte_len,
            &entry.field,
            fragment.container_byte_offset,
            fragment.container_width_bits,
            fragment.destination_lsb,
            fragment.source_lsb,
            fragment.width,
        )?;
        let source_mask = low_mask(fragment.width) << fragment.source_lsb;
        if *covered & source_mask != 0 {
            return Err(MaterializationDiagnostic(format!(
                "scalar field `{}` decode fragments overlap in the logical source",
                entry.field
            )));
        }
        let container_bytes = usize::from(fragment.container_width_bits / 8);
        let start = usize::try_from(fragment.container_byte_offset).map_err(|_| {
            MaterializationDiagnostic("container offset cannot be represented on this host".into())
        })?;
        let end = start
            .checked_add(container_bytes)
            .ok_or_else(|| MaterializationDiagnostic("container byte range overflows".into()))?;
        let container = read_container(&source[start..end], byte_order);
        let value_fragment = (container >> fragment.destination_lsb) & low_mask(fragment.width);
        *value |= value_fragment << fragment.source_lsb;
        *covered |= source_mask;
    }

    decoded
        .into_iter()
        .map(|(field, (width_bits, value, covered))| {
            if covered != low_mask(width_bits) {
                return Err(MaterializationDiagnostic(format!(
                    "scalar field `{field}` decode fragments do not tile its complete {width_bits}-bit source"
                )));
            }
            Ok(ScalarFieldValue {
                field: field.into(),
                width_bits,
                value,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializationDiagnostic(pub String);

impl std::fmt::Display for MaterializationDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for MaterializationDiagnostic {}

/// Derives a phase-aware consumer plan. `resolve` is compiler/provider
/// infrastructure; source code never receives its returned address.
pub fn derive_symbolic_materialization(
    layout: &LayoutPlanReport,
    symbolic_fields: &[SymbolicFieldValue],
    context: MaterializationContext,
    mut resolve: impl FnMut(RelocationTarget) -> Option<u64>,
) -> Result<SymbolicMaterializationPlan, MaterializationDiagnostic> {
    let byte_len = layout
        .size
        .ok_or_else(|| {
            MaterializationDiagnostic(
                "symbolic materialization requires a fixed-size layout plan".into(),
            )
        })
        .and_then(|size| {
            usize::try_from(size).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "fixed layout size {size} cannot be represented on this compiler host"
                ))
            })
        })?;
    let placement = context
        .placement
        .joined_with_layout(layout.align, byte_len)?;

    let mut names = std::collections::BTreeSet::new();
    let mut actions = Vec::new();
    for symbolic in symbolic_fields {
        if !names.insert(symbolic.field.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "symbolic field `{}` is supplied more than once",
                symbolic.field
            )));
        }
        let entries = layout
            .entries
            .iter()
            .filter(|entry| entry.field == symbolic.field)
            .collect::<Vec<_>>();
        if entries.is_empty() {
            return Err(MaterializationDiagnostic(format!(
                "symbolic field `{}` has no entry in the validated layout plan",
                symbolic.field
            )));
        }

        let resolved = resolve(symbolic.target);
        for entry in entries {
            let write = write_from_entry(entry, symbolic)?;
            let action = match resolved {
                Some(source_value) => MaterializationAction::ResolvedWrite {
                    write,
                    source_value,
                },
                None if context.consumption == ConsumptionInstant::AfterOmegaHandoff => {
                    MaterializationAction::RuntimeWriter(write)
                }
                None => match entry.placement {
                    LayoutPlacementReport::At { .. }
                        if context.native_pointer_relocation_bits == Some(symbolic.width_bits) =>
                    {
                        MaterializationAction::NativePointerRelocation {
                            field: symbolic.field.clone(),
                            target: symbolic.target,
                            destination_byte_offset: write.container_byte_offset,
                            width_bits: symbolic.width_bits,
                        }
                    }
                    LayoutPlacementReport::At { .. } => {
                        return Err(MaterializationDiagnostic(format!(
                            "loader consumes symbolic field `{}` before Omega entry, but the target has no native {}-bit pointer relocation",
                            symbolic.field, symbolic.width_bits
                        )));
                    }
                    LayoutPlacementReport::IntegerAt { .. } => {
                        return Err(MaterializationDiagnostic(format!(
                            "loader consumes stored-integer field `{}` before Omega entry; symbolic materialization has no integer fit proof",
                            symbolic.field
                        )));
                    }
                    LayoutPlacementReport::Bits { .. } => {
                        return Err(MaterializationDiagnostic(format!(
                            "loader consumes fragmented symbolic field `{}` before Omega entry; unresolved fragments require a fixed address or a post-handoff writer",
                            symbolic.field
                        )));
                    }
                },
            };
            actions.push(action);
        }
    }

    Ok(SymbolicMaterializationPlan {
        byte_len,
        byte_order: context.byte_order,
        placement,
        actions,
    })
}

fn write_from_entry(
    entry: &LayoutFieldEntryReport,
    symbolic: &SymbolicFieldValue,
) -> Result<MaterializationWrite, MaterializationDiagnostic> {
    let (container, container_width, destination_lsb, source_lsb, width) = match entry.placement {
        LayoutPlacementReport::At { offset } => (
            offset,
            u64::from(symbolic.width_bits),
            0,
            0,
            u64::from(symbolic.width_bits),
        ),
        LayoutPlacementReport::IntegerAt { .. } => {
            return Err(MaterializationDiagnostic(format!(
                "symbolic field `{}` uses stored-integer placement without a concrete fit proof",
                symbolic.field
            )));
        }
        LayoutPlacementReport::Bits {
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        } => (
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        ),
    };
    if container_width == 0 || container_width > 64 || container_width % 8 != 0 || width == 0 {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{}` uses a materializer-incompatible placement",
            symbolic.field
        )));
    }
    let source_end = source_lsb
        .checked_add(width)
        .ok_or_else(|| MaterializationDiagnostic("symbolic source bit range overflows".into()))?;
    if source_end > u64::from(symbolic.width_bits) {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{}` placement reads through bit {source_end}, past its {}-bit source",
            symbolic.field, symbolic.width_bits
        )));
    }
    let destination_end = destination_lsb.checked_add(width).ok_or_else(|| {
        MaterializationDiagnostic("symbolic destination bit range overflows".into())
    })?;
    if destination_end > container_width {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{}` placement writes through bit {destination_end}, past its {container_width}-bit container",
            symbolic.field
        )));
    }
    Ok(MaterializationWrite {
        field: symbolic.field.clone(),
        target: symbolic.target,
        container_byte_offset: container,
        container_width_bits: u16::try_from(container_width)
            .expect("validated materializer container width"),
        destination_lsb: u16::try_from(destination_lsb).expect("validated destination bit index"),
        source_lsb: u16::try_from(source_lsb).expect("validated source bit index"),
        width: u16::try_from(width).map_err(|_| {
            MaterializationDiagnostic(format!(
                "symbolic field `{}` fragment width {width} is too large",
                symbolic.field
            ))
        })?,
    })
}

fn apply_scalar_entry(
    bytes: &mut [u8],
    byte_order: ByteOrder,
    entry: &LayoutFieldEntryReport,
    value: &ScalarFieldValue,
) -> Result<(), MaterializationDiagnostic> {
    let fragment = scalar_fragment(entry, value.width_bits)?;
    validate_fragment(
        bytes.len(),
        &value.field,
        fragment.container_byte_offset,
        fragment.container_width_bits,
        fragment.destination_lsb,
        fragment.source_lsb,
        fragment.width,
    )?;
    apply_fragment(
        bytes,
        byte_order,
        &value.field,
        fragment.container_byte_offset,
        fragment.container_width_bits,
        fragment.destination_lsb,
        fragment.source_lsb,
        fragment.width,
        value.value,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScalarFragment {
    container_byte_offset: u64,
    container_width_bits: u16,
    destination_lsb: u16,
    source_lsb: u16,
    width: u16,
}

fn scalar_fragment(
    entry: &LayoutFieldEntryReport,
    source_width_bits: u16,
) -> Result<ScalarFragment, MaterializationDiagnostic> {
    let (container, container_width, destination_lsb, source_lsb, width) = match entry.placement {
        LayoutPlacementReport::At { offset } => (
            offset,
            u64::from(source_width_bits),
            0,
            0,
            u64::from(source_width_bits),
        ),
        LayoutPlacementReport::IntegerAt { .. } => {
            return Err(MaterializationDiagnostic(format!(
                "scalar field `{}` uses stored-integer placement without a concrete fit proof",
                entry.field
            )));
        }
        LayoutPlacementReport::Bits {
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        } => (
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        ),
    };
    if container_width == 0 || container_width > 64 || container_width % 8 != 0 || width == 0 {
        return Err(MaterializationDiagnostic(format!(
            "scalar field `{}` uses a materializer-incompatible placement",
            entry.field
        )));
    }
    let source_end = source_lsb
        .checked_add(width)
        .ok_or_else(|| MaterializationDiagnostic("scalar source bit range overflows".into()))?;
    if source_end > u64::from(source_width_bits) {
        return Err(MaterializationDiagnostic(format!(
            "scalar field `{}` placement reads through bit {source_end}, past its {}-bit source",
            entry.field, source_width_bits
        )));
    }
    let destination_end = destination_lsb.checked_add(width).ok_or_else(|| {
        MaterializationDiagnostic("scalar destination bit range overflows".into())
    })?;
    if destination_end > container_width {
        return Err(MaterializationDiagnostic(format!(
            "scalar field `{}` placement writes through bit {destination_end}, past its {container_width}-bit container",
            entry.field
        )));
    }

    Ok(ScalarFragment {
        container_byte_offset: container,
        container_width_bits: u16::try_from(container_width)
            .expect("validated materializer container width"),
        destination_lsb: u16::try_from(destination_lsb).expect("validated destination bit index"),
        source_lsb: u16::try_from(source_lsb).expect("validated source bit index"),
        width: u16::try_from(width).map_err(|_| {
            MaterializationDiagnostic(format!(
                "scalar field `{}` fragment width {width} is too large",
                entry.field
            ))
        })?,
    })
}

fn apply_write(
    bytes: &mut [u8],
    byte_order: ByteOrder,
    write: &MaterializationWrite,
    source_value: u64,
) -> Result<(), MaterializationDiagnostic> {
    apply_fragment(
        bytes,
        byte_order,
        &write.field,
        write.container_byte_offset,
        write.container_width_bits,
        write.destination_lsb,
        write.source_lsb,
        write.width,
        source_value,
    )
}

#[allow(clippy::too_many_arguments)]
fn apply_fragment(
    bytes: &mut [u8],
    byte_order: ByteOrder,
    field: &str,
    container_byte_offset: u64,
    container_width_bits: u16,
    destination_lsb: u16,
    source_lsb: u16,
    width: u16,
    source_value: u64,
) -> Result<(), MaterializationDiagnostic> {
    let container_bytes = usize::from(container_width_bits / 8);
    let start = usize::try_from(container_byte_offset).map_err(|_| {
        MaterializationDiagnostic("container offset cannot be represented on this host".into())
    })?;
    let end = start
        .checked_add(container_bytes)
        .ok_or_else(|| MaterializationDiagnostic("container byte range overflows".into()))?;
    let materialization_len = bytes.len();
    let container_slice = bytes.get_mut(start..end).ok_or_else(|| {
        MaterializationDiagnostic(format!(
            "symbolic field `{}` writes outside the {}-byte materialization",
            field, materialization_len
        ))
    })?;
    let mut container_value = read_container(container_slice, byte_order);
    let fragment_mask = low_mask(width);
    let fragment = (source_value >> source_lsb) & fragment_mask;
    let destination_mask = fragment_mask << destination_lsb;
    container_value =
        (container_value & !destination_mask) | ((fragment << destination_lsb) & destination_mask);
    write_container(container_slice, byte_order, container_value);
    Ok(())
}

fn validate_write(
    materialization_len: usize,
    write: &MaterializationWrite,
) -> Result<(), MaterializationDiagnostic> {
    validate_fragment(
        materialization_len,
        &write.field,
        write.container_byte_offset,
        write.container_width_bits,
        write.destination_lsb,
        write.source_lsb,
        write.width,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_fragment(
    materialization_len: usize,
    field: &str,
    container_byte_offset: u64,
    container_width_bits: u16,
    destination_lsb: u16,
    source_lsb: u16,
    width: u16,
) -> Result<(), MaterializationDiagnostic> {
    if container_width_bits == 0
        || container_width_bits > 64
        || !container_width_bits.is_multiple_of(8)
    {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{}` has invalid {}-bit container",
            field, container_width_bits
        )));
    }
    if width == 0
        || width > 64
        || source_lsb.checked_add(width).is_none_or(|end| end > 64)
        || destination_lsb
            .checked_add(width)
            .is_none_or(|end| end > container_width_bits)
    {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{}` has an invalid source or destination bit range",
            field
        )));
    }
    let start = usize::try_from(container_byte_offset).map_err(|_| {
        MaterializationDiagnostic("container offset cannot be represented on this host".into())
    })?;
    let end = start
        .checked_add(usize::from(container_width_bits / 8))
        .ok_or_else(|| MaterializationDiagnostic("container byte range overflows".into()))?;
    if end > materialization_len {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{}` writes outside the {}-byte materialization",
            field, materialization_len
        )));
    }
    Ok(())
}

fn read_container(bytes: &[u8], byte_order: ByteOrder) -> u64 {
    bytes
        .iter()
        .enumerate()
        .fold(0_u64, |value, (index, byte)| {
            let shift = match byte_order {
                ByteOrder::LittleEndian => index * 8,
                ByteOrder::BigEndian => (bytes.len() - 1 - index) * 8,
            };
            value | (u64::from(*byte) << shift)
        })
}

fn write_container(bytes: &mut [u8], byte_order: ByteOrder, value: u64) {
    let byte_len = bytes.len();
    for (index, byte) in bytes.iter_mut().enumerate() {
        let shift = match byte_order {
            ByteOrder::LittleEndian => index * 8,
            ByteOrder::BigEndian => (byte_len - 1 - index) * 8,
        };
        *byte = ((value >> shift) & 0xff) as u8;
    }
}

const fn low_mask(width: u16) -> u64 {
    if width == 64 {
        u64::MAX
    } else {
        (1_u64 << width) - 1
    }
}

const fn checked_lcm(left: u64, right: u64) -> Option<u64> {
    let divisor = gcd(left, right);
    (left / divisor).checked_mul(right)
}

const fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn nonzero_identity(kind: &str, identity: u64) -> Result<u64, MaterializationDiagnostic> {
    if identity == 0 {
        Err(MaterializationDiagnostic(format!(
            "normalized {kind} identity cannot be zero"
        )))
    } else {
        Ok(identity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> RelocationTarget {
        RelocationTarget::Entry(
            EntryStubId::from_normalized_identity(0x55aa).expect("nonzero identity"),
        )
    }

    fn split_layout() -> LayoutPlanReport {
        LayoutPlanReport {
            schema_identity: 1,
            entries: vec![
                LayoutFieldEntryReport {
                    field: "address".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::Bits {
                        container: 0,
                        container_width: 16,
                        destination_lsb: 0,
                        source_lsb: 0,
                        width: 16,
                    },
                },
                LayoutFieldEntryReport {
                    field: "address".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::Bits {
                        container: 2,
                        container_width: 16,
                        destination_lsb: 0,
                        source_lsb: 16,
                        width: 16,
                    },
                },
                LayoutFieldEntryReport {
                    field: "address".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::Bits {
                        container: 8,
                        container_width: 64,
                        destination_lsb: 0,
                        source_lsb: 32,
                        width: 32,
                    },
                },
            ],
            offsets: None,
            size: Some(16),
            align: 1,
        }
    }

    #[test]
    fn normalized_layout_identity_is_order_independent_and_geometry_bound() {
        let forward = split_layout();
        let mut reversed = forward.clone();
        reversed.entries.reverse();
        assert_eq!(
            normalized_layout_plan_fingerprint(&forward),
            normalized_layout_plan_fingerprint(&reversed)
        );

        let mut shifted = forward.clone();
        let LayoutPlacementReport::Bits { container, .. } = &mut shifted.entries[0].placement
        else {
            unreachable!("split layout uses bit fragments")
        };
        *container = 4;
        assert_ne!(
            normalized_layout_plan_fingerprint(&forward),
            normalized_layout_plan_fingerprint(&shifted)
        );
    }

    #[test]
    fn stable_member_identity_makes_source_rename_presentation_only() {
        let mut original = split_layout();
        original.schema_identity = 0x44;
        for entry in &mut original.entries {
            entry.member_identity = Some(7);
        }
        let mut renamed = original.clone();
        for entry in &mut renamed.entries {
            entry.field = "renamed_address".into();
        }
        assert_eq!(
            normalized_layout_plan_fingerprint(&original),
            normalized_layout_plan_fingerprint(&renamed)
        );

        let mut changed_schema = renamed;
        changed_schema.schema_identity = 0x45;
        assert_ne!(
            normalized_layout_plan_fingerprint(&original),
            normalized_layout_plan_fingerprint(&changed_schema)
        );
    }

    #[test]
    fn normalized_layout_identity_distinguishes_dynamic_from_full_width_size() {
        let dynamic = LayoutPlanReport {
            schema_identity: 1,
            entries: Vec::new(),
            offsets: Some(Vec::new()),
            size: None,
            align: 1,
        };
        let fixed = LayoutPlanReport {
            size: Some(u64::MAX),
            ..dynamic.clone()
        };

        assert_ne!(
            normalized_layout_plan_fingerprint(&dynamic),
            normalized_layout_plan_fingerprint(&fixed)
        );
    }

    #[test]
    fn ordinary_scalar_materializer_packs_a_fragmented_control_word() {
        let layout = LayoutPlanReport {
            schema_identity: 1,
            entries: vec![
                LayoutFieldEntryReport {
                    field: "enabled".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::Bits {
                        container: 0,
                        container_width: 64,
                        destination_lsb: 0,
                        source_lsb: 0,
                        width: 1,
                    },
                },
                LayoutFieldEntryReport {
                    field: "mode".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::Bits {
                        container: 0,
                        container_width: 64,
                        destination_lsb: 1,
                        source_lsb: 0,
                        width: 1,
                    },
                },
                LayoutFieldEntryReport {
                    field: "payload".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::Bits {
                        container: 0,
                        container_width: 64,
                        destination_lsb: 12,
                        source_lsb: 0,
                        width: 40,
                    },
                },
                LayoutFieldEntryReport {
                    field: "high_guard".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::Bits {
                        container: 0,
                        container_width: 64,
                        destination_lsb: 63,
                        source_lsb: 0,
                        width: 1,
                    },
                },
            ],
            offsets: None,
            size: Some(8),
            align: 8,
        };
        let values = [
            ScalarFieldValue::new("enabled", 1, 1).expect("enabled"),
            ScalarFieldValue::new("mode", 1, 1).expect("mode"),
            ScalarFieldValue::new("payload", 40, 0x12345).expect("payload"),
            ScalarFieldValue::new("high_guard", 1, 1).expect("high guard"),
        ];
        let mut bytes = [0xa5_u8; 8];
        materialize_scalar_layout_into(&layout, &values, ByteOrder::LittleEndian, &mut bytes)
            .expect("validated scalar layout materializes");

        assert_eq!(
            u64::from_le_bytes(bytes),
            (1_u64 << 63) | (0x12345_u64 << 12) | 0b11
        );

        let decoded = decode_scalar_layout(
            &layout,
            &[
                ScalarFieldSchema::new("enabled", 1).expect("enabled"),
                ScalarFieldSchema::new("mode", 1).expect("mode"),
                ScalarFieldSchema::new("payload", 40).expect("payload"),
                ScalarFieldSchema::new("high_guard", 1).expect("high guard"),
            ],
            ByteOrder::LittleEndian,
            &bytes,
        )
        .expect("the same plan decodes the materialized bytes");
        let values = decoded
            .iter()
            .map(|field| (field.field.as_str(), field.value))
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(values["enabled"], 1);
        assert_eq!(values["mode"], 1);
        assert_eq!(values["payload"], 0x12345);
        assert_eq!(values["high_guard"], 1);
    }

    #[test]
    fn scalar_materialization_is_complete_and_atomic() {
        let layout = LayoutPlanReport {
            schema_identity: 1,
            entries: vec![
                LayoutFieldEntryReport {
                    field: "low".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::Bits {
                        container: 0,
                        container_width: 8,
                        destination_lsb: 0,
                        source_lsb: 0,
                        width: 4,
                    },
                },
                LayoutFieldEntryReport {
                    field: "high".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::Bits {
                        container: 0,
                        container_width: 8,
                        destination_lsb: 4,
                        source_lsb: 0,
                        width: 4,
                    },
                },
            ],
            offsets: None,
            size: Some(1),
            align: 1,
        };
        let mut bytes = [0xa5_u8];
        let error = materialize_scalar_layout_into(
            &layout,
            &[ScalarFieldValue::new("low", 4, 3).expect("low")],
            ByteOrder::LittleEndian,
            &mut bytes,
        )
        .expect_err("missing planned fields reject");
        assert!(error.0.contains("`high`"));
        assert_eq!(bytes, [0xa5]);

        let duplicate = ScalarFieldValue::new("low", 4, 3).expect("duplicate");
        let error = materialize_scalar_layout_into(
            &layout,
            &[duplicate.clone(), duplicate],
            ByteOrder::LittleEndian,
            &mut bytes,
        )
        .expect_err("duplicate supplied fields reject");
        assert!(error.0.contains("more than once"));
        assert_eq!(bytes, [0xa5]);

        let error = decode_scalar_layout(
            &layout,
            &[ScalarFieldSchema::new("low", 4).expect("low")],
            ByteOrder::LittleEndian,
            &bytes,
        )
        .expect_err("an imported scan also requires the complete schema");
        assert!(error.0.contains("`high`"));
    }

    #[test]
    fn unresolved_post_handoff_entry_derives_split_writer() {
        let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
        let plan = derive_symbolic_materialization(
            &split_layout(),
            &[symbolic],
            MaterializationContext {
                consumption: ConsumptionInstant::AfterOmegaHandoff,
                byte_order: ByteOrder::LittleEndian,
                native_pointer_relocation_bits: Some(64),
                placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
            },
            |_| None,
        )
        .expect("post-handoff fragments have a writer path");

        assert_eq!(plan.actions.len(), 3);
        assert!(
            plan.actions
                .iter()
                .all(|action| matches!(action, MaterializationAction::RuntimeWriter(_)))
        );

        let writer = plan
            .derive_post_handoff_writer()
            .expect("runtime actions form a writer program");
        let mut bytes = [0_u8; 16];
        let mut resolutions = 0;
        writer
            .execute(
                &mut bytes,
                PlacementSite {
                    base_address: 0,
                    phase: PlacementPhase::PostHandoff,
                    machine_regime: None,
                    installation_scope: None,
                },
                |target| {
                    assert_eq!(target, entry());
                    resolutions += 1;
                    Some(0x1122_3344_5566_7788)
                },
            )
            .expect("provider resolves and executes the writer");

        assert_eq!(resolutions, 1, "three fragments share one resolution");
        assert_eq!(&bytes[0..4], &[0x88, 0x77, 0x66, 0x55]);
        assert_eq!(&bytes[8..12], &[0x44, 0x33, 0x22, 0x11]);
    }

    #[test]
    fn reusable_writer_fragment_separates_static_geometry_from_invocation_evidence() {
        let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
        let plan = derive_symbolic_materialization(
            &split_layout(),
            &[symbolic],
            MaterializationContext {
                consumption: ConsumptionInstant::AfterOmegaHandoff,
                byte_order: ByteOrder::LittleEndian,
                native_pointer_relocation_bits: None,
                placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
            },
            |_| None,
        )
        .expect("symbolic materialization");
        let writer = plan
            .derive_post_handoff_writer()
            .expect("post-handoff writer");
        let lowering = writer
            .lower_reusable_fragment()
            .expect("address-free reusable fragment");

        assert_eq!(
            lowering.fragment.context_abi(),
            POST_HANDOFF_WRITER_CONTEXT_ABI_V1
        );
        assert_eq!(lowering.fragment.source_slot_count(), 1);
        assert_eq!(lowering.sources.len(), 1);
        assert_eq!(lowering.sources[0].target, entry());
        assert_eq!(
            lowering.sources[0].source,
            PostHandoffWriterSource::Resolve(entry())
        );
        assert!(
            lowering
                .fragment
                .steps()
                .iter()
                .all(|step| step.source_slot == 0),
            "all three fragments of one symbolic target share one private slot"
        );
        assert_eq!(post_handoff_writer_context_byte_len(1), Some(16));

        let replacement = RelocationTarget::Entry(
            EntryStubId::from_normalized_identity(0x66bb).expect("replacement entry"),
        );
        let mut rebound = writer.clone();
        for step in &mut rebound.steps {
            step.write.target = replacement;
            step.source = PostHandoffWriterSource::Resolve(replacement);
        }
        rebound.placement =
            PlacementConstraints::new(None, 16, PlacementPhase::PostHandoff, None, None)
                .expect("stronger invocation placement");
        let rebound = rebound
            .lower_reusable_fragment()
            .expect("same reusable geometry");

        assert_eq!(
            rebound.fragment.fingerprint(),
            lowering.fragment.fingerprint(),
            "target identity and concrete placement are invocation evidence"
        );
        assert_eq!(rebound.fragment, lowering.fragment);
        assert_ne!(rebound.sources, lowering.sources);
        assert_ne!(rebound.placement, lowering.placement);
    }

    #[test]
    fn reusable_writer_fragment_rejects_inconsistent_values_for_one_target() {
        let write = MaterializationWrite {
            field: "address".into(),
            target: entry(),
            container_byte_offset: 0,
            container_width_bits: 64,
            destination_lsb: 0,
            source_lsb: 0,
            width: 32,
        };
        let writer = PostHandoffWriterPlan {
            byte_len: 8,
            byte_order: ByteOrder::LittleEndian,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
            steps: vec![
                PostHandoffWriterStep {
                    write: write.clone(),
                    source: PostHandoffWriterSource::Resolved(1),
                },
                PostHandoffWriterStep {
                    write: MaterializationWrite {
                        destination_lsb: 32,
                        source_lsb: 32,
                        ..write
                    },
                    source: PostHandoffWriterSource::Resolved(2),
                },
            ],
        };

        let error = writer
            .lower_reusable_fragment()
            .expect_err("one symbolic source cannot change between fragments");
        assert!(error.0.contains("inconsistent invocation values"));
        let error = writer
            .validate(
                8,
                PlacementSite {
                    base_address: 0,
                    phase: PlacementPhase::PostHandoff,
                    machine_regime: None,
                    installation_scope: None,
                },
            )
            .expect_err("direct execution validates the same source invariant");
        assert!(error.0.contains("inconsistent invocation values"));
    }

    #[test]
    fn writer_rejects_a_resolved_source_that_does_not_match_its_write_target() {
        let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
        let plan = derive_symbolic_materialization(
            &split_layout(),
            &[symbolic],
            MaterializationContext {
                consumption: ConsumptionInstant::AfterOmegaHandoff,
                byte_order: ByteOrder::LittleEndian,
                native_pointer_relocation_bits: None,
                placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
            },
            |_| None,
        )
        .expect("writer plan");
        let mut writer = plan
            .derive_post_handoff_writer()
            .expect("runtime actions form a writer program");
        let substituted = RelocationTarget::Entry(
            EntryStubId::from_normalized_identity(0x66bb).expect("second entry identity"),
        );
        writer.steps[0].source = PostHandoffWriterSource::Resolve(substituted);

        let mut bytes = [0xa5_u8; 16];
        let error = writer
            .execute(
                &mut bytes,
                PlacementSite {
                    base_address: 0,
                    phase: PlacementPhase::PostHandoff,
                    machine_regime: None,
                    installation_scope: None,
                },
                |_| panic!("mismatched writer target must reject before resolution"),
            )
            .expect_err("writer source substitution must reject");
        assert!(error.0.contains("does not match write target"));
        assert_eq!(bytes, [0xa5; 16]);
    }

    #[test]
    fn writer_validates_every_step_before_direct_destination_writes() {
        let valid = MaterializationWrite {
            field: "valid".into(),
            target: entry(),
            container_byte_offset: 0,
            container_width_bits: 64,
            destination_lsb: 0,
            source_lsb: 0,
            width: 64,
        };
        let invalid = MaterializationWrite {
            field: "outside".into(),
            container_byte_offset: 16,
            ..valid.clone()
        };
        let writer = PostHandoffWriterPlan {
            byte_len: 16,
            byte_order: ByteOrder::LittleEndian,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
            steps: vec![
                PostHandoffWriterStep {
                    write: valid,
                    source: PostHandoffWriterSource::Resolve(entry()),
                },
                PostHandoffWriterStep {
                    write: invalid,
                    source: PostHandoffWriterSource::Resolve(entry()),
                },
            ],
        };
        let mut bytes = [0xa5_u8; 16];
        let error = writer
            .execute(
                &mut bytes,
                PlacementSite {
                    base_address: 0,
                    phase: PlacementPhase::PostHandoff,
                    machine_regime: None,
                    installation_scope: None,
                },
                |_| Some(0x1122_3344_5566_7788),
            )
            .expect_err("invalid later step must reject before direct writes begin");
        assert!(error.0.contains("outside"));
        assert_eq!(bytes, [0xa5; 16]);
    }

    #[test]
    fn fixed_entry_constant_folds_split_little_endian_bytes() {
        let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
        let plan = derive_symbolic_materialization(
            &split_layout(),
            &[symbolic],
            MaterializationContext {
                consumption: ConsumptionInstant::BeforeOmegaEntry,
                byte_order: ByteOrder::LittleEndian,
                native_pointer_relocation_bits: Some(64),
                placement: PlacementConstraints::unconstrained(PlacementPhase::Load),
            },
            |_| Some(0x1122_3344_5566_7788),
        )
        .expect("fixed-address fragments constant-fold");
        let mut bytes = [0_u8; 16];
        plan.materialize_resolved_into(&mut bytes)
            .expect("all writes resolved");

        assert_eq!(&bytes[0..4], &[0x88, 0x77, 0x66, 0x55]);
        assert_eq!(&bytes[8..12], &[0x44, 0x33, 0x22, 0x11]);
    }

    #[test]
    fn unresolved_loader_consumed_fragments_reject() {
        let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
        let error = derive_symbolic_materialization(
            &split_layout(),
            &[symbolic],
            MaterializationContext {
                consumption: ConsumptionInstant::BeforeOmegaEntry,
                byte_order: ByteOrder::LittleEndian,
                native_pointer_relocation_bits: Some(64),
                placement: PlacementConstraints::unconstrained(PlacementPhase::Load),
            },
            |_| None,
        )
        .expect_err("a loader cannot apply split pointer relocations");

        assert!(error.0.contains("before Omega entry"));
    }

    #[test]
    fn whole_pointer_uses_loader_native_relocation() {
        let layout = LayoutPlanReport {
            schema_identity: 1,
            entries: vec![LayoutFieldEntryReport {
                field: "entry".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 8 },
            }],
            offsets: Some(vec![8]),
            size: Some(16),
            align: 8,
        };
        let symbolic = SymbolicFieldValue::new("entry", 64, entry()).expect("symbolic field");
        let plan = derive_symbolic_materialization(
            &layout,
            &[symbolic],
            MaterializationContext {
                consumption: ConsumptionInstant::BeforeOmegaEntry,
                byte_order: ByteOrder::LittleEndian,
                native_pointer_relocation_bits: Some(64),
                placement: PlacementConstraints::unconstrained(PlacementPhase::Load),
            },
            |_| None,
        )
        .expect("whole-pointer native relocation is available");

        assert!(matches!(
            plan.actions.as_slice(),
            [MaterializationAction::NativePointerRelocation {
                destination_byte_offset: 8,
                width_bits: 64,
                ..
            }]
        ));
        assert!(
            plan.derive_post_handoff_writer()
                .expect_err("loader relocation is not a writer instruction")
                .0
                .contains("loader-native")
        );
    }

    #[test]
    fn unresolved_action_cannot_partially_materialize() {
        let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
        let plan = derive_symbolic_materialization(
            &split_layout(),
            &[symbolic],
            MaterializationContext {
                consumption: ConsumptionInstant::AfterOmegaHandoff,
                byte_order: ByteOrder::LittleEndian,
                native_pointer_relocation_bits: None,
                placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
            },
            |_| None,
        )
        .expect("writer plan");
        let mut bytes = [0xa5_u8; 16];
        assert!(plan.materialize_resolved_into(&mut bytes).is_err());
        assert_eq!(bytes, [0xa5; 16]);

        let writer = plan
            .derive_post_handoff_writer()
            .expect("runtime actions form a writer program");
        let mut missing = [0xa5_u8; 16];
        assert!(
            writer
                .execute(
                    &mut missing,
                    PlacementSite {
                        base_address: 0,
                        phase: PlacementPhase::PostHandoff,
                        machine_regime: None,
                        installation_scope: None,
                    },
                    |_| None,
                )
                .is_err()
        );
        assert_eq!(missing, [0xa5; 16]);
    }

    #[test]
    fn placement_constraints_join_layout_alignment_and_validate_all_axes() {
        let regime = MachineRegimeId::from_normalized_identity(11).expect("machine regime");
        let scope =
            ArtifactInstallationScopeId::from_normalized_identity(12).expect("installation scope");
        let constraints = PlacementConstraints::new(
            Some(PlacementAddressRange::new(0x1000, 0x10_0000).expect("low-memory range")),
            4096,
            PlacementPhase::PostHandoff,
            Some(regime),
            Some(scope),
        )
        .expect("placement constraints");
        let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
        let mut layout = split_layout();
        layout.align = 16;
        let plan = derive_symbolic_materialization(
            &layout,
            &[symbolic],
            MaterializationContext {
                consumption: ConsumptionInstant::AfterOmegaHandoff,
                byte_order: ByteOrder::LittleEndian,
                native_pointer_relocation_bits: None,
                placement: constraints,
            },
            |_| None,
        )
        .expect("constrained post-handoff plan");

        assert_eq!(plan.placement.alignment(), 4096);
        plan.placement
            .validate_site(
                plan.byte_len,
                PlacementSite {
                    base_address: 0x8000,
                    phase: PlacementPhase::PostHandoff,
                    machine_regime: Some(regime),
                    installation_scope: Some(scope),
                },
            )
            .expect("all concrete placement facts match");

        let wrong_phase = PlacementSite {
            base_address: 0x8000,
            phase: PlacementPhase::Load,
            machine_regime: Some(regime),
            installation_scope: Some(scope),
        };
        assert!(
            plan.placement
                .validate_site(plan.byte_len, wrong_phase)
                .expect_err("phase is part of the normalized constraint")
                .0
                .contains("phase")
        );
        let writer = plan
            .derive_post_handoff_writer()
            .expect("runtime actions form a writer program");
        let mut unchanged = [0xa5_u8; 16];
        assert!(
            writer
                .execute(&mut unchanged, wrong_phase, |_| Some(1))
                .is_err()
        );
        assert_eq!(unchanged, [0xa5; 16]);

        let misaligned = PlacementSite {
            base_address: 0x8001,
            phase: PlacementPhase::PostHandoff,
            ..wrong_phase
        };
        assert!(
            plan.placement
                .validate_site(plan.byte_len, misaligned)
                .expect_err("layout and policy alignment are mandatory")
                .0
                .contains("aligned")
        );

        let wrong_regime = PlacementSite {
            phase: PlacementPhase::PostHandoff,
            machine_regime: None,
            ..wrong_phase
        };
        assert!(
            plan.placement
                .validate_site(plan.byte_len, wrong_regime)
                .expect_err("machine regime is required")
                .0
                .contains("regime")
        );

        let wrong_scope = PlacementSite {
            machine_regime: Some(regime),
            installation_scope: None,
            ..wrong_regime
        };
        assert!(
            plan.placement
                .validate_site(plan.byte_len, wrong_scope)
                .expect_err("installation scope is required")
                .0
                .contains("scope")
        );

        let outside_range = PlacementSite {
            base_address: 0x10_0000,
            installation_scope: Some(scope),
            ..wrong_scope
        };
        assert!(
            plan.placement
                .validate_site(plan.byte_len, outside_range)
                .expect_err("complete placement must fit the range")
                .0
                .contains("outside")
        );
    }

    #[test]
    fn placement_range_must_fit_the_materialization() {
        let constraints = PlacementConstraints::new(
            Some(PlacementAddressRange::new(0x1000, 0x1008).expect("eight-byte range")),
            1,
            PlacementPhase::PostHandoff,
            None,
            None,
        )
        .expect("placement constraints");
        let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
        let error = derive_symbolic_materialization(
            &split_layout(),
            &[symbolic],
            MaterializationContext {
                consumption: ConsumptionInstant::AfterOmegaHandoff,
                byte_order: ByteOrder::LittleEndian,
                native_pointer_relocation_bits: None,
                placement: constraints,
            },
            |_| None,
        )
        .expect_err("sixteen bytes cannot fit an eight-byte range");

        assert!(error.0.contains("cannot fit"));
    }
}
