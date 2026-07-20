//! Normalized programmable-layout plans and symbolic materialization.
//!
//! Layout policies describe geometry. A materializer consumes validated
//! geometry plus compiler-issued symbolic values; source programs never
//! receive numeric code addresses or an arbitrary byte-patching primitive.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutPlacementReport {
    At {
        offset: i64,
    },
    Bits {
        container: i64,
        container_width: i64,
        destination_lsb: i64,
        source_lsb: i64,
        width: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutFieldEntryReport {
    /// Normalized field name. Compiler-issued keys do not escape into artifact
    /// reports or identity.
    pub field: String,
    pub placement: LayoutPlacementReport,
}

/// A validated layout plan, ready for consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutPlanReport {
    pub entries: Vec<LayoutFieldEntryReport>,
    /// Declaration-order offsets when every field has one fixed `At`
    /// placement. Fragmented plans deliberately have no such projection.
    pub offsets: Option<Vec<i64>>,
    pub size: Option<i64>,
    pub align: i64,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterializationContext {
    pub consumption: ConsumptionInstant,
    pub byte_order: ByteOrder,
    /// Width accepted by the target container's native absolute relocation.
    /// `None` means no such relocation is available.
    pub native_pointer_relocation_bits: Option<u16>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicMaterializationPlan {
    pub byte_len: usize,
    pub byte_order: ByteOrder,
    pub actions: Vec<MaterializationAction>,
}

impl SymbolicMaterializationPlan {
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
            i64::from(symbolic.width_bits),
            0,
            0,
            i64::from(symbolic.width_bits),
        ),
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
    if container < 0
        || container_width <= 0
        || container_width > 64
        || container_width % 8 != 0
        || destination_lsb < 0
        || source_lsb < 0
        || width <= 0
    {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{}` uses a materializer-incompatible placement",
            symbolic.field
        )));
    }
    let source_end = source_lsb
        .checked_add(width)
        .ok_or_else(|| MaterializationDiagnostic("symbolic source bit range overflows".into()))?;
    if source_end > i64::from(symbolic.width_bits) {
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
        container_byte_offset: u64::try_from(container).expect("non-negative container"),
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

fn apply_write(
    bytes: &mut [u8],
    byte_order: ByteOrder,
    write: &MaterializationWrite,
    source_value: u64,
) -> Result<(), MaterializationDiagnostic> {
    let container_bytes = usize::from(write.container_width_bits / 8);
    let start = usize::try_from(write.container_byte_offset).map_err(|_| {
        MaterializationDiagnostic("container offset cannot be represented on this host".into())
    })?;
    let end = start
        .checked_add(container_bytes)
        .ok_or_else(|| MaterializationDiagnostic("container byte range overflows".into()))?;
    let materialization_len = bytes.len();
    let container_slice = bytes.get_mut(start..end).ok_or_else(|| {
        MaterializationDiagnostic(format!(
            "symbolic field `{}` writes outside the {}-byte materialization",
            write.field, materialization_len
        ))
    })?;
    let mut container_value = read_container(container_slice, byte_order);
    let fragment_mask = low_mask(write.width);
    let fragment = (source_value >> write.source_lsb) & fragment_mask;
    let destination_mask = fragment_mask << write.destination_lsb;
    container_value = (container_value & !destination_mask)
        | ((fragment << write.destination_lsb) & destination_mask);
    write_container(container_slice, byte_order, container_value);
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
            entries: vec![
                LayoutFieldEntryReport {
                    field: "address".into(),
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
    fn unresolved_post_handoff_entry_derives_split_writer() {
        let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
        let plan = derive_symbolic_materialization(
            &split_layout(),
            &[symbolic],
            MaterializationContext {
                consumption: ConsumptionInstant::AfterOmegaHandoff,
                byte_order: ByteOrder::LittleEndian,
                native_pointer_relocation_bits: Some(64),
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
            },
            |_| None,
        )
        .expect_err("a loader cannot apply split pointer relocations");

        assert!(error.0.contains("before Omega entry"));
    }

    #[test]
    fn whole_pointer_uses_loader_native_relocation() {
        let layout = LayoutPlanReport {
            entries: vec![LayoutFieldEntryReport {
                field: "entry".into(),
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
            },
            |_| None,
        )
        .expect("writer plan");
        let mut bytes = [0xa5_u8; 16];
        assert!(plan.materialize_resolved_into(&mut bytes).is_err());
        assert_eq!(bytes, [0xa5; 16]);
    }
}
