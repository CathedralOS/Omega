//! Actual Build selection data survives without the source-checking producers.
//!
//! A finished Build hands the Product its requests: each names one
//! `BuildOperation`, and each operation has one admission here that turns
//! its requests into Product policy. None of these admissions has a producer
//! on the lowered-form route yet. An admission therefore refuses a Build
//! that authored or executed a request of its own operations, and answers
//! "none" to a Build that made no such request: that Build asked it for
//! nothing, whatever else it selected. A request is never dropped.

use language_semantics::declaration_selection::{
    BuildOperation, ExecutedProductSelection, ProductDeclarationRequest,
};

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
    use symbols::SymbolHandle;
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TestGroupRegistration {
        pub requirement: SymbolHandle,
        pub selecting_machine: SymbolHandle,
        pub source_span: source::SourceSpan,
        /// Service slots this group's runner backs with private per-test state,
        /// in selection order; empty runs the group without services.
        pub services: Vec<TestGroupService>,
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TestGroupService {
        pub slot: SymbolHandle,
        pub source_span: source::SourceSpan,
    }
}
pub mod assumed_bindings {
    pub use language_semantics::declaration_selection::AssumedBindingContract;
}
pub mod native_bindings {
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::{
        BuildOperation, ExecutedNativeBinding, ProductDeclarationRequest,
    };
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct NativeBindingSelection {
        pub application:
            Option<arena::Handle<typed_trees::product_operand::ProductOperandApplication>>,
        pub subject: SymbolHandle,
        pub requirement: SymbolHandle,
        pub target: language_semantics::declaration_selection::ProductTarget,
        pub locator: semantic_vocabulary::NativeBindingLocator,
        pub producer: language_semantics::declaration_selection::NativeBindingProducer,
        pub source_occurrence_digest: [u8; 32],
        pub supplier_package: semantic_vocabulary::PackageKeyIdentity,
        pub source_span: source::SourceSpan,
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CapturedNativeDeclaration {
        pub application: Option<crate::provider_declarations::RetainedProductApplication>,
        pub source: language_semantics::declaration_selection::NativeBindingSource,
        pub requirement: symbols::RetainedDeclaration,
        pub requirement_identity: String,
        pub dependency_alias: Option<String>,
    }
    pub fn collect(
        _typed: &TypedTrees,
        _activation: Option<SymbolHandle>,
        declared: &[std::sync::Arc<ProductDeclarationRequest>],
        executed: &[ExecutedNativeBinding],
    ) -> Result<Vec<NativeBindingSelection>, Vec<Diagnostic>> {
        // An executed native binding is its own row kind; every one counts.
        if super::build_requests_any(declared, &[], &[BuildOperation::NativeBinding])
            || !executed.is_empty()
        {
            return Err(vec![Diagnostic::error(
                "no product route yet: native binding admission",
            )]);
        }
        Ok(Vec::new())
    }
    pub fn capture_package_intents(
        _typed: &typed_trees::TypedTrees,
        _package: semantic_vocabulary::PackageKeyIdentity,
        formations: &[language_semantics::declaration_selection::NativeBindingFormation],
        executed: &[language_semantics::declaration_selection::ExecutedNativeBinding],
    ) -> Result<
        (
            Vec<CapturedNativeDeclaration>,
            Vec<language_semantics::declaration_selection::CapturedNativeIntent>,
        ),
        Vec<Diagnostic>,
    > {
        if !formations.is_empty() || !executed.is_empty() {
            return Err(vec![Diagnostic::error(
                "no product route yet: native binding intent capture",
            )]);
        }
        Ok((Vec::new(), Vec::new()))
    }
    pub fn collect_captured(
        _typed: &mut TypedTrees,
        declarations: &[CapturedNativeDeclaration],
        intents: &[language_semantics::declaration_selection::CapturedNativeIntent],
    ) -> Result<Vec<NativeBindingSelection>, Vec<Diagnostic>> {
        if !declarations.is_empty() || !intents.is_empty() {
            return Err(vec![Diagnostic::error(
                "no product route yet: captured native binding admission",
            )]);
        }
        Ok(Vec::new())
    }
}
pub mod product_declarations {
    use crate::product::assumed_bindings::AssumedBindingContract;
    use crate::product::test_groups::TestGroupRegistration;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::{
        BuildOperation, ExecutedProductSelection, ProductDeclarationRequest,
    };
    use language_semantics::wire::WireCompatibilityDemand;
    use std::sync::Arc;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ServiceAuditRegistration {
        pub identity: String,
        pub name: String,
    }
    pub struct AdmittedProductDeclarations {
        pub service_audits: Vec<ServiceAuditRegistration>,
        pub assumed_bindings: Vec<AssumedBindingContract>,
        pub wire_demands: Vec<WireCompatibilityDemand>,
        pub test_groups: Vec<TestGroupRegistration>,
    }
    pub fn admit(
        _typed: &TypedTrees,
        _activation: Option<SymbolHandle>,
        declared: &[Arc<ProductDeclarationRequest>],
        executed: &[ExecutedProductSelection],
    ) -> Result<AdmittedProductDeclarations, Vec<Diagnostic>> {
        if super::build_requests_any(
            declared,
            executed,
            &[
                BuildOperation::ServiceAudit,
                BuildOperation::AssumedBindingContract,
                BuildOperation::WireCompatibilityRequest,
                BuildOperation::TestGroupRegistration,
                BuildOperation::TestGroupService,
            ],
        ) {
            return Err(vec![Diagnostic::error(
                "no product route yet: Build product declaration admission",
            )]);
        }
        Ok(AdmittedProductDeclarations {
            service_audits: Vec::new(),
            assumed_bindings: Vec::new(),
            wire_demands: Vec::new(),
            test_groups: Vec::new(),
        })
    }
}
pub mod product_policy {
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::{
        AuthoredBehaviorExclusion, AuthoredRootGrant, BuildOperation, ExecutedEnumExclusion,
        ExecutedProductSelection, ProductDeclarationRequest,
    };
    use std::sync::Arc;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
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
        _typed: &TypedTrees,
        _activation: Option<SymbolHandle>,
        declared: &[Arc<ProductDeclarationRequest>],
        executed: &[ExecutedProductSelection],
    ) -> Result<Vec<AuthoredRootGrant>, Vec<Diagnostic>> {
        if super::build_requests_any(declared, executed, &[BuildOperation::BoundaryAcceptance]) {
            return Err(vec![Diagnostic::error(
                "no product route yet: Build root grant admission",
            )]);
        }
        Ok(Vec::new())
    }
}
pub mod provider_declarations {
    use diagnostics::Diagnostic;
    pub use language_semantics::declaration_selection::CompositionMode;
    use language_semantics::declaration_selection::product_operands::ProductStaticOperandGraph;
    use language_semantics::declaration_selection::{BuildSelectionOrigin, ProductTarget};
    use std::sync::Arc;
    use symbols::RetainedDeclaration;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    pub use typed_trees::provider_declarations::ProviderPlanningContext;
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RetainedProductApplication {
        pub graph: ProductStaticOperandGraph,
        pub sources: Arc<source::SourceMap>,
        pub identity: String,
    }
    impl RetainedProductApplication {
        pub fn declaration_spans(&self) -> impl Iterator<Item = source::SourceSpan> + '_ {
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
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProviderSelectionIdentity {
        pub symbol: SymbolHandle,
        pub application: arena::Handle<typed_trees::product_operand::ProductOperandApplication>,
        pub applied: bool,
        pub application_identity: Option<String>,
        pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub canonical_path: String,
        pub authored_path: String,
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProviderOperatorFamilyCoordinate {
        pub symbol: SymbolHandle,
        pub requirement_identity: String,
        pub static_parameter_count: usize,
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProviderOperatorFamilySelection {
        pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub canonical_path: String,
        pub authored_path: String,
        coordinates: Vec<ProviderOperatorFamilyCoordinate>,
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ProviderSelectionSubject {
        BoundaryTrait(ProviderSelectionIdentity),
        BoundaryRequirement(ProviderSelectionIdentity),
        BoundaryOperatorFamily(ProviderOperatorFamilySelection),
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProviderSelection {
        pub target: Option<language_semantics::declaration_selection::ProductTarget>,
        pub subject: ProviderSelectionSubject,
        pub provider_type: ProviderSelectionIdentity,
        pub composition_mode: CompositionMode,
        pub origin: language_semantics::declaration_selection::BuildSelectionOrigin,
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TargetProviderDefaults {
        pub target: Option<String>,
        pub machines: Vec<symbols::SymbolHandle>,
        pub selections: Vec<ProviderSelection>,
    }
    impl ProviderOperatorFamilySelection {
        pub fn new(
            package: Option<semantic_vocabulary::PackageKeyIdentity>,
            canonical_path: String,
            authored_path: String,
            mut coordinates: Vec<ProviderOperatorFamilyCoordinate>,
        ) -> Result<Self, String> {
            coordinates
                .sort_by(|left, right| left.requirement_identity.cmp(&right.requirement_identity));
            if coordinates.is_empty() {
                return Err(format!(
                    "boundary-operator family `{authored_path}` contains no applicable coordinates"
                ));
            }
            for pair in coordinates.windows(2) {
                if pair[0].requirement_identity == pair[1].requirement_identity {
                    return Err(format!(
                        "boundary-operator family `{authored_path}` contains ambiguous coordinate `{}`",
                        pair[0].requirement_identity
                    ));
                }
            }
            Ok(Self {
                package,
                canonical_path,
                authored_path,
                coordinates,
            })
        }
        pub fn coordinates(&self) -> &[ProviderOperatorFamilyCoordinate] {
            &self.coordinates
        }
    }
    impl ProviderSelectionSubject {
        pub fn package(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
            match self {
                Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => {
                    identity.package
                }
                Self::BoundaryOperatorFamily(family) => family.package,
            }
        }
        pub fn canonical_path(&self) -> &str {
            match self {
                Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => {
                    &identity.canonical_path
                }
                Self::BoundaryOperatorFamily(family) => &family.canonical_path,
            }
        }
        pub fn authored_path(&self) -> &str {
            match self {
                Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => {
                    &identity.authored_path
                }
                Self::BoundaryOperatorFamily(family) => &family.authored_path,
            }
        }
        pub fn selects_schema(
            &self,
            schema_symbol: SymbolHandle,
            requirement_identity: &str,
            application: Option<&str>,
        ) -> bool {
            match self {
                Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => {
                    identity.symbol == schema_symbol
                        && identity
                            .application_identity
                            .as_deref()
                            .is_none_or(|selected| Some(selected) == application)
                }
                Self::BoundaryOperatorFamily(family) => {
                    family.coordinates.iter().any(|coordinate| {
                        coordinate.symbol == schema_symbol
                            && coordinate.requirement_identity == requirement_identity
                    })
                }
            }
        }
        pub fn same_declaration_as(&self, other: &Self) -> bool {
            match (self, other) {
                (Self::BoundaryTrait(left), Self::BoundaryTrait(right)) => {
                    left.symbol == right.symbol
                        && left.application_identity == right.application_identity
                }
                (Self::BoundaryRequirement(left), Self::BoundaryRequirement(right)) => {
                    left.symbol == right.symbol
                        && left.application_identity == right.application_identity
                }
                (Self::BoundaryOperatorFamily(left), Self::BoundaryOperatorFamily(right)) => {
                    left.package == right.package && left.canonical_path == right.canonical_path
                }
                _ => false,
            }
        }
    }
    impl RetainedProviderSelection {
        pub fn rejoin(
            &self,
            _program: &mut TypedTrees,
            _scope: source::DependencyScope,
            _previous: &[ProviderSelection],
            _candidates: &mut symbols::SourceSpanCandidates,
        ) -> Result<ProviderSelection, Diagnostic> {
            Err(Diagnostic::error(
                "no product route yet: retained source provider selection",
            ))
        }
    }
    pub fn collect_product_provider_selections(
        _context: ProviderPlanningContext<'_>,
        _activation: Option<SymbolHandle>,
        declared: &[Arc<language_semantics::declaration_selection::ProductDeclarationRequest>],
        executed: &[language_semantics::declaration_selection::ExecutedProductSelection],
    ) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
        if super::build_requests_any(
            declared,
            executed,
            &[language_semantics::declaration_selection::BuildOperation::ProviderSelection],
        ) {
            return Err(vec![Diagnostic::error(
                "no product route yet: source provider selection admission",
            )]);
        }
        Ok(Vec::new())
    }
    /// The provider defaults a program declares, one row per target.
    ///
    /// A default is declared by a machine whose purpose is
    /// `ProviderDefaults`; the selections come from the calls in its body.
    /// Nothing on the lowered-form route harvests those calls yet, so a
    /// program that declares such a machine in this planning context is
    /// refused by the machine's name. A program that declares none has no
    /// defaults, which is what the empty answer states.
    pub fn retain_provider_defaults(
        context: ProviderPlanningContext<'_>,
    ) -> Result<Vec<TargetProviderDefaults>, Vec<Diagnostic>> {
        let declared = context.typed.machines().iter().find(|machine| {
            machine.purpose == typed_trees::machine::MachinePurpose::ProviderDefaults
                && !machine.target_family_interface
                && context.contains(machine.symbol)
                && (context.scope == source::DependencyScope::Product || machine.target.is_none())
        });
        match declared {
            None => Ok(Vec::new()),
            Some(machine) => Err(vec![Diagnostic::error(format!(
                "no product route yet: source provider defaults: `{}` declares provider defaults",
                machine.name.as_str()
            ))]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn request(operation: BuildOperation) -> Arc<ProductDeclarationRequest> {
        Arc::new(ProductDeclarationRequest {
            source_span: source::SourceSpan::default(),
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
        let typed = typed_trees::TypedTrees::default();
        // The Build ran: its activation is known in every case.
        let activation = Some(symbols::SymbolHandle::invalid());
        let context = provider_declarations::ProviderPlanningContext::new(
            &typed,
            source::DependencyScope::Product,
        );
        [
            native_bindings::collect(&typed, activation, declared, &[]).map(drop),
            product_declarations::admit(&typed, activation, declared, executed).map(drop),
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

    /// The twins: each operation is refused by exactly its own admission,
    /// whether the Build only authored the request or also executed it.
    #[test]
    fn each_request_is_refused_by_the_admission_that_owns_its_operation() {
        for (operation, owner) in [
            (BuildOperation::NativeBinding, "native binding admission"),
            (
                BuildOperation::ServiceAudit,
                "Build product declaration admission",
            ),
            (
                BuildOperation::AssumedBindingContract,
                "Build product declaration admission",
            ),
            (
                BuildOperation::WireCompatibilityRequest,
                "Build product declaration admission",
            ),
            (
                BuildOperation::TestGroupRegistration,
                "Build product declaration admission",
            ),
            (
                BuildOperation::TestGroupService,
                "Build product declaration admission",
            ),
            (
                BuildOperation::ServiceExclusion,
                "Build behavior exclusion admission",
            ),
            (
                BuildOperation::BoundaryAcceptance,
                "Build root grant admission",
            ),
            (
                BuildOperation::ProviderSelection,
                "source provider selection admission",
            ),
        ] {
            let asked = request(operation);
            let expected = [format!("no product route yet: {owner}")];
            assert_eq!(
                refusals(&[Arc::clone(&asked)], &[]),
                expected,
                "{operation:?}"
            );
            // Native bindings execute as their own row kind, not as a
            // product selection.
            if operation != BuildOperation::NativeBinding {
                assert_eq!(
                    refusals(&[], &[executed(&asked)]),
                    expected,
                    "{operation:?}"
                );
            }
        }
    }

    fn provider_defaults_of(source: &str) -> Result<usize, String> {
        let typed = crate::source_jobs::tests::typed(source);
        provider_declarations::retain_provider_defaults(
            provider_declarations::ProviderPlanningContext::new(
                &typed,
                source::DependencyScope::Product,
            ),
        )
        .map(|rows| rows.len())
        .map_err(|diagnostics| diagnostics[0].message.clone())
    }

    /// A program that declares no provider-defaults machine has no
    /// defaults. The twin declares one: its selections are not harvested
    /// yet, so the program is refused by that machine's name.
    #[test]
    fn provider_defaults_are_absent_only_when_no_machine_declares_them() {
        assert_eq!(
            provider_defaults_of(
                "data Native {}\nmachine Native::configure(defaults: &mut Native) { }\n"
            ),
            Ok(0)
        );
        assert_eq!(
            provider_defaults_of(
                "data Native {}\nmachine Native::provider_defaults(defaults: &mut Native) { }\n"
            ),
            Err(concat!(
                "no product route yet: source provider defaults: ",
                "`Native::provider_defaults` declares provider defaults"
            )
            .into())
        );
    }
}
