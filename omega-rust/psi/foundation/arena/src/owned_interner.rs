//! An append-only interner owned by the value whose identities it mints.
//!
//! `OwnedIndex` is derived content: it rebuilds from empty whenever its
//! owner's revision moves. That is wrong for an interner whose handles are
//! stored in durable plans, because a rebuilt table re-mints the same key
//! under a different handle, and a stored handle then names another key.
//! Checked dynamic-slot plans hit exactly this: an interface handle minted
//! before monomorphization read back as an unrelated record type after it.
//!
//! `OwnedInterner` never resets. Equal keys always map to the same handle for
//! the owner's lifetime, keys minted against an earlier revision stay valid,
//! and a clone keeps every handle its source minted. Only key-to-handle
//! interning belongs here; a memo whose answers depend on arena contents
//! stays in an `OwnedIndex`.

use std::sync::Mutex;

#[derive(compiler_storage::Record)]
pub struct OwnedInterner<T> {
    table: Mutex<T>,
}

impl<T: Default> Default for OwnedInterner<T> {
    fn default() -> Self {
        Self {
            table: Mutex::new(T::default()),
        }
    }
}

impl<T: Clone> Clone for OwnedInterner<T> {
    fn clone(&self) -> Self {
        Self {
            table: Mutex::new(
                self.table
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .clone(),
            ),
        }
    }
}

// Two owners with equal arenas are equal whatever keys they have interned so
// far, matching `OwnedIndex`.
impl<T> PartialEq for OwnedInterner<T> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl<T> Eq for OwnedInterner<T> {}

impl<T> std::fmt::Debug for OwnedInterner<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("OwnedInterner")
    }
}

impl<T> OwnedInterner<T> {
    /// Read the table.
    pub fn with<R>(&self, run: impl FnOnce(&T) -> R) -> R {
        let table = self.table.lock().unwrap_or_else(|error| error.into_inner());
        run(&table)
    }

    /// Intern into the table. Keys are only ever added.
    pub fn with_mut<R>(&self, run: impl FnOnce(&mut T) -> R) -> R {
        let mut table = self.table.lock().unwrap_or_else(|error| error.into_inner());
        run(&mut table)
    }
}

#[cfg(test)]
mod tests {
    use super::OwnedInterner;

    #[test]
    fn interned_keys_survive_clone_and_later_interning() {
        let interner = OwnedInterner::<Vec<&str>>::default();
        let first = interner.with_mut(|table| {
            table.push("writer");
            table.len() - 1
        });
        let copy = interner.clone();
        interner.with_mut(|table| table.push("record"));
        assert_eq!(interner.with(|table| table[first]), "writer");
        assert_eq!(copy.with(|table| table[first]), "writer");
    }
}
