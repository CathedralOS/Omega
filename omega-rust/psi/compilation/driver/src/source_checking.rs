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
    pub pending: Vec<build_time_evaluation::PreCheckEvaluation>,
    pub selections: Vec<build_time_evaluation::SourceProviderSelection>,
}

/// Consume a complete typed program. Previous judgments belong to the same
/// invocation; the first is the incremental predecessor, followed by siblings.
pub struct CheckRequest<'request> {
    pub program: typed_trees::TypedTrees,
    pub constants: SourceConstantRequest,
    pub admission: SourceAdmission<'request>,
    pub previous: &'request [&'request RetainedCheck],
    pub retain: bool,
}

/// Successful language judgment, before optional whole-product root selection.
pub struct CheckedSource {
    pub program: Arc<checked_trees::CheckedTrees>,
    pub source_constants: Vec<build_time_evaluation::SourceConstantSupply>,
    pub retained: Option<RetainedCheck>,
    pub reused: bool,
}

/// Shared source product with the exact optional root-selection evidence.
#[derive(Clone)]
pub struct SelectedCheckedSource {
    pub program: Arc<checked_trees::CheckedTrees>,
    pub provider_defaults:
        Arc<[build_time_evaluation::provider_declarations::TargetProviderDefaults]>,
    pub source_constants: Vec<build_time_evaluation::SourceConstantSupply>,
    pub retained: Option<RetainedCheck>,
    pub selection: Option<CheckedTreeProductSelection>,
}

impl CheckRequest<'_> {
    pub fn check(self) -> Result<CheckedSource, Vec<Diagnostic>> {
        let mut program = self.program;
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
                build_time_evaluation::SourceConstantOutcome::Completed { supplies } => {
                    source_constants.extend(supplies);
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
        let reusable = source_constants.is_empty();
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
        Ok(CheckedSource {
            program: outcome.checked,
            source_constants,
            retained: outcome.retained,
            reused: outcome.took_prior,
        })
    }
}

impl CheckedSource {
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
            program,
            provider_defaults: provider_defaults.into(),
            source_constants: self.source_constants,
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
        if !checked.source_constants.is_empty() {
            return checked.select_product(roots);
        }
        if let Some(prior) = self
            .entries
            .iter()
            .find(|prior| Arc::ptr_eq(&prior.input, &checked.program) && prior.roots == roots)
        {
            let mut selected = prior.output.clone();
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
