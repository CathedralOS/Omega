//! The constraint catalog: per-instruction operand constraints, ties and
//! implicit effects, the environment's required constraint keys, and the
//! validator that binds a catalog to its model.

use crate::identities;
use crate::identities::{PhysicalRegisterModelIdentity, RegisterConstraintCatalogIdentity};
use crate::{
    RegisterClassId, RegisterUnit, RegisterUnitId, RegisterViewId, ValidatedPhysicalRegisterModel,
};
use std::collections::BTreeMap;
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

/// Exact ordinary instruction keys selected by one target register
/// environment. Named fields prevent positional key drift in its identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetRegisterEnvironmentConstraintKeys {
    pub load64: Option<RegisterConstraintKey>,
    pub load_packed: Option<RegisterConstraintKey>,
    pub store_packed: Option<RegisterConstraintKey>,
    pub load8: Option<RegisterConstraintKey>,
    pub load16: Option<RegisterConstraintKey>,
    pub load32: Option<RegisterConstraintKey>,
    pub load8_indexed: Option<RegisterConstraintKey>,
    pub copy_bytes: Option<RegisterConstraintKey>,
    pub store: Option<RegisterConstraintKey>,
    pub address_offset: Option<RegisterConstraintKey>,
    pub store64: Option<RegisterConstraintKey>,
    pub frame_address: Option<RegisterConstraintKey>,
    pub hosted_read_byte: Option<RegisterConstraintKey>,
    pub hosted_write_byte_i32: Option<RegisterConstraintKey>,
    pub hosted_exit_process_i32: Option<RegisterConstraintKey>,
    /// Target-owned resultless call keys indexed by argument count, including zero.
    /// Empty means this environment supplies no Unit register-call form.
    pub call_unit: Vec<RegisterConstraintKey>,
    pub call_unit_mixed: Vec<RegisterConstraintKey>,
    /// Target-owned register-call keys indexed by argument count, including zero.
    /// Empty means this environment supplies no scalar register-call form.
    pub call_scalar: Vec<RegisterConstraintKey>,
    /// Argument-count-major rows, with one- and two-fragment results per count.
    /// Empty means direct aggregate calls are unsupported in this environment.
    pub call_aggregate: Vec<RegisterConstraintKey>,
    /// Direct returns indexed by fragment count minus one; no hidden pointer ABI.
    pub return_aggregate: Vec<RegisterConstraintKey>,
    pub materialize_i64: RegisterConstraintKey,
    pub materialize_boolean: RegisterConstraintKey,
    pub copy_i64: RegisterConstraintKey,
    pub float32_to_bits: Option<RegisterConstraintKey>,
    pub float64_to_bits: Option<RegisterConstraintKey>,
    pub bits_to_float32: Option<RegisterConstraintKey>,
    pub bits_to_float64: Option<RegisterConstraintKey>,
    pub add_i64: RegisterConstraintKey,
    pub add_i64_immediate: RegisterConstraintKey,
    pub subtract_i64: RegisterConstraintKey,
    pub multiply_i64: RegisterConstraintKey,
    pub saturating_subtract_unsigned: RegisterConstraintKey,
    pub saturating_add_u64: RegisterConstraintKey,
    pub divide_u64: RegisterConstraintKey,
    pub remainder_i64: RegisterConstraintKey,
    pub saturating_add_clamped: RegisterConstraintKey,
    pub saturating_subtract_clamped: RegisterConstraintKey,
    pub saturating_divide_signed: RegisterConstraintKey,
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

    pub fn into_catalog(self) -> RegisterConstraintCatalog {
        self.catalog
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
    EmptyConstraint(RegisterConstraintId),
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

impl std::fmt::Display for RegisterConstraintCatalogValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid register constraint catalog: {self:?}")
    }
}

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

    let units = physical
        .units
        .iter()
        .map(|unit| (unit.id, unit))
        .collect::<BTreeMap<_, _>>();
    let views = physical
        .views
        .iter()
        .map(|view| (view.id, view))
        .collect::<BTreeMap<_, _>>();
    let classes = physical
        .classes
        .iter()
        .map(|class| (class.id, class))
        .collect::<BTreeMap<_, _>>();

    for constraint in &catalog.constraints {
        if constraint.operands.is_empty()
            && constraint.implicit_uses.is_empty()
            && constraint.implicit_defs.is_empty()
            && constraint.clobbers.is_empty()
        {
            return Err(RegisterConstraintCatalogValidationError::EmptyConstraint(
                constraint.id,
            ));
        }
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
            let Some(class) = classes.get(&operand.class) else {
                return Err(RegisterConstraintCatalogValidationError::UnknownClass {
                    constraint: constraint.id,
                    class: operand.class,
                });
            };
            if let Some(fixed_view) = operand.fixed_view {
                let Some(view) = views.get(&fixed_view) else {
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
                .any(|view| views.get(view).is_some_and(|view| view.allocatable))
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
            &units,
            ConstraintUnitSetKind::ImplicitUses,
        )?;
        validate_constraint_unit_set(
            constraint.id,
            &constraint.implicit_defs,
            &units,
            ConstraintUnitSetKind::ImplicitDefs,
        )?;
        validate_constraint_unit_set(
            constraint.id,
            &constraint.clobbers,
            &units,
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
    known: &BTreeMap<RegisterUnitId, &RegisterUnit>,
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
    if let Some(unit) = set.iter().find(|unit| !known.contains_key(unit)) {
        return Err(RegisterConstraintCatalogValidationError::UnknownUnit {
            constraint,
            unit: *unit,
        });
    }
    Ok(())
}
