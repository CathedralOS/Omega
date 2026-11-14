//! Complete preparation output consumed by the Terminal checking stage.
//! Every demanded generic and conformance module is present before judgment.

use crate::{Checked, program::Elaboration};
use diagnostics::Diagnostic;
use semantic_vocabulary::CallableShapeMismatch;
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;

#[derive(Debug, Clone)]
pub struct CallableFormationFailure {
    pub slot: String,
    pub implementation: SymbolHandle,
    pub source_span: Option<source::SourceSpan>,
    pub cause: CallableFormationCause,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallableFormationCause {
    NoEntry,
    Shape(CallableShapeMismatch),
    UnsuppliedEvidence {
        owner: SymbolHandle,
        contract: usize,
    },
}

impl CallableFormationFailure {
    pub fn reject_diagnostic(&self) -> Diagnostic {
        let diagnostic = Diagnostic::error(&self.detail);
        self.source_span
            .map_or(diagnostic.clone(), |span| diagnostic.with_source_span(span))
    }
}

/// Missing custody or unresolved identities are defects/unavailable checking,
/// never evidence that the source has the wrong calling shape. No synthetic
/// Terminal machine identity is minted for a declaration not yet elaborated.
#[derive(Debug, Clone)]
pub struct CallableFormationDefect {
    pub slot: String,
    pub detail: String,
}

#[derive(Debug)]
pub enum TerminalCompilation {
    FormationFailures(Vec<CallableFormationFailure>),
    FormationDefect(CallableFormationDefect),
    Prepared(PreparedCompilation),
}

#[derive(Debug)]
pub struct PreparedCompilation {
    pub closure: Checked<Elaboration>,
    /// Proof-only calls from each installed source arrival to its continuation.
    /// These never become executable or publishable closures.
    pub arrivals: Vec<Checked<CallableArrivalWitness>>,
    pub forms: Vec<GenericForms>,
    pub witnesses: Vec<Checked<Elaboration>>,
    /// Exact generated machine to template correspondence, fixed by preparation.
    pub specializations: semantic_vocabulary::IdTable<MachineId, Option<SymbolHandle>>,
    /// Root selection position to its independently judged conformance module.
    pub selection_witnesses: Vec<(usize, usize)>,
}

#[derive(Debug)]
pub struct GenericForms {
    pub template: SymbolHandle,
    /// Diagnostic spelling only; dependencies use positions and exact identities.
    pub name: String,
    pub forms: Checked<Vec<Elaboration>>,
    /// Positions in the complete form roster. Missing correspondence stays unknown.
    pub applies: Option<Vec<usize>>,
}

/// A source-authenticated formal and its exact installed-argument projection.
#[derive(Debug)]
pub struct CallableInputOrigin {
    pub occurrence: symbols::ParameterOccurrence,
    pub projected_from: usize,
}

/// The source meaning of a nominal atom, beyond its local proposition number.
/// Applied domains are not approximated by this unapplied declaration receipt.
#[derive(Debug)]
pub struct NominalPremiseOrigin {
    pub atom: semantic_vocabulary::PropositionId,
    pub occurrence: symbols::ParameterOccurrence,
    pub path: symbols::SymbolPath,
    pub domain: symbols::RetainedDeclaration,
}

/// An ordinary symbolic call under the complete installed source premises.
/// Hidden inputs remain in this proof telescope, independently of executable
/// parameter projection. The member arena belongs only to these source paths.
#[derive(Debug)]
pub struct CallableArrivalWitness {
    /// Diagnostic label; the retained declarations and model own identity.
    pub slot: String,
    pub semantic_schema: symbols::RetainedDeclaration,
    pub requirement: symbols::RetainedDeclaration,
    pub implementation: symbols::RetainedDeclaration,
    pub elaboration: Elaboration,
    pub required_inputs: Vec<CallableInputOrigin>,
    pub supplied_inputs: Vec<CallableInputOrigin>,
    pub nominal_premises: Vec<NominalPremiseOrigin>,
    pub members: arena::Arena<symbols::SymbolHandle>,
    pub address_model: Option<semantic_vocabulary::ScalarAddressModel>,
}
