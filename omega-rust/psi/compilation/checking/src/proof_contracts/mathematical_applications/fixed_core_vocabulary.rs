//! Canonical identity for the fixed `core::*` mathematical vocabulary
//! (PROOF-CONTRACT-MIGRATION).
//!
//! `core::Level`, `core::Type`, `core::Strict` and `core::Squash` are
//! compiler-known declarations, not ordinary types: the mathematical
//! bindings spec fixes their identity so a same-spelled authored
//! declaration can never become a sort, and no display alias changes it.
//!
//! The declarations are compiler-installed root children at the fixed
//! slots [`symbols::FixedCoreDeclaration`] defines, so a reference that
//! resolved classifies by symbol identity alone. A reference still carrying
//! no symbol — the pipeline that produced it never bound the spelling — is
//! classified by its `core::*` spelling as the transitional case; a
//! resolved symbol is whatever it resolved to, however it is spelled.

use symbols::{FixedCoreDeclaration, SymbolHandle, SymbolTable};
use typed_trees::name::Identifier;

/// One fixed `core::*` vocabulary member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedCore {
    /// `core::Level` — the universe-level domain.
    Level,
    /// `core::Type` — the sort of types; `core::Type<u>` applies one level.
    Type,
    /// `core::Strict` — the strict-proposition sort.
    Strict,
    /// `core::Squash` — the witness-hiding proposition squash.
    Squash,
}

impl FixedCore {
    /// The spelling the compiler-known declaration resolves under.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Level => "core::Level",
            Self::Type => "core::Type",
            Self::Strict => "core::Strict",
            Self::Squash => "core::Squash",
        }
    }

    /// The `Term::Sort` constructor this member spells when it takes a
    /// universe level; `None` for members that take a type or no argument.
    pub fn universe_sort(self) -> Option<fn(proof_admission::Level) -> proof_admission::Sort> {
        match self {
            Self::Type => Some(proof_admission::Sort::Type),
            Self::Strict => Some(proof_admission::Sort::Strict),
            Self::Level | Self::Squash => None,
        }
    }

    /// The symbol-table identity this member occupies — the closed
    /// compiler-installed declaration, not a spelling.
    const fn declaration(self) -> FixedCoreDeclaration {
        match self {
            Self::Level => FixedCoreDeclaration::Level,
            Self::Type => FixedCoreDeclaration::Type,
            Self::Strict => FixedCoreDeclaration::Strict,
            Self::Squash => FixedCoreDeclaration::Squash,
        }
    }
}

/// The fixed `core::*` declarations' canonical symbols in one program's
/// symbol table, resolved once rather than re-spelled at every reference.
///
/// An invalid handle records that no such declaration exists yet — the
/// supplied-declarations leg installs them and this map fills in, at which
/// point [`Self::classify`]'s symbol arm fires and the spelling arm stops
/// being the only path.
pub struct FixedCoreVocabulary {
    level: SymbolHandle,
    ty: SymbolHandle,
    strict: SymbolHandle,
    squash: SymbolHandle,
}

impl FixedCoreVocabulary {
    /// Resolve the fixed vocabulary against `symbols` by exact
    /// compiler-installed slot identity. A table without the fixed
    /// declarations — a synthetic test fixture or a foreign producer's
    /// evidence — leaves every handle invalid, and `classify` then has only
    /// the transitional spelling arm.
    pub fn resolve(symbols: &SymbolTable) -> Self {
        let find = |member: FixedCore| -> SymbolHandle {
            symbols
                .fixed_core_declaration_symbol(member.declaration())
                .unwrap_or_else(SymbolHandle::invalid)
        };
        Self {
            level: find(FixedCore::Level),
            ty: find(FixedCore::Type),
            strict: find(FixedCore::Strict),
            squash: find(FixedCore::Squash),
        }
    }

    /// The fixed-core identity of one reference: canonical symbol identity
    /// when the reference resolved to a fixed declaration, the
    /// compiler-known spelling only while the declarations do not exist to
    /// resolve. A resolved symbol that is not a fixed declaration is never
    /// the core vocabulary, however it is spelled.
    pub fn classify(&self, symbol: SymbolHandle, name: &Identifier) -> Option<FixedCore> {
        if symbol.is_valid() {
            if symbol == self.level {
                return Some(FixedCore::Level);
            }
            if symbol == self.ty {
                return Some(FixedCore::Type);
            }
            if symbol == self.strict {
                return Some(FixedCore::Strict);
            }
            if symbol == self.squash {
                return Some(FixedCore::Squash);
            }
            return None;
        }
        FIXED_CORE_FALLBACK
            .iter()
            .find(|(_, spelling)| spelling == &name.as_str())
            .map(|(member, _)| *member)
    }
}

/// The unresolved-spelling fallback: the fixed `core::*` names in iteration
/// order, spelled once on `FixedCore::name`. The arm is reached only when a
/// reference carries no resolved symbol, so it cannot misname a declaration
/// the program actually has.
const FIXED_CORE_FALLBACK: &[(FixedCore, &str)] = &[
    (FixedCore::Level, FixedCore::Level.name()),
    (FixedCore::Type, FixedCore::Type.name()),
    (FixedCore::Strict, FixedCore::Strict.name()),
    (FixedCore::Squash, FixedCore::Squash.name()),
];
