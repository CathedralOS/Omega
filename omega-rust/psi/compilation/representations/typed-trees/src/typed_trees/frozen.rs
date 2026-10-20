//! Readonly ownership of the prepared typed form. Checking publishes facts,
//! but cannot change the tables whose occurrence handles those facts name.
//! This is an ownership seal on TypedTrees, not another pipeline program form.
//! A transform must consume the owner and construct a new result; there is
//! deliberately no mutable borrow or copy-on-write API.
//!
//! Freezing also seals the declaration indexes: no table can move while the
//! seal holds, so checking workers share one built copy without locking.

use super::TypedTrees;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrozenTypedTrees(TypedTrees);

impl FrozenTypedTrees {
    /// Transfer the program into construction scratch for another transform.
    pub fn into_inner(mut self) -> TypedTrees {
        self.0.unseal_declaration_indexes();
        self.0
    }
}

impl From<TypedTrees> for FrozenTypedTrees {
    fn from(mut program: TypedTrees) -> Self {
        program.seal_declaration_indexes();
        Self(program)
    }
}

impl std::ops::Deref for FrozenTypedTrees {
    type Target = TypedTrees;

    fn deref(&self) -> &TypedTrees {
        &self.0
    }
}

impl AsRef<TypedTrees> for FrozenTypedTrees {
    fn as_ref(&self) -> &TypedTrees {
        &self.0
    }
}
