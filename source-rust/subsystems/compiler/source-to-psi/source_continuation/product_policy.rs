use super::SourceContinuation;
use crate::declaration_selection::CapturedNativeIntent;
use crate::declaration_selection::ExecutedEnumExclusion;
use crate::declaration_selection::ExecutedNativeBinding;
use crate::declaration_selection::ExecutedProductSelection;
use crate::declaration_selection::ProductDeclarationRequest;
use crate::product::native_binding_declarations::{CapturedNativeDeclaration, NativeBindingSelection};
use crate::product::declarations::AdmittedProductDeclarations;
use crate::product::provider_declarations::ProviderSelection;
use crate::source_state::opaque_representations::OpaqueRepresentationSelection;
use behavior_exclusion_requests::AuthoredBehaviorExclusion;
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbols::SymbolHandle;
use symbols::root_grants::AuthoredRootGrant;

pub struct ProductPolicyRequest<'request> {
    pub activation: Option<SymbolHandle>,
    pub declared: &'request [Arc<ProductDeclarationRequest>],
    pub executed: &'request [ExecutedProductSelection],
    pub native_bindings: &'request [ExecutedNativeBinding],
    pub exclusions: &'request [ExecutedEnumExclusion],
}

pub struct AdmittedProductPolicy {
    pub providers: Vec<ProviderSelection>,
    pub representations: Vec<OpaqueRepresentationSelection>,
    pub native_bindings: Vec<NativeBindingSelection>,
    pub grants: Vec<AuthoredRootGrant>,
    pub declarations: AdmittedProductDeclarations,
    pub exclusions: Vec<AuthoredBehaviorExclusion>,
}

impl SourceContinuation {
    pub fn capture_package_native_bindings(
        &mut self,
        package: semantic_vocabulary::PackageKeyIdentity,
        formations: &[crate::declaration_selection::NativeBindingFormation],
        executed: &[ExecutedNativeBinding],
    ) -> Result<(Vec<CapturedNativeDeclaration>, Vec<CapturedNativeIntent>), Vec<Diagnostic>>
    {
        if formations.is_empty() && executed.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }
        let source = Arc::clone(&self.input);
        let lookups = crate::source_state::product_operand::ProductSourceLookups::new(
            source.typing.typed(),
        );
        for formation in formations {
            let author = lookups
                .authors
                .resolve(formation.author)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let [operand] = formation.request.operands.as_ref() else {
                return Err(vec![Diagnostic::error(
                    "native formation lost its exact requirement operand",
                )]);
            };
            crate::preparation::prepare_product_requirement_operand(
                Arc::make_mut(&mut self.input).typing.typed_mut(),
                &lookups,
                operand,
                author,
                None,
            )?;
        }
        crate::product::native_binding_declarations::capture_package_intents(
            self.typed(),
            package,
            formations,
            executed,
        )
    }

    pub fn admit_product_policy(
        &mut self,
        request: ProductPolicyRequest<'_>,
    ) -> Result<AdmittedProductPolicy, Vec<Diagnostic>> {
        use crate::declaration_selection::BuildOperation;
        let formations = self.product_formation_authors(request.declared, |request| {
            matches!(
                request.operation,
                BuildOperation::NativeBinding
                    | BuildOperation::ServiceAudit
                    | BuildOperation::TestGroupRegistration
                    | BuildOperation::TestGroupService
            ) || super::provider_requests::has_provider_application(request)
        })?;
        let providers = self.admit_product_provider_requests(
            request.activation,
            request.declared,
            request.executed,
            &formations,
        )?;
        let declaration_formations = formations
            .iter()
            .filter(|(request, _)| {
                matches!(
                    request.operation,
                    BuildOperation::ServiceAudit
                        | BuildOperation::TestGroupRegistration
                        | BuildOperation::TestGroupService
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut native_formations = formations
            .iter()
            .filter(|(request, _)| request.operation == BuildOperation::NativeBinding)
            .peekable();
        if native_formations.peek().is_some() {
            let source = Arc::clone(&self.input);
            let lookups = crate::source_state::product_operand::ProductSourceLookups::new(
                source.typing.typed(),
            );
            for (request, author) in native_formations {
                let [operand] = request.operands.as_ref() else {
                    return Err(vec![Diagnostic::error(
                        "native binding lost its exact requirement operand",
                    )]);
                };
                crate::preparation::prepare_product_requirement_operand(
                    Arc::make_mut(&mut self.input).typing.typed_mut(),
                    &lookups,
                    operand,
                    *author,
                    None,
                )?;
            }
        }
        let typed = self.typed();
        let representations = super::product_declarations::collect_opaque_representations(
            typed,
            request.activation,
            request.declared,
            request.executed,
        )?;
        let native_bindings = crate::product::native_binding_declarations::collect(
            typed,
            request.activation,
            request.declared,
            request.native_bindings,
            &formations,
        )?;
        let grants = super::product_declarations::collect_product_root_grants(
            typed,
            request.activation,
            request.declared,
            request.executed,
        )?;
        let declarations = crate::product::declarations::admit(
            typed,
            request.activation,
            request.declared,
            request.executed,
            &declaration_formations,
        )?;
        crate::product::declarations::require_supported_exclusions(
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
            exclusions: Vec::new(),
        })
    }

    pub fn admit_captured_native_bindings(
        &mut self,
        declarations: &[CapturedNativeDeclaration],
        intents: &[CapturedNativeIntent],
    ) -> Result<Vec<NativeBindingSelection>, Vec<Diagnostic>> {
        crate::product::native_binding_declarations::collect_captured(
            Arc::make_mut(&mut self.input).typing.typed_mut(),
            declarations,
            intents,
        )
    }
}
