//! Prepare and check one complete typed program under fresh invocation authority.
//!
//! Product and Build consumers supply admission data, not a preparation/checking
//! sequence. Retained language judgments are matched here against their exact
//! preparation inputs; package authority is admitted even when work is reused.
//! Product root selection consumes the successful result, so it cannot hide an
//! invalid authored body. Host effects and provider dispatch are not inputs.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    ExecutedProductSelection, ProductDeclarationRequest,
};
use representation_selections::OpaqueRepresentationSelection;
use std::sync::Arc;

mod constant_entries;

pub use typed_trees_to_checked_trees::{CheckedTreeProductSelection, RetainedCheck};

static WHOLE_PROGRAM_REUSE_HITS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Exact typed-input and preparation-admission matches. A different program misses.
pub fn whole_program_reuse_hits() -> u64 {
    WHOLE_PROGRAM_REUSE_HITS.load(std::sync::atomic::Ordering::Relaxed)
}

fn record_whole_program_reuse() {
    WHOLE_PROGRAM_REUSE_HITS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Evaluated source permissions and representation custody for this invocation.
/// These are not provider selections or execution authority.
#[derive(Default)]
pub struct SourceAdmission<'request> {
    pub completion: validation::source_completion::SourceCompletionAdmission<'request>,
    pub representations: ProductRepresentationInput<'request>,
    pub layout_scopes: &'request [build_time_evaluation::FixedLayoutTargetScope],
    pub layout_selection_authority:
        Option<Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>>,
    pub asm_authority: Option<validation::AsmAuthorityAdmission>,
    pub wire_demands: &'request [language_semantics::wire::WireCompatibilityDemand],
    pub declarations: Option<&'request dyn validation::DeclarationSelectionAuthority>,
}

/// Retained execution custody, not permission to mint semantic-copy facts.
/// Authored and generated source may have completed since Build selected these
/// applications; checking must reclose them against the final typed graph.
#[derive(Default)]
pub struct ProductRepresentationInput<'request> {
    pub activation: Option<symbols::SymbolHandle>,
    pub declared: &'request [Arc<ProductDeclarationRequest>],
    pub executed: &'request [ExecutedProductSelection],
    pub retained: &'request [OpaqueRepresentationSelection],
}

struct AdmittedRepresentations {
    selections: Vec<OpaqueRepresentationSelection>,
    copy_properties: Vec<validation::OpaqueDataPropertyReceipt>,
}

impl ProductRepresentationInput<'_> {
    fn admit(
        &self,
        program: &typed_trees::TypedTrees,
    ) -> Result<AdmittedRepresentations, Vec<Diagnostic>> {
        let selections = representation_planning::rederive_opaque_representation_selections(
            program,
            self.activation,
            self.declared,
            self.executed,
            self.retained,
        )?;
        let copy_properties = selections
            .iter()
            .filter(|selection| {
                selection.copy_disposition()
                    == representation_selections::OpaqueRepresentationCopyDisposition::CheckedSemanticCopy
            })
            .map(|selection| validation::OpaqueDataPropertyReceipt::copy(selection.opaque()))
            .collect();
        Ok(AdmittedRepresentations {
            selections,
            copy_properties,
        })
    }
}

/// Consume a complete typed program. Previous judgments belong to the same
/// invocation; the first is the incremental predecessor, followed by siblings.
pub struct CheckRequest<'request> {
    pub program: typed_trees::TypedTrees,
    pub admission: SourceAdmission<'request>,
    pub previous: &'request [&'request RetainedCheck],
    pub retain: bool,
}

/// Successful language judgment, before optional whole-product root selection.
pub struct CheckedSource {
    pub program: Arc<checked_trees::CheckedTrees>,
    pub retained: Option<RetainedCheck>,
    pub reused: bool,
}

/// Shared source product with the exact optional root-selection evidence.
#[derive(Clone)]
pub struct SelectedCheckedSource {
    pub program: Arc<checked_trees::CheckedTrees>,
    pub provider_defaults:
        Arc<[build_time_evaluation::provider_declarations::TargetProviderDefaults]>,
    pub retained: Option<RetainedCheck>,
    pub selection: Option<CheckedTreeProductSelection>,
}

impl CheckRequest<'_> {
    pub fn check(self) -> Result<CheckedSource, Vec<Diagnostic>> {
        let _profile = phase_profile::guard("05");
        let mut program = self.program;
        // A retained language judgment cannot bypass final application custody.
        // Reclosure also derives copy properties; callers cannot supply them.
        let representations = self.admission.representations.admit(&program)?;
        phase_profile::scope("05.a.source_completion", || {
            validation::source_completion::prepare_source_completion(
                &mut program,
                self.admission.completion,
            )
        })?;
        build_time_evaluation::evaluate_fixed_layout_applications(
            &mut program,
            self.admission.layout_scopes,
            self.admission.layout_selection_authority.clone(),
        )?;
        constant_entries::retain_evaluation_entries(&mut program)?;
        let mut preparation = typed_trees_to_typed_trees::PreparationRequest::settled()
            .with_opaque_property_receipts(&representations.copy_properties)
            .with_opaque_representation_selections(&representations.selections)
            .with_wire_compatibility_demands(self.admission.wire_demands);
        if let Some(authority) = self.admission.asm_authority {
            preparation = preparation.with_asm_authority(authority);
        }
        let checking = typed_trees_to_checked_trees::CheckingRequest::new()
            .with_declaration_authority(self.admission.declarations);
        let admission = preparation.admission();
        let matching = phase_profile::scope("05.b.retained_match", || {
            self.previous
                .iter()
                .copied()
                .find(|prior| prior.judges_preparation_input(&program, &admission))
        });
        let outcome = if let Some(prior) = matching {
            phase_profile::count("05.b.retained_match.hit", 1);
            record_whole_program_reuse();
            let outcome =
                typed_trees_to_checked_trees::reuse_retained_check(prior, &checking, self.retain)?;
            build_time_evaluation::admit_access_lifecycle_plans(
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
            let program = phase_profile::scope("05.c.access_lifecycle_plans", || {
                build_time_evaluation::retain_access_lifecycle_plans(
                    program,
                    self.admission.declarations,
                )
            })?;
            let prior_dependency_bodies: Vec<_> = self
                .previous
                .iter()
                .flat_map(|prior| prior.strict_dependency_bodies().iter().cloned())
                .collect();
            let prior_frozen_bodies: Vec<_> = self
                .previous
                .iter()
                .flat_map(|prior| prior.strict_frozen_dependency_bodies().iter().cloned())
                .collect();
            phase_profile::scope("05.d.lower_typed_trees", || {
                typed_trees_to_checked_trees::lower_typed_trees_reusing(
                    program,
                    preparation,
                    retained_input,
                    &checking,
                    &prior_dependency_bodies,
                    &prior_frozen_bodies,
                    self.retain,
                )
            })?
        };
        Ok(CheckedSource {
            program: outcome.checked,
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
