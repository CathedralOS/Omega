use psi_arena::{Handle, HandleSpan};
use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::ExpressionHandle;

use crate::CheckedValueOrigin;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ContractProofFactKind {
    #[default]
    Requires,
    Ensures,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ContractProofFactOwner {
    #[default]
    Unknown,
    Machine {
        machine_symbol: SymbolHandle,
    },
    MachineState {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    StateSignature {
        owner_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    OperatorUse {
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
        operator_symbol: SymbolHandle,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BoundaryQualificationAuthorization {
    /// The boundary trait that owns the admitted requirement.
    pub requirement_symbol: SymbolHandle,
    /// The exact requirement signature whose result is qualified.
    pub signature_symbol: SymbolHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContractProofFact {
    pub kind: ContractProofFactKind,
    pub owner: ContractProofFactOwner,
    pub fact: Handle<psi_typed_trees::domain::ProofFact>,
    /// Exact erased evidence term declared by a named machine contract.
    /// The arena handle is term identity; proposition identity and eventual
    /// producer provenance remain separate records.
    pub evidence_term: Option<Handle<CheckedEvidenceTerm>>,
    /// Present only for an exact `ensures result in Domain` fact published by
    /// a boundary requirement whose result carrier matches the domain target.
    pub qualification_authorization: Option<BoundaryQualificationAuthorization>,
}

/// Producer-side carrier for one outcome-specific guarantee row. These rows
/// stay separate from unconditional contract facts. A named row owns an erased
/// output identity so the producer can discharge it, but that identity is not
/// published to callers until guarded result-arm selection is implemented.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutcomeSpecificGuaranteeFact {
    pub machine_symbol: SymbolHandle,
    pub result_data: SymbolHandle,
    pub result_case: SymbolHandle,
    pub public_selector: Option<String>,
    pub fact: Handle<psi_typed_trees::domain::ProofFact>,
    pub evidence_term: Option<Handle<CheckedEvidenceTerm>>,
}

/// Caller-side availability for the guarded guarantees of one exact result
/// arm. The arm coordinate is the scope boundary: rows are never installed at
/// the producer call's unconditional ensures point.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutcomeSpecificArmFact {
    pub caller_machine_symbol: SymbolHandle,
    pub caller_state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub result_call_statement_index: usize,
    pub result_data: SymbolHandle,
    pub result_case: SymbolHandle,
    pub result_expression: ExpressionHandle,
    pub rows: Vec<OutcomeSpecificArmRowFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutcomeSpecificArmRowFact {
    pub guarantee: Handle<OutcomeSpecificGuaranteeFact>,
    /// Canonical caller-side proposition after call arguments and the concrete
    /// saved result occurrence have been substituted.
    pub instantiated_proposition: Option<crate::CheckedPropositionApplication>,
    pub instantiated_identity: Option<String>,
    /// Structured checked validity input retained before normalized labels
    /// erase caller-place structure. The result occurrence always
    /// participates; `referenced_occurrences` are expressions in the
    /// producer contract and are instantiated through the exact source call.
    /// The interface identity carries any additional witness scope without
    /// reconstructing it from display strings.
    pub validity: OutcomeSpecificValidityFact,
    /// Present only for an explicitly selected named row.
    pub selected_term: Option<Handle<CheckedEvidenceTerm>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutcomeSpecificValidityFact {
    pub result_occurrence: ExpressionHandle,
    pub referenced_occurrences: Vec<ExpressionHandle>,
    pub evidence_interface_scope: Option<OutcomeSpecificEvidenceInterfaceScopeFact>,
}

/// Checked structural lifetime input contributed by a witness-bearing
/// proposition's carrierless evidence interface. Type handles preserve the
/// interface's exact reference-region nodes; retained occurrences name the
/// value scopes intersected with them at the caller arm.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutcomeSpecificEvidenceInterfaceScopeFact {
    pub interface: crate::CheckedEvidenceInterfaceIdentity,
    pub evidence_type: psi_typed_trees::types::TypeReferenceHandle,
    pub reference_regions: Vec<psi_typed_trees::types::TypeReferenceHandle>,
    pub retained_occurrences: Vec<ExpressionHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedEvidenceTerm {
    /// Public output-field name for `ensures`, local input alias for `requires`.
    pub name: String,
    pub owner: ContractProofFactOwner,
    pub kind: ContractProofFactKind,
    /// Position within the matching erased requires/ensures lane.
    pub lane_position: usize,
    /// Exact normalized proposition application inhabited by this term.
    pub proposition: crate::CheckedPropositionApplication,
    /// Canonical carrierless interface retained by the proposition endpoint.
    pub evidence_type: String,
    /// Exact instantiated interface identity. `None` keeps an unresolved
    /// generic endpoint fail-closed for producer selection while preserving
    /// the diagnostic spelling above.
    pub evidence_interface: Option<crate::CheckedEvidenceInterfaceIdentity>,
}

impl Default for CheckedEvidenceTerm {
    fn default() -> Self {
        Self {
            name: String::new(),
            owner: ContractProofFactOwner::Unknown,
            kind: ContractProofFactKind::Requires,
            lane_position: 0,
            proposition: crate::CheckedPropositionApplication {
                declaration: SymbolHandle::invalid(),
                binder_arguments: Vec::new(),
                arguments: Vec::new(),
                evidence_interface: None,
            },
            evidence_type: String::new(),
            evidence_interface: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContractProofFactRef {
    pub fact: Handle<ContractProofFact>,
}

/// One explicit erased call-lane binding. Both ends are exact checked term
/// identities; position is semantic and source names are never matched to
/// callee names.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContractEvidenceArgument {
    pub source: Handle<CheckedEvidenceTerm>,
    pub parameter: Handle<CheckedEvidenceTerm>,
    pub lane_position: usize,
}

/// The exact checked source of one erased outgoing evidence assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceAssignmentSource {
    /// Preserve the identity of one exact incoming term.
    Forwarded { term: Handle<CheckedEvidenceTerm> },
    /// Introduce a fresh term through one explicitly selected, complete,
    /// subjectless conformance. Rows are retained so later proof consumers do
    /// not repeat selection or reconstruct completeness from a name.
    ProducerConformance {
        conformance: SymbolHandle,
        evidence_trait: SymbolHandle,
        rows: Vec<crate::DynamicConformanceRowFact>,
    },
}

impl Default for EvidenceAssignmentSource {
    fn default() -> Self {
        Self::Forwarded {
            term: Handle::invalid(),
        }
    }
}

/// One erased outgoing evidence slot assigned from an exact incoming term or
/// an explicit producer conformance. `output` retains the public field and
/// proposition identity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EvidenceForwardingFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub output: Handle<CheckedEvidenceTerm>,
    pub source: EvidenceAssignmentSource,
}

/// One immediate call whose selected proof outputs are bound to
/// fresh caller-local evidence terms. A scalar Type result additionally
/// names the exact ordinary runtime call coordinate.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofOutputCallFact {
    pub caller_machine_symbol: SymbolHandle,
    pub caller_state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub source_statement_index: usize,
    pub runtime_call: Option<ProofOutputRuntimeCallFact>,
    pub target_machine_symbol: SymbolHandle,
    pub target_state_symbol: SymbolHandle,
    /// Exact erased inputs supplied to the callee's named `requires` lane.
    /// These remain separate from an ordinary runtime call because a pure
    /// proof producer may erase completely.
    pub evidence_arguments: Vec<ProofOutputEvidenceArgumentFact>,
    pub outputs: Vec<ProofOutputFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofOutputEvidenceArgumentFact {
    pub input_position: usize,
    pub callee_input: Handle<CheckedEvidenceTerm>,
    pub source: Handle<CheckedEvidenceTerm>,
    pub instantiated_proposition: crate::CheckedPropositionApplication,
    pub instantiated_identity: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProofOutputRuntimeCallFact {
    pub statement_index: usize,
    pub call_ordinal: usize,
}

/// One exact published proof disposition from a proof-output call.
/// `callee_output` is the published lane declaration. `output` is the distinct
/// term introduced in the caller, or `None` when the copyable proposition is
/// omitted or explicitly discarded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofOutputFact {
    pub output_position: usize,
    pub callee_output: Handle<CheckedEvidenceTerm>,
    /// The callee proposition after substituting this call's ordinary value
    /// arguments. It differs from `callee_output` whenever formal names and
    /// caller expressions differ, including when the output is not captured.
    pub instantiated_proposition: crate::CheckedPropositionApplication,
    pub instantiated_identity: String,
    pub output: Option<Handle<CheckedEvidenceTerm>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContractCallFact {
    pub caller_machine_symbol: SymbolHandle,
    pub caller_state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub target_machine_symbol: SymbolHandle,
    pub target_state_symbol: SymbolHandle,
    pub requires: HandleSpan<ContractProofFactRef>,
    pub ensures: HandleSpan<ContractProofFactRef>,
    pub evidence_arguments: HandleSpan<ContractEvidenceArgument>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContractExitFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub ensures: HandleSpan<ContractProofFactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContractOperatorUseFact {
    pub expression: ExpressionHandle,
    pub origin: CheckedValueOrigin,
    pub operator_symbol: SymbolHandle,
    pub requires: HandleSpan<ContractProofFactRef>,
    pub ensures: HandleSpan<ContractProofFactRef>,
}
