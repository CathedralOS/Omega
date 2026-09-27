//! Canonical identity for the fixed `core::*` mathematical vocabulary
//! (PROOF-CONTRACT-MIGRATION).
//!
//! `core::Level`, `core::Type`, `core::Strict` and `core::Squash` are
//! compiler-known declarations, not ordinary types: the mathematical
//! bindings spec fixes their identity so a same-spelled authored
//! declaration can never become a sort, and no display alias changes it.
//!
//! Once the fixed declarations exist, references resolve to their symbols
//! and classification is symbol identity alone. Until they land, resolution
//! leaves the `core::*` names unbound, so an *unresolved* authored spelling
//! is the only marker a reference can carry — the spelling match below is
//! that transitional case, never a classification of resolved symbols. A
//! reference that did resolve is whatever its symbol is, however it is
//! spelled.

use symbols::{SymbolHandle, SymbolKind, SymbolTable};
use typed_trees::name::Identifier;

/// One fixed `core::*` vocabulary member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FixedCore {
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
    /// The spelling the compiler-known declaration will resolve under.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Level => "core::Level",
            Self::Type => "core::Type",
            Self::Strict => "core::Strict",
            Self::Squash => "core::Squash",
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
pub(crate) struct FixedCoreVocabulary {
    level: SymbolHandle,
    ty: SymbolHandle,
    strict: SymbolHandle,
    squash: SymbolHandle,
}

impl FixedCoreVocabulary {
    /// Resolve the fixed vocabulary against `symbols`. The declarations land
    /// as `SymbolKind::MathematicalDefinition` root children under their
    /// `core::*` names; until they exist every handle stays invalid.
    pub(crate) fn resolve(symbols: &SymbolTable) -> Self {
        let find = |member: FixedCore| -> SymbolHandle {
            symbols
                .root_children_named(member.name())
                .find(|symbol| symbols.get(*symbol).kind == SymbolKind::MathematicalDefinition)
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
    pub(crate) fn classify(&self, symbol: SymbolHandle, name: &Identifier) -> Option<FixedCore> {
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
/// order. The arm is reached only when a reference carries no resolved
/// symbol, so it cannot misname a declaration the program actually has.
const FIXED_CORE_FALLBACK: &[(FixedCore, &str)] = &[
    (FixedCore::Level, "core::Level"),
    (FixedCore::Type, "core::Type"),
    (FixedCore::Strict, "core::Strict"),
    (FixedCore::Squash, "core::Squash"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use symbols::{SymbolNameRef, SymbolTableBuilder};
    use typed_trees::name::Identifier;

    /// A symbol table carrying a fixed `core::Type` declaration plus one
    /// ordinary declaration that happens to share nothing but a spelling —
    /// the shape the supplied-declarations leg creates.
    fn table() -> (SymbolTable, SymbolHandle, SymbolHandle) {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = SymbolTableBuilder::child_handles(builder.insert_children(
            root,
            [
                (
                    SymbolKind::MathematicalDefinition,
                    SymbolNameRef::Static("core::Type"),
                ),
                (SymbolKind::Data, SymbolNameRef::Static("UserData")),
            ],
        ));
        let mut children = children;
        let fixed_type = children.next().expect("fixed core::Type");
        let user_data = children.next().expect("user data");
        (builder.finish(), fixed_type, user_data)
    }

    /// A resolved reference to the fixed `core::Type` classifies by its
    /// symbol — no spelling read is needed.
    #[test]
    fn a_resolved_fixed_symbol_classifies_by_identity() {
        let (symbols, fixed_type, _) = table();
        let vocabulary = FixedCoreVocabulary::resolve(&symbols);
        assert_eq!(
            vocabulary.classify(
                fixed_type,
                &Identifier::generated_static("any-authored-spelling"),
            ),
            Some(FixedCore::Type),
        );
    }

    /// A resolved symbol that is not a fixed declaration keeps its own
    /// identity however it is spelled — a same-spelled user symbol never
    /// becomes the sort (the spec's "display aliases cannot change those
    /// identities").
    #[test]
    fn a_resolved_same_spelled_symbol_is_not_the_fixed_vocabulary() {
        let (symbols, _, user_data) = table();
        let vocabulary = FixedCoreVocabulary::resolve(&symbols);
        assert_eq!(
            vocabulary.classify(user_data, &Identifier::generated_static("core::Type")),
            None,
        );
    }

    /// Until the fixed declarations exist, an unresolved `core::*` spelling
    /// is the only marker a reference can carry — the transitional arm.
    #[test]
    fn an_unresolved_core_spelling_classifies_until_declarations_land() {
        let mut builder = SymbolTableBuilder::new();
        builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let symbols = builder.finish();
        let vocabulary = FixedCoreVocabulary::resolve(&symbols);
        let invalid = SymbolHandle::invalid();
        assert_eq!(
            vocabulary.classify(invalid, &Identifier::generated_static("core::Level")),
            Some(FixedCore::Level),
        );
        assert_eq!(
            vocabulary.classify(invalid, &Identifier::generated_static("core::Type")),
            Some(FixedCore::Type),
        );
        assert_eq!(
            vocabulary.classify(invalid, &Identifier::generated_static("core::Strict")),
            Some(FixedCore::Strict),
        );
        assert_eq!(
            vocabulary.classify(invalid, &Identifier::generated_static("core::Squash")),
            Some(FixedCore::Squash),
        );
        assert_eq!(
            vocabulary.classify(invalid, &Identifier::generated_static("core::Vec")),
            None,
        );
    }
}
