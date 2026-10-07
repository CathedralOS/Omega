use super::*;

/// A proof-formula declaration. It has no result, executable body, effects, or
/// runtime representation; its parameters are an erased proof-side telescope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropositionDefinition {
    pub name: Identifier,
    pub is_public: bool,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub parameters: HandleSpan<StateParameterHandle>,
    /// Exact authored semantic-token extent of a transparent formula.
    pub transparent_formula_source_span: Option<source::SourceSpan>,
    pub body: PropositionBody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropositionBody {
    /// Owner-declared fact with no recoverable witness interface.
    Primitive,
    /// Exactly one owner-authorized carrierless evidence interface.
    Witness {
        evidence: crate::types::TypeReferenceHandle,
    },
    /// Source/debug alias expanded before normalized semantic identity.
    Transparent {
        proposition: crate::expression::ExpressionHandle,
    },
}

/// A top-level mathematical declaration: `let` (a transparent checked term
/// definition) or `boundary let` (a named assumption with no body). These are
/// the selected source forms for named mathematical terms and assumptions
/// under PROOF-CONTRACT-MIGRATION
/// (wiki/spec/proofs/mathematical_bindings.md): ordinary `requires`/`ensures`
/// contracts stay on machines, while this declaration's binders, parameter
/// telescope and result elaborate into one proof-kernel declaration — a level
/// arity, a statement built from nested dependent function arrows, and an
/// optional transparent definition body. No `forall`, `exists`, `claim`,
/// `proposition` or `implies` spelling is added.
///
/// This is deliberately not an [`Item`] variant yet: the symbol-resolution
/// lowering that admits declarations is a separately owned leg, so parsed
/// declarations live on their own root collection
/// (`SyntaxTreeRoots::mathematical_definitions`) and resolution refuses them
/// explicitly until that leg lands.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MathematicalDefinition {
    pub name: Identifier,
    pub is_public: bool,
    /// Generic binders in declaration order. `u: core::Level` and
    /// `A: core::Type<u>` parse as ordinary `Value` binders; the carrier's
    /// resolved declaration is what marks a binder as a universe or type
    /// parameter contributing to the kernel declaration's level arity. Bare
    /// `A` and `const N: usize` keep their ordinary `Type`/`Const` kinds.
    pub type_parameters: HandleSpan<TypeParameter>,
    /// The ordered ordinary parameter telescope `(name: Type, ...)`. A
    /// binder scopes over every later parameter and the result, matching the
    /// kernel's nested Π construction.
    pub parameters: HandleSpan<MathematicalParameterHandle>,
    /// The declared result type after `:`, itself a [`MathematicalTypeNode`].
    pub result: MathematicalTypeHandle,
    pub body: MathematicalDefinitionBody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MathematicalDefinitionBody {
    /// `= term;` — a transparent checked term definition. Substitution of a
    /// use site with this exact term is the declaration's meaning; nothing
    /// executable is implied by it.
    Definition(crate::expression::ExpressionHandle),
    /// `boundary let ...;` — the bodyless named-assumption form. Its term and
    /// transitive dependencies require the receiver's admission, keyed on
    /// this declaration's exact identity; it is never an
    /// implementation-search slot.
    Assumption,
}

impl Default for MathematicalDefinitionBody {
    fn default() -> Self {
        // A definition with an invalid term — an unusable placeholder, never
        // mistaken for a real assumption's authority.
        Self::Definition(crate::expression::ExpressionHandle::invalid())
    }
}

/// One ordinary parameter of a mathematical declaration's telescope. Unlike
/// [`StateParameterNode`] its type is a [`MathematicalTypeHandle`] (it may be
/// an arrow), and it carries no runtime binding facts — mathematical values
/// are not executable data.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MathematicalParameterNode {
    pub name: Identifier,
    /// The binding occurrence's `[erased]` marker (contracts.md): an erased
    /// proof reference demands no executable value while retaining trust.
    pub relevance: language_core::BindingRelevance,
    pub ty: MathematicalTypeHandle,
}

/// A type in mathematical-declaration position: every ordinary type form plus
/// the dependent function former `->`. It stays separate from
/// [`crate::types::TypeReferenceNode`] so that admitting `->` into the shared
/// type grammar remains its own resolution decision; this leg only records
/// the authored shape faithfully.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MathematicalTypeNode {
    /// An ordinary type reference — `i32`, `A`, `core::Type<u>`,
    /// `core::Squash<A>`.
    Ordinary(crate::types::TypeReferenceHandle),
    /// `A -> B` or `(value: A) -> F(value)` — a dependent function type
    /// (mathematical_bindings.md: arrow types are dependent Π-types). `->`
    /// associates right, and `binder` scopes over the codomain only.
    Arrow {
        binder: Option<Identifier>,
        domain: MathematicalTypeHandle,
        codomain: MathematicalTypeHandle,
    },
    /// `F(value)` / `C(x, y)` — a dependent type family applied to term
    /// arguments, the shape a dependent codomain takes when its binder is
    /// used. The shared `TypeReferenceNode` grammar has no value-argument
    /// application form (its `Generic`/`ConstExpression` arguments are
    /// `<>`-spelled), so this leg records the authored `(args)` application
    /// here for the resolution leg to elaborate.
    Application {
        callee: MathematicalTypeHandle,
        arguments: HandleSpan<crate::expression::ExpressionHandle>,
    },
}

impl Default for MathematicalTypeNode {
    fn default() -> Self {
        Self::Ordinary(crate::types::TypeReferenceHandle::invalid())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasureDefinition {
    /// Fully-qualified declaration path, e.g. `Card::PowerOrder`.
    pub name: HandleSpan<Identifier>,
    /// The single parameter being measured (e.g. `card: Card`).
    pub parameter: StateParameterHandle,
    /// Well-founded domain type, always `usize` for now.
    pub return_type: crate::types::TypeReferenceHandle,
    /// `true` when declared with the `lexicographic { .. }` body form.
    pub lexicographic: bool,
    /// Body component expressions: exactly one for the simple form, or the
    /// ordered tuple of components for the lexicographic form.
    pub body: HandleSpan<crate::expression::ExpressionHandle>,
    pub token_count: usize,
}

impl Default for MeasureDefinition {
    fn default() -> Self {
        Self {
            name: HandleSpan::empty(),
            parameter: StateParameterHandle::invalid(),
            return_type: crate::types::TypeReferenceHandle::invalid(),
            lexicographic: false,
            body: HandleSpan::empty(),
            token_count: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofFact {
    Expression(crate::expression::ExpressionHandle),
    Membership(ProofMembershipFact),
}

impl Default for ProofFact {
    fn default() -> Self {
        Self::Expression(crate::expression::ExpressionHandle::invalid())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofMembershipFact {
    pub value: crate::expression::ExpressionHandle,
    pub domain: HandleSpan<Identifier>,
    /// Indexed domain application after the path (`value in Resident<P, T>`),
    /// read with the same type-position grammar a domain constraint uses.
    /// Empty for an unindexed membership.
    pub domain_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
}

impl Default for ProofMembershipFact {
    fn default() -> Self {
        Self {
            value: crate::expression::ExpressionHandle::invalid(),
            domain: HandleSpan::empty(),
            domain_arguments: HandleSpan::empty(),
        }
    }
}

/// One `satisfies` binding on a machine (rearrange settle 2026-07-18):
/// `satisfies Trait::requirement` or `satisfies Trait::requirement as Alias`.
/// A REQUIREMENT-named binding
/// conforms this machine to that single requirement (the machine-by-machine
/// carrier model; the alias names the satisfier for plural algebras -- Nat
/// under (max, add) is the tropical semiring).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SatisfiesClause {
    pub trait_name: Identifier,
    /// Complete explicit target-trait lifetime application, in trait-binder
    /// order. These remain source names until the realizing machine telescope
    /// is available during typed lowering.
    pub lifetime_arguments: Vec<Identifier>,
    pub arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    pub requirement: Option<Identifier>,
    pub alias: Option<Identifier>,
    /// Transitional bootstrap spelling parsed as the old compiler-known
    /// closed sum. New source uses `via_expression`; keeping the carriers
    /// distinct prevents a string pair from masquerading as an evaluated
    /// locator.
    pub via: Option<ExternalBinding>,
    /// Ordinary expression authored after `via`. The first production rung
    /// admits one exact zero-argument direct machine call whose closed
    /// compiler-owned `Binding` result is evaluated hermetically. Invalid
    /// means this clause used no ordinary expression (or used `via`).
    pub via_expression: crate::expression::ExpressionHandle,
    /// Exact authored `via` keyword occurrence. This is explanatory custody
    /// for package review, independent from the interned binding identity.
    pub via_keyword_source_span: Option<source::SourceSpan>,
}

/// A generic evidence binder `Order satisfies Ranked<Element>`: an
/// implementation parameter whose argument is one named conformance of that
/// exact trait application. It names code, not data; the trait arguments carry
/// every data type the interface needs, and no parameter is a subject
/// ([named conformances](../../../../../../../../wiki/spec/language/conformances.md#declaration-and-selection)).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GenericConformanceBound {
    pub binder: Identifier,
    pub trait_name: Identifier,
    pub arguments: HandleSpan<crate::types::TypeReferenceHandle>,
}
