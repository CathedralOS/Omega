//! Prepare and check one complete typed program under fresh invocation authority.
//!
//! Product and Build consumers supply admission data, not a preparation/checking
//! sequence. Retained language judgments are matched here against their exact
//! preparation inputs; package authority is admitted even when work is reused.
//! Product root selection consumes the successful result, so it cannot hide an
//! invalid authored body. Host effects and provider dispatch are not inputs.

use diagnostics::Diagnostic;
use representation_selections::OpaqueRepresentationSelection;
use std::sync::Arc;

pub use typed_trees_to_checked_trees::{CheckedTreeProductSelection, RetainedCheck};

/// Evaluated source permissions and representation facts for this invocation.
/// These are not provider selections or execution authority.
#[derive(Default)]
pub struct SourceAdmission<'request> {
    pub completion: validation::source_completion::SourceCompletionAdmission<'request>,
    /// Original Build-selected applications prepared by the same Psi judgment.
    pub static_application_demands:
        &'request [typed_trees::static_applications::StaticApplicationDemand],
    /// Relationship inputs retain each original required/supplied occurrence.
    pub assumed_binding_contract_demands:
        &'request [typed_trees::static_applications::AssumedBindingContractDemand],
    pub opaque_properties: &'request [validation::OpaqueDataPropertyReceipt],
    pub opaque_representations: &'request [OpaqueRepresentationSelection],
    pub asm_authority: Option<validation::AsmAuthorityAdmission>,
    pub wire_demands: &'request [language_semantics::wire::WireCompatibilityDemand],
    pub declarations: Option<&'request dyn validation::DeclarationSelectionAuthority>,
}

/// One source-owned continuation and the executed Build choices it may read.
/// This is constant input custody, separate from package completion admission
/// and target settlement. Build checking normally has no pending Product jobs.
#[derive(Default)]
pub struct SourceConstantRequest {
    /// Already successful early source observations from this completion owner.
    /// Executions and snapshot admissions remain distinct; never replay folds.
    pub checked_inputs: Vec<build_time_evaluation::CheckedConstantInput>,
    /// Observations from source preparation, before Build/Product selection.
    pub source_preparation_inputs: Vec<build_time_evaluation::CheckedConstantInput>,
    pub pending: Vec<build_time_evaluation::PreCheckEvaluation>,
    pub selections: Vec<build_time_evaluation::SourceProviderSelection>,
}

/// One ordinary source program or an authentic owned Build projection. Both
/// use the same checking operation; consumers cannot attach projection metadata
/// to a replacement mutable program through a public tuple constructor.
pub struct CheckingProgram(CheckingProgramKind);

enum CheckingProgramKind {
    Source(typed_trees::TypedTrees),
    Build(crate::source_compilation::BuildSourceProjection),
}

impl From<typed_trees::TypedTrees> for CheckingProgram {
    fn from(program: typed_trees::TypedTrees) -> Self {
        Self(CheckingProgramKind::Source(program))
    }
}

impl From<crate::source_compilation::BuildSourceProjection> for CheckingProgram {
    fn from(program: crate::source_compilation::BuildSourceProjection) -> Self {
        Self(CheckingProgramKind::Build(program))
    }
}

/// Consume a complete typed program. Previous judgments belong to the same
/// invocation; the first is the incremental predecessor, followed by siblings.
pub struct CheckRequest<'request> {
    pub program: CheckingProgram,
    pub constants: SourceConstantRequest,
    pub admission: SourceAdmission<'request>,
    pub previous: &'request [&'request RetainedCheck],
    pub retain: bool,
}

/// Coverage actually retained for completed source constants. Original binding
/// Associations and the separately observed checked execution inputs are not
/// a complete consumed-use or invocation-judgment roster. Earlier folds
/// outside this source-completion route remain uncovered; an empty roster cannot
/// claim those dependencies were absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceConstantInputCoverage {
    OriginalAssociations,
}

/// The original successful source judgment before Product pruning/erasure.
/// Only the actual check operation constructs this immutable input record.
/// It is compiler-local source custody, not a portable artifact, complete
/// semantic-dependency commitment, receiving authority or installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginalCheckedSourceInput {
    checked: Arc<checked_trees::CheckedTrees>,
    source_constants: Arc<[build_time_evaluation::SourceConstantSupply]>,
    executed_constant_inputs: Arc<[build_time_evaluation::ExecutedCheckedConstantInput]>,
    snapshot_admissions: Arc<[build_time_evaluation::CheckedConstantSnapshotAdmission]>,
    source_preparation_inputs: Arc<[build_time_evaluation::CheckedConstantInput]>,
    source_constant_coverage: SourceConstantInputCoverage,
}
impl OriginalCheckedSourceInput {
    pub fn checked(&self) -> &Arc<checked_trees::CheckedTrees> {
        &self.checked
    }
    pub fn source_constants(&self) -> &[build_time_evaluation::SourceConstantSupply] {
        &self.source_constants
    }
    /// Actual checked execution inputs from early source evaluation and pending
    /// continuation completion, independently of offered supplies. Build
    /// execution is retained through its actual projection; pre-resolution
    /// observations remain separate in source_preparation_inputs.
    pub fn executed_constant_inputs(
        &self,
    ) -> &[build_time_evaluation::ExecutedCheckedConstantInput] {
        &self.executed_constant_inputs
    }
    /// Ordinary checked concrete-premise judgments, not interpreter executions.
    pub fn snapshot_admissions(
        &self,
    ) -> &[build_time_evaluation::CheckedConstantSnapshotAdmission] {
        &self.snapshot_admissions
    }
    /// Earlier source-preparation observations retain their own program owners;
    /// they are not relabeled as this Build or Product execution.
    pub fn source_preparation_inputs(&self) -> &[build_time_evaluation::CheckedConstantInput] {
        &self.source_preparation_inputs
    }
    pub const fn source_constant_coverage(&self) -> SourceConstantInputCoverage {
        self.source_constant_coverage
    }
}

/// Successful language judgment, before optional whole-product root selection.
pub struct CheckedSource {
    original_input: Arc<OriginalCheckedSourceInput>,
    prepared_build: Option<CheckedBuildMachineProgram>,
    program: Arc<checked_trees::CheckedTrees>,
    pub retained: Option<RetainedCheck>,
    pub reused: bool,
}

/// The actual successful facade judgment of one privately owned Build input.
/// No public constructor joins an arbitrary checked body to a projection.
pub struct CheckedBuildMachineProgram {
    program: build_time_evaluation::PreparedBuildMachineProgram,
    projection: crate::source_compilation::BuildSourceProjection,
    original_input: Arc<OriginalCheckedSourceInput>,
}
impl CheckedBuildMachineProgram {
    /// Actual original Build judgment, retained separately from selected
    /// comparison execution facts and every later Product source owner.
    pub fn original_source_input(&self) -> &Arc<OriginalCheckedSourceInput> {
        &self.original_input
    }

    pub fn projection(&self) -> &validation::execution_projection::BuildProgramProjection {
        &self.projection.projection
    }
    pub fn source_input_identity(&self) -> &crate::source_compilation::BuildSourceInputIdentity {
        &self.projection.identity
    }
    pub fn program(&self) -> &build_time_evaluation::PreparedBuildMachineProgram {
        &self.program
    }
    pub fn with_selected_comparison_executions(
        self,
        executions: Vec<checked_trees::CheckedSelectedFloatComparisonExecution>,
    ) -> Self {
        Self {
            program: self.program.with_selected_comparison_executions(executions),
            projection: self.projection,
            original_input: self.original_input,
        }
    }
    pub fn activation(
        &self,
        entry: &build_time_evaluation::PreparedBuildMachineEntry,
        arguments: Vec<build_time_evaluation::BuildTimeValue>,
        mode: build_time_evaluation::BuildMachineExecutionMode,
    ) -> Result<
        build_time_evaluation::PreparedBuildMachineActivation,
        build_time_evaluation::BuildMachineEvaluationError,
    > {
        if entry.symbol() != self.projection.projection.entry() {
            return Err(build_time_evaluation::BuildMachineEvaluationError::Entry(
                "selected Build activation names a different projected entry".into(),
            ));
        }
        self.program.source_root_activation(entry, arguments, mode)
    }
}
impl std::ops::Deref for CheckedBuildMachineProgram {
    type Target = build_time_evaluation::PreparedBuildMachineProgram;
    fn deref(&self) -> &Self::Target {
        &self.program
    }
}

/// Shared source product with the exact optional root-selection evidence.
#[derive(Clone)]
pub struct SelectedCheckedSource {
    pub original_input: Arc<OriginalCheckedSourceInput>,
    pub program: Arc<checked_trees::CheckedTrees>,
    pub provider_defaults:
        Arc<[build_time_evaluation::provider_declarations::TargetProviderDefaults]>,
    pub retained: Option<RetainedCheck>,
    pub selection: Option<CheckedTreeProductSelection>,
}

impl CheckRequest<'_> {
    pub fn check(self) -> Result<CheckedSource, Vec<Diagnostic>> {
        let (mut program, build_projection) = match self.program.0 {
            CheckingProgramKind::Source(program) => (program, None),
            CheckingProgramKind::Build(projection) => {
                (projection.program().clone(), Some(projection))
            }
        };
        validation::source_completion::prepare_source_completion(
            &mut program,
            self.admission.completion,
        )?;
        let mut preparation = typed_trees_to_typed_trees::PreparationRequest::settled()
            .with_static_application_demands(self.admission.static_application_demands)
            .with_assumed_binding_contract_demands(self.admission.assumed_binding_contract_demands)
            .with_opaque_property_receipts(self.admission.opaque_properties)
            .with_opaque_representation_selections(self.admission.opaque_representations)
            .with_wire_compatibility_demands(self.admission.wire_demands);
        if let Some(authority) = self.admission.asm_authority {
            preparation = preparation.with_asm_authority(authority);
        }
        let checking = typed_trees_to_checked_trees::CheckingRequest::new()
            .with_declaration_authority(self.admission.declarations);
        let mut source_constants = Vec::new();
        let mut checked_constant_inputs = self.constants.checked_inputs;
        let mut source_preparation_inputs = self.constants.source_preparation_inputs;
        if let Some(projection) = &build_projection {
            checked_constant_inputs.extend(projection.constant_inputs.iter().cloned());
            source_preparation_inputs.extend(projection.source_preparation_inputs.iter().cloned());
        }
        // Each continuation checks its demanded source dependencies before
        // evaluation. The finished input then owns one ordinary Product check;
        // an earlier language judgment cannot stand in for its new constants.
        for pending in self.constants.pending {
            match pending.complete_source(
                &mut program,
                &self.constants.selections,
                build_time_evaluation::BuildTimeCheckingAdmission {
                    preparation,
                    checking,
                },
            )? {
                build_time_evaluation::SourceConstantOutcome::Completed {
                    supplies,
                    checked_inputs,
                } => {
                    source_constants.extend(supplies);
                    checked_constant_inputs.extend(checked_inputs);
                }
                build_time_evaluation::SourceConstantOutcome::Retained { reason, .. } => {
                    return Err(vec![Diagnostic::error(format!(
                        "unimplemented: {reason}; the original portable constant continuation remains unfinished",
                    ))]);
                }
            }
        }
        let admission = preparation.admission();
        // Completed source supplies do not yet retain a complete reusable
        // checked-closure/issuer identity. Equal folded values cannot donate
        // the prior judgment's source application or provider authority.
        let reusable = source_constants.is_empty()
            && checked_constant_inputs.is_empty()
            && source_preparation_inputs.is_empty();
        let previous = if reusable { self.previous } else { &[] };
        let retain = self.retain && reusable;
        let matching = previous
            .iter()
            .copied()
            .find(|prior| prior.judges_preparation_input(&program, &admission));
        let outcome = if let Some(prior) = matching {
            typed_trees_to_checked_trees::reuse_retained_check(prior, &checking, retain)?
        } else {
            let retained_input = retain.then(|| (Arc::new(program.clone()), admission));
            let (program, preparation) =
                typed_trees_to_typed_trees::prepare_typed_trees(program, &preparation)?;
            typed_trees_to_checked_trees::lower_typed_trees_reusing(
                program,
                preparation,
                retained_input,
                &checking,
                previous.first().copied(),
                retain,
            )?
        };
        let mut executed_constant_inputs = Vec::new();
        let mut snapshot_admissions = Vec::new();
        for input in checked_constant_inputs {
            match input {
                build_time_evaluation::CheckedConstantInput::Executed(input) => {
                    executed_constant_inputs.push(input)
                }
                build_time_evaluation::CheckedConstantInput::SnapshotAdmission(input) => {
                    snapshot_admissions.push(input)
                }
            }
        }
        let original_input = Arc::new(OriginalCheckedSourceInput {
            checked: Arc::clone(&outcome.checked),
            source_constants: source_constants.into(),
            executed_constant_inputs: executed_constant_inputs.into(),
            snapshot_admissions: snapshot_admissions.into(),
            source_preparation_inputs: source_preparation_inputs.into(),
            source_constant_coverage: SourceConstantInputCoverage::OriginalAssociations,
        });
        let prepared_build = build_projection
            .map(|projection| {
                projection
                    .projection
                    .validate_checked_entry(&outcome.checked.typed)?;
                Ok::<_, Vec<Diagnostic>>(CheckedBuildMachineProgram {
                    program:
                        build_time_evaluation::PreparedBuildMachineProgram::from_checked_shared(
                            Arc::clone(&outcome.checked),
                        ),
                    projection,
                    original_input: Arc::clone(&original_input),
                })
            })
            .transpose()?;
        trace_source_constant_inputs(&original_input);
        Ok(CheckedSource {
            original_input,
            prepared_build,
            program: outcome.checked,
            retained: outcome.retained,
            reused: outcome.took_prior,
        })
    }
}

impl CheckedSource {
    /// Read the actual successful output; a caller cannot replace it before
    /// Product selection while retaining this judgment's original input.
    pub fn program(&self) -> &Arc<checked_trees::CheckedTrees> {
        &self.program
    }

    pub fn into_prepared_build(
        self,
    ) -> Result<(CheckedBuildMachineProgram, Option<RetainedCheck>), Vec<Diagnostic>> {
        let prepared = self.prepared_build.ok_or_else(|| {
            vec![Diagnostic::error(
                "source judgment has no owned Build projection",
            )]
        })?;
        Ok((prepared, self.retained))
    }

    /// Select all authored Product roots after checking, or retain the complete
    /// checked program when no pruning was requested. An explicit empty root set
    /// remains an invalid request, not permission to skip checking or selection.
    pub fn select_product(
        self,
        roots: Option<Vec<symbols::SymbolHandle>>,
    ) -> Result<SelectedCheckedSource, Vec<Diagnostic>> {
        // Defaults describe every authored target, including declarations root
        // pruning may remove. Decode them from the checked source before pruning;
        // realization later chooses a row without reading declaration bodies.
        let provider_defaults =
            build_time_evaluation::provider_declarations::retain_provider_defaults(
                build_time_evaluation::provider_declarations::ProviderPlanningContext::new(
                    &self.program.typed,
                    source::DependencyScope::Product,
                ),
            )?;
        let (program, selection) =
            typed_trees_to_checked_trees::select_checked_tree_product(self.program, roots)?;
        Ok(SelectedCheckedSource {
            original_input: self.original_input,
            program,
            provider_defaults: provider_defaults.into(),
            retained: self.retained,
            selection,
        })
    }
}

/// Invocation-owned Product selections over already-shared checked programs.
/// Root pruning is a transform, not a new judgment. Equal roots of the same
/// immutable input retain its one output rather than cloning and pruning it
/// once per realization. Authority admission happens before this operation.
#[derive(Default)]
pub struct ProductSelections {
    entries: Vec<SelectedProduct>,
}

struct SelectedProduct {
    input: Arc<checked_trees::CheckedTrees>,
    roots: Option<Vec<symbols::SymbolHandle>>,
    output: SelectedCheckedSource,
}

impl ProductSelections {
    pub fn select(
        &mut self,
        checked: CheckedSource,
        roots: Option<Vec<symbols::SymbolHandle>>,
    ) -> Result<SelectedCheckedSource, Vec<Diagnostic>> {
        // Pruning may share a checked graph only when no incomplete source
        // supply association would be moved onto another producer's handles.
        if !checked.original_input.source_constants().is_empty()
            || !checked.original_input.executed_constant_inputs().is_empty()
            || !checked.original_input.snapshot_admissions().is_empty()
            || !checked
                .original_input
                .source_preparation_inputs()
                .is_empty()
        {
            return checked.select_product(roots);
        }
        if let Some(prior) = self
            .entries
            .iter()
            .find(|prior| Arc::ptr_eq(&prior.input, &checked.program) && prior.roots == roots)
        {
            let mut selected = prior.output.clone();
            selected.original_input = Arc::clone(&checked.original_input);
            selected.retained = checked.retained;
            return Ok(selected);
        }
        let input = checked.program.clone();
        let output = checked.select_product(roots.clone())?;
        self.entries.push(SelectedProduct {
            input,
            roots,
            output: output.clone(),
        });
        Ok(output)
    }
}

/// Temporary opt-in observation of the actual successful facade output. This
/// reads retained records only; names/source coordinates are diagnostic payload,
/// never identity, complete dependency coverage or new checking authority.
fn trace_source_constant_inputs(original: &OriginalCheckedSourceInput) {
    if std::env::var_os("OMEGA_SOURCE_CONSTANT_INPUT_TRACE").is_none() {
        return;
    }
    eprintln!(
        "source-constant-inputs coverage={:?} supplies={} executed={}",
        original.source_constant_coverage(),
        original.source_constants().len(),
        original.executed_constant_inputs().len(),
    );
    for (ordinal, input) in original.executed_constant_inputs().iter().enumerate() {
        let checked = input.checked();
        let symbols = &checked.symbols;
        let entry = input.entry();
        let result = match input.result() {
            build_time_evaluation::BuildTimeValue::Unit => "unit".to_owned(),
            build_time_evaluation::BuildTimeValue::Int(value) => format!("integer:{value}"),
            build_time_evaluation::BuildTimeValue::Bool(value) => format!("boolean:{value}"),
            build_time_evaluation::BuildTimeValue::Float(value) => {
                format!("float-bits:{:016x}", value.to_bits())
            }
            build_time_evaluation::BuildTimeValue::UndeterminedNaN(format) => {
                format!("undetermined-nan:{format:?}")
            }
            build_time_evaluation::BuildTimeValue::Text(bytes) => {
                format!("text-bytes:{}", bytes.len())
            }
            build_time_evaluation::BuildTimeValue::Struct { fields, .. } => {
                format!("struct-fields:{}", fields.len())
            }
            build_time_evaluation::BuildTimeValue::Case { payload, .. } => {
                format!("case-fields:{}", payload.len())
            }
            build_time_evaluation::BuildTimeValue::Array(values) => {
                format!("array-elements:{}", values.len())
            }
        };
        eprintln!(
            "source-constant-input ordinal={ordinal} entry={} origin={:?} package={:?} source={:?} arguments={} result={result} checked-machines={} product-machines={}",
            symbols.display_path(entry, "::"),
            symbols.symbol_source_origin(entry),
            symbols.symbol_package_identity(entry),
            symbols.symbol_provenance_source_span(entry),
            input.arguments().len(),
            checked.typed.machines().len(),
            input.product_declarations().machines().len(),
        );
        // Bound the display, not the retained closure. The count above remains
        // the actual full roster; no omitted debug row becomes absent evidence.
        for machine in checked.typed.machines().iter().take(32) {
            eprintln!(
                "source-constant-closure ordinal={ordinal} machine={} origin={:?} source={:?}",
                symbols.display_path(machine.symbol, "::"),
                symbols.symbol_source_origin(machine.symbol),
                symbols.symbol_provenance_source_span(machine.symbol),
            );
        }
        if checked.typed.machines().len() > 32 {
            eprintln!(
                "source-constant-closure ordinal={ordinal} omitted={}",
                checked.typed.machines().len() - 32
            );
        }
    }
}
