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

mod constant_closure;
mod layout_closure;
pub use constant_closure::{
    ConstantProofApplication, ConstantProofClosure, target_application_declarations,
};
pub use layout_closure::check_closed_layout_applications;

pub use typed_trees_to_checked_trees::{CheckedTreeProductSelection, RetainedCheck};

/// Evaluated source permissions and representation facts for this invocation.
/// These are not provider selections or execution authority.
#[derive(Default)]
pub struct SourceAdmission<'request> {
    pub completion: validation::source_completion::SourceCompletionAdmission<'request>,
    pub catalogues: validation::source_catalogues::SourceCatalogueAdmission<'request>,
    pub opaque_properties: &'request [validation::OpaqueDataPropertyReceipt],
    pub opaque_representations: &'request [OpaqueRepresentationSelection],
    pub asm_authority: Option<validation::AsmAuthorityAdmission>,
    pub wire_demands: &'request [language_semantics::wire::WireCompatibilityDemand],
    pub declarations: Option<&'request dyn validation::DeclarationSelectionAuthority>,
}

/// Consume a complete typed program. Previous judgments belong to the same
/// invocation; the first is the incremental predecessor, followed by siblings.
pub struct CheckRequest<'request> {
    pub program: typed_trees::TypedTrees,
    pub pending_constants: Vec<build_time_evaluation::PreCheckEvaluation>,
    pub admission: SourceAdmission<'request>,
    pub previous: &'request [&'request RetainedCheck],
    pub retain: bool,
}

/// Successful language judgment, before optional whole-product root selection.
pub struct CheckedSource {
    pub program: Arc<checked_trees::CheckedTrees>,
    pub constants: Option<build_time_evaluation::RetainedConstantEvaluation>,
    pub retained: Option<RetainedCheck>,
    pub reused: bool,
}

/// Shared source product with the exact optional root-selection evidence.
#[derive(Clone)]
pub struct SelectedCheckedSource {
    pub program: Arc<checked_trees::CheckedTrees>,
    pub constants: Option<build_time_evaluation::RetainedConstantEvaluation>,
    pub provider_defaults:
        Arc<[build_time_evaluation::provider_declarations::TargetProviderDefaults]>,
    pub retained: Option<RetainedCheck>,
    pub selection: Option<CheckedTreeProductSelection>,
}

impl CheckRequest<'_> {
    pub fn check(self) -> Result<CheckedSource, Vec<Diagnostic>> {
        let _profile = phase_profile::guard("05");
        let mut program = self.program;
        let mut constants = build_time_evaluation::RetainedConstantEvaluation::capture(
            &program,
            self.pending_constants,
        );
        phase_profile::scope("05.a.source_completion", || {
            validation::source_completion::prepare_source_completion(
                &mut program,
                self.admission.completion,
            )
        })?;
        phase_profile::scope("05.a.source_catalogues", || {
            validation::source_catalogues::prepare_source_catalogues(
                &mut program,
                &self.admission.catalogues,
            )
        })?;
        let deferred_layout_applications = if constants.is_some() {
            typed_trees::references::collect_program_type_occurrences(&program)
                .into_iter()
                .filter_map(|occurrence| program.layout_storage_arguments(occurrence.reference))
                .filter(|storage| {
                    program
                        .closed_data_field_definition(storage.semantic_type)
                        .is_some()
                        && program
                            .plan_laid_layout_for_type(storage.application)
                            .is_none()
                })
                .map(|storage| storage.application)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let mut preparation = typed_trees_to_typed_trees::PreparationRequest::settled()
            .with_deferred_layout_applications(&deferred_layout_applications)
            .with_opaque_property_receipts(self.admission.opaque_properties)
            .with_opaque_representation_selections(self.admission.opaque_representations)
            .with_wire_compatibility_demands(self.admission.wire_demands);
        if let Some(authority) = self.admission.asm_authority {
            preparation = preparation.with_asm_authority(authority);
        }
        let checking = typed_trees_to_checked_trees::CheckingRequest::new()
            .with_declaration_authority(self.admission.declarations);
        let admission = preparation.admission();
        if !deferred_layout_applications.is_empty() {
            constants = constants
                .map(|continuation| continuation.with_layout_preparation(admission.clone()));
        }
        let matching = phase_profile::scope("05.b.retained_match", || {
            self.previous
                .iter()
                .copied()
                .find(|prior| prior.judges_preparation_input(&program, &admission))
        });
        let outcome = if let Some(prior) = matching {
            phase_profile::count("05.b.retained_match.hit", 1);
            let outcome =
                typed_trees_to_checked_trees::reuse_retained_check(prior, &checking, self.retain)?;
            build_time_evaluation::admit_placement_lifecycle_plans(
                &outcome.checked.typed,
                self.admission.declarations,
            )?;
            outcome
        } else {
            let retained_input = phase_profile::scope("05.b.retained_input_clone", || {
                self.retain.then(|| (Arc::new(program.clone()), admission))
            });
            let (program, preparation) = phase_profile::scope("05.c.prepare_typed_trees", || {
                typed_trees_to_typed_trees::prepare_typed_trees(program, &preparation)
            })?;
            // Whole-value lifecycle demands become concrete through the same
            // specialization as ordinary calls. Add geometry only: preparation's
            // executable bodies and mathematical judgments remain unchanged.
            let program = phase_profile::scope("05.c.placement_lifecycle_plans", || {
                build_time_evaluation::retain_placement_lifecycle_plans(
                    program,
                    self.admission.declarations,
                )
            })?;
            let prior_dependency_bodies: Vec<_> = self
                .previous
                .iter()
                .flat_map(|prior| prior.strict_dependency_bodies().iter().cloned())
                .collect();
            phase_profile::scope("05.d.lower_typed_trees", || {
                typed_trees_to_checked_trees::lower_typed_trees_reusing(
                    program,
                    preparation,
                    retained_input,
                    &checking,
                    &prior_dependency_bodies,
                    self.retain,
                )
            })?
        };
        Ok(CheckedSource {
            program: outcome.checked,
            constants,
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
            constants: self.constants,
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
        if let Some(prior) = self
            .entries
            .iter()
            .find(|prior| Arc::ptr_eq(&prior.input, &checked.program) && prior.roots == roots)
        {
            let mut selected = prior.output.clone();
            selected.retained = checked.retained;
            // Sharing a checked judgment does not reuse the prior invocation's
            // deferred work or its authored evaluation storage.
            selected.constants = checked.constants;
            return Ok(selected);
        }
        let input = checked.program.clone();
        let output = checked.select_product(roots.clone())?;
        let mut cached = output.clone();
        // Root selection retains its checked output, not another owner's
        // authored forest or execution continuation.
        cached.constants = None;
        self.entries.push(SelectedProduct {
            input,
            roots,
            output: cached,
        });
        Ok(output)
    }
}
