//! External bindings: the mechanisms a machine reaches outside code by, and
//! the interner that gives each binding one structural identity.

use crate::ExternalBindingId;

/// Closed mechanism tag for one irreducible external realization. This is
/// retained independently from the transitional binding interner so semantic
/// consumers never classify a rendered `Binding::Case(...)` string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalBindingMechanism {
    Syscall,
    CompilerIntrinsic,
    VtableSlot,
    VtableField,
    TableFunction,
}

impl ExternalBindingMechanism {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Syscall => "syscall",
            Self::CompilerIntrinsic => "compiler_intrinsic",
            Self::VtableSlot => "vtable_slot",
            Self::VtableField => "vtable_field",
            Self::TableFunction => "table_function",
        }
    }

    /// Tag 1 belonged to the retired string-backed import mechanism and stays
    /// unused. These tags fold into contract identity, so renumbering the
    /// survivors would change every external realization's fingerprint.
    pub const fn identity_tag(self) -> u8 {
        match self {
            Self::Syscall => 2,
            Self::CompilerIntrinsic => 3,
            Self::VtableSlot => 4,
            Self::VtableField => 5,
            Self::TableFunction => 6,
        }
    }
}

/// Closed, structural identity for one irreducible external binding. These
/// values are interned directly; no display rendering is parsed or compared.
/// An import has no spelling here: a foreign locator is evaluated as a typed
/// `Binding` value through a `via` producer, never as authored strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalBindingIdentity {
    Syscall { number: i64 },
    CompilerIntrinsic,
    VtableSlot { index: i64 },
    VtableField { field: String },
    TableFunction { field: String },
}

impl ExternalBindingIdentity {
    pub const fn mechanism(&self) -> ExternalBindingMechanism {
        match self {
            Self::Syscall { .. } => ExternalBindingMechanism::Syscall,
            Self::CompilerIntrinsic => ExternalBindingMechanism::CompilerIntrinsic,
            Self::VtableSlot { .. } => ExternalBindingMechanism::VtableSlot,
            Self::VtableField { .. } => ExternalBindingMechanism::VtableField,
            Self::TableFunction { .. } => ExternalBindingMechanism::TableFunction,
        }
    }
}

/// Deterministic EXTERNAL-BINDING interner. `NULL`/0 stays "not computed";
/// ids start at 1.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExternalBindingTable {
    identities: Vec<ExternalBindingIdentity>,
}

impl ExternalBindingTable {
    pub fn intern(&mut self, identity: ExternalBindingIdentity) -> ExternalBindingId {
        if let Some(index) = self
            .identities
            .iter()
            .position(|existing| existing == &identity)
        {
            return ExternalBindingId(index as u32 + 1);
        }
        self.identities.push(identity);
        ExternalBindingId(self.identities.len() as u32)
    }

    /// Recover the exact structured identity retained by an interned binding.
    /// Invalid/zero and out-of-table ids fail closed instead of exposing an
    /// implementation index or inviting a syntax-tree fallback.
    pub fn identity(&self, binding: ExternalBindingId) -> Option<&ExternalBindingIdentity> {
        let index = usize::try_from(binding.0).ok()?.checked_sub(1)?;
        self.identities.get(index)
    }

    pub fn identities(
        &self,
    ) -> impl ExactSizeIterator<Item = (ExternalBindingId, &ExternalBindingIdentity)> {
        self.identities
            .iter()
            .enumerate()
            .map(|(index, identity)| (ExternalBindingId(index as u32 + 1), identity))
    }
}
