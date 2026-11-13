/*
We retain evaluated ordinary source `via` bindings as the bridge between typed
source expressions and provider derivation. We retain arena coordinates only
for exact replay inside the same compilation; our installed import owns stable
locator and receipt identity without arena handles.

Running a `via` producer is build-time evaluation and belongs to Build
evaluation (`build_settings::via_binding_evaluation`). That driver asks
`ViaProducer::resolve` which machine a leaf names, runs it once, and hands the
result back as a `ViaBindingEvaluation`: the value, its measured usage, the
admitted producer closure, the Binding coordinate widths and the evaluator's
semantics marker. `EvaluatedViaBindingRows` decodes each value into its atomic
normalized import alongside its durable evaluation receipt
(`binding_retention.rs` and `binding_digests.rs`) and closes the table. All
authored members are disclosed under their own targets, leaving execution
narrowing to selected provider derivation.
*/

mod binding_digests;
mod binding_retention;

use diagnostics::Diagnostic;
use effects::provider_plan::EvaluatedBindingUsage;
use source::SourceSpan;
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
    via_expression: typed_trees::expression::ExpressionHandle,
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
    pub const fn target(&self) -> Option<TargetProfile> {
        self.target
    }
    pub fn rows(&self) -> &[EvaluatedViaBindingRow] {
        &self.rows
    }

    /// The table of a program that authors no ordinary `via` leaf.
    pub fn without_bindings(scope: source::DependencyScope, target: Option<TargetProfile>) -> Self {
        Self {
            scope,
            target,
            identities: terminal_psi::TerminalIdentityTable::from(vec![String::new()]),
            rows: Vec::new(),
        }
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

/// The native `Binding` vocabulary every `via` producer returns, with the
/// digest of the toolchain source that declares it.
pub struct BindingVocabulary {
    binding: SymbolHandle,
    source_digest: [u8; 32],
}

/// The one machine a `via` leaf names, resolved to the body that runs for the
/// selected target, with the conformance row the result will satisfy.
#[derive(Clone, Copy)]
pub struct ViaProducer<'program> {
    producer: &'program typed_trees::machine::Machine,
    entry: &'program typed_trees::state::State,
    satisfied_owner: SymbolHandle,
    requirement: SymbolHandle,
    via_expression: typed_trees::expression::ExpressionHandle,
    via_source_span: SourceSpan,
}

impl ViaProducer<'_> {
    /// The machine the caller evaluates.
    pub const fn machine(&self) -> SymbolHandle {
        self.producer.symbol
    }
    pub const fn return_type(&self) -> typed_trees::types::TypeReferenceHandle {
        self.entry.return_type
    }
    pub const fn via_source_span(&self) -> SourceSpan {
        self.via_source_span
    }
}

/// What running one `via` producer yielded. Provider planning never runs the
/// producer, so everything the receipt commits to arrives here as data.
pub struct ViaBindingEvaluation<'evaluation> {
    pub value: &'evaluation build_time_values::BuildTimeValue,
    pub usage: evaluation_accounting::BuildEvaluationUsage,
    pub closure: &'evaluation [SymbolHandle], // Machines admitted into the producer's evaluation.
    pub widths: [u64; 3],                     // Binding coordinate widths of the producer's result.
    pub evaluation_semantics_marker: u32,     // Identity of the evaluator semantics that ran it.
}

/// The rows of one program's `via` evaluations while its driver is still
/// running producers. Receipts intern their identities into one shared table.
pub struct EvaluatedViaBindingRows {
    identities: terminal_psi::TerminalIdentityTableBuilder,
    rows: Vec<EvaluatedViaBindingRow>,
}

impl EvaluatedViaBindingRows {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            identities: terminal_psi::TerminalIdentityTableBuilder::default(),
            rows: Vec::with_capacity(capacity),
        }
    }

    pub fn retain(
        &mut self,
        typed: &typed_trees::TypedTrees,
        vocabulary: &BindingVocabulary,
        target: TargetProfile,
        realization_machine: SymbolHandle,
        producer: ViaProducer<'_>,
        evaluation: ViaBindingEvaluation<'_>,
    ) -> Result<(), Diagnostic> {
        let row = binding_retention::retain_one(
            typed,
            vocabulary,
            target,
            realization_machine,
            producer,
            evaluation,
            &mut self.identities,
        )?;
        self.rows.push(row);
        Ok(())
    }

    /// Closes the table. `diagnostics` are the driver's own refusals; a
    /// duplicate row adds to them, and any diagnostic refuses the whole table.
    pub fn finish(
        self,
        scope: source::DependencyScope,
        target: Option<TargetProfile>,
        mut diagnostics: Vec<Diagnostic>,
    ) -> Result<EvaluatedViaBindingTable, Vec<Diagnostic>> {
        let Self {
            identities,
            mut rows,
        } = self;
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
                    Diagnostic::error(
                        "ordinary external `via` has duplicate evaluated binding rows",
                    )
                    .with_source_span(pair[1].via_source_span),
                );
            }
        }
        if diagnostics.is_empty() {
            Ok(EvaluatedViaBindingTable {
                scope,
                target,
                identities: identities.build(),
                rows,
            })
        } else {
            Err(diagnostics)
        }
    }
}
