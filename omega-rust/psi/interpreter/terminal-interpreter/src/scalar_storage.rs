//! Mutable scalar homes keep one source of contents. A resident replaces its
//! local payload; ordinary reads and writes then reach that original home.
//! Copying observes a value into new local storage, while a move retains the
//! same backing. Target encodings and native pointers remain with the host.

use crate::{
    effects::TerminalEffectRejection,
    errors::{TerminalInterpretError, malformed},
    runtime_rows::RuntimeRows,
    values::TerminalScalarValue,
};
use evaluation_accounting::EvaluationInvocationAccount;
use semantic_vocabulary::ScalarType;
use std::{collections::BTreeMap, rc::Rc};

/// One admitted scalar endpoint of original storage. The host owns its actual
/// geometry and lifetime. This interface grants no extra access: the engine
/// must authenticate the operation's ordinary loan and storage restrictions.
pub trait TerminalScalarBacking: std::fmt::Debug {
    fn read(&self) -> Result<TerminalScalarValue, TerminalEffectRejection>;
    fn write(&self, value: TerminalScalarValue) -> Result<(), TerminalEffectRejection>;
}

#[derive(Debug, Clone)]
pub(crate) enum ScalarCell {
    Local(TerminalScalarValue),
    Resident {
        scalar_type: ScalarType,
        backing: Rc<dyn TerminalScalarBacking>,
    },
}

impl ScalarCell {
    pub(crate) fn resident(
        original: TerminalScalarValue,
        backing: Rc<dyn TerminalScalarBacking>,
    ) -> Result<Self, TerminalInterpretError> {
        let cell = Self::Resident {
            scalar_type: original.scalar_type(),
            backing,
        };
        if cell.read()? != original {
            return Err(TerminalInterpretError::ResidentStorage(
                TerminalEffectRejection::new(
                    "resident initialization differs from original scalar contents",
                ),
            ));
        }
        Ok(cell)
    }
    pub(crate) fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Local(value) => value.scalar_type(),
            Self::Resident { scalar_type, .. } => *scalar_type,
        }
    }
    pub(crate) fn read(&self) -> Result<TerminalScalarValue, TerminalInterpretError> {
        match self {
            Self::Local(value) => Ok(*value),
            Self::Resident {
                scalar_type,
                backing,
            } => {
                let value = backing
                    .read()
                    .map_err(TerminalInterpretError::ResidentStorage)?;
                if value.scalar_type() != *scalar_type
                    || !crate::scalar_operations::terminal_scalar_belongs_to_type(value)
                {
                    return Err(malformed());
                }
                Ok(value)
            }
        }
    }

    pub(crate) fn write(
        &mut self,
        value: TerminalScalarValue,
    ) -> Result<(), TerminalInterpretError> {
        match self {
            Self::Local(previous) => *previous = value,
            Self::Resident {
                scalar_type,
                backing,
            } => {
                if value.scalar_type() != *scalar_type
                    || !crate::scalar_operations::terminal_scalar_belongs_to_type(value)
                {
                    return Err(malformed());
                }
                backing
                    .write(value)
                    .map_err(TerminalInterpretError::ResidentStorage)?;
            }
        }
        Ok(())
    }

    pub(crate) fn relocated(&self, transfer: bool) -> Result<Self, TerminalInterpretError> {
        if transfer {
            Ok(self.clone())
        } else {
            self.read().map(Self::Local)
        }
    }
}

/// Funded mutable cells. Inserting a new value into a resident changes its
/// original storage instead of silently replacing it with a detached payload.
#[derive(Debug)]
pub(crate) struct ScalarStorageRows<Key>(RuntimeRows<Key, ScalarCell>);

impl<Key: Ord> ScalarStorageRows<Key> {
    pub(crate) fn new(account: Option<&EvaluationInvocationAccount>) -> Self {
        Self(RuntimeRows::new(account))
    }
    pub(crate) fn insert(
        &mut self,
        key: Key,
        value: TerminalScalarValue,
    ) -> Result<Option<ScalarCell>, TerminalInterpretError> {
        if let Some(cell) = self.0.get_mut(&key) {
            let previous = cell.clone();
            cell.write(value)?;
            Ok(Some(previous))
        } else {
            self.0.insert(key, ScalarCell::Local(value))
        }
    }
    /// Transfer already staged custody. Callers distinguish a move from a
    /// value copy through ScalarCell::relocated before staging this row.
    pub(crate) fn insert_cell(
        &mut self,
        key: Key,
        cell: ScalarCell,
    ) -> Result<Option<ScalarCell>, TerminalInterpretError> {
        if self
            .0
            .get(&key)
            .is_some_and(|cell| matches!(cell, ScalarCell::Resident { .. }))
        {
            return self.insert(key, cell.read()?);
        }
        self.0.insert(key, cell)
    }
    pub(crate) fn prune(&mut self, root: &crate::values::StructuralRuntimePlace)
    where
        Key: crate::structural_storage::StorageKey,
    {
        crate::structural_storage::prune_rows(&mut self.0, root);
    }
    pub(crate) fn get(&self, key: &Key) -> Option<&ScalarCell> {
        self.0.get(key)
    }
    pub(crate) fn get_mut(&mut self, key: &Key) -> Option<&mut ScalarCell> {
        self.0.get_mut(key)
    }
    pub(crate) fn remove(&mut self, key: &Key) -> Option<ScalarCell> {
        self.0.remove(key)
    }
    pub(crate) fn contains_key(&self, key: &Key) -> bool {
        self.0.contains_key(key)
    }
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
    pub(crate) fn keys(&self) -> impl Iterator<Item = &Key> {
        self.0.keys()
    }
    pub(crate) fn retain(&mut self, keep: impl FnMut(&Key, &mut ScalarCell) -> bool) {
        self.0.retain(keep);
    }
    pub(crate) fn take(&mut self) -> Self {
        Self(self.0.take())
    }
    pub(crate) fn validate_append(&self, other: &Self) -> Result<(), TerminalInterpretError> {
        self.0.validate_append(&other.0)
    }
    pub(crate) fn append(&mut self, other: &mut Self) -> Result<(), TerminalInterpretError>
    where
        Key: crate::structural_storage::StorageKey,
    {
        self.0.validate_append(&other.0)?;
        other
            .prepare_resident_replacement(self, None)?
            .commit(other)?;
        self.0.append(&mut other.0)
    }

    /// Capture incoming values and original homes before any external write.
    /// A complete subtree replacement must carry every resident it will prune;
    /// a changed active layout needs its original allocation codec.
    pub(crate) fn prepare_resident_replacement(
        &self,
        original: &Self,
        root: Option<&crate::values::StructuralRuntimePlace>,
    ) -> Result<ResidentReplacementWrites<Key>, TerminalInterpretError>
    where
        Key: crate::structural_storage::StorageKey,
    {
        if let Some(root) = root {
            for (key, cell) in crate::structural_storage::cells(original, root) {
                if matches!(cell, ScalarCell::Resident { .. }) && !self.0.contains_key(key) {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "resident replacement needs a codec for its changed active layout",
                    ));
                }
            }
        }
        let mut writes = Vec::new();
        for (key, incoming) in self.0.iter() {
            if let Some(
                resident @ ScalarCell::Resident {
                    scalar_type,
                    backing,
                },
            ) = original.0.get(key)
            {
                if matches!(incoming, ScalarCell::Resident { backing: other, .. } if Rc::ptr_eq(backing, other))
                {
                    continue;
                }
                let value = incoming.read()?;
                if value.scalar_type() != *scalar_type {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "resident replacement needs a codec for its changed scalar declaration",
                    ));
                }
                writes.push((key.clone(), resident.clone(), value));
            }
        }
        Ok(ResidentReplacementWrites(writes))
    }
}

impl<Key> std::borrow::Borrow<BTreeMap<Key, ScalarCell>> for ScalarStorageRows<Key> {
    fn borrow(&self) -> &BTreeMap<Key, ScalarCell> {
        self.0.borrow()
    }
}

/// Values are captured once while every replacement is still staged. Native
/// write failure faults the operation; it never restores a detached snapshot.
pub(crate) struct ResidentReplacementWrites<Key>(Vec<(Key, ScalarCell, TerminalScalarValue)>);

impl<Key: Ord> ResidentReplacementWrites<Key> {
    pub(crate) fn commit(
        self,
        staged: &mut ScalarStorageRows<Key>,
    ) -> Result<(), TerminalInterpretError> {
        if self.0.iter().any(|(key, _, _)| !staged.0.contains_key(key)) {
            return Err(malformed());
        }
        for (key, mut resident, value) in self.0 {
            resident.write(value)?;
            // Every receiving key already owns a funded staging row.
            staged.0.insert(key, resident)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[derive(Debug)]
    struct Backing {
        value: RefCell<TerminalScalarValue>,
        faulted: Cell<bool>,
    }
    impl TerminalScalarBacking for Backing {
        fn read(&self) -> Result<TerminalScalarValue, TerminalEffectRejection> {
            if self.faulted.get() {
                return Err(TerminalEffectRejection::new("original home is faulted"));
            }
            Ok(*self.value.borrow())
        }
        fn write(&self, value: TerminalScalarValue) -> Result<(), TerminalEffectRejection> {
            if self.faulted.get() {
                return Err(TerminalEffectRejection::new("original home is faulted"));
            }
            *self.value.borrow_mut() = value;
            Ok(())
        }
    }
    fn word(value: u128) -> TerminalScalarValue {
        TerminalScalarValue::Integer {
            scalar_type: semantic_vocabulary::IntegerType::new(
                semantic_vocabulary::IntegerSign::Unsigned,
                64,
            )
            .unwrap(),
            value: semantic_vocabulary::IntegerValue::Unsigned(value),
        }
    }
    fn backing() -> Rc<Backing> {
        Rc::new(Backing {
            value: RefCell::new(word(37)),
            faulted: Cell::new(false),
        })
    }

    #[test]
    fn resident_observations_and_moves_share_one_original_but_copies_capture_values() {
        let home = backing();
        let resident = ScalarCell::resident(word(37), home.clone()).unwrap();
        *home.value.borrow_mut() = word(91);
        assert_eq!(resident.read().unwrap(), word(91));
        let mut moved = resident.relocated(true).unwrap();
        let copied = resident.relocated(false).unwrap();
        moved.write(word(99)).unwrap();
        assert_eq!(home.read().unwrap(), word(99));
        assert_eq!(resident.read().unwrap(), word(99));
        assert_eq!(copied.read().unwrap(), word(91));
        let mut rows = ScalarStorageRows::new(None);
        rows.insert_cell(1, moved).unwrap();
        rows.insert(1, word(101)).unwrap();
        assert_eq!(home.read().unwrap(), word(101));
        assert_eq!(rows.get(&1).unwrap().read().unwrap(), word(101));
    }

    #[test]
    fn mismatched_or_faulted_original_storage_cannot_fall_back_to_local_contents() {
        let home = backing();
        assert!(ScalarCell::resident(word(91), home.clone()).is_err());
        let mut resident = ScalarCell::resident(word(37), home.clone()).unwrap();
        assert!(resident.write(TerminalScalarValue::Boolean(true)).is_err());
        assert_eq!(home.read().unwrap(), word(37));
        home.faulted.set(true);
        assert!(matches!(
            resident.read(),
            Err(TerminalInterpretError::ResidentStorage(_))
        ));
        assert!(resident.relocated(false).is_err());
        assert!(resident.write(word(99)).is_err());
        assert_eq!(*home.value.borrow(), word(37));
    }

    #[test]
    fn aggregate_replacement_keeps_the_original_home_after_pruning_old_rows() {
        let home = backing();
        let root = crate::values::StructuralRuntimePlace {
            opaque_identity: 11,
            path: Vec::new(),
        };
        let mut original = ScalarStorageRows::new(None);
        original
            .insert_cell(
                root.clone(),
                ScalarCell::resident(word(37), home.clone()).unwrap(),
            )
            .unwrap();
        let mut replacement = ScalarStorageRows::new(None);
        replacement.insert(root.clone(), word(91)).unwrap();
        let writes = replacement
            .prepare_resident_replacement(&original, Some(&root))
            .unwrap();
        assert_eq!(
            home.read().unwrap(),
            word(37),
            "preflight performs no writes"
        );
        writes.commit(&mut replacement).unwrap();
        original.prune(&root);
        original.append(&mut replacement).unwrap();
        assert_eq!(home.read().unwrap(), word(91));
        *home.value.borrow_mut() = word(99);
        assert_eq!(original.get(&root).unwrap().read().unwrap(), word(99));
        let mut next = ScalarStorageRows::new(None);
        next.insert(root.clone(), word(101)).unwrap();
        original.append(&mut next).unwrap();
        assert_eq!(
            home.read().unwrap(),
            word(101),
            "overlapping append retains the resident"
        );
    }

    #[test]
    fn incomplete_or_retyped_resident_replacement_refuses_before_writing() {
        let home = backing();
        let root = crate::values::StructuralRuntimePlace {
            opaque_identity: 11,
            path: Vec::new(),
        };
        let mut original = ScalarStorageRows::new(None);
        original
            .insert_cell(
                root.clone(),
                ScalarCell::resident(word(37), home.clone()).unwrap(),
            )
            .unwrap();
        let mut replacement = ScalarStorageRows::new(None);
        assert!(
            replacement
                .prepare_resident_replacement(&original, Some(&root))
                .is_err()
        );
        replacement
            .insert(root.clone(), TerminalScalarValue::Boolean(true))
            .unwrap();
        assert!(
            replacement
                .prepare_resident_replacement(&original, Some(&root))
                .is_err()
        );
        assert_eq!(home.read().unwrap(), word(37));
    }
}
