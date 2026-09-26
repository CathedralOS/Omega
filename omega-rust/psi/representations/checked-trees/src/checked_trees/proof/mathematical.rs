use symbols::SymbolHandle;

/// A checked top-level mathematical declaration: the checked proof surface a
/// parsed `let`/`boundary let` elaborates into under PROOF-CONTRACT-MIGRATION
/// (wiki/spec/proofs/mathematical_bindings.md).
///
/// The record mirrors the proof kernel's `Declaration`
/// (`mathematical_core::signature`) rather than introducing a second logical
/// representation: the generic binders supply the universe parameters the
/// declaration is polymorphic over, the ordered parameter telescope plus
/// `result` is the statement's nested dependent-Π spine, and `body` is either
/// a transparent checked term or the absence that makes the declaration a
/// named assumption whose term and transitive dependencies require the
/// receiver's admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedMathematicalDeclaration {
    /// The resolved declaration symbol. For `boundary let` this is the exact
    /// declaration/trust identity transitive-assumption admission keys on.
    pub symbol: SymbolHandle,
    pub name: String,
    pub is_public: bool,
    /// Generic binders in declaration order.
    pub binders: Vec<CheckedMathematicalBinder>,
    /// The ordered ordinary parameter telescope. A binder's occurrences
    /// scope over every later parameter and the result.
    pub parameters: Vec<CheckedMathematicalParameter>,
    /// The declared result type identity — the innermost codomain of the
    /// statement's nested Π.
    pub result: String,
    pub body: CheckedMathematicalBody,
}

impl CheckedMathematicalDeclaration {
    /// Universe-level binders this declaration is polymorphic over — the
    /// `level_arity` the elaborated kernel declaration carries.
    pub fn level_arity(&self) -> usize {
        self.binders
            .iter()
            .filter(|binder| matches!(binder.kind, CheckedMathematicalBinderKind::Level))
            .count()
    }

    /// Whether the declaration is a `boundary let` named assumption — a
    /// bodyless declaration whose admission the receiver must grant, never an
    /// implementation-search slot.
    pub fn is_assumption(&self) -> bool {
        matches!(self.body, CheckedMathematicalBody::Assumption)
    }
}

/// One generic binder of a checked mathematical declaration, in declaration
/// order. The kind records which fixed `core` declaration the authored
/// carrier resolved to — that resolution, not a binder keyword, is what makes
/// `u: core::Level` a universe parameter and `A: core::Type<u>` a type
/// parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedMathematicalBinder {
    pub name: String,
    pub kind: CheckedMathematicalBinderKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedMathematicalBinderKind {
    /// `u: core::Level` — a universe-level binder.
    Level,
    /// `A: core::Type<u>` — a mathematical type binder. `universe` is the
    /// authored level argument's identity when one was declared; a bare `A`
    /// records `None` for the elaborator to classify or refuse.
    Type { universe: Option<String> },
    /// `const N: usize` — a compile-time value binder.
    Const { type_identity: String },
    /// `name: Carrier` — a mathematical subject binder in the generic
    /// telescope whose carrier is neither `core::Level` nor `core::Type`
    /// (for example `<pred: A -> core::Strict<0>>`-shaped binders once the
    /// grammar admits them).
    Value { type_identity: String },
}

/// One ordinary parameter of a checked mathematical declaration's telescope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedMathematicalParameter {
    pub name: String,
    /// The binding occurrence's `[erased]` relevance: an erased proof
    /// reference demands no executable value while retaining trust.
    pub relevance: language_core::BindingRelevance,
    /// The declared type identity; an arrow type spells its nested Π.
    pub type_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedMathematicalBody {
    /// `= term;` — a transparent checked term definition. The term identity
    /// substitutes for every use under conversion; nothing executable is
    /// implied by it.
    Definition { term_identity: String },
    /// `boundary let ...;` — a named assumption with no body. Its term and
    /// transitive dependencies require the receiver's admission, keyed on the
    /// declaration's exact identity.
    Assumption,
}

#[cfg(test)]
mod tests {
    use super::{
        CheckedMathematicalBinder, CheckedMathematicalBinderKind, CheckedMathematicalBody,
        CheckedMathematicalDeclaration, CheckedMathematicalParameter,
    };
    use symbols::SymbolHandle;

    #[test]
    fn level_arity_counts_only_universe_binders() {
        let declaration = CheckedMathematicalDeclaration {
            symbol: SymbolHandle::invalid(),
            name: "compose".to_owned(),
            is_public: false,
            binders: vec![
                CheckedMathematicalBinder {
                    name: "u".to_owned(),
                    kind: CheckedMathematicalBinderKind::Level,
                },
                CheckedMathematicalBinder {
                    name: "A".to_owned(),
                    kind: CheckedMathematicalBinderKind::Type {
                        universe: Some("u".to_owned()),
                    },
                },
                CheckedMathematicalBinder {
                    name: "v".to_owned(),
                    kind: CheckedMathematicalBinderKind::Level,
                },
                CheckedMathematicalBinder {
                    name: "N".to_owned(),
                    kind: CheckedMathematicalBinderKind::Const {
                        type_identity: "usize".to_owned(),
                    },
                },
            ],
            parameters: Vec::new(),
            result: "A".to_owned(),
            body: CheckedMathematicalBody::Definition {
                term_identity: "…".to_owned(),
            },
        };

        assert_eq!(declaration.level_arity(), 2);
        assert!(!declaration.is_assumption());
    }

    #[test]
    fn boundary_let_shape_is_a_named_assumption() {
        let declaration = CheckedMathematicalDeclaration {
            symbol: SymbolHandle::invalid(),
            name: "choose".to_owned(),
            is_public: false,
            binders: Vec::new(),
            parameters: vec![CheckedMathematicalParameter {
                name: "inhabited".to_owned(),
                relevance: language_core::BindingRelevance::Relevant,
                type_identity: "core::Squash<A>".to_owned(),
            }],
            result: "A".to_owned(),
            body: CheckedMathematicalBody::Assumption,
        };

        assert!(declaration.is_assumption());
        assert_eq!(declaration.level_arity(), 0);
    }
}
