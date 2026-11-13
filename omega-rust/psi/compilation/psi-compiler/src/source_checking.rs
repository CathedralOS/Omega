//! Prepare source for the lowered-form checker without producing an old judgment.
//!
//! Frozen typed source is an input to structural lowering, never an acceptance
//! certificate. Only the lowered-form checker can publish an executable product.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    ExecutedProductSelection, ProductDeclarationRequest,
};
use representation_selections::OpaqueRepresentationSelection;
use std::sync::Arc;

mod constant_entries {
    pub(super) fn retain_evaluation_entries(
        program: &mut typed_trees::TypedTrees,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        let mut entries = Vec::new();
        for declaration in program.const_declarations() {
            if declaration.evaluated_value.is_some() || !declaration.evaluation_entry.is_valid() {
                continue;
            }
            if !program
                .tables
                .machines
                .is_valid(declaration.evaluation_entry)
            {
                return Err(vec![diagnostics::Diagnostic::error(
                    "retained constant lost its complete evaluation entry",
                )]);
            }
            let entry = program.tables.machines.get(declaration.evaluation_entry);
            if program.machine_by_symbol(entry.symbol).is_none() {
                entries.push(entry.clone());
            }
        }
        for entry in entries {
            program.push_machine(entry);
        }
        Ok(())
    }
}
#[derive(Default)]
pub struct SourceAdmission<'request> {
    pub external_roots: &'request [symbols::SymbolHandle],
    pub completion: validation::source_completion::SourceCompletionAdmission<'request>,
    pub representations: ProductRepresentationInput<'request>,
    pub layout_scopes: &'request [build_time_evaluation::FixedLayoutTargetScope],
    pub layout_selection_authority:
        Option<Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>>,
    pub asm_authority: Option<validation::AsmAuthorityAdmission>,
    pub wire_demands: &'request [language_semantics::wire::WireCompatibilityDemand],
    pub declarations: Option<&'request dyn validation::DeclarationSelectionAuthority>,
}

#[derive(Default)]
pub struct ProductRepresentationInput<'request> {
    pub activation: Option<symbols::SymbolHandle>,
    pub declared: &'request [Arc<ProductDeclarationRequest>],
    pub executed: &'request [ExecutedProductSelection],
    pub retained: &'request [OpaqueRepresentationSelection],
}

/// Accepts the pre-Psi04 typed frontier. Preparing an already specialized
/// frontier again is not part of this boundary.
pub struct SourcePreparationRequest<'request> {
    pub program: typed_trees::TypedTrees,
    pub admission: SourceAdmission<'request>,
}

pub struct PreparedSource {
    pub program: Arc<typed_trees::FrozenTypedTrees>,
}

impl SourcePreparationRequest<'_> {
    pub fn prepare(self) -> Result<PreparedSource, Vec<Diagnostic>> {
        // Opaque property receipts came from the deleted checker. Supplying one
        // cannot stand in for evidence from the new route.
        if !self.admission.external_roots.is_empty() || self.admission.declarations.is_some() {
            return Err(vec![Diagnostic::error(
                "no product route yet: source selection authority admission",
            )]);
        }
        let representations = &self.admission.representations;
        if representations.activation.is_some()
            || !representations.declared.is_empty()
            || !representations.executed.is_empty()
            || !representations.retained.is_empty()
        {
            return Err(vec![Diagnostic::error(
                "no product route yet: opaque representation admission",
            )]);
        }
        let mut program = self.program;
        validation::source_completion::prepare_source_completion(
            &mut program,
            self.admission.completion,
        )?;
        build_time_evaluation::evaluate_fixed_layout_applications(
            &mut program,
            self.admission.layout_scopes,
            self.admission.layout_selection_authority,
        )?;
        constant_entries::retain_evaluation_entries(&mut program)?;
        let mut preparation = typed_trees_to_typed_trees::PreparationRequest::settled()
            .with_wire_compatibility_demands(self.admission.wire_demands);
        if let Some(authority) = self.admission.asm_authority {
            preparation = preparation.with_asm_authority(authority);
        }
        let (program, _) = typed_trees_to_typed_trees::prepare_typed_trees(program, &preparation)?;
        Ok(PreparedSource {
            program: Arc::new(program),
        })
    }
}
