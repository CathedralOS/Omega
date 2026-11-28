//! Lend a component the caller’s scalar backing until its invocation ends.

use crate::boundary::effects::TerminalEffectRejection;
use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::values::TerminalScalarValue;
use crate::engine::scalar_operations::terminal_scalar_belongs_to_type;
use crate::storage::scalar_storage::{ScalarCell, TerminalScalarBacking};
use semantic_vocabulary::ScalarType;
use std::cell::Cell;
use std::rc::{Rc, Weak};
use terminal_psi::StructuralAccess;

/// Shared references can perform atomic events only on actual atomic leaves.
/// Ordinary scalar leaves of the same record retain shared access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScalarLoanAccess {
    pub(crate) borrow: StructuralAccess,
    pub(crate) atomic: Option<semantic_vocabulary::AtomicCellType>,
}

impl ScalarLoanAccess {
    pub(crate) fn may_alias(self, other: Self) -> bool {
        self.borrow == StructuralAccess::SharedBorrow && self == other
    }

    pub(crate) fn writable(self) -> bool {
        self.borrow == StructuralAccess::MutableBorrow || self.atomic.is_some()
    }
}

pub(crate) struct ScalarLoanScope(Rc<()>);

impl ScalarLoanScope {
    pub(crate) fn new() -> Self {
        Self(Rc::new(()))
    }

    pub(crate) fn lend(
        &self,
        original: &mut ScalarCell,
        access: ScalarLoanAccess,
    ) -> Result<ScalarCell, TerminalInterpretError> {
        if !matches!(access.borrow, terminal_psi::reading_borrow_pair_pattern!()) {
            return Err(malformed());
        }
        if access
            .atomic
            .is_some_and(|atom| atom.scalar_type() != original.scalar_type())
        {
            return Err(malformed());
        }
        if let ScalarCell::Local(value) = original {
            let scalar_type = value.scalar_type();
            let backing = Rc::new(GuestScalarHome {
                value: Cell::new(*value),
                scalar_type,
            });
            *original = ScalarCell::Resident {
                scalar_type,
                backing,
            };
        }
        let ScalarCell::Resident {
            scalar_type,
            backing,
        } = original
        else {
            return Err(malformed());
        };
        if let Some(atom) = access.atomic {
            // One guest step is indivisible for a guest-owned cell. A host
            // backing can have concurrent writers outside that scheduler.
            let home: &dyn std::any::Any = backing.as_ref();
            if !home.is::<GuestScalarHome>()
                && !home
                    .downcast_ref::<ComponentScalarView>()
                    .is_some_and(|parent| parent.access.atomic == Some(atom))
            {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "component atomic loan requires the original guest cell",
                ));
            }
        }
        Ok(ScalarCell::Resident {
            scalar_type: *scalar_type,
            backing: Rc::new(ComponentScalarView {
                original: Rc::clone(backing),
                lifetime: Rc::downgrade(&self.0),
                access,
                scalar_type: *scalar_type,
            }),
        })
    }
}

#[derive(Debug)]
struct GuestScalarHome {
    value: Cell<TerminalScalarValue>,
    scalar_type: ScalarType,
}

impl TerminalScalarBacking for GuestScalarHome {
    fn read(&self) -> Result<TerminalScalarValue, TerminalEffectRejection> {
        Ok(self.value.get())
    }

    fn write(&self, value: TerminalScalarValue) -> Result<(), TerminalEffectRejection> {
        validate_scalar(self.scalar_type, value)?;
        self.value.set(value);
        Ok(())
    }
}

#[derive(Debug)]
struct ComponentScalarView {
    original: Rc<dyn TerminalScalarBacking>,
    lifetime: Weak<()>,
    access: ScalarLoanAccess,
    scalar_type: ScalarType,
}

impl ComponentScalarView {
    fn validate(&self, value: TerminalScalarValue) -> Result<(), TerminalEffectRejection> {
        validate_scalar(self.scalar_type, value)?;
        if let Some(semantic_vocabulary::AtomicCellType::BoundedInteger(bounds)) =
            self.access.atomic
            && !matches!(value, TerminalScalarValue::Integer { value, .. } if bounds.contains(value))
        {
            return Err(TerminalEffectRejection::new(
                "component atomic value exceeds its declared bounds",
            ));
        }
        Ok(())
    }

    fn live(&self) -> Result<Rc<()>, TerminalEffectRejection> {
        self.lifetime
            .upgrade()
            .ok_or_else(|| TerminalEffectRejection::new("component scalar loan has ended"))
    }
}

impl TerminalScalarBacking for ComponentScalarView {
    fn read(&self) -> Result<TerminalScalarValue, TerminalEffectRejection> {
        let _lifetime = self.live()?;
        let value = self.original.read()?;
        self.validate(value)?;
        Ok(value)
    }

    fn write(&self, value: TerminalScalarValue) -> Result<(), TerminalEffectRejection> {
        let _lifetime = self.live()?;
        if !self.access.writable() {
            return Err(TerminalEffectRejection::new(
                "shared component scalar loan cannot mutate its original home",
            ));
        }
        self.validate(value)?;
        self.original.write(value)
    }
}

fn validate_scalar(
    expected: ScalarType,
    value: TerminalScalarValue,
) -> Result<(), TerminalEffectRejection> {
    if value.scalar_type() != expected || !terminal_scalar_belongs_to_type(value) {
        return Err(TerminalEffectRejection::new(
            "component scalar loan changed its stored carrier",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

    fn ordinary(borrow: StructuralAccess) -> ScalarLoanAccess {
        ScalarLoanAccess {
            borrow,
            atomic: None,
        }
    }

    fn word(value: u128) -> TerminalScalarValue {
        TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            value: IntegerValue::Unsigned(value),
        }
    }

    #[test]
    fn component_views_write_original_cells_and_cannot_outlive_their_owner() {
        let mut original = ScalarCell::Local(word(0));
        let owner = ScalarLoanScope::new();
        let mut view = owner
            .lend(&mut original, ordinary(StructuralAccess::MutableBorrow))
            .unwrap();
        view.write(word(73)).unwrap();
        assert_eq!(original.read().unwrap(), word(73));
        let mut retained = view.clone();
        drop(owner);
        assert!(retained.write(word(91)).is_err());
        assert!(retained.read().is_err());
        assert_eq!(original.read().unwrap(), word(73));
    }

    #[test]
    fn shared_atomic_views_alias_the_guest_cell_for_one_instruction_at_a_time() {
        let mut original = ScalarCell::Local(word(37));
        let scope = ScalarLoanScope::new();
        let access = ScalarLoanAccess {
            borrow: StructuralAccess::SharedBorrow,
            atomic: semantic_vocabulary::AtomicCellType::from_scalar(word(0).scalar_type()),
        };
        let mut first = scope.lend(&mut original, access).unwrap();
        let mut second = scope.lend(&mut original, access).unwrap();
        first.write(word(73)).unwrap();
        assert_eq!(second.read().unwrap(), word(73));
        second.write(word(19)).unwrap();
        assert_eq!(original.read().unwrap(), word(19));
        let mut external = ScalarCell::Resident {
            scalar_type: word(0).scalar_type(),
            backing: Rc::new(
                crate::storage::scalar_storage::TerminalInvocationScalar::new(word(0)).unwrap(),
            ),
        };
        assert!(scope.lend(&mut external, access).is_err());
        drop(scope);
        assert!(first.write(word(0)).is_err());
        assert!(second.read().is_err());
    }

    #[test]
    fn a_nested_atomic_loan_preserves_the_original_bounds_and_parent_lifetime() {
        let bounds = semantic_vocabulary::BoundedIntegerType::new(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(10),
        )
        .unwrap();
        let access = ScalarLoanAccess {
            borrow: StructuralAccess::SharedBorrow,
            atomic: Some(semantic_vocabulary::AtomicCellType::BoundedInteger(bounds)),
        };
        let mut original = ScalarCell::Local(word(3));
        let parent = ScalarLoanScope::new();
        let mut shared = parent.lend(&mut original, access).unwrap();
        let child = ScalarLoanScope::new();
        let mut nested = child.lend(&mut shared, access).unwrap();
        assert!(nested.write(word(11)).is_err());
        assert_eq!(original.read().unwrap(), word(3));
        let widened = ScalarLoanAccess {
            atomic: semantic_vocabulary::AtomicCellType::from_scalar(word(0).scalar_type()),
            ..access
        };
        assert!(child.lend(&mut shared, widened).is_err());
        nested.write(word(7)).unwrap();
        assert_eq!(original.read().unwrap(), word(7));
        drop(parent);
        assert!(nested.read().is_err());
    }

    #[test]
    fn nested_views_preserve_the_immediate_parents_access_and_lifetime() {
        let mut original = ScalarCell::Local(word(37));
        let parent = ScalarLoanScope::new();
        let mut shared = parent
            .lend(&mut original, ordinary(StructuralAccess::SharedBorrow))
            .unwrap();
        let child = ScalarLoanScope::new();
        let mut nested = child
            .lend(&mut shared, ordinary(StructuralAccess::MutableBorrow))
            .unwrap();
        assert_eq!(nested.read().unwrap(), word(37));
        assert!(nested.write(word(73)).is_err());
        assert_eq!(original.read().unwrap(), word(37));
        drop(parent);
        assert!(nested.read().is_err());
        assert_eq!(original.read().unwrap(), word(37));
    }
}
