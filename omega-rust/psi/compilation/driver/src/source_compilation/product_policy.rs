//! The post-join source operation for Product choices. Package authentication
//! precedes this operation; evaluation custody and realization follow it.

use super::SourceContinuation;
use build_time_evaluation::native_bindings::NativeBindingSelection;
use build_time_evaluation::product_declarations::AdmittedProductDeclarations;
use build_time_evaluation::provider_declarations::ProviderSelection;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredBehaviorExclusion, AuthoredRootGrant, CapturedNativeDeclaration, CapturedNativeIntent,
    ExecutedEnumExclusion, ExecutedNativeBinding, ExecutedProductSelection,
    ProductDeclarationRequest,
};
use representation_selections::OpaqueRepresentationSelection;
use std::sync::Arc;
use symbols::SymbolHandle;

/// Source-issued requests and reached effects from one completed root Build.
pub struct ProductPolicyRequest<'request> {
    pub activation: Option<SymbolHandle>,
    pub declared: &'request [Arc<ProductDeclarationRequest>],
    pub executed: &'request [ExecutedProductSelection],
    pub native_bindings: &'request [ExecutedNativeBinding],
    pub exclusions: &'request [ExecutedEnumExclusion],
}

/// Admitted source choices, not a checked executable or a physical verdict.
pub struct AdmittedProductPolicy {
    pub providers: Vec<ProviderSelection>,
    pub representations: Vec<OpaqueRepresentationSelection>,
    pub native_bindings: Vec<NativeBindingSelection>,
    pub grants: Vec<AuthoredRootGrant>,
    pub declarations: AdmittedProductDeclarations,
    pub exclusions: Vec<AuthoredBehaviorExclusion>,
}

impl SourceContinuation {
    /// Select exact native declarations from this ordinary Product frontier.
    /// Product bodies remain pending; this is the same source-selection owner
    /// used by Build's declaration queries, not a final checking prerequisite.
    pub fn capture_package_native_bindings(
        &self,
        package: semantic_vocabulary::PackageKeyIdentity,
        formations: &[language_semantics::declaration_selection::NativeBindingFormation],
        executed: &[ExecutedNativeBinding],
    ) -> Result<(Vec<CapturedNativeDeclaration>, Vec<CapturedNativeIntent>), Vec<Diagnostic>> {
        build_time_evaluation::native_bindings::capture_package_intents(
            self.typed(),
            package,
            formations,
            executed,
        )
    }

    /// Form every authored request, then retain only executed choices. Provider
    /// application preparation may extend typing; all later judgments borrow
    /// that same completed source view. No Product body executes here.
    pub fn admit_product_policy(
        &mut self,
        request: ProductPolicyRequest<'_>,
    ) -> Result<AdmittedProductPolicy, Vec<Diagnostic>> {
        let providers = self.admit_product_provider_requests(
            request.activation,
            request.declared,
            request.executed,
        )?;
        let typed = self.typed();
        let representations = representation_planning::collect_opaque_representation_selections(
            typed,
            request.activation,
            request.declared,
            request.executed,
        )?;
        let native_bindings = build_time_evaluation::native_bindings::collect(
            typed,
            request.activation,
            request.declared,
            request.native_bindings,
        )?;
        let grants = build_time_evaluation::product_policy::collect_product_root_grants(
            typed,
            request.activation,
            request.declared,
            request.executed,
        )?;
        let declarations = build_time_evaluation::product_declarations::admit(
            typed,
            request.activation,
            request.declared,
            request.executed,
        )?;
        let exclusions = build_time_evaluation::product_policy::collect_behavior_exclusions(
            typed,
            request.activation,
            request.declared,
            request.executed,
            request.exclusions,
        )?;
        Ok(AdmittedProductPolicy {
            providers,
            representations,
            native_bindings,
            grants,
            declarations,
            exclusions,
        })
    }

    /// Resolve authenticated dependency intent in this Product namespace.
    /// Packages retain source/profile/evaluation custody; Psi receives only the
    /// captured declarations and effects, not a package manager or provider plan.
    pub fn admit_captured_native_bindings(
        &self,
        declarations: &[CapturedNativeDeclaration],
        intents: &[CapturedNativeIntent],
    ) -> Result<Vec<NativeBindingSelection>, Vec<Diagnostic>> {
        build_time_evaluation::native_bindings::collect_captured(
            self.typed(),
            declarations,
            intents,
        )
    }
}
