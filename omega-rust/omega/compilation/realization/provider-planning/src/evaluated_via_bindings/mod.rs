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

use crate::ProviderPlanningContext;
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
    via_expression: checked_trees::typed_trees::expression::ExpressionHandle,
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
    pub const fn via_expression(&self) -> checked_trees::typed_trees::expression::ExpressionHandle {
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
    scope: source::DependencyScope,
    target: Option<TargetProfile>,
    /// Identity table every receipt-carried `TerminalIdentityIndex` on the
    /// rows resolves against.
    identities: terminal_psi::TerminalIdentityTable,
    rows: Vec<EvaluatedViaBindingRow>,
}

impl EvaluatedViaBindingTable {
    pub const fn scope(&self) -> source::DependencyScope {
        self.scope
    }
    /// The table the binding receipts' `TerminalIdentityIndex` values index.
    pub const fn identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.identities
    }
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
}

struct BindingVocabulary {
    binding: SymbolHandle,
    source_digest: [u8; 32],
}

/// Evaluate every ordinary `via` leaf exactly once and retain the resulting
/// atomic normalized import plus its durable evaluation receipt.
/// Evaluation discloses all authored members under their own targets. The
/// selected provider derivation, not this population, narrows execution.
pub fn evaluate_via_bindings(
    context: ProviderPlanningContext<'_>,
    selected_target: Option<TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
) -> Result<EvaluatedViaBindingTable, Vec<Diagnostic>> {
    evaluate_program_via_bindings(
        build_time_evaluation::BuildTimeProgram::Typed(context.typed),
        context.scope,
        selected_target,
        package_inputs,
    )
}

pub fn evaluate_checked_via_bindings(
    checked: &checked_trees::CheckedTrees,
    scope: source::DependencyScope,
    selected_target: Option<TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
) -> Result<EvaluatedViaBindingTable, Vec<Diagnostic>> {
    evaluate_program_via_bindings(
        build_time_evaluation::BuildTimeProgram::Checked(checked),
        scope,
        selected_target,
        package_inputs,
    )
}

fn evaluate_program_via_bindings(
    program: build_time_evaluation::BuildTimeProgram<'_>,
    scope: source::DependencyScope,
    selected_target: Option<TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
) -> Result<EvaluatedViaBindingTable, Vec<Diagnostic>> {
    let typed = match program {
        build_time_evaluation::BuildTimeProgram::Typed(typed) => typed,
        build_time_evaluation::BuildTimeProgram::Checked(checked) => &checked.typed,
    };
    let context = ProviderPlanningContext::new(typed, scope);
    let pending = typed
        .machines()
        .iter()
        .filter(|machine| context.contains(machine.symbol))
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
            scope: context.scope,
            target: selected_target,
            identities: terminal_psi::TerminalIdentityTable::from(vec![String::new()]),
            rows: Vec::new(),
        });
    }
    let vocabulary = exact_binding_vocabulary(context)?;
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let selection_authority = package_inputs.cloned().map(|inputs| {
        Arc::new(inputs) as Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>
    });
    let admission = BuildTimeAdmissionPlan::infer(typed, selection_authority);
    let mut rows = Vec::with_capacity(pending.len());
    let mut diagnostics = Vec::new();

    for (machine, conformance) in pending {
        let target = match typed.authored_machine_target(machine.symbol) {
            Ok(Some(authored)) => TargetProfile::from_canonical_target_name(authored),
            Ok(None) => selected_target.ok_or_else(|| {
                Diagnostic::error(
                    "ordinary external `via` evaluation requires one selected target profile",
                )
            }),
            Err(reason) => Err(Diagnostic::error(reason)),
        };
        let target = match target {
            Ok(target) => target,
            Err(diagnostic) => {
                diagnostics.push(
                    diagnostic.with_source_span(
                        typed
                            .expression_table
                            .source_span(conformance.via_expression),
                    ),
                );
                continue;
            }
        };
        match evaluate_one(
            program,
            &admission,
            &vocabulary,
            target,
            machine,
            conformance,
            &mut identities,
        ) {
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
            scope: context.scope,
            target: selected_target,
            identities: identities.build(),
            rows,
        })
    } else {
        Err(diagnostics)
    }
}
