//! Actual Build selection data survives without the source-checking producers.
//! Nonempty selection/admission requests require the new product route.

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
        ExecutedNativeBinding, ProductDeclarationRequest,
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
        activation: Option<SymbolHandle>,
        declared: &[std::sync::Arc<ProductDeclarationRequest>],
        executed: &[ExecutedNativeBinding],
    ) -> Result<Vec<NativeBindingSelection>, Vec<Diagnostic>> {
        if activation.is_some() || !declared.is_empty() || !executed.is_empty() {
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
        ExecutedProductSelection, ProductDeclarationRequest,
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
        activation: Option<SymbolHandle>,
        declared: &[Arc<ProductDeclarationRequest>],
        executed: &[ExecutedProductSelection],
    ) -> Result<AdmittedProductDeclarations, Vec<Diagnostic>> {
        if activation.is_some() || !declared.is_empty() || !executed.is_empty() {
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
        AuthoredBehaviorExclusion, AuthoredRootGrant, ExecutedEnumExclusion,
        ExecutedProductSelection, ProductDeclarationRequest,
    };
    use std::sync::Arc;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    pub fn collect_behavior_exclusions(
        _typed: &TypedTrees,
        activation: Option<SymbolHandle>,
        declared: &[Arc<ProductDeclarationRequest>],
        executed: &[ExecutedProductSelection],
        enums: &[ExecutedEnumExclusion],
    ) -> Result<Vec<AuthoredBehaviorExclusion>, Vec<Diagnostic>> {
        if activation.is_some() || !declared.is_empty() || !executed.is_empty() || !enums.is_empty()
        {
            return Err(vec![Diagnostic::error(
                "no product route yet: Build behavior exclusion admission",
            )]);
        }
        Ok(Vec::new())
    }
    pub fn collect_product_root_grants(
        _typed: &TypedTrees,
        activation: Option<SymbolHandle>,
        declared: &[Arc<ProductDeclarationRequest>],
        executed: &[ExecutedProductSelection],
    ) -> Result<Vec<AuthoredRootGrant>, Vec<Diagnostic>> {
        if activation.is_some() || !declared.is_empty() || !executed.is_empty() {
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
        activation: Option<SymbolHandle>,
        declared: &[Arc<language_semantics::declaration_selection::ProductDeclarationRequest>],
        executed: &[language_semantics::declaration_selection::ExecutedProductSelection],
    ) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
        if activation.is_some() || !declared.is_empty() || !executed.is_empty() {
            return Err(vec![Diagnostic::error(
                "no product route yet: source provider selection admission",
            )]);
        }
        Ok(Vec::new())
    }
    pub fn retain_provider_defaults(
        _context: ProviderPlanningContext<'_>,
    ) -> Result<Vec<TargetProviderDefaults>, Vec<Diagnostic>> {
        Err(vec![Diagnostic::error(
            "no product route yet: source provider defaults",
        )])
    }
}
