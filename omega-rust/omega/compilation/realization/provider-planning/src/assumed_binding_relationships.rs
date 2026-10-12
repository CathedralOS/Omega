//! Post-Build relationship preparation against the checked Product owner.
//!
//! Executed registration is intent. This owner checks independent runtime and
//! operational compatibility and joins the selected supplier without making a
//! conformance for the required declaration. The result is an obligation, not
//! installation or trust: public semantic differences, their authority routes,
//! dependency closure and exact receiving consent still belong to admission.
//! Start at `prepare_assumed_binding_relationships`; declaration projection,
//! positional correspondence and native compatibility have separate owners.
//! `retain_assumed_binding_physical_dependencies` rejoins reached physical rows
//! after ordinary intrinsic execution identity production; its original typed
//! bindings remain evidence inputs, never native verification or consent.
//! `retain_assumed_binding_declaration_revisions` joins the original public
//! coordinates to seal-owned source units; unavailable root/source ownership
//! stays separate from complete dependency identity and receiving acceptance.
//! Accepted proof uses retain canonical content from their original mathematical
//! certificates. That component never stands for source correspondence, the
//! complete dependency closure or receiving consent.
//! Reached mathematical applications retain their original unreduced term,
//! source owner and captured domains beside the original authored signature;
//! formation provenance never substitutes for an accepted proposition judgment.
//! Calling compatibility retains both original evaluated application commitments.
//! Domain establishment routes are queried through the retained declaration owner;
//! neither those authored routes nor checked issuer authorization is a receipt.
//! Root scope rejoins the actually evaluated Build declaration to original
//! sealed source custody. It retains source/target provenance separately from
//! the selected executable entry, complete relationship identity and consent.
//! Structural content retains the already checked positional correspondence,
//! custody decisions and original public/calling applications for later consent.

mod application_content;
pub use application_content::AssumedBindingApplicationContent;
mod calling_compatibility;
mod compatibility;
mod declaration_revisions;
mod declarations;
pub use calling_compatibility::AssumedBindingCallingCompatibility;
pub use declaration_revisions::{
    AssumedBindingDeclarationKind, AssumedBindingDeclarationRevision,
    AssumedBindingDeclarationRevisions, retain_assumed_binding_declaration_revisions,
};
mod dependencies;
mod domain_authority;
mod mathematical_dependencies;
mod physical_dependencies;
mod proof_content;
mod root_scope;
pub use proof_content::AssumedBindingMathematicalJudgmentContent;
pub use root_scope::{
    AssumedBindingRootOwner, AssumedBindingRootScope, retain_assumed_binding_root_scopes,
};
mod semantic_content;
mod semantic_obligations;
mod structural_content;
pub use mathematical_dependencies::{
    AssumedBindingMathematicalAssumption, AssumedBindingMathematicalCohortContent,
    AssumedBindingMathematicalDependencies, AssumedBindingMathematicalRoot,
};
pub use physical_dependencies::{
    AssumedBindingPhysicalProviderDependency, AssumedBindingPhysicalProviderEvidence,
    retain_assumed_binding_physical_dependencies,
};
pub use semantic_content::AssumedBindingSemanticContent;
pub use structural_content::AssumedBindingStructuralContent;

pub use dependencies::{
    AssumedBindingMathematicalApplicationUse, AssumedBindingProofJudgmentUse,
    AssumedBindingSupplierDependencies, AssumedBindingSupplierSelection,
};
pub use domain_authority::{
    AssumedBindingDomainDemand, AssumedBindingDomainSource, AssumedBindingDomainSubject,
};
pub use semantic_obligations::{
    AssumedBindingContractSide, AssumedBindingDirectionalObligations, AssumedBindingPublicClaim,
    AssumedBindingResultCase, AssumedBindingSemanticConnection, AssumedBindingSemanticDirection,
    AssumedBindingSemanticJudgment, AssumedBindingSemanticObligation, AssumedBindingSemanticSource,
    AssumedBindingSemanticSubject,
};

use crate::assumed_binding_registrations::{
    AssumedBindingRegistration, ProductBoundaryRequirementReference,
};
use crate::calling_policy_plans::{BoundaryCallingPlanRealization, MaterializedBoundarySignature};
use crate::{ProviderPlanningContext, SelectedProviderReviewProvenance};
use calling_conventions::ValidatedBoundaryEntryPlan;
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderPlanRow;
use representation_planning::OpaqueRepresentationSelection;
use std::sync::Arc;
use symbols::SymbolHandle;
use target::NativeTarget;

/// Exactly one positional runtime subject; no parameter name selects a join.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssumedBindingSubjectCorrespondence {
    Parameter {
        ordinal: u32,
        required: SymbolHandle,
        supplied: SymbolHandle,
    },
    Result,
}

/// Independent compatibility produced from two authentic public declarations.
/// Fields are private so registration or consent cannot manufacture this row.
/// Its declaration handles belong to the final checked Product owner retained
/// beside it. This is deliberately not an accepted relationship receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedAssumedBindingRelationship {
    declaration_owner: Arc<CheckedTrees>,
    registration: AssumedBindingRegistration,
    subjects: Vec<AssumedBindingSubjectCorrespondence>,
    application_content: Option<AssumedBindingApplicationContent>,
    structural_content: Option<AssumedBindingStructuralContent>,
    required_declaration_coordinate: declaration_revisions::DeclarationCoordinate,
    supplied_declaration_coordinate: declaration_revisions::DeclarationCoordinate,
    declaration_revisions: Option<AssumedBindingDeclarationRevisions>,
    root_scope: Option<AssumedBindingRootScope>,
    required_public_contract: [u8; 32],
    supplied_public_contract: [u8; 32],
    required_signature: MaterializedBoundarySignature,
    supplied_signature: MaterializedBoundarySignature,
    calling_plan: ValidatedBoundaryEntryPlan,
    calling_compatibility: AssumedBindingCallingCompatibility,
    supplier_owner: Arc<[SelectedProviderReviewProvenance]>,
    supplier_ordinal: usize,
    supplier_row: usize,
    supplier_application:
        Option<crate::selected_applications::SelectedCheckedRequirementApplication>,
    /// Every intermediate intent remains necessary when the supplied member
    /// itself has an assumed link; the endpoint never erases these premises.
    supplier_links: Vec<ProductBoundaryRequirementReference>,
    semantic_connection: AssumedBindingSemanticConnection,
    semantic_content: Option<AssumedBindingSemanticContent>,
    supplier_dependencies: AssumedBindingSupplierDependencies,
    domain_demands: Vec<AssumedBindingDomainDemand>,
    physical_dependencies: Option<arena::Arena<AssumedBindingPhysicalProviderDependency>>,
}

impl PreparedAssumedBindingRelationship {
    /// The authentic checked source owner remains available even when later
    /// product selection removes an unused declaration or implementation body.
    pub fn declaration_owner(&self) -> &CheckedTrees {
        &self.declaration_owner
    }

    pub fn required(&self) -> &ProductBoundaryRequirementReference {
        &self.registration.required
    }

    pub fn supplied(&self) -> &ProductBoundaryRequirementReference {
        &self.registration.supplied
    }

    pub fn registration(&self) -> &AssumedBindingRegistration {
        &self.registration
    }

    pub fn subjects(&self) -> &[AssumedBindingSubjectCorrespondence] {
        &self.subjects
    }

    /// Complete original description content, including declaration-owned
    /// invocation families. Missing portable source/region custody is unavailable;
    /// absence never changes source admission or manufactures a runtime tuple.
    pub const fn application_content(&self) -> Option<AssumedBindingApplicationContent> {
        self.application_content
    }

    /// Produced positional and structural compatibility content. Absence is
    /// missing portable application evidence, never assumed compatibility.
    pub const fn structural_content(&self) -> Option<AssumedBindingStructuralContent> {
        self.structural_content
    }

    /// None means missing seal-owned revision evidence, including unmanaged
    /// roots; it is never an empty or accepted declaration identity.
    pub const fn declaration_revisions(&self) -> Option<AssumedBindingDeclarationRevisions> {
        self.declaration_revisions
    }

    /// Source and target provenance of the actual consuming Build root. This
    /// supplies no executable-entry occurrence, receiving consent or dispatch.
    pub const fn root_scope(&self) -> Option<AssumedBindingRootScope> {
        self.root_scope
    }

    pub const fn required_public_contract(&self) -> [u8; 32] {
        self.required_public_contract
    }

    pub const fn supplied_public_contract(&self) -> [u8; 32] {
        self.supplied_public_contract
    }

    pub fn required_signature(&self) -> &MaterializedBoundarySignature {
        &self.required_signature
    }

    pub fn supplied_signature(&self) -> &MaterializedBoundarySignature {
        &self.supplied_signature
    }

    pub fn calling_plan(&self) -> &ValidatedBoundaryEntryPlan {
        &self.calling_plan
    }

    /// The actual evaluated applications on both sides survive preparation.
    /// This is compatibility evidence, not an assumed relationship receipt.
    pub const fn calling_compatibility(&self) -> AssumedBindingCallingCompatibility {
        self.calling_compatibility
    }

    pub fn selected_supplier(&self) -> &SelectedProviderReviewProvenance {
        &self.supplier_owner[self.supplier_ordinal]
    }

    pub fn selected_supplier_row(&self) -> &ProviderPlanRow {
        &self.selected_supplier().plan.rows[self.supplier_row]
    }

    pub fn supplier_application(
        &self,
    ) -> Option<&crate::selected_applications::SelectedCheckedRequirementApplication> {
        self.supplier_application.as_ref()
    }

    pub fn supplier_dependencies(&self) -> &AssumedBindingSupplierDependencies {
        &self.supplier_dependencies
    }

    /// `None` is an unfinished post-selection producer, never empty closure.
    /// Physical binding evidence grants no provider rights or receiving consent.
    pub fn physical_dependencies(
        &self,
    ) -> Option<&arena::Arena<AssumedBindingPhysicalProviderDependency>> {
        self.physical_dependencies.as_ref()
    }

    pub fn domain_demands(&self) -> &[AssumedBindingDomainDemand] {
        &self.domain_demands
    }

    /// Original authored establishment routes for one exact demand. Empty is
    /// an authenticated declaration with no routes; None is missing original
    /// source association. Routes describe possible issuance, not a receipt.
    pub fn domain_establishment_routes(
        &self,
        demand_ordinal: usize,
    ) -> Option<&[language_semantics::DomainEstablishmentRoute]> {
        domain_authority::establishment_routes(
            self.declaration_owner(),
            self.domain_demands.get(demand_ordinal)?,
        )
    }

    /// Original selected owner for row coordinates in the dependency inventory.
    pub fn dependency_supplier(
        &self,
        selection: AssumedBindingSupplierSelection,
    ) -> Option<&SelectedProviderReviewProvenance> {
        self.supplier_owner.get(selection.provider_ordinal)
    }

    pub fn supplier_links(&self) -> &[ProductBoundaryRequirementReference] {
        &self.supplier_links
    }

    /// Canonical public differences and judgments. None means a missing
    /// original clause/qualification association, not an empty difference.
    /// This component does not stand for complete supplier dependencies.
    pub const fn semantic_content(&self) -> Option<AssumedBindingSemanticContent> {
        self.semantic_content
    }

    pub fn semantic_connection(&self) -> &AssumedBindingSemanticConnection {
        &self.semantic_connection
    }
}

/// Prepare the closed executed roster once against the same checked Product
/// used by provider settlement. This does not verify a downstream stage's
/// output, assert the semantic connection or install runtime dispatch.
pub fn prepare_assumed_binding_relationships(
    program: &Arc<CheckedTrees>,
    registrations: &[AssumedBindingRegistration],
    selected: &[SelectedProviderReviewProvenance],
    calling_plans: &[BoundaryCallingPlanRealization],
    native_target: NativeTarget,
    opaque_representations: &[OpaqueRepresentationSelection],
) -> Result<Vec<PreparedAssumedBindingRelationship>, Vec<Diagnostic>> {
    if registrations.is_empty() {
        return Ok(Vec::new());
    }
    let context = ProviderPlanningContext::new(&program.typed, source::DependencyScope::Product);
    validate_roster(registrations, selected).map_err(|diagnostic| vec![diagnostic])?;
    let supplier_owner: Arc<[SelectedProviderReviewProvenance]> = Arc::from(selected.to_vec());
    let mut prepared = Vec::with_capacity(registrations.len());
    for registration in registrations {
        let prepare = || {
            let required = declarations::requirement(context, &registration.required)?;
            let supplied = declarations::requirement(context, &registration.supplied)?;
            let subjects = compatibility::subject_correspondence(program, &required, &supplied)?;
            compatibility::public_operational_compatibility(program, &required, &supplied)?;
            compatibility::crash_compatibility(program, &required, &supplied)?;
            let (required_signature, supplied_signature, calling_plan, calling_compatibility) =
                compatibility::native_compatibility(
                    context,
                    &required,
                    &supplied,
                    calling_plans,
                    native_target,
                    opaque_representations,
                )?;
            let supplier_links = supplier_links(registrations, &registration.supplied)?;
            let endpoint = supplier_links
                .last()
                .ok_or_else(|| Diagnostic::error("assumed binding lost its supplied endpoint"))?;
            let matches = selected
                .iter()
                .enumerate()
                .flat_map(|(supplier_ordinal, supplier)| {
                    supplier
                        .provider
                        .row_requirements
                        .iter()
                        .enumerate()
                        .filter_map(move |(ordinal, requirement)| {
                            (*requirement == endpoint.symbol).then_some((supplier_ordinal, ordinal))
                        })
                })
                .collect::<Vec<_>>();
            let [(supplier_ordinal, supplier_row)] = matches.as_slice() else {
                return Err(Diagnostic::error(format!(
                    "assumed binding supplied endpoint has {} selected realization rows; exactly one is required",
                    matches.len(),
                )));
            };
            let supplier = &selected[*supplier_ordinal];
            let row = supplier.plan.rows.get(*supplier_row).ok_or_else(|| {
                Diagnostic::error("assumed binding selected supplier lost its row-aligned plan")
            })?;
            let method = supplier
                .plan
                .schema
                .method_for_row(row, &supplier.plan.identities)
                .ok_or_else(|| {
                    Diagnostic::error(
                        "assumed binding selected supplier has no exact declared schema row",
                    )
                })?;
            let endpoint_declaration = declarations::requirement(context, endpoint)?;
            // Calling realization stays in the separately retained validated
            // application; public schema content has no target application.
            let mut public_method = method.clone();
            public_method.calling_plan_commitment = None;
            public_method.calling_plan_report_fingerprint = None;
            let expected_schema = if endpoint_declaration.symbol() == endpoint.symbol {
                endpoint_declaration.schema.clone()
            } else {
                declarations::authored_schema(context, endpoint.symbol)?
            };
            let [expected_method] = expected_schema.methods.as_slice() else {
                return Err(Diagnostic::error(
                    "assumed binding supplied endpoint lost its exact public schema row",
                ));
            };
            if !public_method.content_eq(
                &supplier.plan.schema.identities,
                expected_method,
                &expected_schema.identities,
            ) {
                return Err(Diagnostic::error(
                    "assumed binding selected supplier does not retain its exact public requirement contract",
                ));
            }
            // Concrete source bodies retain their declared application while
            // physical leaves keep the original selected payload independently.
            let supplier_application = if matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
            ) {
                Some(
                    crate::selected_applications::selected_checked_requirement_application(
                        program,
                        supplier,
                        *supplier_row,
                        endpoint_declaration.symbol(),
                    )?,
                )
            } else {
                if endpoint_declaration.symbol() != endpoint.symbol {
                    return Err(Diagnostic::error(
                        "selected physical supplier needs its exact applied requirement payload",
                    ));
                }
                None
            };
            let supplier_dependencies = dependencies::capture(
                program,
                registrations,
                selected,
                AssumedBindingSupplierSelection {
                    provider_ordinal: *supplier_ordinal,
                    row_ordinal: *supplier_row,
                },
                supplier_application.as_ref(),
            )?;
            let domain_demands = domain_authority::capture(program, &required, &supplied);
            let required_public_contract = required.public_contract(program)?;
            let supplied_public_contract = supplied.public_contract(program)?;
            let application_content =
                application_content::capture(&registration.required, &registration.supplied);
            let structural_content = structural_content::capture(
                program,
                &required,
                &supplied,
                &subjects,
                application_content,
                (required_public_contract, supplied_public_contract),
                calling_compatibility,
            );
            let demand =
                checked_trees::typed_trees::static_applications::AssumedBindingContractDemand {
                    required: registration.required.application_demand(),
                    supplied: registration.supplied.application_demand(),
                    selecting_machine: registration.selecting_machine,
                    source_span: registration.source_span,
                };
            let semantic_connection = program
                .facts
                .assumed_binding_contracts
                .connection(&demand, required.symbol(), supplied.symbol())
                .ok_or_else(|| {
                    Diagnostic::error(
                        "assumed binding relationship lost its exact checked source judgment",
                    )
                })?
                .clone();
            let semantic_content = semantic_content::capture(
                program,
                &required,
                &supplied,
                required_public_contract,
                supplied_public_contract,
                &semantic_connection,
            );
            Ok(PreparedAssumedBindingRelationship {
                declaration_owner: Arc::clone(program),
                registration: registration.clone(),
                subjects,
                application_content,
                structural_content,
                required_declaration_coordinate:
                    declaration_revisions::DeclarationCoordinate::capture(&required),
                supplied_declaration_coordinate:
                    declaration_revisions::DeclarationCoordinate::capture(&supplied),
                declaration_revisions: None,
                root_scope: None,
                required_public_contract,
                supplied_public_contract,
                required_signature,
                supplied_signature,
                calling_plan,
                calling_compatibility,
                supplier_owner: Arc::clone(&supplier_owner),
                supplier_ordinal: *supplier_ordinal,
                supplier_row: *supplier_row,
                supplier_application,
                supplier_links,
                semantic_connection,
                semantic_content,
                supplier_dependencies,
                domain_demands,
                physical_dependencies: None,
            })
        };
        prepared.push(
            prepare().map_err(|diagnostic| {
                vec![diagnostic.with_source_span(registration.source_span)]
            })?,
        );
    }
    Ok(prepared)
}

fn validate_roster(
    registrations: &[AssumedBindingRegistration],
    selected: &[SelectedProviderReviewProvenance],
) -> Result<(), Diagnostic> {
    for (ordinal, registration) in registrations.iter().enumerate() {
        if registrations[..ordinal]
            .iter()
            .any(|earlier| earlier.required == registration.required)
        {
            return Err(Diagnostic::error(
                "assumed binding requirement has more than one executed supply",
            )
            .with_source_span(registration.source_span));
        }
        if selected.iter().any(|provider| {
            provider
                .provider
                .row_requirements
                .contains(&registration.required.symbol)
        }) {
            return Err(Diagnostic::error(
                "assumed binding competes with an ordinary selected realization of the required member",
            ).with_source_span(registration.source_span));
        }
        supplier_links(registrations, &registration.required)?;
    }
    Ok(())
}

fn supplier_links(
    registrations: &[AssumedBindingRegistration],
    first: &ProductBoundaryRequirementReference,
) -> Result<Vec<ProductBoundaryRequirementReference>, Diagnostic> {
    let mut links = Vec::new();
    let mut current = first;
    loop {
        if links.contains(current) {
            return Err(Diagnostic::error(
                "assumed binding supplier links contain a cycle",
            ));
        }
        links.push((*current).clone());
        let Some(registration) = registrations
            .iter()
            .find(|registration| &registration.required == current)
        else {
            return Ok(links);
        };
        current = &registration.supplied;
    }
}
