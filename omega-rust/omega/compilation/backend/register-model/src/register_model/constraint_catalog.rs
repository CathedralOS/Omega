//! The constraint catalog: per-instruction operand constraints, ties and
//! implicit effects, the environment's required constraint keys, and the
//! validator that binds a catalog to its model.

use crate::register_model::identities;
use crate::register_model::identities::{
    PhysicalRegisterModelIdentity, RegisterConstraintCatalogIdentity,
};
use crate::{
    PhysicalRegisterModel, RegisterClassId, RegisterUnitId, RegisterViewId,
    ValidatedPhysicalRegisterModel,
};
use target::Architecture;

/// Dense, catalog-local identity for an instruction constraint row.
///
/// IDs are canonical only when they match the row's zero-based position in a
/// validated [`RegisterConstraintCatalog`]. The key, rather than this ID, is
/// the stable identity used to join an instruction inventory to its row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegisterConstraintId(pub u16);

/// The semantic family that owns a register-constraint key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegisterConstraintFamily {
    Call,
    Return,
    SystemCall,
    InlineAssembly,
    Instruction,
}

impl RegisterConstraintFamily {
    /// Stable byte discriminant used by every constraint-family encoder.
    pub const fn tag(self) -> u8 {
        match self {
            Self::Call => 0,
            Self::Return => 1,
            Self::SystemCall => 2,
            Self::InlineAssembly => 3,
            Self::Instruction => 4,
        }
    }
}

/// Stable target-owned identity for one constrained instruction form.
///
/// `variant` is assigned by the target owner. It distinguishes calling
/// conventions, syscall ABIs, inline-assembly forms, or instruction forms
/// without relying on display names or declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegisterConstraintKey {
    pub family: RegisterConstraintFamily,
    pub variant: u32,
}

impl RegisterConstraintKey {
    /// The key constructor every named family below delegates to.
    const fn of(family: RegisterConstraintFamily, variant: u32) -> Self {
        Self { family, variant }
    }

    /// The family half of a key row is spelled here, once. A catalog or target
    /// names only the variant it owns, so the family spelling cannot drift
    /// between the files that mint keys of the same family.
    pub const fn instruction(variant: u32) -> Self {
        Self::of(RegisterConstraintFamily::Instruction, variant)
    }

    pub const fn call(variant: u32) -> Self {
        Self::of(RegisterConstraintFamily::Call, variant)
    }

    pub const fn ret(variant: u32) -> Self {
        Self::of(RegisterConstraintFamily::Return, variant)
    }

    pub const fn system_call(variant: u32) -> Self {
        Self::of(RegisterConstraintFamily::SystemCall, variant)
    }

    pub const fn inline_assembly(variant: u32) -> Self {
        Self::of(RegisterConstraintFamily::InlineAssembly, variant)
    }
}

/// Canonical identity bytes for one constraint key: the family tag byte
/// followed by the variant word. The family half is [`RegisterConstraintFamily::tag`]'s
/// row, so a new family member cannot drift here.
pub fn encode_constraint_key(bytes: &mut Vec<u8>, key: RegisterConstraintKey) {
    bytes.push(key.family.tag());
    crate::codec_bytes::push_encoded(bytes, key.variant.to_le_bytes());
}

/// Exact ordinary instruction keys selected by one target register
/// environment. Named fields prevent positional key drift in its identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetRegisterEnvironmentConstraintKeys {
    pub placed_read: Option<RegisterConstraintKey>,
    pub placed_write: Option<RegisterConstraintKey>,
    pub port_write: Option<RegisterConstraintKey>,
    pub system_register_read: Option<RegisterConstraintKey>,
    pub system_register_write: Option<RegisterConstraintKey>,
    pub control_register_read: Option<RegisterConstraintKey>,
    pub control_register_write: Option<RegisterConstraintKey>,
    pub port_register_read: Option<RegisterConstraintKey>,
    pub port_register_write: Option<RegisterConstraintKey>,
    pub model_specific_read: Option<RegisterConstraintKey>,
    pub model_specific_write: Option<RegisterConstraintKey>,
    pub processor_halt: Option<RegisterConstraintKey>,
    pub interrupt_control: Option<RegisterConstraintKey>,
    pub memory_fence: Option<RegisterConstraintKey>,
    pub atomic_fence: Option<RegisterConstraintKey>,
    pub atomic_load: Option<RegisterConstraintKey>,
    pub atomic_store: Option<RegisterConstraintKey>,
    pub atomic_update: Option<RegisterConstraintKey>,
    pub atomic_compare_exchange: Option<RegisterConstraintKey>,
    /// Bitwise RMW may need a retry sequence with a different register contract.
    pub atomic_bitwise_update: Option<RegisterConstraintKey>,
    pub crash: RegisterConstraintKey,
    pub load64: Option<RegisterConstraintKey>,
    pub load_packed: Option<RegisterConstraintKey>,
    pub store_packed: Option<RegisterConstraintKey>,
    pub load8: Option<RegisterConstraintKey>,
    pub load16: Option<RegisterConstraintKey>,
    pub load32: Option<RegisterConstraintKey>,
    pub load8_indexed: Option<RegisterConstraintKey>,
    pub copy_bytes: Option<RegisterConstraintKey>,
    /// Bulk zero fill: two preserved inputs (destination, count) and one
    /// early-clobber cursor scratch register.
    pub zero_bytes: Option<RegisterConstraintKey>,
    /// Byte-sequence predicate scan: two preserved inputs (base, count), two
    /// early-clobber scratch registers, one defined 0/1 result operand.
    pub scan_byte_sequence: Option<RegisterConstraintKey>,
    /// Exact codec-selected read: preserved address/codec, four early-clobber homes.
    pub load_encoded_scalar: Option<RegisterConstraintKey>,
    /// Exact codec-selected write: preserved address/codec/value, three early-clobber homes.
    pub store_encoded_scalar: Option<RegisterConstraintKey>,
    /// Byte-sequence content compare: four preserved inputs (left base/left
    /// count/right base/right count), three early-clobber scratch registers,
    /// one defined 0/1 result operand.
    pub compare_byte_sequences: Option<RegisterConstraintKey>,
    pub store: Option<RegisterConstraintKey>,
    pub address_offset: Option<RegisterConstraintKey>,
    pub store64: Option<RegisterConstraintKey>,
    pub frame_address: Option<RegisterConstraintKey>,
    /// PC-relative byte-literal image address; absent where the target has no
    /// read-only image storage to name.
    pub byte_literal_image_address: Option<RegisterConstraintKey>,
    pub hosted_read_byte: Option<RegisterConstraintKey>,
    pub hosted_write_byte_i32: Option<RegisterConstraintKey>,
    pub save_floating_control: Option<RegisterConstraintKey>,
    pub restore_floating_control: Option<RegisterConstraintKey>,
    pub hosted_exit_process_i32: Option<RegisterConstraintKey>,
    /// Target-owned resultless call keys indexed by argument count, including zero.
    /// Empty means this environment supplies no Unit register-call form.
    pub call_unit: Vec<RegisterConstraintKey>,
    /// Canonical mixed integer/FP register call rows, independently matched to
    /// the ABI placements.
    pub call_unit_mixed: Vec<RegisterConstraintKey>,
    /// Target-owned register-call keys indexed by argument count, including zero.
    /// The legacy integer-only prefix retains its arity order; appended rows
    /// include FP. Empty means this environment supplies no scalar register-call
    /// form.
    pub call_scalar: Vec<RegisterConstraintKey>,
    /// Argument-count-major rows matched by their complete ABI operand roster,
    /// with one- and two-fragment results per count. Empty means direct
    /// aggregate calls are unsupported in this environment.
    pub call_aggregate: Vec<RegisterConstraintKey>,
    /// Per-plan normalized foreign call rows, matched against the evaluated
    /// boundary-entry plan's complete fixed ABI operand views. The roster
    /// carries result-bearing and Unit-result arities alike. Empty means this
    /// environment supplies no foreign call forms.
    pub call_normalized_foreign: Vec<RegisterConstraintKey>,
    /// Direct returns indexed by fragment count minus one; no hidden pointer ABI.
    pub return_aggregate: Vec<RegisterConstraintKey>,
    pub materialize_i64: RegisterConstraintKey,
    pub materialize_boolean: RegisterConstraintKey,
    pub copy_i64: RegisterConstraintKey,
    pub float32_to_bits: Option<RegisterConstraintKey>,
    pub float64_to_bits: Option<RegisterConstraintKey>,
    pub bits_to_float32: Option<RegisterConstraintKey>,
    pub bits_to_float64: Option<RegisterConstraintKey>,
    /// Nearest-even IEEE arithmetic on general-register payloads: two inputs
    /// and one result, with fixed scratch floating registers clobbered.
    pub ieee_float_arithmetic: Option<RegisterConstraintKey>,
    /// Explicit binary rounding with exact local control preservation. An ISA
    /// may share the ordinary row when its existing scratch suffices.
    pub ieee_float_directed_binary: Option<RegisterConstraintKey>,
    /// Explicit unary rounding (directed square root) with the same local
    /// control preservation and one general-register input. An ISA may share
    /// the ordinary unary row when its existing scratch suffices.
    pub ieee_float_directed_unary: Option<RegisterConstraintKey>,
    /// The same IEEE forms with one general-register input (negation, square
    /// root and conversions) and one result.
    pub ieee_float_unary: Option<RegisterConstraintKey>,
    /// Unary IEEE integer conversions whose target encoding also clobbers
    /// control state such as x86-64 RFLAGS.
    pub ieee_float_conversion_control: Option<RegisterConstraintKey>,
    pub ieee_float_ternary: Option<RegisterConstraintKey>,
    pub add_i64: RegisterConstraintKey,
    pub add_i64_immediate: RegisterConstraintKey,
    pub subtract_i64: RegisterConstraintKey,
    pub multiply_i64: RegisterConstraintKey,
    pub saturating_subtract_unsigned: RegisterConstraintKey,
    pub saturating_add_u64: RegisterConstraintKey,
    pub divide_u64: RegisterConstraintKey,
    pub remainder_u64: RegisterConstraintKey,
    pub remainder_i64: RegisterConstraintKey,
    pub divide_i64: RegisterConstraintKey,
    /// Two-source variable-count shift row. The owning target decides how the
    /// count source is constrained (x86-64 pins it to `rcx`; AArch64 leaves
    /// all operands allocatable).
    pub shift_i64: RegisterConstraintKey,
    pub saturating_add_clamped: RegisterConstraintKey,
    pub saturating_subtract_clamped: RegisterConstraintKey,
    pub saturating_divide_signed: RegisterConstraintKey,
    /// Saturating multiplication of every carrier but u64: two inputs, an
    /// early-clobber result, and an early-clobber scratch (the clamp bound, or
    /// the i64 product's high half or saturated value).
    pub saturating_multiply_clamped: RegisterConstraintKey,
    /// Saturating u64 multiplication: the full product's high half decides
    /// overflow, so x86-64 pins `MUL` to RAX:RDX (right operand in RCX) while
    /// AArch64 keeps the clamped four-operand shape for `UMULH`.
    pub saturating_multiply_u64: RegisterConstraintKey,
    /// Trapping add, subtract and multiply (every carrier but the x86-64 u64
    /// multiply): two inputs, an early-clobber result, and an early-clobber
    /// scratch for the narrow range check or the i64 product's high half.
    pub trapping_binary: RegisterConstraintKey,
    /// Trapping divide and remainder at every carrier, and the u64 multiply.
    /// x86-64 pins the dividend and result to RAX, the divisor or right operand
    /// to RCX, and the scratch to RDX (`IDIV`/`DIV`/`MUL`); AArch64 keeps the
    /// allocatable four-operand shape of `trapping_binary`.
    pub trapping_fixed_pair: RegisterConstraintKey,
    /// Trapping shifts: value and count read, early-clobber result and scratch
    /// defined. x86-64 pins the count to RCX for `CL`.
    pub trapping_shift: RegisterConstraintKey,
    /// Trapping conversion: one input, an early-clobber result, and an
    /// early-clobber scratch for the representability check.
    pub trapping_convert: RegisterConstraintKey,
    /// Post-call IEEE adapters over one raw result or two inputs plus raw result.
    /// The trapping form is a completed raw result followed by its result
    /// definition; target predicates declare their actual scratch and control
    /// clobbers separately.
    pub float_result_trapping: Option<RegisterConstraintKey>,
    /// The saturating form takes the original left, right, and completed raw
    /// IEEE result, then the output. No arithmetic is repeated and no
    /// floating-control read is implied.
    pub float_result_saturating: Option<RegisterConstraintKey>,
    pub subtract_i64_immediate: RegisterConstraintKey,
    pub compare_i64_zero: RegisterConstraintKey,
    pub compare_i64: RegisterConstraintKey,
    pub compare_i64_immediate: RegisterConstraintKey,
    pub conditional_branch: RegisterConstraintKey,
    pub jump: RegisterConstraintKey,
    pub return_i64: RegisterConstraintKey,
    pub return_float: Vec<RegisterConstraintKey>,
    pub return_unit: RegisterConstraintKey,
}

/// Dataflow access performed by an explicit instruction operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterOperandAccess {
    Use,
    Def,
    UseDef,
}

impl RegisterOperandAccess {
    const fn reads(self) -> bool {
        matches!(self, Self::Use | Self::UseDef)
    }

    const fn writes(self) -> bool {
        matches!(self, Self::Def | Self::UseDef)
    }

    /// The canonical wire tag codecs write for this access kind.
    pub const fn access_tag(self) -> u8 {
        match self {
            Self::Use => 0,
            Self::Def => 1,
            Self::UseDef => 2,
        }
    }
}

/// The operand views annotated with exactly `access`, in request order.
pub fn views_with_access(
    views: &[RegisterViewId],
    accesses: &[RegisterOperandAccess],
    access: RegisterOperandAccess,
) -> Vec<RegisterViewId> {
    views
        .iter()
        .zip(accesses)
        .filter_map(|(view, actual)| (*actual == access).then_some(*view))
        .collect()
}

/// Allocation constraints for one explicit instruction operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterOperandConstraint {
    pub operand: u16,
    pub access: RegisterOperandAccess,
    pub class: RegisterClassId,
    pub fixed_view: Option<RegisterViewId>,
    /// Canonical one-way tie to an earlier operand number.
    pub tied_to: Option<u16>,
    /// The write happens before unrelated input operands have all been read.
    pub early_clobber: bool,
}

/// Complete register effects for one constrained instruction form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterInstructionConstraint {
    pub id: RegisterConstraintId,
    pub key: RegisterConstraintKey,
    pub operands: Vec<RegisterOperandConstraint>,
    pub implicit_uses: Vec<RegisterUnitId>,
    pub implicit_defs: Vec<RegisterUnitId>,
    pub clobbers: Vec<RegisterUnitId>,
}

impl RegisterInstructionConstraint {
    /// The operand constraint numbered `operand`, when declared.
    pub fn operand(&self, operand: u16) -> Option<&RegisterOperandConstraint> {
        self.operands.iter().find(|row| row.operand == operand)
    }

    /// The row's read contract: the numbered operands whose incoming values
    /// the form reads, in declared order. Admission checks an encoded
    /// alternative's external reads against exactly this list.
    pub fn operand_reads(&self) -> Vec<u16> {
        self.operands
            .iter()
            .filter(|operand| operand.access.reads())
            .map(|operand| operand.operand)
            .collect()
    }

    /// The row's write contract: the numbered operands whose homes the form
    /// defines, in declared order. Admission checks an encoded alternative's
    /// external writes against exactly this list.
    pub fn operand_writes(&self) -> Vec<u16> {
        self.operands
            .iter()
            .filter(|operand| operand.access.writes())
            .map(|operand| operand.operand)
            .collect()
    }
}

/// A target register-constraint inventory and its keyed definitions.
///
/// Both vectors are strictly key-sorted in the validated form. `required`
/// must match the row keys exactly, making omitted and unexpected instruction
/// forms deterministic validation failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterConstraintCatalog {
    pub architecture: Architecture,
    pub required: Vec<RegisterConstraintKey>,
    pub constraints: Vec<RegisterInstructionConstraint>,
}

impl RegisterConstraintCatalog {
    /// The constraint row keyed `key`, when the catalog defines it.
    pub fn constraint(&self, key: RegisterConstraintKey) -> Option<&RegisterInstructionConstraint> {
        find_register_constraint(&self.constraints, key)
    }

    /// The row among `keys` whose declared operand views and accesses match
    /// `views` and `accesses` positionally, when one exists.
    pub fn row_matching(
        &self,
        keys: &[RegisterConstraintKey],
        views: &[RegisterViewId],
        accesses: &[RegisterOperandAccess],
    ) -> Option<&RegisterInstructionConstraint> {
        self.constraints.iter().find(|row| {
            keys.contains(&row.key)
                && row.operands.len() == views.len()
                && row.operands.len() == accesses.len()
                && row.operands.iter().zip(views.iter().zip(accesses)).all(
                    |(operand, (view, access))| {
                        operand.fixed_view == Some(*view) && operand.access == *access
                    },
                )
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRegisterConstraintCatalog {
    architecture: Architecture,
    catalog: RegisterConstraintCatalog,
    physical: PhysicalRegisterModelIdentity,
    identity: RegisterConstraintCatalogIdentity,
}

impl ValidatedRegisterConstraintCatalog {
    pub const fn architecture(&self) -> Architecture {
        self.architecture
    }

    pub const fn catalog(&self) -> &RegisterConstraintCatalog {
        &self.catalog
    }

    pub const fn identity(&self) -> RegisterConstraintCatalogIdentity {
        self.identity
    }

    pub const fn physical_identity(&self) -> PhysicalRegisterModelIdentity {
        self.physical
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterConstraintCatalogValidationError {
    ArchitectureMismatch,
    NonCanonicalConstraintIds,
    NonCanonicalRequiredKeys,
    NonCanonicalConstraintKeys,
    MissingRequiredConstraint(RegisterConstraintKey),
    UnexpectedConstraint(RegisterConstraintKey),
    NonCanonicalOperands(RegisterConstraintId),
    UnknownClass {
        constraint: RegisterConstraintId,
        class: RegisterClassId,
    },
    UnknownFixedView {
        constraint: RegisterConstraintId,
        view: RegisterViewId,
    },
    FixedViewClassMismatch {
        constraint: RegisterConstraintId,
        operand: u16,
    },
    UnallocatableOperandClass {
        constraint: RegisterConstraintId,
        operand: u16,
    },
    InvalidOperandTie {
        constraint: RegisterConstraintId,
        operand: u16,
    },
    IncompatibleOperandTie {
        constraint: RegisterConstraintId,
        operand: u16,
        tied_to: u16,
    },
    InvalidEarlyClobber {
        constraint: RegisterConstraintId,
        operand: u16,
    },
    NonCanonicalImplicitUses(RegisterConstraintId),
    NonCanonicalImplicitDefs(RegisterConstraintId),
    NonCanonicalClobbers(RegisterConstraintId),
    UnknownUnit {
        constraint: RegisterConstraintId,
        unit: RegisterUnitId,
    },
    DefClobberOverlap {
        constraint: RegisterConstraintId,
        unit: RegisterUnitId,
    },
}

invalid_display!(
    RegisterConstraintCatalogValidationError,
    "invalid register constraint catalog"
);

impl std::error::Error for RegisterConstraintCatalogValidationError {}

/// Validates a closed register-constraint inventory against an independently
/// validated physical register model.
pub fn validate_register_constraint_catalog(
    catalog: RegisterConstraintCatalog,
    model: &ValidatedPhysicalRegisterModel,
) -> Result<ValidatedRegisterConstraintCatalog, RegisterConstraintCatalogValidationError> {
    let physical = model.model();
    if catalog.architecture != physical.architecture {
        return Err(RegisterConstraintCatalogValidationError::ArchitectureMismatch);
    }
    if catalog
        .constraints
        .iter()
        .enumerate()
        .any(|(expected, constraint)| usize::from(constraint.id.0) != expected)
    {
        return Err(RegisterConstraintCatalogValidationError::NonCanonicalConstraintIds);
    }
    if catalog.required.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(RegisterConstraintCatalogValidationError::NonCanonicalRequiredKeys);
    }
    if catalog
        .constraints
        .windows(2)
        .any(|pair| pair[0].key >= pair[1].key)
    {
        return Err(RegisterConstraintCatalogValidationError::NonCanonicalConstraintKeys);
    }

    validate_required_constraint_inventory(&catalog)?;

    for constraint in &catalog.constraints {
        // A register-free instruction can still have memory or ordering effects.
        // This structural catalog checks declared register constraints; the target
        // owner authenticates the exact keyed row and its machine effects.
        if constraint
            .operands
            .windows(2)
            .any(|pair| pair[0].operand >= pair[1].operand)
        {
            return Err(
                RegisterConstraintCatalogValidationError::NonCanonicalOperands(constraint.id),
            );
        }

        for operand in &constraint.operands {
            let Some(class) = physical.class(operand.class) else {
                return Err(RegisterConstraintCatalogValidationError::UnknownClass {
                    constraint: constraint.id,
                    class: operand.class,
                });
            };
            if let Some(fixed_view) = operand.fixed_view {
                let Some(view) = physical.view(fixed_view) else {
                    return Err(RegisterConstraintCatalogValidationError::UnknownFixedView {
                        constraint: constraint.id,
                        view: fixed_view,
                    });
                };
                if view.class != operand.class {
                    return Err(
                        RegisterConstraintCatalogValidationError::FixedViewClassMismatch {
                            constraint: constraint.id,
                            operand: operand.operand,
                        },
                    );
                }
            } else if !class
                .views
                .iter()
                .any(|view| physical.view(*view).is_some_and(|view| view.allocatable))
            {
                return Err(
                    RegisterConstraintCatalogValidationError::UnallocatableOperandClass {
                        constraint: constraint.id,
                        operand: operand.operand,
                    },
                );
            }
            if operand.early_clobber && !operand.access.writes() {
                return Err(
                    RegisterConstraintCatalogValidationError::InvalidEarlyClobber {
                        constraint: constraint.id,
                        operand: operand.operand,
                    },
                );
            }
        }
        validate_operand_ties(constraint)?;

        validate_constraint_unit_set(
            constraint.id,
            &constraint.implicit_uses,
            physical,
            ConstraintUnitSetKind::ImplicitUses,
        )?;
        validate_constraint_unit_set(
            constraint.id,
            &constraint.implicit_defs,
            physical,
            ConstraintUnitSetKind::ImplicitDefs,
        )?;
        validate_constraint_unit_set(
            constraint.id,
            &constraint.clobbers,
            physical,
            ConstraintUnitSetKind::Clobbers,
        )?;
        if let Some(unit) = constraint
            .implicit_defs
            .iter()
            .find(|unit| constraint.clobbers.binary_search(unit).is_ok())
        {
            return Err(
                RegisterConstraintCatalogValidationError::DefClobberOverlap {
                    constraint: constraint.id,
                    unit: *unit,
                },
            );
        }
    }

    let identity = identities::register_constraint_catalog_identity(model.identity(), &catalog);
    Ok(ValidatedRegisterConstraintCatalog {
        architecture: catalog.architecture,
        catalog,
        physical: model.identity(),
        identity,
    })
}

fn validate_required_constraint_inventory(
    catalog: &RegisterConstraintCatalog,
) -> Result<(), RegisterConstraintCatalogValidationError> {
    let mut required = catalog.required.iter().copied().peekable();
    let mut actual = catalog
        .constraints
        .iter()
        .map(|constraint| constraint.key)
        .peekable();
    loop {
        match (required.peek().copied(), actual.peek().copied()) {
            (Some(expected), Some(found)) if expected == found => {
                required.next();
                actual.next();
            }
            (Some(expected), Some(found)) if expected < found => {
                return Err(
                    RegisterConstraintCatalogValidationError::MissingRequiredConstraint(expected),
                );
            }
            (Some(_), Some(found)) => {
                return Err(RegisterConstraintCatalogValidationError::UnexpectedConstraint(found));
            }
            (Some(expected), None) => {
                return Err(
                    RegisterConstraintCatalogValidationError::MissingRequiredConstraint(expected),
                );
            }
            (None, Some(found)) => {
                return Err(RegisterConstraintCatalogValidationError::UnexpectedConstraint(found));
            }
            (None, None) => return Ok(()),
        }
    }
}

fn validate_operand_ties(
    constraint: &RegisterInstructionConstraint,
) -> Result<(), RegisterConstraintCatalogValidationError> {
    for operand in &constraint.operands {
        let Some(tied_to) = operand.tied_to else {
            continue;
        };
        if tied_to >= operand.operand {
            return Err(
                RegisterConstraintCatalogValidationError::InvalidOperandTie {
                    constraint: constraint.id,
                    operand: operand.operand,
                },
            );
        }
        let Ok(tied_index) = constraint
            .operands
            .binary_search_by_key(&tied_to, |candidate| candidate.operand)
        else {
            return Err(
                RegisterConstraintCatalogValidationError::InvalidOperandTie {
                    constraint: constraint.id,
                    operand: operand.operand,
                },
            );
        };
        let tied = &constraint.operands[tied_index];
        if tied.tied_to.is_some() {
            return Err(
                RegisterConstraintCatalogValidationError::InvalidOperandTie {
                    constraint: constraint.id,
                    operand: operand.operand,
                },
            );
        }
        let fixed_views_compatible = match (operand.fixed_view, tied.fixed_view) {
            (Some(left), Some(right)) => left == right,
            _ => true,
        };
        let pair_reads = operand.access.reads() || tied.access.reads();
        let pair_writes = operand.access.writes() || tied.access.writes();
        if operand.class != tied.class || !fixed_views_compatible || !pair_reads || !pair_writes {
            return Err(
                RegisterConstraintCatalogValidationError::IncompatibleOperandTie {
                    constraint: constraint.id,
                    operand: operand.operand,
                    tied_to,
                },
            );
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum ConstraintUnitSetKind {
    ImplicitUses,
    ImplicitDefs,
    Clobbers,
}

fn validate_constraint_unit_set(
    constraint: RegisterConstraintId,
    set: &[RegisterUnitId],
    model: &PhysicalRegisterModel,
    kind: ConstraintUnitSetKind,
) -> Result<(), RegisterConstraintCatalogValidationError> {
    if set.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(match kind {
            ConstraintUnitSetKind::ImplicitUses => {
                RegisterConstraintCatalogValidationError::NonCanonicalImplicitUses(constraint)
            }
            ConstraintUnitSetKind::ImplicitDefs => {
                RegisterConstraintCatalogValidationError::NonCanonicalImplicitDefs(constraint)
            }
            ConstraintUnitSetKind::Clobbers => {
                RegisterConstraintCatalogValidationError::NonCanonicalClobbers(constraint)
            }
        });
    }
    if let Some(unit) = set.iter().find(|unit| model.unit(**unit).is_none()) {
        return Err(RegisterConstraintCatalogValidationError::UnknownUnit {
            constraint,
            unit: *unit,
        });
    }
    Ok(())
}

pub fn call_constraint_keys(variants: std::ops::Range<u16>) -> Vec<RegisterConstraintKey> {
    variants
        .map(|variant| RegisterConstraintKey::call(u32::from(variant)))
        .collect()
}

/// The constraint row keyed `key` in a constraint row list.
pub fn find_register_constraint<'rows>(
    rows: &'rows [RegisterInstructionConstraint],
    key: RegisterConstraintKey,
) -> Option<&'rows RegisterInstructionConstraint> {
    rows.iter().find(|row| row.key == key)
}

/// The mutable constraint row keyed `key` in a constraint row list.
pub fn find_register_constraint_mut<'rows>(
    rows: &'rows mut [RegisterInstructionConstraint],
    key: RegisterConstraintKey,
) -> Option<&'rows mut RegisterInstructionConstraint> {
    rows.iter_mut().find(|row| row.key == key)
}
