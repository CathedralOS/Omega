/*
We evaluate and retain ordinary source `via` bindings as the bridge between
typed source expressions and provider derivation. We retain arena coordinates
only for exact replay inside the same compilation; our installed import owns
stable locator and receipt identity without arena handles.

We evaluate every ordinary `via` leaf once and retain its atomic normalized
import alongside its durable evaluation receipt (`binding_evaluation.rs` and
`binding_digests.rs`). Evaluation discloses all authored members under their
own targets, leaving execution narrowing to selected provider derivation.
*/

mod binding_digests;
mod binding_evaluation;

use crate::ProviderPlanningContext;
use crate::evaluated_via_bindings::binding_evaluation::{evaluate_one, exact_binding_vocabulary};
use build_time_evaluation::BuildTimeAdmissionPlan;
use diagnostics::Diagnostic;
use effects::provider_plan::EvaluatedBindingUsage;
use package_compilation::PackageCompilationInputs;
use source::SourceSpan;
use std::sync::Arc;
use symbols::SymbolHandle;
use target::TargetProfile;

const MATERIALIZER_SCHEMA_VERSION: u32 = 1;

pub use effects::provider_plan::EvaluatedNativeBinding;
pub(crate) use language_semantics::native_bindings::decode_binding_value;
pub(crate) type DecodedBindingValue = semantic_vocabulary::NativeBindingLocator;

// The usage-schema version names the engine that measured these counts: schema
// 7 counts checked-tree cells and Text bytes, schema 8 counts Terminal logical
// storage cells and owned payload bytes. The retained row keeps that version
// beside the counts, so a consumer never reads one engine's cells as another's.
pub(crate) fn retained_usage(
    usage: evaluation_accounting::BuildEvaluationUsage,
) -> Result<EvaluatedBindingUsage, String> {
    usage
        .validate()
        .map_err(|reason| format!("evaluated binding usage is invalid: {reason:?}"))?;
    EvaluatedBindingUsage::from_evaluator(
        usage.usage_schema_version,
        usage.step_schedule_marker,
        usage.work_units,
        usage.invocation_work_budget,
        usage.build_log_bytes,
        usage.filesystem_operation_attempts,
        usage.peak_live_storage_cells,
        usage.peak_live_payload_bytes,
        usage.result_storage_cells,
        usage.result_payload_bytes,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedViaBindingRow {
    realization_machine: SymbolHandle,
    satisfied_owner: SymbolHandle,
    requirement: SymbolHandle,
    via_expression: checked_trees::typed_trees::expression::ExpressionHandle,
    producer_machine: SymbolHandle,
    producer_entry_state: SymbolHandle,
    via_source_span: SourceSpan,
    evaluated: EvaluatedNativeBinding,
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
    pub const fn evaluated(&self) -> &EvaluatedNativeBinding {
        &self.evaluated
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedViaBindingTable {
    scope: source::DependencyScope,
    target: Option<TargetProfile>,
    identities: terminal_psi::TerminalIdentityTable, // Resolves receipt-carried `TerminalIdentityIndex` values.
    rows: Vec<EvaluatedViaBindingRow>,
}

impl EvaluatedViaBindingTable {
    pub const fn scope(&self) -> source::DependencyScope {
        self.scope
    }
    pub const fn identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.identities
    }
    pub(crate) fn retain_extended_identities(
        &mut self,
        identities: terminal_psi::TerminalIdentityTable,
    ) -> Result<(), Vec<Diagnostic>> {
        if !identities.entries().starts_with(self.identities.entries()) {
            return Err(vec![Diagnostic::error(
                "native Build producer identity extension changed retained evaluation coordinates",
            )]);
        }
        self.identities = identities;
        Ok(())
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
