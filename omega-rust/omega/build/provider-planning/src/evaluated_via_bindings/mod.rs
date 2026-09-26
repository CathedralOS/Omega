//! Exact evaluation and retention of ordinary source `via` bindings.
//!
//! The table is the sole bridge between a typed source expression and provider
//! derivation. It retains arena coordinates only for exact replay inside the
//! same compilation; the installed import owns stable locator and receipt
//! identity and contains no arena handles.
//!
//! This file owns the evaluated via-binding rows and table and evaluates
//! the bindings. `binding_evaluation.rs` evaluates one binding and validates
//! its shape, `binding_values.rs` decodes binding values and fields,
//! `binding_digests.rs` derives the producer, source, evaluation and
//! materialization digests and `tests.rs` holds the binding tests.

mod binding_digests;
mod binding_evaluation;
mod binding_values;
#[cfg(test)]
mod tests;

use crate::evaluated_via_bindings::binding_digests::at;
use crate::evaluated_via_bindings::binding_evaluation::{evaluate_one, exact_binding_vocabulary};
use build_time_evaluation::BuildTimeAdmissionPlan;
use diagnostics::Diagnostic;
use effects::provider_plan::{
    EvaluatedBindingReceipt, EvaluatedForeignImport, EvaluatedForeignSyscall, ProviderBinding,
};
use package_compilation::PackageCompilationInputs;
use source::SourceSpan;
use std::sync::Arc;
use symbols::SymbolHandle;
use target::TargetProfile;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;

const MATERIALIZER_SCHEMA_VERSION: u32 = 1;

/// One exact normalized result of evaluating the compiler-owned `Binding`
/// sum. The variant remains explicit so ordinary syscall evidence cannot be
/// reinterpreted as a legacy syscall carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaluatedViaBinding {
    Import(EvaluatedForeignImport),
    Syscall(EvaluatedForeignSyscall),
}

impl EvaluatedViaBinding {
    pub const fn target(&self) -> TargetProfile {
        match self {
            Self::Import(evaluated) => evaluated.locator().target(),
            Self::Syscall(evaluated) => evaluated.target(),
        }
    }

    pub const fn receipt(&self) -> &EvaluatedBindingReceipt {
        match self {
            Self::Import(evaluated) => evaluated.receipt(),
            Self::Syscall(evaluated) => evaluated.receipt(),
        }
    }

    pub const fn identity_digest(&self) -> target::ForeignLocatorIdentityDigest {
        match self {
            Self::Import(evaluated) => evaluated.locator().identity_digest(),
            Self::Syscall(evaluated) => evaluated.identity_digest(),
        }
    }

    pub fn provider_binding(&self) -> ProviderBinding {
        match self {
            Self::Import(evaluated) => ProviderBinding::Import {
                evaluated: evaluated.clone(),
            },
            Self::Syscall(evaluated) => ProviderBinding::Syscall {
                number: evaluated.number(),
            },
        }
    }

    pub const fn as_import(&self) -> Option<&EvaluatedForeignImport> {
        match self {
            Self::Import(evaluated) => Some(evaluated),
            Self::Syscall(_) => None,
        }
    }

    pub const fn as_syscall(&self) -> Option<&EvaluatedForeignSyscall> {
        match self {
            Self::Import(_) => None,
            Self::Syscall(evaluated) => Some(evaluated),
        }
    }
}

/// Exact typed-program join for one ordinary `via` expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedViaBindingRow {
    realization_machine: SymbolHandle,
    satisfied_owner: SymbolHandle,
    requirement: SymbolHandle,
    via_expression: typed_trees::expression::ExpressionHandle,
    producer_machine: SymbolHandle,
    producer_entry_state: SymbolHandle,
    via_source_span: SourceSpan,
    evaluated: EvaluatedViaBinding,
}

impl EvaluatedViaBindingRow {
    pub const fn realization_machine(&self) -> SymbolHandle {
        self.realization_machine
    }
    pub const fn satisfied_owner(&self) -> SymbolHandle {
        self.satisfied_owner
    }
    pub const fn requirement(&self) -> SymbolHandle {
        self.requirement
    }
    pub const fn via_expression(&self) -> typed_trees::expression::ExpressionHandle {
        self.via_expression
    }
    pub const fn producer_machine(&self) -> SymbolHandle {
        self.producer_machine
    }
    pub const fn producer_entry_state(&self) -> SymbolHandle {
        self.producer_entry_state
    }
    pub const fn via_source_span(&self) -> SourceSpan {
        self.via_source_span
    }
    pub const fn evaluated(&self) -> &EvaluatedViaBinding {
        &self.evaluated
    }
}

/// Complete evaluated ordinary-`via` population for one typed compilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedViaBindingTable {
    target: Option<TargetProfile>,
    rows: Vec<EvaluatedViaBindingRow>,
}

impl EvaluatedViaBindingTable {
    pub const fn target(&self) -> Option<TargetProfile> {
        self.target
    }
    pub fn rows(&self) -> &[EvaluatedViaBindingRow] {
        &self.rows
    }

    pub fn exact(
        &self,
        realization_machine: SymbolHandle,
        satisfied_owner: SymbolHandle,
        requirement: SymbolHandle,
    ) -> Option<&EvaluatedViaBindingRow> {
        self.rows.iter().find(|row| {
            row.realization_machine == realization_machine
                && row.satisfied_owner == satisfied_owner
                && row.requirement == requirement
        })
    }

    /// Replay every arena-local join before a later trust boundary consumes
    /// this table. Stable receipt data remains immutable; this catches a
    /// substituted conformance, expression, producer entry, or producer
    /// identity in a retained/mutated typed program.
    pub fn validate_against_typed(&self, typed: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
        let expected = typed
            .machines()
            .iter()
            .filter(|machine| crate::service_schema::is_product_declaration(typed, machine.symbol))
            .flat_map(|machine| {
                typed
                    .machine_trait_conformances(machine)
                    .iter()
                    .filter(|conformance| conformance.via_expression.is_valid())
                    .map(move |conformance| (machine, conformance))
            })
            .collect::<Vec<_>>();
        let mut diagnostics = Vec::new();
        if expected.len() != self.rows.len() {
            diagnostics.push(Diagnostic::error(format!(
                "evaluated `via` binding table retains {} rows for {} exact typed expressions",
                self.rows.len(),
                expected.len(),
            )));
        }
        for (machine, conformance) in expected {
            let source_span = typed
                .expression_table
                .source_span(conformance.via_expression);
            if conformance.external_binding.is_some() {
                diagnostics.push(at(
                    source_span,
                    "ordinary external `via` replay found a legacy binding on the same conformance",
                ));
                continue;
            }
            if machine.body_is_present
                || !matches!(
                    machine.supply_mode,
                    language_semantics::MachineSupplyMode::ExternalRealization {
                        binding: None,
                        mechanism: None,
                    }
                )
                || conformance.external_binding_source_span.is_none()
            {
                diagnostics.push(at(
                    source_span,
                    "ordinary external `via` replay found a mixed, body-bearing, or source-uncustodied supply carrier",
                ));
                continue;
            }
            let matches = self
                .rows
                .iter()
                .filter(|row| {
                    row.realization_machine == machine.symbol
                        && row.satisfied_owner == conformance.symbol
                        && row.requirement == conformance.requirement_symbol
                })
                .collect::<Vec<_>>();
            let [row] = matches.as_slice() else {
                diagnostics.push(at(
                    source_span,
                    format!(
                        "ordinary external `via` replay found {} evaluated rows for one exact conformance",
                        matches.len(),
                    ),
                ));
                continue;
            };
            let call = match typed
                .expression_table
                .expression(conformance.via_expression)
            {
                ExpressionNode::Call(call) => Some(call),
                _ => None,
            };
            let producers = call
                .into_iter()
                .flat_map(|call| {
                    typed.machines().iter().filter_map(move |producer| {
                        typed
                            .machine_states(producer)
                            .iter()
                            .find(|state| state.symbol == call.target_symbol)
                            .map(|state| (producer, state))
                    })
                })
                .collect::<Vec<_>>();
            let producer = match producers.as_slice() {
                [(producer, entry)]
                    if producer.symbol == row.producer_machine
                        && entry.symbol == row.producer_entry_state
                        && typed
                            .machine_states(producer)
                            .first()
                            .is_some_and(|first| first.symbol == entry.symbol) =>
                {
                    Some(*producer)
                }
                _ => None,
            };
            let producer_matches_receipt = producer.is_some_and(|producer| {
                typed
                    .normalized_machine_overload_identity(producer)
                    .is_some_and(|identity| {
                        identity.identity() == row.evaluated.receipt().producer_callable_identity()
                    })
                    && typed.symbols.symbol_package_identity(producer.symbol)
                        == row.evaluated.receipt().producer_package()
            });
            if row.via_expression != conformance.via_expression
                || row.via_source_span != source_span
                || !producer_matches_receipt
                || self.target != Some(row.evaluated.target())
                || row.evaluated.receipt().locator_identity_digest()
                    != row.evaluated.identity_digest()
            {
                diagnostics.push(at(
                    source_span,
                    "ordinary external `via` replay disagrees with its retained expression, producer, target, or receipt",
                ));
            }
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }
}

struct BindingVocabulary {
    binding: SymbolHandle,
    source_digest: [u8; 32],
}

/// Evaluate every ordinary `via` leaf exactly once and retain the resulting
/// atomic normalized import plus its durable evaluation receipt.
pub fn evaluate_via_bindings(
    typed: &TypedTrees,
    selected_target: Option<TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
) -> Result<EvaluatedViaBindingTable, Vec<Diagnostic>> {
    let pending = typed
        .machines()
        .iter()
        .filter(|machine| crate::service_schema::is_product_declaration(typed, machine.symbol))
        .flat_map(|machine| {
            typed
                .machine_trait_conformances(machine)
                .iter()
                .filter(|conformance| conformance.via_expression.is_valid())
                .map(move |conformance| (machine, conformance))
        })
        .collect::<Vec<_>>();
    if pending.is_empty() {
        return Ok(EvaluatedViaBindingTable {
            target: selected_target,
            rows: Vec::new(),
        });
    }
    let Some(target) = selected_target else {
        return Err(vec![Diagnostic::error(
            "ordinary external `via` evaluation requires one selected target profile",
        )]);
    };
    let vocabulary = exact_binding_vocabulary(typed)?;
    let selection_authority = package_inputs.cloned().map(|inputs| {
        Arc::new(inputs) as Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>
    });
    let admission = BuildTimeAdmissionPlan::infer(typed, selection_authority);
    let mut rows = Vec::with_capacity(pending.len());
    let mut diagnostics = Vec::new();

    for (machine, conformance) in pending {
        match evaluate_one(typed, &admission, &vocabulary, target, machine, conformance) {
            Ok(row) => rows.push(row),
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }
    rows.sort_by_key(|row| {
        (
            row.realization_machine.arena_index(),
            row.satisfied_owner.arena_index(),
            row.requirement.arena_index(),
            row.producer_entry_state.arena_index(),
        )
    });
    for pair in rows.windows(2) {
        if pair[0].realization_machine == pair[1].realization_machine
            && pair[0].satisfied_owner == pair[1].satisfied_owner
            && pair[0].requirement == pair[1].requirement
        {
            diagnostics.push(
                Diagnostic::error("ordinary external `via` has duplicate evaluated binding rows")
                    .with_source_span(pair[1].via_source_span),
            );
        }
    }
    if diagnostics.is_empty() {
        Ok(EvaluatedViaBindingTable {
            target: Some(target),
            rows,
        })
    } else {
        Err(diagnostics)
    }
}
