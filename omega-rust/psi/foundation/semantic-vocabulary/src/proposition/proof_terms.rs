//! Proof terms: the erased terms a contract's erased lane carries for
//! formals whose carriers admit no scalar or runtime argument slot.

use crate::ValueId;
use crate::proposition::{PropositionError, ScalarTerm};

/// One erased term in a call's or edge's erased lane. Proof terms carry
/// semantic identity only — they own no runtime value, storage, or
/// evaluation. A `Construction` is a closed data value such as `Nat::Zero`
/// or an erased record carrier's exact literal; a `Formal` names the
/// caller's own erased proof formal so a proof binding passes through a
/// nested call unchanged; a `Scalar` is one primitive payload leaf inside a
/// record/enum construction, expressed over the caller's scalar namespace
/// like any erased scalar actual.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofTerm {
    /// A closed data construction. Fields stay in authored
    /// declaration order so the canonical encoding is stable.
    Construction {
        /// Canonical semantic identity of the constructed data type.
        type_identity: String,
        /// Canonical identity of the selected case within the type. Absent
        /// exactly when the checked literal selected no case.
        case_identity: Option<String>,
        /// Payload fields, each retaining its field identity.
        fields: Vec<ProofTermField>,
    },
    /// One erased proof formal of the enclosing scope. `position` is the
    /// dense index into the enclosing block's `erased_proof_formals` roster.
    Formal { position: u32 },
    /// A primitive payload leaf of an erased record/enum construction. The
    /// scalar term's `ValueId`s refer to the caller's own admitted scalar
    /// scope — the same namespace erased scalar actuals name.
    Scalar(ScalarTerm),
}

impl ProofTerm {
    /// Structural validity: construction identities are populated, fields
    /// validate, and the term carries no runtime reference.
    pub fn validate(&self) -> Result<(), PropositionError> {
        // An explicit worklist keeps deep constructions off the call stack:
        // `Field` steps keep each field-identity check in front of its own
        // term, matching the error order the recursive validator produced.
        enum Step<'a> {
            Term(&'a ProofTerm),
            Field(&'a ProofTermField),
        }
        let mut pending = vec![Step::Term(self)];
        while let Some(step) = pending.pop() {
            match step {
                Step::Term(ProofTerm::Construction {
                    type_identity,
                    case_identity,
                    fields,
                }) => {
                    if type_identity.is_empty() {
                        return Err(PropositionError::EmptyProofTermTypeIdentity);
                    }
                    if case_identity.as_deref().is_some_and(str::is_empty) {
                        return Err(PropositionError::EmptyProofTermCaseIdentity);
                    }
                    for field in fields.iter().rev() {
                        pending.push(Step::Field(field));
                    }
                }
                Step::Term(ProofTerm::Formal { .. }) => {}
                Step::Term(ProofTerm::Scalar(term)) => term.validate()?,
                Step::Field(field) => {
                    if field.field_identity.is_empty() {
                        return Err(PropositionError::EmptyProofTermFieldIdentity);
                    }
                    pending.push(Step::Term(&field.term));
                }
            }
        }
        Ok(())
    }

    /// Visit every erased-proof-formal position this term names.
    pub fn visit_formal_positions(&self, mut visit: impl FnMut(u32)) {
        // Field terms push in reverse so positions still surface in authored
        // field order without recursing on the construction depth.
        let mut pending = vec![self];
        while let Some(term) = pending.pop() {
            match term {
                Self::Construction { fields, .. } => {
                    for field in fields.iter().rev() {
                        pending.push(&field.term);
                    }
                }
                Self::Formal { position } => visit(*position),
                // A scalar leaf names caller scalar values, never erased
                // proof formals — no position to visit.
                Self::Scalar(_) => {}
            }
        }
    }

    /// Visit every `ValueId` a `Scalar` leaf carries. Returning `false` from
    /// `visit` short-circuits the traversal and propagates `false`, matching
    /// `ScalarTerm::visit_value_ids`.
    pub fn visit_scalar_value_ids(&self, mut visit: impl FnMut(ValueId) -> bool) -> bool {
        let mut pending = vec![self];
        while let Some(term) = pending.pop() {
            match term {
                Self::Construction { fields, .. } => {
                    for field in fields.iter().rev() {
                        pending.push(&field.term);
                    }
                }
                Self::Formal { .. } => {}
                Self::Scalar(scalar) => {
                    if !scalar.visit_value_ids(&mut visit) {
                        return false;
                    }
                }
            }
        }
        true
    }
}

/// One named payload field of a proof-term construction. Field order is the
/// authored order; the verifier replays it verbatim.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofTermField {
    /// Canonical field identity inside the selected case.
    pub field_identity: String,
    /// The field's own proof term.
    pub term: ProofTerm,
}

impl ProofTermField {
    pub fn validate(&self) -> Result<(), PropositionError> {
        if self.field_identity.is_empty() {
            return Err(PropositionError::EmptyProofTermFieldIdentity);
        }
        self.term.validate()
    }
}
