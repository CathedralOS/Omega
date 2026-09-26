use crate::data::TypeParameter;
use crate::expression::ExpressionHandle;
use crate::name::DiagnosticName;
use crate::types::TypeReference;
use arena::{Handle, HandleSpan};
use symbols::SymbolHandle;

/// A mathematical `let`/`boundary let` declaration after lexical symbol
/// assignment (PROOF-CONTRACT-MIGRATION). This is a distinct root category,
/// separate from `proposition`: it has a dependent result type, an ordered
/// ordinary telescope, and either a transparent definition term or the
/// named-assumption absence a `boundary let` declares. It owns no executable
/// signature, effects, or runtime body; the typed-trees elaboration leg turns
/// this resolved shape into the checked proof surface.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MathematicalDefinition {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub is_public: bool,
    /// Generic binders in authored order (`<u: core::Level, A: core::Type<u>,
    /// N: usize>`). The binder kind stays the authored `Type`/`Const`/`Value`
    /// carrier at this stage — universe/level classification is a later leg.
    pub binders: HandleSpan<TypeParameter>,
    pub parameters: HandleSpan<MathematicalParameter>,
    /// The declared result: an ordinary type reference, a dependent arrow
    /// `(x: T) -> U`, or a type-level application `F(x)` — never absent in
    /// source; an invalid handle marks only a default placeholder.
    pub result: MathematicalTypeHandle,
    pub body: MathematicalBody,
}

/// One telescope parameter (`name [erased]?: MathematicalType`). The type is
/// the mathematical type grammar, so `f: A -> B` is legal here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MathematicalParameter {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub relevance: language_core::BindingRelevance,
    pub ty: MathematicalTypeHandle,
}

pub type MathematicalTypeHandle = Handle<MathematicalType>;

/// The declaration-local type grammar for mathematical declarations. Arrows
/// and value-level applications stay out of `TypeReference` deliberately —
/// they are proof-surface structure, not machine type syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MathematicalType {
    /// An ordinary resolved type reference (`usize`, `A`, `core::Type<u>`).
    Ordinary(TypeReference),
    /// `domain -> codomain`; `binder` retains the authored `(x: domain)`
    /// dependent name for the typed-tree telescope that scopes it over the
    /// codomain. `None` is the plain `A -> B` form.
    Arrow {
        binder: Option<DiagnosticName>,
        domain: MathematicalTypeHandle,
        codomain: MathematicalTypeHandle,
    },
    /// A type-level application `callee(arg, ...)` whose arguments are
    /// mathematical expressions (`F(value)`, `C(x, y)`, `F(x)(y)`).
    Application {
        callee: MathematicalTypeHandle,
        arguments: HandleSpan<ExpressionHandle>,
    },
}

impl Default for MathematicalType {
    /// Placeholder for arena defaults only; authored source always produces a
    /// concrete variant.
    fn default() -> Self {
        Self::Ordinary(TypeReference::default())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum MathematicalBody {
    /// `boundary let ...;` — a named assumption with no body. The default is
    /// the bodyless placeholder, matching how every other root arena defaults.
    #[default]
    Assumption,
    /// `= term;` — a transparent mathematical term. Its authored expression
    /// is retained with exact source spans; nothing executable is implied.
    Definition(ExpressionHandle),
}
