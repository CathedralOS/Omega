//! Carry Build's completed choices into application compilation.
//!
//! A provider choice must match its authored declaration request before it
//! becomes selection intent. Service audit registrations also require a
//! completed event at an exact authored occurrence. An unrequested policy
//! needs no admission, and an unimplemented request cannot simply vanish.

use crate::declaration_selection::BuildOperation;
use crate::declaration_selection::ExecutedProductSelection;
use crate::declaration_selection::ProductDeclarationRequest;

#[path = "product/authored_requests.rs"]
mod authored_requests;

#[path = "product/opaque_representations.rs"]
pub mod opaque_representations;

#[path = "product/root_grants.rs"]
mod root_grants;

#[path = "product/provider_selections.rs"]
mod provider_selection_requests;

#[path = "product/retained_provider_selections.rs"]
mod retained_provider_selections;

#[cfg(test)]
#[path = "product/provider_selection_tests.rs"]
mod provider_selection_tests;

/// Whether a Build asked for one of `operations`: it authored such a
/// request (`declared`, executed or not) or executed one. Every admission
/// of this module, and the opaque-representation admission of the source
/// preparation, decides by this whether it has anything to admit.
pub fn build_requests_any(
    declared: &[std::sync::Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
    operations: &[BuildOperation],
) -> bool {
    declared
        .iter()
        .any(|request| operations.contains(&request.operation))
        || executed
            .iter()
            .any(|event| operations.contains(&event.request.operation))
}

pub mod test_groups {
    pub use source_declarations::source_selections::{TestGroupRegistration, TestGroupService};
}
pub mod assumed_bindings {
    pub use source_declarations::source_selections::AssumedBindingContract;
}
#[path = "product/native_bindings.rs"]
pub mod native_bindings;
#[path = "product/product_declarations.rs"]
pub mod product_declarations;
pub mod product_policy {
    use crate::declaration_selection::BuildOperation;
    use crate::declaration_selection::ExecutedEnumExclusion;
    use crate::declaration_selection::ExecutedProductSelection;
    use crate::declaration_selection::ProductDeclarationRequest;
    use crate::source_state::TypedTrees;
    use declaration_contracts::selection::AuthoredBehaviorExclusion;
    use declaration_contracts::selection::AuthoredRootGrant;
    use diagnostics::Diagnostic;
    use std::sync::Arc;
    use symbols::SymbolHandle;
    pub fn collect_behavior_exclusions(
        _typed: &TypedTrees,
        _activation: Option<SymbolHandle>,
        declared: &[Arc<ProductDeclarationRequest>],
        executed: &[ExecutedProductSelection],
        enums: &[ExecutedEnumExclusion],
    ) -> Result<Vec<AuthoredBehaviorExclusion>, Vec<Diagnostic>> {
        // An executed enum exclusion is its own row kind; every one counts.
        if super::build_requests_any(declared, executed, &[BuildOperation::ServiceExclusion])
            || !enums.is_empty()
        {
            return Err(vec![Diagnostic::error(
                "no product route yet: Build behavior exclusion admission",
            )]);
        }
        Ok(Vec::new())
    }
    pub fn collect_product_root_grants(
        typed: &TypedTrees,
        activation: Option<SymbolHandle>,
        declared: &[Arc<ProductDeclarationRequest>],
        executed: &[ExecutedProductSelection],
    ) -> Result<Vec<AuthoredRootGrant>, Vec<Diagnostic>> {
        super::root_grants::collect(typed, activation, declared, executed)
    }
}
pub mod provider_declarations {
    use crate::declaration_selection::BuildSelectionOrigin;
    use crate::declaration_selection::product_operands::ProductStaticOperandGraph;
    pub use crate::source_state::provider_declarations::ProviderPlanningContext;
    pub use declaration_contracts::selection::CompositionMode;
    use declaration_contracts::selection::ProductTarget;
    use diagnostics::Diagnostic;
    use source_admission::RetainedDeclaration;
    use std::sync::Arc;
    use symbols::SymbolHandle;
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RetainedProductApplication {
        pub graph: ProductStaticOperandGraph,
        pub sources: Arc<crate::source_inputs::SourceMap>,
        pub identity: String,
    }
    impl RetainedProductApplication {
        pub fn declaration_spans(&self) -> impl Iterator<Item = source_text::SourceSpan> + '_ {
            self.graph
                .names
                .iter()
                .filter(|(_, name)| name.builtin_type.is_none() && name.builtin_function.is_none())
                .filter_map(|(_, name)| name.declaration)
        }
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RetainedProviderSelection {
        pub subject: RetainedDeclaration,
        pub subject_identity: String,
        pub subject_application: Option<RetainedProductApplication>,
        pub provider: RetainedDeclaration,
        pub provider_identity: String,
        pub provider_application: Option<RetainedProductApplication>,
        pub target: ProductTarget,
        pub composition_mode: CompositionMode,
        pub origin: BuildSelectionOrigin,
    }
    pub use crate::provider_choice_inputs::ProviderOperatorFamilyCoordinate;
    pub use crate::provider_choice_inputs::ProviderOperatorFamilySelection;
    pub use crate::provider_choice_inputs::ProviderSelection;
    pub use crate::provider_choice_inputs::ProviderSelectionIdentity;
    pub use crate::provider_choice_inputs::ProviderSelectionSubject;
    pub use crate::provider_choice_inputs::TargetProviderDefaults;
    pub use crate::provider_choice_inputs::source_application;
    pub use provider_choices::ProviderApplicationId;
    pub(super) fn application_id(
        handle: arena::Handle<crate::source_state::product_operand::ProductOperandApplication>,
    ) -> ProviderApplicationId {
        ProviderApplicationId::from_parts(handle.arena_index(), handle.generation())
    }
    pub fn collect_product_provider_selections(
        context: ProviderPlanningContext<'_>,
        activation: Option<SymbolHandle>,
        declared: &[Arc<crate::declaration_selection::ProductDeclarationRequest>],
        executed: &[crate::declaration_selection::ExecutedProductSelection],
    ) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
        super::provider_selection_requests::collect(context, activation, declared, executed)
    }
    /// The provider defaults a program declares, one row per target.
    ///
    pub use crate::source_provider_choices::retain_provider_defaults;
    pub(super) use crate::source_provider_choices::authored_defaults::{form, identity};
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn request(operation: BuildOperation) -> Arc<ProductDeclarationRequest> {
        Arc::new(ProductDeclarationRequest {
            source_span: source_text::SourceSpan::default(),
            operation,
            operands: Box::new([]),
        })
    }

    fn executed(request: &Arc<ProductDeclarationRequest>) -> ExecutedProductSelection {
        ExecutedProductSelection {
            static_environment: None,
            target: None,
            request: Arc::clone(request),
            selecting_machine: None,
            composition: None,
        }
    }

    /// Runs every admission that reads product selections and reports which
    /// of them refused.
    fn refusals(
        declared: &[Arc<ProductDeclarationRequest>],
        executed: &[ExecutedProductSelection],
    ) -> Vec<String> {
        let typed = crate::source_state::TypedTrees::default();
        // The Build ran: its activation is known in every case.
        let activation = Some(symbols::SymbolHandle::invalid());
        let context = provider_declarations::ProviderPlanningContext::new(
            &typed,
            declaration_contracts::source_identity::DependencyScope::Product,
        );
        [
            native_bindings::collect(&typed, activation, declared, &[], &[]).map(drop),
            product_declarations::admit(&typed, activation, declared, executed, &[]).map(drop),
            product_policy::collect_behavior_exclusions(
                &typed,
                activation,
                declared,
                executed,
                &[],
            )
            .map(drop),
            product_policy::collect_product_root_grants(&typed, activation, declared, executed)
                .map(drop),
            provider_declarations::collect_product_provider_selections(
                context, activation, declared, executed,
            )
            .map(drop),
        ]
        .into_iter()
        .filter_map(Result::err)
        .map(|diagnostics| diagnostics[0].message.clone())
        .collect()
    }

    /// A Build that ran and asked for nothing these admissions own is
    /// refused by none of them. `OptimizationSelection` belongs to none.
    #[test]
    fn a_build_that_asks_an_admission_for_nothing_is_not_refused_by_it() {
        assert_eq!(refusals(&[], &[]), Vec::<String>::new());
        let other = request(BuildOperation::OptimizationSelection);
        assert_eq!(
            refusals(&[Arc::clone(&other)], &[executed(&other)]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn an_authored_audit_or_test_group_requires_its_original_formation() {
        for operation in [
            BuildOperation::ServiceAudit,
            BuildOperation::TestGroupRegistration,
            BuildOperation::TestGroupService,
        ] {
            assert_eq!(
                refusals(&[request(operation)], &[]),
                ["Build declaration lost its original Build formations"],
                "{operation:?}"
            );
        }
    }

    /// The twins: each unimplemented operation is refused by its own admission,
    /// whether the Build only authored the request or also executed it.
    #[test]
    fn each_request_is_refused_by_the_admission_that_owns_its_operation() {
        for (operation, owner) in [
            (
                BuildOperation::AssumedBindingContract,
                "Build product declaration admission",
            ),
            (
                BuildOperation::WireCompatibilityRequest,
                "Build product declaration admission",
            ),
            (
                BuildOperation::ServiceExclusion,
                "Build behavior exclusion admission",
            ),
        ] {
            let asked = request(operation);
            let expected = [format!("no product route yet: {owner}")];
            assert_eq!(
                refusals(&[Arc::clone(&asked)], &[]),
                expected,
                "{operation:?}"
            );
            assert_eq!(
                refusals(&[], &[executed(&asked)]),
                expected,
                "{operation:?}"
            );
        }
    }

    /// The defaults a program declares, as (target, slot, provider) rows,
    /// or the first refusal.
    fn provider_defaults_of(source: &str) -> Result<Vec<(Option<String>, String, String)>, String> {
        let typed = crate::evaluation::source_jobs::tests::typed(source);
        provider_declarations::retain_provider_defaults(
            provider_declarations::ProviderPlanningContext::new(
                &typed,
                declaration_contracts::source_identity::DependencyScope::Product,
            ),
        )
        .map(|rows| {
            rows.into_iter()
                .flat_map(|row| {
                    let target = row.target.clone();
                    row.selections.into_iter().map(move |selection| {
                        (
                            target.clone(),
                            selection.subject.authored_path().to_owned(),
                            selection.provider_type.authored_path,
                        )
                    })
                })
                .collect()
        })
        .map_err(|diagnostics| diagnostics[0].message.clone())
    }

    const SLOT: &str = "boundary trait Service {\n    machine go() reaches none crashes none;\n}\ndata Native {}\nNativeProvider satisfies Service { machine go() reaches none crashes none {} }\n";

    /// A program that declares no provider-defaults machine has no defaults,
    /// and one that declares a default states it as slot and provider.
    #[test]
    fn a_provider_default_is_read_from_its_select_provider_call() {
        assert_eq!(
            provider_defaults_of(&format!(
                "{SLOT}machine Native::configure(defaults: @mut Native) {{ }}\n"
            )),
            Ok(Vec::new())
        );
        assert_eq!(
            provider_defaults_of(&format!(
                "{SLOT}machine Native::provider_defaults(defaults: @mut Native) {{\n    Native::select_provider<Service, NativeProvider>(defaults);\n}}\n"
            )),
            Ok(vec![(
                None,
                "Service".to_owned(),
                "NativeProvider".to_owned()
            )])
        );
    }

    /// A defaults method may live in a data namespace, but the namespace's
    /// type is not a static provider value. It obeys the same check as an
    /// explicit Build choice (`provider_selection_tests`).
    #[test]
    fn a_provider_default_cannot_use_its_data_namespace_as_the_provider_value() {
        let refused = provider_defaults_of(&format!(
            "{SLOT}machine Native::provider_defaults(defaults: @mut Native) {{\n    Native::select_provider<Service, Native>(defaults);\n}}\n"
        ))
        .unwrap_err();
        assert_eq!(
            refused,
            "provider selection `Native` is not a static operation record from a named implementation; a data type is not a provider value"
        );
    }

    /// The twins: the same machine selecting for a slot that is not a
    /// boundary, and holding a statement that is not a selection. Neither is
    /// skipped.
    #[test]
    fn a_provider_default_that_is_not_a_selection_of_a_boundary_is_refused() {
        let refused = provider_defaults_of(&format!(
            "{SLOT}machine Native::provider_defaults(defaults: @mut Native) {{\n    Native::select_provider<Native, NativeProvider>(defaults);\n}}\n"
        ))
        .unwrap_err();
        assert!(
            refused.contains("subject `Native` does not resolve to an exact boundary trait"),
            "{refused}"
        );
        let refused = provider_defaults_of(&format!(
            "{SLOT}machine other() {{ }}\nmachine Native::provider_defaults(defaults: @mut Native) {{\n    other();\n}}\n"
        ))
        .unwrap_err();
        assert_eq!(
            refused,
            "no product route yet: source provider defaults: `Native::provider_defaults` holds a statement that is not a `select_provider` call"
        );
    }
}
