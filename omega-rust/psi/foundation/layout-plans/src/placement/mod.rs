//! Placement vocabulary: byte order, consumption instants, placement phases,
//! address ranges, machine regimes, installation scopes, and the placement
//! constraints a materialization must satisfy.

use crate::layout_reports::IntegerInterpretation;
use crate::materialization::MaterializationDiagnostic;
use crate::symbolic_values::{RelocationTarget, normalized_layout_identity};

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

normalized_layout_identity!(
    /// Compiler-issued identity of a machine-state regime (for example, x86
    /// long mode). It is a normalized policy identity, not a user-selected
    /// name.
    MachineRegimeId,
    "machine regime"
);

normalized_layout_identity!(
    /// Compiler-issued identity of the attenuated artifact-installation
    /// authority required by a placement. This cites scope; it is not the
    /// capability value.
    ArtifactInstallationScopeId,
    "artifact installation scope"
);

/// Normalized requirements a concrete placement must satisfy. The layout's
/// own alignment is joined into this record during materialization derivation;
/// policy constraints can strengthen it but never weaken it.
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
    pub stored_integer_fit: Option<StoredIntegerFit>,
}

/// Value-domain constraint retained when a symbolic source lands in a
/// narrower stored-integer encoding. Post-handoff resolution must discharge
/// this constraint before any destination byte changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredIntegerFit {
    pub source_width_bits: u16,
    pub stored_width_bits: u16,
    pub interpretation: IntegerInterpretation,
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
