//! A component borrows the existing scalar endpoint, never captured contents.
//! The original row keeps its owner. A child holds a call-scoped view of that
//! same cell; retaining the view cannot extend the call or its parent loan.

use super::{ScalarCell, TerminalScalarBacking};
use crate::effects::TerminalEffectRejection;
use crate::errors::{TerminalInterpretError, malformed};
use crate::scalar_operations::terminal_scalar_belongs_to_type;
use crate::values::TerminalScalarValue;
use semantic_vocabulary::ScalarType;
use std::cell::Cell;
use std::rc::{Rc, Weak};
use terminal_psi::StructuralAccess;

/// The pending invocation owns the only strong lifetime token. This owner is
/// not cloneable; a backing view carries only a weak reference to it.
pub(crate) struct ScalarLoanScope(Rc<()>);

impl ScalarLoanScope {
    pub(crate) fn new() -> Self {
        Self(Rc::new(()))
    }

    pub(crate) fn lend(
        &self,
        original: &mut ScalarCell,
        access: StructuralAccess,
    ) -> Result<ScalarCell, TerminalInterpretError> {
        if !matches!(access, terminal_psi::reading_borrow_pair_pattern!()) {
            return Err(malformed());
        }
        // Promotion changes the private representation of the original home.
        // Its contents move into one cell, which the original row retains.
        // No pre-call snapshot is retained for later writeback.
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
    access: StructuralAccess,
    scalar_type: ScalarType,
}

impl ComponentScalarView {
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
        validate_scalar(self.scalar_type, value)?;
        Ok(value)
    }

    fn write(&self, value: TerminalScalarValue) -> Result<(), TerminalEffectRejection> {
        let _lifetime = self.live()?;
        if self.access != StructuralAccess::MutableBorrow {
            return Err(TerminalEffectRejection::new(
                "shared component scalar loan cannot mutate its original home",
            ));
        }
        validate_scalar(self.scalar_type, value)?;
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
            .lend(&mut original, StructuralAccess::MutableBorrow)
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
    fn nested_views_preserve_the_immediate_parents_access_and_lifetime() {
        let mut original = ScalarCell::Local(word(37));
        let parent = ScalarLoanScope::new();
        let mut shared = parent
            .lend(&mut original, StructuralAccess::SharedBorrow)
            .unwrap();
        let child = ScalarLoanScope::new();
        let mut nested = child
            .lend(&mut shared, StructuralAccess::MutableBorrow)
            .unwrap();
        assert_eq!(nested.read().unwrap(), word(37));
        assert!(nested.write(word(73)).is_err());
        assert_eq!(original.read().unwrap(), word(37));
        drop(parent);
        assert!(nested.read().is_err());
        assert_eq!(original.read().unwrap(), word(37));
    }
}
