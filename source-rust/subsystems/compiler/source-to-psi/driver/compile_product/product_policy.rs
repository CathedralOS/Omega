//! Bind Build's authored and executed requests against a Product's typed declarations.
use super::{ProductPolicyInputs, provider_requests};
use crate::compile_product::declarations::AdmittedProductDeclarations;
use crate::compile_product::native_binding_declarations::NativeBindingSelection;
use crate::compile_product::provider_declarations::ProviderSelection;
use crate::declaration_selection::CapturedNativeIntent;
use crate::declaration_selection::ExecutedNativeBinding;
use crate::source_state::opaque_representations::OpaqueRepresentationSelection;
use behavior_exclusion_requests::AuthoredBehaviorExclusion;
use diagnostics::Diagnostic;
use retained_product_selections::CapturedNativeDeclaration;
use symbols::SymbolHandle;
use symbols::root_grants::AuthoredRootGrant;
use typed_trees::TypedTrees;

pub(crate) struct AdmittedProductPolicy {
    pub providers: Vec<ProviderSelection>,
    pub representations: Vec<OpaqueRepresentationSelection>,
    pub native_bindings: Vec<NativeBindingSelection>,
    pub grants: Vec<AuthoredRootGrant>,
    pub declarations: AdmittedProductDeclarations,
    pub exclusions: Vec<AuthoredBehaviorExclusion>,
}

pub(crate) fn capture_package_native_bindings(
    typed: &mut TypedTrees,
    package: semantic_vocabulary::PackageKeyIdentity,
    formations: &[crate::declaration_selection::NativeBindingFormation],
    executed: &[ExecutedNativeBinding],
) -> Result<(Vec<CapturedNativeDeclaration>, Vec<CapturedNativeIntent>), Vec<Diagnostic>>
{
    if formations.is_empty() && executed.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let source = typed.clone();
    let lookups = crate::source_state::product_operand::ProductSourceLookups::new(
        &source,
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
        typed_trees::product_operand::formation::prepare_product_requirement_operand(
            typed,
            &lookups,
            operand,
            author,
            None,
        )?;
    }
    crate::compile_product::native_binding_declarations::capture_package_intents(
        typed,
        package,
        formations,
        executed,
    )
}

pub(super) fn admit_product_policy(
    typed: &mut TypedTrees,
    activation: Option<SymbolHandle>,
    request: &ProductPolicyInputs<'_>,
    test: Option<&crate::registered_tests::TestRoot>,
) -> Result<AdmittedProductPolicy, Vec<Diagnostic>> {
    use crate::declaration_selection::BuildOperation;
    let formations = provider_requests::product_formation_authors(typed, request.declared, |request| {
        matches!(
            request.operation,
            BuildOperation::NativeBinding
                | BuildOperation::ServiceAudit
                | BuildOperation::TestGroupRegistration
        ) || super::provider_requests::has_product_application(request)
    })?;
    // A test product obtains its provider events from the exact registered group.
    // Its groups must therefore close their own applications before that choice.
    provider_requests::prepare_product_applications(
        typed,
        &formations,
        request.executed,
        |operation| operation == BuildOperation::TestGroupRegistration,
    )?;
    let declaration_formations = formations
        .iter()
        .filter(|(request, _)| {
            matches!(
                request.operation,
                BuildOperation::ServiceAudit
                    | BuildOperation::TestGroupRegistration
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let declarations = crate::compile_product::declarations::admit(
        typed,
        activation,
        request.declared,
        request.executed,
        &declaration_formations,
    )?;
    let provider_events = match test {
        Some(test) => {
            &test
                .registration(typed, &declarations.test_groups)?
                .configuration
                .providers[..]
        }
        None => request.executed,
    };
    let providers = provider_requests::admit_product_provider_requests(
        typed,
        activation,
        request.declared,
        provider_events,
        &formations,
    )?;
    let mut native_formations = formations
        .iter()
        .filter(|(request, _)| request.operation == BuildOperation::NativeBinding)
        .peekable();
    if native_formations.peek().is_some() {
        let source = typed.clone();
        let lookups = crate::source_state::product_operand::ProductSourceLookups::new(
            &source,
        );
        for (request, author) in native_formations {
            let [operand] = request.operands.as_ref() else {
                return Err(vec![Diagnostic::error(
                    "native binding lost its exact requirement operand",
                )]);
            };
            typed_trees::product_operand::formation::prepare_product_requirement_operand(
                typed,
                &lookups,
                operand,
                *author,
                None,
            )?;
        }
    }
    let representations = super::declaration_lookup::collect_opaque_representations(
        typed,
        activation,
        request.declared,
        request.executed,
    )?;
    let native_bindings = crate::compile_product::native_binding_declarations::collect(
        typed,
        activation,
        request.declared,
        request.native_bindings,
        &formations,
    )?;
    let grants = super::declaration_lookup::collect_product_root_grants(
        typed,
        activation,
        request.declared,
        request.executed,
    )?;
    let exclusions = super::behavior_exclusions::admit(
        typed,
        activation,
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
