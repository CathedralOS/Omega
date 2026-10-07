//! Readonly ownership of the prepared typed form. Checking publishes facts,
//! but cannot change the tables whose occurrence handles those facts name.
//! This is an ownership seal on TypedTrees, not another pipeline program form.
//! A transform must consume the owner and construct a new result; there is
//! deliberately no mutable borrow or copy-on-write API.

use super::TypedTrees;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrozenTypedTrees(TypedTrees);

impl FrozenTypedTrees {
    /// Transfer the program into construction scratch for another transform.
    pub fn into_inner(self) -> TypedTrees {
        self.0
    }
}

impl From<TypedTrees> for FrozenTypedTrees {
    fn from(program: TypedTrees) -> Self {
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
