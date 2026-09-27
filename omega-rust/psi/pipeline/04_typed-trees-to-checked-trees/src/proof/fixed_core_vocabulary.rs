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
    /// The spelling the compiler-known declaration resolves under.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Level => "core::Level",
            Self::Type => "core::Type",
            Self::Strict => "core::Strict",
            Self::Squash => "core::Squash",
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
pub(crate) struct FixedCoreVocabulary {
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
    pub(crate) fn resolve(symbols: &SymbolTable) -> Self {
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
/// order, spelled once on `FixedCore::name`. The arm is reached only when a
/// reference carries no resolved symbol, so it cannot misname a declaration
/// the program actually has.
const FIXED_CORE_FALLBACK: &[(FixedCore, &str)] = &[
    (FixedCore::Level, FixedCore::Level.name()),
    (FixedCore::Type, FixedCore::Type.name()),
    (FixedCore::Strict, FixedCore::Strict.name()),
    (FixedCore::Squash, FixedCore::Squash.name()),
];

#[cfg(test)]
mod tests {
    use super::*;
    use symbols::{
        SymbolKind, SymbolNameRef, SymbolTableBuilder, builtin_function_symbols,
        builtin_type_symbols, fixed_core_declaration_symbols,
    };
    use typed_trees::name::Identifier;

    /// A symbol table carrying the builtin slots, the installed fixed
    /// `core::*` declarations, then one same-spelled authored declaration
    /// and one ordinary declaration — the shape the real builder produces.
    /// Returns `(table, fixed core::Type, authored core::Type, user data)`.
    fn table() -> (SymbolTable, SymbolHandle, SymbolHandle, SymbolHandle) {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = SymbolTableBuilder::child_handles(
            builder.insert_children(
                root,
                builtin_type_symbols()
                    .into_iter()
                    .chain(builtin_function_symbols())
                    .chain(fixed_core_declaration_symbols())
                    .chain([
                        (
                            SymbolKind::MathematicalDefinition,
                            SymbolNameRef::Static("core::Type"),
                        ),
                        (SymbolKind::Data, SymbolNameRef::Static("UserData")),
                    ]),
            ),
        );
        let mut children = children;
        let offset = symbols::BUILTIN_TYPE_COUNT + symbols::BuiltinFunction::COUNT;
        for _ in 0..offset + FixedCoreDeclaration::Type.ordinal() {
            children.next();
        }
        let fixed_type = children.next().expect("fixed core::Type slot");
        for _ in 0..FixedCoreDeclaration::COUNT - 1 - FixedCoreDeclaration::Type.ordinal() {
            children.next();
        }
        let authored_twin = children.next().expect("same-spelled authored declaration");
        let user_data = children.next().expect("user data");
        (builder.finish(), fixed_type, authored_twin, user_data)
    }

    /// A resolved reference to the fixed `core::Type` classifies by its
    /// symbol — no spelling read is needed.
    #[test]
    fn a_resolved_fixed_symbol_classifies_by_identity() {
        let (symbols, fixed_type, _, _) = table();
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
        let (symbols, _, _, user_data) = table();
        let vocabulary = FixedCoreVocabulary::resolve(&symbols);
        assert_eq!(
            vocabulary.classify(user_data, &Identifier::generated_static("core::Type")),
            None,
        );
    }

    /// The strongest false twin: an *authored* `MathematicalDefinition`
    /// spelled `core::Type` in the same table. Slot identity is the only
    /// thing that distinguishes them, so a resolved reference to the twin
    /// must never classify as the fixed sort.
    #[test]
    fn a_same_spelled_authored_declaration_is_not_the_fixed_vocabulary() {
        let (symbols, _, authored_twin, _) = table();
        let vocabulary = FixedCoreVocabulary::resolve(&symbols);
        assert_eq!(
            vocabulary.classify(authored_twin, &Identifier::generated_static("core::Type")),
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
