//! Proof terms: the erased terms a contract's erased lane carries for
//! formals whose carriers admit no scalar or runtime argument slot.

use crate::proposition::{PropositionError, ScalarTerm};

/// Exact namespace of a proof formal. Invocation inputs remain immutable for
/// the activation; block inputs are rebound by explicit control-flow edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofFormalScope {
    Invocation,
    Block,
}

/// An exact input to a closed mathematical function. Scalar observations keep
/// their independently formed source carrier identity and ordinary SSA term;
/// they do not become signature assumptions or executable proof values.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MathematicalArgument {
    Formal {
        position: u32,
    },
    Scalar {
        type_identity: String,
        term: ScalarTerm,
    },
}

/// One erased term in a call's or edge's erased lane. Proof terms carry
/// semantic identity only — they own no runtime value, storage, or
/// evaluation. A `Construction` is a closed data value such as `Nat::Zero`
/// or an erased record carrier's exact literal; a `Formal` names the
/// caller's own erased proof formal so a proof binding passes through a
/// nested call unchanged; an `Array` retains ordered elements and its exact
/// type; a `Scalar` is a primitive payload leaf, expressed over the caller's scalar namespace
/// like any erased scalar actual.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofTerm {
    /// A checked mathematical judgment in the canonical certificate wire form.
    /// The complete signature retains assumptions even after runtime erasure.
    /// A receiver must independently bind its claimed type to the formal's
    /// retained carrier; the textual source identity alone grants no authority.
    Mathematical {
        type_identity: String,
        certificate: Vec<u8>,
    },
    /// Ordinary application of a closed mathematical function to this scope's
    /// exact scalar observations and erased formals. Receiving checks every domain and the result
    /// against independent source carriers; this is never an open certificate.
    MathematicalApplication {
        type_identity: String,
        certificate: Vec<u8>,
        arguments: Vec<MathematicalArgument>,
        formal_scope: ProofFormalScope,
    },
    /// A fixed array in the erased lane. Its normalized type retains element
    /// and extent identity; elements remain ordered semantic evidence.
    Array {
        type_identity: String,
        elements: Vec<ProofTerm>,
    },
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
    /// An immutable proof input of the enclosing machine invocation.
    InvocationFormal { position: u32 },
    /// A primitive payload leaf of an erased record/enum construction. The
    /// scalar term's `ValueId`s refer to the caller's own admitted scalar
    /// scope — the same namespace erased scalar actuals name.
    Scalar(ScalarTerm),
}

impl ProofTerm {
    /// Visit caller-side scalar identities retained by this erased term.
    /// Closed certificates and proof formals introduce no runtime value uses.
    pub fn visit_value_ids(&self, mut visit: impl FnMut(crate::ValueId) -> bool) -> bool {
        let mut pending = vec![self];
        while let Some(term) = pending.pop() {
            match term {
                Self::Scalar(term) => {
                    if !term.visit_value_ids(&mut visit) {
                        return false;
                    }
                }
                Self::Construction { fields, .. } => {
                    pending.extend(fields.iter().rev().map(|field| &field.term))
                }
                Self::Array { elements, .. } => pending.extend(elements.iter().rev()),
                Self::MathematicalApplication { arguments, .. } => {
                    for argument in arguments {
                        if let MathematicalArgument::Scalar { term, .. } = argument
                            && !term.visit_value_ids(&mut visit)
                        {
                            return false;
                        }
                    }
                }
                Self::Mathematical { .. } | Self::Formal { .. } | Self::InvocationFormal { .. } => {
                }
            }
        }
        true
    }

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
                Step::Term(ProofTerm::Array {
                    type_identity,
                    elements,
                }) => {
                    if type_identity.is_empty() {
                        return Err(PropositionError::EmptyProofTermTypeIdentity);
                    }
                    pending.extend(elements.iter().rev().map(Step::Term));
                }
                Step::Term(ProofTerm::Mathematical {
                    type_identity,
                    certificate,
                }) => {
                    if type_identity.is_empty() || certificate.is_empty() {
                        return Err(PropositionError::EmptyProofTermTypeIdentity);
                    }
                }
                Step::Term(ProofTerm::MathematicalApplication {
                    type_identity,
                    certificate,
                    arguments,
                    ..
                }) => {
                    if type_identity.is_empty() || certificate.is_empty() || arguments.is_empty() {
                        return Err(PropositionError::EmptyProofTermTypeIdentity);
                    }
                    for argument in arguments {
                        if let MathematicalArgument::Scalar {
                            type_identity,
                            term,
                        } = argument
                        {
                            if type_identity.is_empty() {
                                return Err(PropositionError::EmptyProofTermTypeIdentity);
                            }
                            term.validate()?;
                        }
                    }
                }
                Step::Term(ProofTerm::Formal { .. } | ProofTerm::InvocationFormal { .. }) => {}
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
