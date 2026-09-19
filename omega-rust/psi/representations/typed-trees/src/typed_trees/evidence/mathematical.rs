use crate::data::TypeParameter;
use crate::expression::ExpressionHandle;
use crate::name::Identifier;
use crate::types::TypeReferenceHandle;
use arena::{Handle, HandleSpan};
use symbols::SymbolHandle;

/// A typed mathematical `let`/`boundary let` declaration
/// (PROOF-CONTRACT-MIGRATION, wiki/spec/proofs/mathematical_bindings.md).
///
/// This is the resolved declaration carried into the typed program with its
/// authored grammar intact: binders stay the shared `TypeParameter` kind until
/// the universe/level classification leg, the ordered telescope plus `result`
/// is the nested dependent-Π spine the elaboration leg abstracts over, and the
/// body is either a transparent typed term or the named-assumption absence a
/// `boundary let` declares. It owns no executable signature, effects, or
/// runtime body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MathematicalDefinition {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub is_public: bool,
    /// Generic binders in authored order, lowered through the shared
    /// `TypeParameter` machinery. `u: core::Level`-carrier binders are still
    /// the authored `Value` kind here; the universe/level classification leg
    /// owns that judgment.
    pub binders: HandleSpan<TypeParameter>,
    pub parameters: HandleSpan<MathematicalParameter>,
    /// The declared result type as a declaration-local mathematical type:
    /// `Ordinary` references, dependent `Arrow`s and type-level
    /// `Application`s all carry through; checked elaboration owns their
    /// interpretation.
    pub result: MathematicalTypeHandle,
    pub body: MathematicalBody,
}

/// One telescope parameter (`name [erased]?: MathematicalType`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MathematicalParameter {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    /// The binding occurrence's `[erased]` relevance.
    pub relevance: language_core::BindingRelevance,
    pub ty: MathematicalTypeHandle,
}

pub type MathematicalTypeHandle = Handle<MathematicalType>;

/// The declaration-local mathematical type grammar carried through typing.
/// `Ordinary` wraps a lowered type reference; `Arrow` is a dependent Π-type
/// whose optional binder scopes over the codomain; `Application` applies a
/// family to mathematical arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MathematicalType {
    /// An ordinary lowered type reference (`usize`, `A`, `core::Type<u>`).
    Ordinary(TypeReferenceHandle),
    /// `domain -> codomain`; `binder` is the authored `(x: domain)` dependent
    /// name scoping over the codomain.
    Arrow {
        binder: Option<Identifier>,
        domain: MathematicalTypeHandle,
        codomain: MathematicalTypeHandle,
    },
    /// A type-level application `callee(arg, ...)` with lowered mathematical
    /// arguments (`F(value)`, `C(x, y)`).
    Application {
        callee: MathematicalTypeHandle,
        arguments: HandleSpan<ExpressionHandle>,
    },
}

impl Default for MathematicalType {
    /// Placeholder for arena defaults only; typed output always produces a
    /// concrete variant.
    fn default() -> Self {
        Self::Ordinary(TypeReferenceHandle::invalid())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum MathematicalBody {
    /// `boundary let ...;` — a named assumption with no body. The default is
    /// the bodyless placeholder, matching how every other root arena defaults.
    #[default]
    Assumption,
    /// `= term;` — a transparent typed term definition.
    Definition(ExpressionHandle),
}
