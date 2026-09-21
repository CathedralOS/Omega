//! Checked proof-only terms: the erased actuals recorded for proof-only
//! erased formals, and the role keys that locate them.

use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

/// One erased formal whose carrier is proof-only (`Nat` and friends): it
/// admits no scalar lane, so the contract term lane carries its semantic
/// type identity instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedErasedProofParameterPlan {
    /// The parameter's dense authored position in the signature.
    pub source_position: u32,
    /// The parameter's symbol; callers resolve `Formal{parameter_symbol}`
    /// occurrences into this roster's dense proof-lane position.
    pub parameter_symbol: SymbolHandle,
    /// Canonical semantic identity of the proof-only type (`Nat`).
    pub type_identity: String,
}

/// Where one checked proof term belongs. Mirroring the scalar-expression
/// roles keeps each erased actual unambiguous under its call coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckedProofTermRole {
    /// Proof-only actual for one erased proof formal of a direct bound call,
    /// keyed by the call's binding ordinal and the dense ordinal in the
    /// callee's erased-proof roster.
    ErasedCallArgument {
        binding_ordinal: u32,
        erased_ordinal: u32,
    },
    /// Proof-only actual for one erased proof formal of an in-module Unit
    /// call, keyed by the exact call coordinate and the erased-proof
    /// ordinal in the callee's contract roster.
    ErasedUnitCallArgument {
        call_ordinal: u32,
        erased_ordinal: u32,
    },
    /// Erased proof actual on a transition edge, keyed by dense
    /// erased-proof argument order.
    TransitionArgument { argument_ordinal: u32 },
    /// False-arm continuation operands keep a distinct key from the primary
    /// transition arguments.
    TransitionContinuationArgument { argument_ordinal: u32 },
}

/// One proof-only erased actual recorded at its exact authored position.
/// The term is construction-shaped or a pass-through of the caller's own
/// erased proof formal; neither owns a runtime operand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedProofTerm {
    /// A closed proof-only data construction (`Nat::Zero`, `Nat::Succ`).
    /// Symbols stay resolved until the terminal lane canonicalizes them.
    Construction {
        /// The constructed data type's definition symbol.
        data_symbol: SymbolHandle,
        /// The data type's canonical semantic identity — the same normalized
        /// spelling erased proof formals declare, so contract comparisons
        /// never depend on symbol-table lookups downstream.
        type_identity: String,
        /// The selected case's symbol, when the type has cases.
        case_symbol: Option<SymbolHandle>,
        /// Payload fields in authored order.
        fields: Vec<CheckedProofTermField>,
    },
    /// A pass-through of the caller's own erased proof formal, keyed by the
    /// formal's symbol so scope lookup survives roster reorderings.
    Formal { parameter_symbol: SymbolHandle },
}

impl Default for CheckedProofTerm {
    fn default() -> Self {
        Self::Formal {
            parameter_symbol: SymbolHandle::invalid(),
        }
    }
}

/// One named payload field of a checked proof-term construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedProofTermField {
    /// The field's declared symbol inside the selected case. The terminal
    /// lane canonicalizes its identity at lowering.
    pub field_symbol: SymbolHandle,
    /// The field's own proof term.
    pub term: CheckedProofTerm,
}

/// A proof term pinned to its authored location and role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedLocatedProofTerm {
    /// The state whose statement authored the term.
    pub state: SymbolHandle,
    /// The statement ordinal inside that state.
    pub statement_ordinal: u32,
    /// Which erased-argument slot this term fills.
    pub role: CheckedProofTermRole,
    /// The authored argument expression, for diagnostics.
    pub expression: ExpressionHandle,
    /// The checked proof term itself.
    pub term: CheckedProofTerm,
}

/// The recorded proof terms of one program, keyed like the scalar
/// expression facts so lookups stay coordinate-exact.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedProofTerms {
    /// Every recorded proof term, in record order.
    pub terms: Vec<CheckedLocatedProofTerm>,
}

impl CheckedProofTerms {
    /// Select the unique term recorded at one exact coordinate.
    pub fn term_at(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
        role: CheckedProofTermRole,
    ) -> Option<&CheckedProofTerm> {
        let mut matches = self.terms.iter().filter(|term| {
            term.state == state && term.statement_ordinal == statement_ordinal && term.role == role
        });
        let term = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        Some(&term.term)
    }
}
