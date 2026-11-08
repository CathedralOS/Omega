/*
We define normalized programmable-layout plans, symbolic field materialization,
and portable aggregate geometry across Psi and Omega consumers. Rather than
exposing raw code addresses or arbitrary byte-patching primitives to source
programs, we pair validated layout geometry reports (`layout_reports`) with
compiler-issued symbolic field identities (`symbolic_values`) and placement
constraints (`placement`) to derive deterministic `SymbolicMaterializationPlan`
actions (`symbolic_materialization`) and apply or decode them into byte storage
(`materialization`).
*/

/*
Programmable layouts must support bit-packed integer fragments, nested record
and conventional sum interiors, and post-handoff relocation targets while
remaining completely target-neutral inside Psi. Centralizing layout plan
fingerprints, conventional sum packing (`aggregate_geometry`), private callback
placement validation, and atomic resolved-write materialization in one crate
ensures that the Psi interpreter and Omega native writers derive identical byte
layouts and reject out-of-range fragments or unresolved symbolic targets before
mutating destination bytes.
*/

/*
`numbered_constructor!` and `numbered_variant!` stamp compiler-retained member
identities onto positional constructors after full positional validation.
Submodules own layout report normalization (`layout_reports`), byte-level field
staging and decoding (`materialization`), and symbolic plan derivation
(`symbolic_materialization`).
*/

#![forbid(unsafe_code)]

macro_rules! numbered_constructor {
    ($(#[$attribute:meta])* $name:ident(field $(, $argument:ident : $type:ty)* $(,)?) -> $base:ident) => {
        $(#[$attribute])*
        pub fn $name(
            field: impl Into<String>,
            member_identity: u64,
            $($argument: $type),*
        ) -> Result<Self, MaterializationDiagnostic> {
            let mut value = Self::$base(field, $($argument),*)?;
            value.member_identity = Some(member_identity);
            Ok(value)
        }
    };
}

macro_rules! numbered_variant {
    ($(#[$attribute:meta])* $name:ident(field $(, $argument:ident : $type:ty)* $(,)?) -> $base:ident) => {
        $(#[$attribute])*
        pub fn $name(field: impl Into<String>, member_identity: u64, $($argument: $type),*) -> Self {
            Self {
                member_identity: Some(member_identity),
                ..Self::$base(field, $($argument),*)
            }
        }
    };
}

mod aggregate_geometry {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StorageGeometry {
        pub byte_size: u64,
        pub byte_alignment: u64,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ConventionalSumGeometry {
        pub tag_byte_offset: u64,
        pub tag: StorageGeometry,
        pub storage: StorageGeometry,
        pub common_field_offsets: Vec<u64>,
        pub payload_byte_offset: u64,
        pub case_field_offsets: Vec<Vec<u64>>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AggregateGeometryError {
        EmptyCaseSet,
        ZeroAlignment,
        SizeOverflow,
    }

    pub fn conventional_sum_geometry(
        tag: StorageGeometry,
        common_fields: impl IntoIterator<Item = StorageGeometry>,
        cases: impl IntoIterator<Item = impl IntoIterator<Item = StorageGeometry>>,
    ) -> Result<ConventionalSumGeometry, AggregateGeometryError> {
        let cases: Vec<Vec<StorageGeometry>> = cases
            .into_iter()
            .map(|fields| fields.into_iter().collect())
            .collect();
        if cases.is_empty() {
            return Err(AggregateGeometryError::EmptyCaseSet);
        }
        valid_alignment(tag.byte_alignment)?;
        let (common_field_offsets, common) = pack_fields_at(common_fields, tag.byte_size)?;
        let mut payload_alignment = 1;
        for field in cases.iter().flatten() {
            valid_alignment(field.byte_alignment)?;
            payload_alignment = payload_alignment.max(field.byte_alignment);
        }
        let payload_byte_offset = align_up(common.byte_size, payload_alignment)?;
        let mut end = common.byte_size;
        let mut case_field_offsets = Vec::with_capacity(cases.len());
        for fields in cases {
            let (offsets, geometry) = pack_fields_at(fields, payload_byte_offset)?;
            end = end.max(geometry.byte_size);
            case_field_offsets.push(offsets);
        }
        let byte_alignment = tag
            .byte_alignment
            .max(common.byte_alignment)
            .max(payload_alignment);
        Ok(ConventionalSumGeometry {
            tag_byte_offset: 0,
            tag,
            storage: StorageGeometry {
                byte_size: align_up(end, byte_alignment)?,
                byte_alignment,
            },
            common_field_offsets,
            payload_byte_offset,
            case_field_offsets,
        })
    }

    fn pack_fields_at(
        fields: impl IntoIterator<Item = StorageGeometry>,
        base: u64,
    ) -> Result<(Vec<u64>, StorageGeometry), AggregateGeometryError> {
        let mut end = base;
        let mut alignment = 1;
        let mut offsets = Vec::new();
        for field in fields {
            valid_alignment(field.byte_alignment)?;
            end = align_up(end, field.byte_alignment)?;
            offsets.push(end);
            end = end
                .checked_add(field.byte_size)
                .ok_or(AggregateGeometryError::SizeOverflow)?;
            alignment = alignment.max(field.byte_alignment);
        }
        Ok((
            offsets,
            StorageGeometry {
                byte_size: align_up(end, alignment)?,
                byte_alignment: alignment,
            },
        ))
    }

    fn valid_alignment(alignment: u64) -> Result<(), AggregateGeometryError> {
        if alignment == 0 {
            Err(AggregateGeometryError::ZeroAlignment)
        } else {
            Ok(())
        }
    }

    fn align_up(value: u64, alignment: u64) -> Result<u64, AggregateGeometryError> {
        valid_alignment(alignment)?;
        let remainder = value % alignment;
        if remainder == 0 {
            Ok(value)
        } else {
            value
                .checked_add(alignment - remainder)
                .ok_or(AggregateGeometryError::SizeOverflow)
        }
    }
}
mod layout_reports;
mod materialization;
mod placement {
    use crate::layout_reports::IntegerInterpretation;
    use crate::materialization::MaterializationDiagnostic;
    use crate::symbolic_values::{RelocationTarget, nonzero_identity};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ByteOrder {
        LittleEndian,
        BigEndian,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ConsumptionInstant {
        BeforeOmegaEntry,
        AfterOmegaHandoff,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum PlacementPhase {
        Build,
        Load,
        PostHandoff,
    }

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

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct PlacementConstraints {
        permitted_range: Option<PlacementAddressRange>,
        pub(crate) alignment: u64,
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

        pub(crate) fn joined_with_layout(
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
            if self.installation_scope.is_some()
                && site.installation_scope != self.installation_scope
            {
                return Err(MaterializationDiagnostic(format!(
                    "placement installation scope {:?} does not satisfy required scope {:?}",
                    site.installation_scope, self.installation_scope
                )));
            }
            Ok(())
        }
    }

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
        pub stored_integer_fit: Option<StoredIntegerFit>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StoredIntegerFit {
        pub source_width_bits: u16,
        pub stored_width_bits: u16,
        pub interpretation: IntegerInterpretation,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum MaterializationAction {
        ResolvedWrite {
            write: MaterializationWrite,
            source_value: u64,
        },
        NativePointerRelocation {
            field: String,
            target: RelocationTarget,
            destination_byte_offset: u64,
            width_bits: u16,
        },
        RuntimeWriter(MaterializationWrite),
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
}
mod private_callback_placements {
    use crate::{LayoutPlacementReport, validate_fragment};

    pub fn validate_private_callback_placements(
        placements: &[LayoutPlacementReport],
        pointer_size: u64,
        pointer_alignment: u64,
        storage_size: u64,
        storage_alignment: u64,
    ) -> Result<Vec<(u64, u64)>, String> {
        if pointer_size == 0 || pointer_size > 8 || !pointer_alignment.is_power_of_two() {
            return Err("private callback source has invalid target pointer geometry".into());
        }
        let source_bits = pointer_size * 8;
        if let [LayoutPlacementReport::At { offset }] = placements {
            if storage_alignment < pointer_alignment || !offset.is_multiple_of(pointer_alignment) {
                return Err("private callback whole-pointer placement is not aligned".into());
            }
            let end = offset
                .checked_add(pointer_size)
                .filter(|end| *end <= storage_size)
                .ok_or("private callback whole-pointer placement exceeds its storage")?;
            return Ok(vec![(
                offset
                    .checked_mul(8)
                    .ok_or("private callback bit offset overflows")?,
                end.checked_mul(8)
                    .ok_or("private callback bit extent overflows")?,
            )]);
        }
        if placements.is_empty() {
            return Err("private callback slot has no placement".into());
        }
        let storage_size = usize::try_from(storage_size)
            .map_err(|_| "private callback storage exceeds host range")?;
        let mut sources = Vec::with_capacity(placements.len());
        let mut destinations = Vec::with_capacity(placements.len());
        for placement in placements {
            let LayoutPlacementReport::Bits {
                container,
                container_width,
                destination_lsb,
                source_lsb,
                width,
            } = *placement
            else {
                return Err("private callback slots require one whole pointer or a complete set of Bits fragments; narrowed IntegerAt storage requires an address-fit contract".into());
            };
            let narrow = |value| {
                u16::try_from(value)
                    .map_err(|_| "private callback fragment exceeds the supported scalar width")
            };
            validate_fragment(
                storage_size,
                "private callback",
                container,
                narrow(container_width)?,
                narrow(destination_lsb)?,
                narrow(source_lsb)?,
                narrow(width)?,
            )
            .map_err(|error| error.to_string())?;
            let source_end = source_lsb
                .checked_add(width)
                .filter(|end| *end <= source_bits)
                .ok_or("private callback fragment exceeds its target pointer width")?;
            let destination_start = container
                .checked_mul(8)
                .and_then(|base| base.checked_add(destination_lsb))
                .ok_or("private callback fragment destination overflows")?;
            let destination_end = destination_start
                .checked_add(width)
                .ok_or("private callback fragment destination overflows")?;
            sources.push((source_lsb, source_end));
            destinations.push((destination_start, destination_end));
        }
        sources.sort_unstable();
        let mut covered = 0;
        for (start, end) in sources {
            if start != covered {
                return Err(
                    "private callback fragments have missing or overlapping source bits".into(),
                );
            }
            covered = end;
        }
        if covered != source_bits {
            return Err(
                "private callback fragments do not cover the complete target pointer".into(),
            );
        }
        destinations.sort_unstable();
        if destinations.windows(2).any(|pair| pair[0].1 > pair[1].0) {
            return Err("private callback fragments overlap in destination storage".into());
        }
        Ok(destinations)
    }

    pub fn compose_private_callback_placements(
        placements: &[LayoutPlacementReport],
        base: u64,
    ) -> Result<Vec<LayoutPlacementReport>, String> {
        placements
            .iter()
            .copied()
            .map(|mut placement| {
                let offset = match &mut placement {
                    LayoutPlacementReport::At { offset }
                    | LayoutPlacementReport::IntegerAt { offset, .. } => offset,
                    LayoutPlacementReport::Bits { container, .. } => container,
                };
                *offset = offset
                    .checked_add(base)
                    .ok_or("private callback composed offset overflows")?;
                Ok(placement)
            })
            .collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn fragment(
            container: u64,
            container_width: u64,
            source_lsb: u64,
            width: u64,
        ) -> LayoutPlacementReport {
            LayoutPlacementReport::Bits {
                container,
                container_width,
                destination_lsb: 0,
                source_lsb,
                width,
            }
        }

        fn split_gate() -> [LayoutPlacementReport; 3] {
            [
                fragment(0, 16, 0, 16),
                fragment(6, 16, 16, 16),
                fragment(8, 32, 32, 32),
            ]
        }

        #[test]
        fn split_address_leaves_descriptor_fields_outside_changed_ranges() {
            let ranges = validate_private_callback_placements(&split_gate(), 8, 8, 16, 8).unwrap();
            assert_eq!(ranges, [(0, 16), (48, 64), (64, 96)]);
            for (start, end) in [(16, 32), (32, 40), (40, 48), (96, 128)] {
                assert!(
                    ranges
                        .iter()
                        .all(|(written_start, written_end)| *written_end <= start
                            || end <= *written_start)
                );
            }
            let nested = compose_private_callback_placements(&split_gate(), 24).unwrap();
            assert_eq!(
                validate_private_callback_placements(&nested, 8, 8, 40, 8).unwrap(),
                [(192, 208), (240, 256), (256, 288)]
            );
        }

        #[test]
        fn source_coverage_rejects_missing_repeated_and_target_excess_bits() {
            let mut placements = split_gate();
            placements[1] = fragment(6, 16, 17, 15);
            assert!(
                validate_private_callback_placements(&placements, 8, 8, 16, 8)
                    .unwrap_err()
                    .contains("missing or overlapping")
            );
            placements[1] = fragment(6, 16, 0, 16);
            assert!(
                validate_private_callback_placements(&placements, 8, 8, 16, 8)
                    .unwrap_err()
                    .contains("missing or overlapping")
            );
            assert!(
                validate_private_callback_placements(&split_gate(), 4, 4, 16, 8)
                    .unwrap_err()
                    .contains("target pointer width")
            );
        }

        #[test]
        fn container_bounds_and_changed_bit_overlap_are_separate_obligations() {
            let disjoint = [
                LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 64,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 32,
                },
                LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 64,
                    destination_lsb: 32,
                    source_lsb: 32,
                    width: 32,
                },
            ];
            assert_eq!(
                validate_private_callback_placements(&disjoint, 8, 8, 8, 1).unwrap(),
                [(0, 32), (32, 64)]
            );
            let mut overlap = disjoint;
            overlap[1] = fragment(0, 64, 32, 32);
            assert!(
                validate_private_callback_placements(&overlap, 8, 8, 8, 8)
                    .unwrap_err()
                    .contains("overlap in destination")
            );
            let oversized = [fragment(0, 64, 0, 32)];
            assert!(validate_private_callback_placements(&oversized, 4, 4, 4, 4).is_err());
            assert!(compose_private_callback_placements(&split_gate(), u64::MAX).is_err());
        }

        #[test]
        fn whole_pointer_still_requires_alignment_and_full_extent() {
            assert!(
                validate_private_callback_placements(
                    &[LayoutPlacementReport::At { offset: 6 }],
                    8,
                    8,
                    16,
                    8
                )
                .is_err()
            );
            assert!(
                validate_private_callback_placements(
                    &[LayoutPlacementReport::At { offset: 8 }],
                    8,
                    8,
                    12,
                    8
                )
                .is_err()
            );
            assert!(
                validate_private_callback_placements(
                    &[LayoutPlacementReport::At { offset: 0 }],
                    8,
                    8,
                    8,
                    1
                )
                .is_err()
            );
            assert!(validate_private_callback_placements(&[], 8, 8, 16, 8).is_err());
        }
    }
}
mod symbolic_materialization;
mod symbolic_values {
    use crate::layout_reports::{
        CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT, ConventionalRecordSumChildHop,
        ConventionalRecordSumChildInterior, ConventionalRecursiveRecordSumPathsLayoutReport,
        ConventionalSumLayoutReport, LayoutPlanReport,
    };
    use crate::materialization::MaterializationDiagnostic;
    use crate::materialization::field_values::validate_authored_width;

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
    pub struct SymbolicFieldPathSegment {
        pub field: String,
        pub(crate) member_identity: Option<u64>,
        pub(crate) element_index: Option<u64>,
        inner: Option<Box<SymbolicFieldPathSegment>>,
    }

    impl SymbolicFieldPathSegment {
        pub fn new(field: impl Into<String>) -> Self {
            Self {
                field: field.into(),
                member_identity: None,
                element_index: None,
                inner: None,
            }
        }

        numbered_variant!(
            new_numbered(field) -> new
        );

        pub fn new_indexed(field: impl Into<String>, element_index: u64) -> Self {
            Self {
                element_index: Some(element_index),
                ..Self::new(field)
            }
        }

        numbered_variant!(new_indexed_numbered(field, element_index: u64) -> new_indexed);

        pub const fn element_index(&self) -> Option<u64> {
            self.element_index
        }

        pub fn with_inner_segment(mut self, inner: SymbolicFieldPathSegment) -> Self {
            self.inner = Some(Box::new(inner));
            self
        }

        pub const fn inner(&self) -> Option<&SymbolicFieldPathSegment> {
            match &self.inner {
                Some(inner) => Some(&**inner),
                None => None,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SymbolicFieldValue {
        pub field: String,
        pub(crate) member_identity: Option<u64>,
        pub(crate) element_index: Option<u64>,
        pub(crate) inner: Option<SymbolicFieldPathSegment>,
        pub width_bits: u16,
        pub target: RelocationTarget,
    }

    impl SymbolicFieldValue {
        pub fn new(
            field: impl Into<String>,
            width_bits: u16,
            target: RelocationTarget,
        ) -> Result<Self, MaterializationDiagnostic> {
            validate_authored_width("symbolic", width_bits)?;
            Ok(Self {
                field: field.into(),
                member_identity: None,
                element_index: None,
                inner: None,
                width_bits,
                target,
            })
        }

        numbered_constructor!(
            new_numbered(field, width_bits: u16, target: RelocationTarget) -> new
        );

        pub fn new_indexed(
            field: impl Into<String>,
            element_index: u64,
            width_bits: u16,
            target: RelocationTarget,
        ) -> Result<Self, MaterializationDiagnostic> {
            let mut value = Self::new(field, width_bits, target)?;
            value.element_index = Some(element_index);
            Ok(value)
        }

        numbered_constructor!(
            new_indexed_numbered(
                field,
                element_index: u64,
                width_bits: u16,
                target: RelocationTarget,
            ) -> new_indexed
        );

        pub const fn element_index(&self) -> Option<u64> {
            self.element_index
        }

        pub fn with_inner_segment(mut self, inner: SymbolicFieldPathSegment) -> Self {
            self.inner = Some(inner);
            self
        }

        pub const fn inner(&self) -> Option<&SymbolicFieldPathSegment> {
            self.inner.as_ref()
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum SymbolicFieldInterior {
        Record(LayoutPlanReport),
        Sum(ConventionalSumLayoutReport),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SymbolicFieldInteriorLayout {
        pub hop: ConventionalRecordSumChildHop,
        pub interior: SymbolicFieldInterior,
    }

    impl SymbolicFieldInteriorLayout {
        pub const fn interior(&self) -> &SymbolicFieldInterior {
            &self.interior
        }

        pub const fn repetition(&self) -> Option<(u64, u64)> {
            match self.hop {
                ConventionalRecordSumChildHop::Field => None,
                ConventionalRecordSumChildHop::Index {
                    element_count,
                    element_stride,
                } => Some((element_count, element_stride)),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SymbolicFieldInnerLayout {
        pub field: String,
        pub(crate) member_identity: Option<u64>,
        pub inner_layout: SymbolicFieldInteriorLayout,
        pub(crate) inner_layouts: Vec<SymbolicFieldInnerLayout>,
    }

    impl SymbolicFieldInnerLayout {
        pub fn new(field: impl Into<String>, inner_layout: LayoutPlanReport) -> Self {
            Self {
                field: field.into(),
                member_identity: None,
                inner_layout: SymbolicFieldInteriorLayout {
                    hop: ConventionalRecordSumChildHop::Field,
                    interior: SymbolicFieldInterior::Record(inner_layout),
                },
                inner_layouts: Vec::new(),
            }
        }

        numbered_variant!(new_numbered(field, inner_layout: LayoutPlanReport) -> new);

        pub fn new_sum(field: impl Into<String>, sum_layout: ConventionalSumLayoutReport) -> Self {
            Self {
                field: field.into(),
                member_identity: None,
                inner_layout: SymbolicFieldInteriorLayout {
                    hop: ConventionalRecordSumChildHop::Field,
                    interior: SymbolicFieldInterior::Sum(sum_layout),
                },
                inner_layouts: Vec::new(),
            }
        }

        numbered_variant!(
            new_sum_numbered(field, sum_layout: ConventionalSumLayoutReport) -> new_sum
        );

        pub fn new_sum_array(
            field: impl Into<String>,
            element_layout: ConventionalSumLayoutReport,
            element_count: u64,
            element_stride: u64,
        ) -> Self {
            Self {
                field: field.into(),
                member_identity: None,
                inner_layout: SymbolicFieldInteriorLayout {
                    hop: ConventionalRecordSumChildHop::Index {
                        element_count,
                        element_stride,
                    },
                    interior: SymbolicFieldInterior::Sum(element_layout),
                },
                inner_layouts: Vec::new(),
            }
        }

        numbered_variant!(
            new_sum_array_numbered(
                field,
                element_layout: ConventionalSumLayoutReport,
                element_count: u64,
                element_stride: u64,
            ) -> new_sum_array
        );

        pub fn new_record_array(
            field: impl Into<String>,
            element_layout: LayoutPlanReport,
            element_count: u64,
            element_stride: u64,
        ) -> Self {
            Self {
                field: field.into(),
                member_identity: None,
                inner_layout: SymbolicFieldInteriorLayout {
                    hop: ConventionalRecordSumChildHop::Index {
                        element_count,
                        element_stride,
                    },
                    interior: SymbolicFieldInterior::Record(element_layout),
                },
                inner_layouts: Vec::new(),
            }
        }

        numbered_variant!(
            new_record_array_numbered(
                field,
                element_layout: LayoutPlanReport,
                element_count: u64,
                element_stride: u64,
            ) -> new_record_array
        );

        pub fn from_recursive_sum_paths(
            report: &ConventionalRecursiveRecordSumPathsLayoutReport,
        ) -> Result<Vec<Self>, MaterializationDiagnostic> {
            Self::fold_recursive_sum_paths(report, 0)
        }

        fn fold_recursive_sum_paths(
            report: &ConventionalRecursiveRecordSumPathsLayoutReport,
            depth: usize,
        ) -> Result<Vec<Self>, MaterializationDiagnostic> {
            let mut carriers = Vec::new();
            carriers
                .try_reserve_exact(report.children.len())
                .map_err(|_| {
                    MaterializationDiagnostic(
                        "recursive record/sum path carrier fold exceeds compiler resources".into(),
                    )
                })?;
            for child in &report.children {
                let carrier = match (&child.hop, &child.interior) {
                    (
                        ConventionalRecordSumChildHop::Field,
                        ConventionalRecordSumChildInterior::Sum(layout),
                    ) => match child.member_identity {
                        Some(identity) => {
                            Self::new_sum_numbered(child.field.clone(), identity, layout.clone())
                        }
                        None => Self::new_sum(child.field.clone(), layout.clone()),
                    },
                    (
                        ConventionalRecordSumChildHop::Index {
                            element_count,
                            element_stride,
                        },
                        ConventionalRecordSumChildInterior::Sum(layout),
                    ) => match child.member_identity {
                        Some(identity) => Self::new_sum_array_numbered(
                            child.field.clone(),
                            identity,
                            layout.clone(),
                            *element_count,
                            *element_stride,
                        ),
                        None => Self::new_sum_array(
                            child.field.clone(),
                            layout.clone(),
                            *element_count,
                            *element_stride,
                        ),
                    },
                    (hop, ConventionalRecordSumChildInterior::Record(inner)) => {
                        if depth + 1 >= CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT {
                            return Err(MaterializationDiagnostic(format!(
                                "recursive record/sum path report nests beyond the compiler's {CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT}-segment record path bound"
                            )));
                        }
                        let mut carrier = match (hop, child.member_identity) {
                            (ConventionalRecordSumChildHop::Field, Some(identity)) => {
                                Self::new_numbered(
                                    child.field.clone(),
                                    identity,
                                    inner.outer_layout.clone(),
                                )
                            }
                            (ConventionalRecordSumChildHop::Field, None) => {
                                Self::new(child.field.clone(), inner.outer_layout.clone())
                            }
                            (
                                ConventionalRecordSumChildHop::Index {
                                    element_count,
                                    element_stride,
                                },
                                Some(identity),
                            ) => Self::new_record_array_numbered(
                                child.field.clone(),
                                identity,
                                inner.outer_layout.clone(),
                                *element_count,
                                *element_stride,
                            ),
                            (
                                ConventionalRecordSumChildHop::Index {
                                    element_count,
                                    element_stride,
                                },
                                None,
                            ) => Self::new_record_array(
                                child.field.clone(),
                                inner.outer_layout.clone(),
                                *element_count,
                                *element_stride,
                            ),
                        };
                        for nested in Self::fold_recursive_sum_paths(inner, depth + 1)? {
                            carrier = carrier.with_inner_layout(nested);
                        }
                        carrier
                    }
                };
                carriers.push(carrier);
            }
            Ok(carriers)
        }

        pub fn with_inner_layout(mut self, inner: SymbolicFieldInnerLayout) -> Self {
            self.inner_layouts.push(inner);
            self
        }

        pub fn inner_layouts(&self) -> &[SymbolicFieldInnerLayout] {
            &self.inner_layouts
        }
    }

    pub(crate) fn nonzero_identity(
        kind: &str,
        identity: u64,
    ) -> Result<u64, MaterializationDiagnostic> {
        if identity == 0 {
            Err(MaterializationDiagnostic(format!(
                "normalized {kind} identity cannot be zero"
            )))
        } else {
            Ok(identity)
        }
    }
}

pub use aggregate_geometry::{
    AggregateGeometryError, ConventionalSumGeometry, StorageGeometry, conventional_sum_geometry,
};
pub use layout_reports::{
    CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT, ConventionalNestedRecordSumOccurrenceLayoutReport,
    ConventionalNestedRecordSumPathLayoutReport, ConventionalNestedRecordSumPathsLayoutReport,
    ConventionalRecordSumChildHop, ConventionalRecordSumChildInterior,
    ConventionalRecordSumChildLayoutReport, ConventionalRecursiveRecordSumPathsLayoutReport,
    ConventionalSumArrayFieldLayoutReport, ConventionalSumCaseLayoutReport,
    ConventionalSumFieldLayoutReport, ConventionalSumLayoutReport,
    ConventionalSumPayloadFieldLayoutReport, IntegerInterpretation, LayoutFieldEntryReport,
    LayoutPlacementReport, LayoutPlanReport, NativeLayoutPlanReport,
    PrivateCallbackLayoutDemandReport, conventional_sum_layout_reports_match_for_replay,
    layout_placement_sort_key, layout_plan_reports_match_for_replay,
    normalized_conventional_sum_layout_report_fingerprint,
    normalized_layout_plan_report_fingerprint, normalized_native_layout_plan_report_fingerprint,
};
pub use materialization::field_codec::{
    decode_aggregate_field, decode_aggregate_field_into, stage_scalar_field, update_scalar_field,
};
pub use materialization::field_values::{
    AggregateFieldSchema, AggregateFieldValue, ScalarFieldSchema, ScalarFieldValue,
};
pub use materialization::stored_integer_writes::{
    apply_write, validate_fragment, validate_stored_integer_fit, validate_stored_integer_fit_shape,
    validate_write, validate_write_source_value,
};
pub use materialization::{
    MaterializationDiagnostic, SymbolicMaterializationPlan, decode_scalar_layout,
    materialize_aggregate_layout_into, materialize_scalar_layout_into, stage_aggregate_layout,
};
pub use placement::{
    ArtifactInstallationScopeId, ByteOrder, ConsumptionInstant, MachineRegimeId,
    MaterializationAction, MaterializationContext, MaterializationWrite, PlacementAddressRange,
    PlacementConstraints, PlacementPhase, PlacementSite, StoredIntegerFit,
};
pub use symbolic_materialization::{
    derive_symbolic_materialization, derive_symbolic_materialization_with_inner_layouts,
};
pub use symbolic_values::{
    DataSymbolId, EntryStubId, RelocationTarget, SymbolicFieldInnerLayout, SymbolicFieldInterior,
    SymbolicFieldInteriorLayout, SymbolicFieldPathSegment, SymbolicFieldValue,
};

#[cfg(test)]
mod tests;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    use crate::{
        ArtifactInstallationScopeId, ByteOrder, ConsumptionInstant, ConventionalRecordSumChildHop,
        ConventionalRecordSumChildInterior, ConventionalRecordSumChildLayoutReport,
        ConventionalRecursiveRecordSumPathsLayoutReport, ConventionalSumCaseLayoutReport,
        ConventionalSumLayoutReport, ConventionalSumPayloadFieldLayoutReport, DataSymbolId,
        EntryStubId, IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport,
        LayoutPlanReport, MachineRegimeId, MaterializationContext, PlacementAddressRange,
        PlacementConstraints, PlacementPhase, RelocationTarget, SymbolicFieldInnerLayout,
        SymbolicFieldPathSegment, SymbolicFieldValue,
    };

    pub fn symbolic_path(segments: &[&str]) -> SymbolicFieldPathSegment {
        let (innermost, outer) = segments.split_last().expect("a path needs a head");
        let mut segment = SymbolicFieldPathSegment::new(*innermost);
        for name in outer.iter().rev() {
            segment = SymbolicFieldPathSegment::new(*name).with_inner_segment(segment);
        }
        segment
    }

    pub fn post_handoff_context_with(placement: PlacementConstraints) -> MaterializationContext {
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement,
        }
    }

    pub fn post_handoff_context() -> MaterializationContext {
        post_handoff_context_with(PlacementConstraints::unconstrained(
            PlacementPhase::PostHandoff,
        ))
    }

    pub fn layout_field_entry(
        field: &str,
        placement: LayoutPlacementReport,
    ) -> LayoutFieldEntryReport {
        LayoutFieldEntryReport {
            field: field.into(),
            member_identity: None,
            placement,
        }
    }

    pub fn numbered_layout_field_entry(
        field: &str,
        member_identity: u64,
        placement: LayoutPlacementReport,
    ) -> LayoutFieldEntryReport {
        LayoutFieldEntryReport {
            field: field.into(),
            member_identity: Some(member_identity),
            placement,
        }
    }

    pub fn layout_report(
        entries: Vec<LayoutFieldEntryReport>,
        offsets: Option<Vec<u64>>,
        size: Option<u64>,
        align: u64,
    ) -> LayoutPlanReport {
        LayoutPlanReport {
            schema_report_fingerprint: 1,
            entries,
            offsets,
            size,
            align,
        }
    }

    pub fn split_layout() -> LayoutPlanReport {
        LayoutPlanReport {
            schema_report_fingerprint: 1,
            entries: vec![
                layout_field_entry(
                    "address",
                    LayoutPlacementReport::Bits {
                        container: 0,
                        container_width: 16,
                        destination_lsb: 0,
                        source_lsb: 0,
                        width: 16,
                    },
                ),
                layout_field_entry(
                    "address",
                    LayoutPlacementReport::Bits {
                        container: 2,
                        container_width: 16,
                        destination_lsb: 0,
                        source_lsb: 16,
                        width: 16,
                    },
                ),
                layout_field_entry(
                    "address",
                    LayoutPlacementReport::Bits {
                        container: 8,
                        container_width: 64,
                        destination_lsb: 0,
                        source_lsb: 32,
                        width: 32,
                    },
                ),
            ],
            offsets: None,
            size: Some(16),
            align: 1,
        }
    }

    pub fn repeated_layout() -> LayoutPlanReport {
        LayoutPlanReport {
            schema_report_fingerprint: 1,
            entries: (0..4)
                .map(|index| {
                    numbered_layout_field_entry(
                        "handlers",
                        9,
                        LayoutPlacementReport::At { offset: index * 8 },
                    )
                })
                .collect(),
            offsets: Some(vec![0, 8, 16, 24]),
            size: Some(32),
            align: 8,
        }
    }

    pub fn deeply_nested_layout() -> (LayoutPlanReport, SymbolicFieldInnerLayout) {
        (
            LayoutPlanReport {
                schema_report_fingerprint: 1,
                entries: vec![
                    layout_field_entry("header", LayoutPlacementReport::At { offset: 0 }),
                    layout_field_entry("slot", LayoutPlacementReport::At { offset: 8 }),
                ],
                offsets: Some(vec![0, 8]),
                size: Some(40),
                align: 8,
            },
            SymbolicFieldInnerLayout::new(
                "slot",
                LayoutPlanReport {
                    schema_report_fingerprint: 2,
                    entries: vec![
                        layout_field_entry("pad", LayoutPlacementReport::At { offset: 0 }),
                        layout_field_entry("sub", LayoutPlacementReport::At { offset: 8 }),
                    ],
                    offsets: Some(vec![0, 8]),
                    size: Some(32),
                    align: 8,
                },
            )
            .with_inner_layout(SymbolicFieldInnerLayout::new(
                "sub",
                LayoutPlanReport {
                    schema_report_fingerprint: 3,
                    entries: vec![
                        layout_field_entry("entry", LayoutPlacementReport::At { offset: 0 }),
                        layout_field_entry("flags", LayoutPlacementReport::At { offset: 8 }),
                        layout_field_entry("args", LayoutPlacementReport::At { offset: 16 }),
                    ],
                    offsets: Some(vec![0, 8, 16]),
                    size: Some(24),
                    align: 8,
                },
            )),
        )
    }

    pub fn sum_layout() -> ConventionalSumLayoutReport {
        ConventionalSumLayoutReport {
            schema_report_fingerprint: 7,
            tag_offset: 0,
            tag_size: 4,
            tag_align: 4,
            common_fields: Vec::new(),
            cases: vec![
                ConventionalSumCaseLayoutReport {
                    case: "Empty".into(),
                    member_identity: None,
                    ordinal: 0,
                    payload_fields: Vec::new(),
                },
                ConventionalSumCaseLayoutReport {
                    case: "Run".into(),
                    member_identity: None,
                    ordinal: 1,
                    payload_fields: vec![
                        ConventionalSumPayloadFieldLayoutReport {
                            field: "callback".into(),
                            member_identity: None,
                            offset: 8,
                            size: 8,
                            align: 8,
                        },
                        ConventionalSumPayloadFieldLayoutReport {
                            field: "clock".into(),
                            member_identity: None,
                            offset: 16,
                            size: 8,
                            align: 8,
                        },
                    ],
                },
                ConventionalSumCaseLayoutReport {
                    case: "Small".into(),
                    member_identity: None,
                    ordinal: 2,
                    payload_fields: vec![ConventionalSumPayloadFieldLayoutReport {
                        field: "flags".into(),
                        member_identity: None,
                        offset: 8,
                        size: 1,
                        align: 1,
                    }],
                },
            ],
            size: 24,
            align: 8,
        }
    }

    pub fn post_handoff_split_writer_plan() -> crate::SymbolicMaterializationPlan {
        let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
        crate::derive_symbolic_materialization(
            &split_layout(),
            &[symbolic],
            post_handoff_context(),
            |_| None,
        )
        .expect("writer plan")
    }

    pub fn writer_field_offsets(plan: &crate::SymbolicMaterializationPlan) -> Vec<(&str, u64)> {
        plan.actions
            .iter()
            .map(|action| match action {
                crate::MaterializationAction::RuntimeWriter(write) => {
                    (write.field.as_str(), write.container_byte_offset)
                }
                other => panic!("an unresolved symbolic derives a runtime writer, found {other:?}"),
            })
            .collect()
    }

    pub fn stored_integer_layout() -> LayoutPlanReport {
        LayoutPlanReport {
            schema_report_fingerprint: 1,
            entries: vec![layout_field_entry(
                "address",
                LayoutPlacementReport::IntegerAt {
                    offset: 0,
                    stored_width: 32,
                    interpretation: IntegerInterpretation::Unsigned,
                },
            )],
            offsets: None,
            size: Some(4),
            align: 4,
        }
    }

    pub fn record_layout(fingerprint: u64, fields: &[(&str, u64)], size: u64) -> LayoutPlanReport {
        LayoutPlanReport {
            schema_report_fingerprint: fingerprint,
            entries: fields
                .iter()
                .map(|&(field, offset)| {
                    layout_field_entry(field, LayoutPlacementReport::At { offset })
                })
                .collect(),
            offsets: Some(fields.iter().map(|&(_, offset)| offset).collect()),
            size: Some(size),
            align: 8,
        }
    }

    pub fn recursive_record_array_report() -> ConventionalRecursiveRecordSumPathsLayoutReport {
        let element = || ConventionalRecursiveRecordSumPathsLayoutReport {
            outer_layout: record_layout(3, &[("choice", 0), ("pad", 24)], 32),
            children: vec![ConventionalRecordSumChildLayoutReport {
                field: "choice".into(),
                member_identity: None,
                hop: ConventionalRecordSumChildHop::Field,
                interior: ConventionalRecordSumChildInterior::Sum(sum_layout()),
            }],
        };
        ConventionalRecursiveRecordSumPathsLayoutReport {
            outer_layout: record_layout(1, &[("header", 0), ("middle", 8)], 128),
            children: vec![ConventionalRecordSumChildLayoutReport {
                field: "middle".into(),
                member_identity: None,
                hop: ConventionalRecordSumChildHop::Field,
                interior: ConventionalRecordSumChildInterior::Record(
                    ConventionalRecursiveRecordSumPathsLayoutReport {
                        outer_layout: record_layout(
                            2,
                            &[("inner", 0), ("route", 32), ("neighbors", 56)],
                            120,
                        ),
                        children: vec![
                            ConventionalRecordSumChildLayoutReport {
                                field: "inner".into(),
                                member_identity: None,
                                hop: ConventionalRecordSumChildHop::Field,
                                interior: ConventionalRecordSumChildInterior::Record(element()),
                            },
                            ConventionalRecordSumChildLayoutReport {
                                field: "route".into(),
                                member_identity: None,
                                hop: ConventionalRecordSumChildHop::Field,
                                interior: ConventionalRecordSumChildInterior::Sum(sum_layout()),
                            },
                            ConventionalRecordSumChildLayoutReport {
                                field: "neighbors".into(),
                                member_identity: None,
                                hop: ConventionalRecordSumChildHop::Index {
                                    element_count: 2,
                                    element_stride: 32,
                                },
                                interior: ConventionalRecordSumChildInterior::Record(element()),
                            },
                        ],
                    },
                ),
            }],
        }
    }

    pub fn entry() -> RelocationTarget {
        RelocationTarget::Entry(
            EntryStubId::from_normalized_identity(0x55aa).expect("nonzero identity"),
        )
    }

    pub fn data() -> RelocationTarget {
        RelocationTarget::Data(
            DataSymbolId::from_normalized_identity(0x1234).expect("nonzero identity"),
        )
    }

    pub fn constrained_post_handoff_plan() -> (
        crate::SymbolicMaterializationPlan,
        MachineRegimeId,
        ArtifactInstallationScopeId,
    ) {
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
        let plan = crate::derive_symbolic_materialization(
            &layout,
            &[symbolic],
            post_handoff_context_with(constraints),
            |_| None,
        )
        .expect("constrained post-handoff plan");
        (plan, regime, scope)
    }
}

mod target_facts {
    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct LayoutTargetFacts {
        pub profile: String,
        pub pointer_size: u64,
        pub pointer_alignment: u64,
        pub case_tag_size: u64,
        pub case_tag_alignment: u64,
        pub little_endian: bool,
    }
    impl LayoutTargetFacts {
        pub fn validate(&self) -> Result<(), &'static str> {
            if self.profile.is_empty()
                || self.pointer_size == 0
                || !self.pointer_alignment.is_power_of_two()
                || !self.pointer_size.is_multiple_of(self.pointer_alignment)
                || self.case_tag_size == 0
                || !self.case_tag_alignment.is_power_of_two()
                || !self.case_tag_size.is_multiple_of(self.case_tag_alignment)
            {
                return Err(
                    "layout target facts have no complete immutable representation profile",
                );
            }
            Ok(())
        }
    }
}
pub use target_facts::LayoutTargetFacts;

pub use private_callback_placements::{
    compose_private_callback_placements, validate_private_callback_placements,
};
