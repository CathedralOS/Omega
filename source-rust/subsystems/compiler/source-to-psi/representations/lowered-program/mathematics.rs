//! The mathematical declarations of one compilation as a single kernel
//! signature. Stage 05 writes it from the typed declarations; stage 06 checks
//! it once with the kernel and judges each law against the entries before it.
//!
//! Data that contains itself has no storage, so it and the machines over it
//! exist only here: the data as a W type, its constructors and each machine
//! defined by cases as definitions, and each law as a statement whose proof
//! stage 06 produces. A law's own statement is not among the entries its
//! proof is checked against, so a law cannot supply its own evidence.

use mathematical_kernel::{Declaration, PlainData, TermArena, TermHandle};
use source_text::SourceSpan;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default)]
pub struct MathematicalSignature {
    pub arena: TermArena,
    /// In dependency order: a declaration refers only to earlier ones.
    pub declarations: Vec<Declaration>,
    /// What each declaration is, by the same index.
    pub entries: Vec<SignatureEntry>,
    pub laws: Vec<Law>,
}

impl MathematicalSignature {
    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty() && self.laws.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct SignatureEntry {
    pub kind: SignatureEntryKind,
    /// The source spelling, for diagnostics only.
    pub name: String,
}

#[derive(Debug, Clone)]
pub enum SignatureEntryKind {
    /// A plain data declaration: the closed W type of its cases.
    Data {
        declaration: SymbolHandle,
        shape: PlainData,
        /// The data's source type identity text. Stage 06 publishes it as
        /// the identity of the data's mathematical carrier row; nothing
        /// compares it while checking.
        type_identity: String,
    },
    /// A case of the data declared at index `data`, as a function of its fields.
    Constructor { data: u32, case: u32 },
    /// A machine defined by cases on one parameter.
    Function { machine: SymbolHandle },
    /// The statement of `laws[law]`, closed over its formals. It has no body:
    /// a proof that cites the law depends on it as an assumption, and the
    /// law's own proof is judged separately.
    Statement { law: u32 },
}

/// One `ensures` equality of a machine with no result, and how its body
/// argues for it.
#[derive(Debug, Clone)]
pub struct Law {
    pub machine: SymbolHandle,
    /// Which `ensures` clause of the machine this is.
    pub clause: u32,
    /// The index of this law's statement entry.
    pub statement: u32,
    /// The closed type of each formal, in order.
    pub formals: Vec<TermHandle>,
    /// The equality, under one binder per formal.
    pub goal: TermHandle,
    /// The same formals' equality declared by the trait requirement this
    /// machine satisfies, with each operation resolved to its satisfier.
    pub declared: Option<TermHandle>,
    pub proof: LawProof,
    /// For diagnostics only.
    pub formal_names: Vec<String>,
    pub machine_name: String,
    pub text: String,
    pub source: Option<SourceSpan>,
}

#[derive(Debug, Clone)]
pub enum LawProof {
    /// The body cites its evidence without a case split.
    Direct(LawCase),
    /// The body dispatches on one formal: an induction over the data
    /// declared at entry `data`, with one script per case in declared order.
    Cases {
        formal: u32,
        data: u32,
        cases: Vec<LawCase>,
    },
}

#[derive(Debug, Clone, Default)]
pub struct LawCase {
    pub citations: Vec<Citation>,
    /// The fields the case binds, for diagnostics only.
    pub field_names: Vec<String>,
    pub source: Option<SourceSpan>,
}

/// A call the body makes for its conclusion. Each argument is a term under
/// the law's formals followed by the fields of the enclosing case.
#[derive(Debug, Clone)]
pub struct Citation {
    pub evidence: Evidence,
    pub arguments: Vec<TermHandle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evidence {
    /// Another law, by its statement entry. Stage 05 admits only an earlier
    /// entry, so citations cannot form a cycle.
    Statement(u32),
    /// The law itself at one field of the case: the induction hypothesis.
    /// `arguments` then holds the remaining formals' actuals in order.
    Hypothesis { field: u32 },
}
