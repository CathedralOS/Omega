//! Settle source inputs before the lowered-form checker judges them. This
//! preparation retains declarations and Build selections, never a checked body
//! or a reusable verdict from the retired checker.

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
    pub completion: typed_trees::boundary_completions::SourceCompletionAdmission<'request>,
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

/// Finish source preparation without asking the retired checker for a verdict.
/// These operations settle explicit Build selections and produce the exact typed
/// frontier the lowered-form checker owns. They never create a checked body.
pub fn prepare_product_typed_frontier(
    mut program: typed_trees::TypedTrees,
    admission: &SourceAdmission<'_>,
) -> Result<typed_trees::FrozenTypedTrees, Vec<Diagnostic>> {
    let representations = admission.representations.admit(&program)?;
    typed_trees::boundary_completions::prepare_source_completion(
        &mut program,
        admission.completion,
    )?;
    build_time_evaluation::evaluate_fixed_layout_applications(
        &mut program,
        admission.layout_scopes,
        admission.layout_selection_authority.clone(),
    )?;
    constant_entries::retain_evaluation_entries(&mut program)?;
    let preparation = typed_preparation(admission, &representations);
    crate::prepare_build_provider_requirements(&mut program)?;
    let (program, _) = typed_trees_to_typed_trees::prepare_typed_trees(program, &preparation)?;
    build_time_evaluation::retain_access_lifecycle_plans(
        program,
        admission.declarations,
        admission.layout_scopes,
    )
}

fn typed_preparation<'a>(
    admission: &'a SourceAdmission<'_>,
    representations: &'a AdmittedRepresentations,
) -> typed_trees_to_typed_trees::PreparationRequest<'a> {
    let mut preparation = typed_trees_to_typed_trees::PreparationRequest::settled()
        .with_opaque_property_receipts(&representations.copy_properties)
        .with_opaque_representation_selections(&representations.selections)
        .with_wire_compatibility_demands(admission.wire_demands);
    if let Some(authority) = admission.asm_authority {
        preparation = preparation.with_asm_authority(authority);
    }
    preparation
}
