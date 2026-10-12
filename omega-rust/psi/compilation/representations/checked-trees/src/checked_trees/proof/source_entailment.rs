//! Original source contract entailment decisions and their observed reads.
//!
//! The source judging operation records positive guarantees only after its
//! diagnostic and all-exit acceptance boundary. Its mathematical reads name the
//! original formed applications, not normalized scratch or kernel AcceptedFact.
//! A declined decision has no entailment; an observed read roster certifies
//! neither complete semantic closure nor every proof producer's consumption.

use arena::{Arena, Handle, HandleSpan};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

use super::CheckedMathematicalApplication;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CheckedSourceContractDecision {
    pub machine: SymbolHandle,
    /// Zero is an unproved/declined source decision, not an empty acceptance.
    pub entailment: Handle<CheckedSourceContractEntailment>,
}

/// Positive guarantees from one original source contract judging operation.
/// Captures, terms and application handles resolve beside the original
/// ProofFacts mathematical signature/application/capture arenas.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CheckedSourceContractEntailment {
    pub machine: SymbolHandle,
    pub guarantees: HandleSpan<ExpressionHandle>,
    /// Original guarantee occurrences proved on every completed normal-return
    /// path after its effects. Other proof routes retain entry-preservation
    /// obligations; this span is not complete semantic dependency evidence.
    pub normal_return_guarantees: HandleSpan<ExpressionHandle>,
    /// Actual prepared application reads during this source operation, including
    /// premise intake and attempts the operation consulted. They are not a
    /// claim that each read supports every guarantee or was a kernel premise.
    /// An empty span means no prepared reads were observed in this operation,
    /// never that unavailable, memo-only or other producer dependencies are empty.
    pub mathematical_reads: HandleSpan<Handle<CheckedMathematicalApplication>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedSourceContractEntailments {
    decisions: Arena<CheckedSourceContractDecision>,
    entailments: Arena<CheckedSourceContractEntailment>,
    guarantees: Arena<ExpressionHandle>,
    mathematical_reads: Arena<Handle<CheckedMathematicalApplication>>,
}

impl CheckedSourceContractEntailments {
    pub fn contains_machine(&self, machine: SymbolHandle) -> bool {
        self.decisions
            .iter()
            .any(|(_, decision)| decision.machine == machine)
    }

    /// Publish an original decision. The caller supplies only positively judged
    /// guarantees surviving its original acceptance boundary. An empty input
    /// retains a declined machine row and discards every attempted read. The
    /// normal-return subset names only guarantees from this same acceptance.
    pub fn record_decision(
        &mut self,
        machine: SymbolHandle,
        guarantees: Vec<ExpressionHandle>,
        mathematical_reads: Vec<Handle<CheckedMathematicalApplication>>,
        normal_return_guarantees: Vec<ExpressionHandle>,
    ) {
        let entailment = if guarantees.is_empty() {
            Handle::default()
        } else {
            let normal_return_guarantees = self.guarantees.insert_many(normal_return_guarantees);
            let guarantees = self.guarantees.insert_many(guarantees);
            let mathematical_reads = self.mathematical_reads.insert_many(mathematical_reads);
            self.entailments.append(CheckedSourceContractEntailment {
                machine,
                guarantees,
                normal_return_guarantees,
                mathematical_reads,
            })
        };
        self.decisions.append(CheckedSourceContractDecision {
            machine,
            entailment,
        });
    }

    pub fn entailments(&self) -> &Arena<CheckedSourceContractEntailment> {
        &self.entailments
    }

    fn entailment_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedSourceContractEntailment> {
        let entailment = self
            .decisions
            .iter()
            .find_map(|(_, decision)| (decision.machine == machine).then_some(decision.entailment))
            .filter(|entailment| entailment.is_valid())?;
        Some(self.entailments.get(entailment))
    }

    pub fn guarantees_for_machine(&self, machine: SymbolHandle) -> &[ExpressionHandle] {
        self.entailment_for_machine(machine)
            .map_or(&[], |entailment| {
                self.guarantees.span_or_empty(entailment.guarantees)
            })
    }

    /// Read the original source decision's effect-aware normal-return subset.
    /// Absence is unavailable support, never an empty proof of arbitrary exits.
    pub fn normal_return_guarantees_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> &[ExpressionHandle] {
        self.entailment_for_machine(machine)
            .map_or(&[], |entailment| {
                self.guarantees
                    .span_or_empty(entailment.normal_return_guarantees)
            })
    }

    pub fn mathematical_reads(
        &self,
        entailment: &CheckedSourceContractEntailment,
    ) -> &[Handle<CheckedMathematicalApplication>] {
        self.mathematical_reads
            .span_or_empty(entailment.mathematical_reads)
    }
}
