/*
We represent declaration selections, target and policy predicates (`product_target`, `product_policy`), product
operand bindings (`product_operands`, `product`).
*/

use declaration_contracts::selection::ProductTarget;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub struct ResolvedAuthoredCallBinding {
    pub selected: SymbolHandle,
    pub original: SymbolHandle,
}

mod build_selection_origin {
    use source_admission::RetainedDeclaration;
    use source_text::Span;
    use symbols::SymbolKind;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BuildSelectionOrigin {
        activation: RetainedDeclaration,
        author: RetainedDeclaration,
        occurrence: Span,
        event_ordinal: Option<u64>,
    }

    impl BuildSelectionOrigin {
        pub fn from_retained_parts(
            activation: RetainedDeclaration,
            author: RetainedDeclaration,
            occurrence: Span,
            event_ordinal: Option<u64>,
        ) -> Result<Self, &'static str> {
            if activation.kind() != SymbolKind::Machine || author.kind() != SymbolKind::Machine {
                return Err("Build selection activation and author must be machine declarations");
            }
            if occurrence.start >= occurrence.end
                || author
                    .source()
                    .source
                    .get(occurrence.start..occurrence.end)
                    .is_none()
            {
                return Err("Build selection has an invalid authored occurrence");
            }
            let portable = |declaration: RetainedDeclaration| {
                RetainedDeclaration::from_retained_parts(
                    declaration.source().clone(),
                    declaration.span(),
                    declaration.kind(),
                )
            };
            Ok(Self {
                activation: portable(activation)?,
                author: portable(author)?,
                occurrence,
                event_ordinal,
            })
        }

        pub fn activation(&self) -> &RetainedDeclaration {
            &self.activation
        }
        pub fn author(&self) -> &RetainedDeclaration {
            &self.author
        }
        pub const fn occurrence(&self) -> Span {
            self.occurrence
        }
        pub const fn event_ordinal(&self) -> Option<u64> {
            self.event_ordinal
        }
    }
}

mod product {
    use super::BuildOperation;
    use declaration_contracts::selection::{CompositionMode, NativeBindingProducer, ProductTarget};

    #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
    pub enum BuildSelectionRequest<Requirement> {
        ProviderReadiness(ProviderReadinessRequest<Requirement>),
        Entry(RootBindingRequest),
        Declaration(std::sync::Arc<ProductDeclarationRequest>),
        Exclusion(EnumExclusionRequest),
    }

    impl<Requirement> Default for BuildSelectionRequest<Requirement> {
        fn default() -> Self {
            Self::Entry(RootBindingRequest::default())
        }
    }

    impl<Requirement> BuildSelectionRequest<Requirement> {
        pub fn requires_lexical_subject(&self) -> bool {
            match self {
                Self::Exclusion(_) => true,
                Self::Declaration(request) => matches!(
                    request.operation,
                    super::BuildOperation::ProviderSelection
                        | super::BuildOperation::RepresentationSelection
                        | super::BuildOperation::NativeBinding
                        | super::BuildOperation::ServiceExclusion
                        | super::BuildOperation::BoundaryAcceptance
                        | super::BuildOperation::ServiceAudit
                        | super::BuildOperation::AssumedBindingContract
                        | super::BuildOperation::WireCompatibilityRequest
                        | super::BuildOperation::TestGroupRegistration
                        | super::BuildOperation::TestGroupService
                ),
                Self::Entry(_) | Self::ProviderReadiness(_) => false,
            }
        }

        pub fn source_span(&self) -> source_text::SourceSpan {
            match self {
                Self::ProviderReadiness(request) => request.source_span,
                Self::Entry(request) => request.source_span,
                Self::Declaration(request) => request.source_span,
                Self::Exclusion(request) => request.source_span,
            }
        }

        pub fn entry(&self) -> Option<&RootBindingRequest> {
            match self {
                Self::Entry(request) => Some(request),
                Self::ProviderReadiness(_) | Self::Declaration(_) | Self::Exclusion(_) => None,
            }
        }

        pub fn designated_operands(&self) -> &[ProductDeclarationOperand] {
            match self {
                Self::ProviderReadiness(request) => std::slice::from_ref(&request.backing),
                Self::Declaration(request) => &request.operands,
                Self::Entry(_) | Self::Exclusion(_) => &[],
            }
        }

        pub fn map_requirement<Next, Error>(
            self,
            map: impl FnOnce(Requirement) -> Result<Next, Error>,
        ) -> Result<BuildSelectionRequest<Next>, Error> {
            Ok(match self {
                Self::ProviderReadiness(request) => {
                    BuildSelectionRequest::ProviderReadiness(ProviderReadinessRequest {
                        source_span: request.source_span,
                        requirement: map(request.requirement)?,
                        backing: request.backing,
                    })
                }
                Self::Entry(request) => BuildSelectionRequest::Entry(request),
                Self::Declaration(request) => BuildSelectionRequest::Declaration(request),
                Self::Exclusion(request) => BuildSelectionRequest::Exclusion(request),
            })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
    pub struct ProviderReadinessRequest<Requirement> {
        pub source_span: source_text::SourceSpan,
        pub requirement: Requirement,
        pub backing: ProductDeclarationOperand,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
    pub enum ExclusionVocabulary {
        CrashCause,
        PhysicalAuthorityClass,
    }

    impl ExclusionVocabulary {
        pub const fn type_name(self) -> &'static str {
            match self {
                Self::CrashCause => "CrashCause",
                Self::PhysicalAuthorityClass => "PhysicalAuthorityClass",
            }
        }

        pub const fn helper(self) -> &'static str {
            match self {
                Self::CrashCause => "build_exclusion_crash_code",
                Self::PhysicalAuthorityClass => "build_exclusion_physical_code",
            }
        }

        pub const fn cases(self) -> &'static [&'static str] {
            match self {
                Self::CrashCause => &["Trap", "Abort"],
                Self::PhysicalAuthorityClass => &[
                    "FilesystemContentRead",
                    "FilesystemContentWrite",
                    "FilesystemMetadataQuery",
                    "DirectoryEnumeration",
                    "FilesystemNamespaceMutation",
                    "FilesystemMetadataMutation",
                    "ProcessOutput",
                    "ProcessTermination",
                    "MachineControl",
                    "PortIo",
                    "InterruptControl",
                    "InterruptEntry",
                    "RootMemoryAccess",
                    "ProcessInput",
                ],
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
    pub struct EnumExclusionRequest {
        pub source_span: source_text::SourceSpan,
        pub vocabulary: ExclusionVocabulary,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ExecutedEnumExclusion {
        pub request: EnumExclusionRequest,
        pub code: u64,
        pub selecting_machine: source_text::SourceSpan,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
    pub enum ProductOperandRole {
        BoundarySubject,
        RootGrantSubject,
        Provider,
        Requirement,
        // An ordinary (non-boundary) trait machine requirement: the exact
        // identity a test group registers (wiki/spec/build/testing.md).
        TestRequirement,
        Data,
        Representation,
        WireCompatibilityFact,
        Formal,
        Field,
    }

    impl BuildOperation {
        pub const fn product_operand_role(self, position: usize) -> Option<ProductOperandRole> {
            use ProductOperandRole as Role;
            match (self, position) {
                (Self::ProviderSelection, 0) | (Self::ServiceExclusion | Self::ServiceAudit, 0) => {
                    Some(Role::BoundarySubject)
                }
                (Self::ProviderSelection, 1) => Some(Role::Provider),
                (Self::BoundaryAcceptance, 0) => Some(Role::RootGrantSubject),
                (Self::ParameterDescription, 0) => Some(Role::Formal),
                (Self::FieldDescription, 0) => Some(Role::Field),
                (Self::AssumedBindingContract, 0 | 1) | (Self::NativeBinding, 0) => {
                    Some(Role::Requirement)
                }
                (Self::TestGroupRegistration | Self::TestGroupService, 0) => {
                    Some(Role::TestRequirement)
                }
                (Self::TestGroupService, 1) => Some(Role::BoundarySubject),
                (Self::RepresentationSelection, 0) | (Self::WireCompatibilityRequest, 0..=3) => {
                    Some(Role::Data)
                }
                (Self::RepresentationSelection, 1) => Some(Role::Representation),
                (Self::WireCompatibilityRequest, 4..) => Some(Role::WireCompatibilityFact),
                _ => None,
            }
        }

        pub const fn product_operand_count_valid(self, count: usize) -> bool {
            match self {
                Self::NativeBinding | Self::ParameterDescription | Self::FieldDescription => {
                    count == 1
                }
                Self::ProviderSelection
                | Self::RepresentationSelection
                | Self::AssumedBindingContract
                | Self::TestGroupService => count == 2,
                Self::ServiceExclusion
                | Self::ServiceAudit
                | Self::BoundaryAcceptance
                | Self::TestGroupRegistration => count == 1,
                Self::WireCompatibilityRequest => count >= 5,
                _ => false,
            }
        }

        // The checked recording call must agree with the authored request.
        // Keep its ordinary operands beside its name so preparation and
        // validation cannot admit a different Product operation's protocol.
        pub const fn product_recording_operation(
            self,
        ) -> Option<(
            declaration_contracts::build_requests::SelectionProtocolOperation,
            usize,
        )> {
            use declaration_contracts::build_requests::SelectionProtocolOperation as Protocol;
            match self {
                Self::NativeBinding => Some((Protocol::BindNative, 3)),
                Self::ParameterDescription => Some((Protocol::DescribeParameter, 0)),
                Self::FieldDescription => Some((Protocol::DescribeField, 0)),
                Self::ProviderSelection => Some((Protocol::SelectProvider, 3)),
                operation if operation.selects_product_declarations() => {
                    Some((Protocol::Select, 1))
                }
                _ => None,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
    pub struct ProductDeclarationRequest {
        pub source_span: source_text::SourceSpan,
        pub operation: super::BuildOperation,
        pub operands: Box<[ProductDeclarationOperand]>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct ProductDeclarationOperand {
        pub path: String,
        pub source_span: source_text::SourceSpan,
        pub application: Option<std::sync::Arc<super::product_operands::ProductStaticOperandGraph>>,
    }

    impl ProductDeclarationRequest {
        pub fn validate_shape(&self) -> Result<(), String> {
            if !self
                .operation
                .product_operand_count_valid(self.operands.len())
            {
                return Err("Build product request has the wrong declaration operand count".into());
            }
            if self.source_span.span.start >= self.source_span.span.end {
                return Err("Build product request has no authored occurrence".into());
            }
            for (position, operand) in self.operands.iter().enumerate() {
                if operand.path.is_empty()
                    || operand.source_span.span.start >= operand.source_span.span.end
                {
                    return Err("Build product operand has no authored declaration locator".into());
                }
                if self.operation.product_operand_role(position)
                    == Some(ProductOperandRole::WireCompatibilityFact)
                {
                    let fact = declaration_contracts::wire::WireCompatibilityFact::from_name(
                        &operand.path,
                    )
                    .ok_or_else(|| format!("unknown wire compatibility fact `{}`", operand.path))?;
                    if self.operands[..position]
                        .iter()
                        .enumerate()
                        .any(|(index, previous)| {
                            self.operation.product_operand_role(index)
                                == Some(ProductOperandRole::WireCompatibilityFact)
                                && declaration_contracts::wire::WireCompatibilityFact::from_name(
                                    &previous.path,
                                ) == Some(fact)
                        })
                    {
                        return Err(format!(
                            "wire compatibility fact `{}` is requested twice",
                            operand.path
                        ));
                    }
                }
            }
            Ok(())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ExecutedBuildSelection {
        Entry(ExecutedRootBinding),
        Declaration(ExecutedProductSelection),
        Exclusion(ExecutedEnumExclusion),
        Native(ExecutedNativeBinding),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ExecutedProductSelection {
        pub static_environment:
            Option<std::sync::Arc<super::product_operands::ProductStaticEnvironment>>,
        pub target: Option<super::ProductTarget>,
        pub request: std::sync::Arc<ProductDeclarationRequest>,
        pub selecting_machine: Option<source_text::SourceSpan>,
        pub composition: Option<CompositionMode>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ExecutedNativeBinding {
        pub request: std::sync::Arc<ProductDeclarationRequest>,
        pub target: super::ProductTarget,
        pub locator: semantic_vocabulary::NativeBindingLocator,
        pub selecting_machine: source_text::SourceSpan,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct RootBindingRequest {
        pub source_span: source_text::SourceSpan,
        pub slot: String,
        pub implementation: RootBindingOperand,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub enum RootBindingOperand {
        AuthoredPath(String),
        #[default]
        Description,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct DescribedProductEntry {
        pub declaration: source_admission::RetainedDeclaration,
        pub machine_name: String,
        pub slot: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct DescribedProductSchema {
        pub declaration: source_admission::RetainedDeclaration,
        pub canonical_path: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct DescribedProductProvider {
        pub declaration: source_admission::RetainedDeclaration,
        pub canonical_path: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum RootBindingSelection {
        AuthoredPath(String),
        Described(DescribedProductEntry),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ExecutedRootBinding {
        pub source_span: source_text::SourceSpan,
        pub slot: String,
        pub implementation: RootBindingSelection,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct NativeBindingSource {
        pub producer: NativeBindingProducer,
        pub requirement_path: String,
        pub source_occurrence_digest: [u8; 32],
        pub resolution_stratum: source_admission::SourceResolutionStratum,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CapturedNativeIntent {
        pub declaration: u32,
        pub target: ProductTarget,
        pub locator: semantic_vocabulary::NativeBindingLocator,
    }

    #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
    pub struct NativeBindingFormation {
        pub request: std::sync::Arc<ProductDeclarationRequest>,
        pub author: source_text::SourceSpan,
        pub author_package: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub root_scope: bool,
        pub declaration_path: String,
        pub callable_identity: String,
        pub source_occurrence_digest: [u8; 32],
        pub resolution_stratum: source_admission::SourceResolutionStratum,
    }
    impl NativeBindingFormation {
        pub fn seal(
            &self,
            package: semantic_vocabulary::PackageKeyIdentity,
        ) -> Result<NativeBindingSource, String> {
            if self.author_package.is_some_and(|owner| owner != package) || !self.root_scope {
                return Err(
                    "native output cannot substitute another checked Build package's scope".into(),
                );
            }
            let [operand] = self.request.operands.as_ref() else {
                return Err("native formation lost its requirement".into());
            };
            Ok(NativeBindingSource {
                producer: NativeBindingProducer {
                    package,
                    declaration_path: self.declaration_path.clone(),
                    callable_identity: self.callable_identity.clone(),
                },
                requirement_path: operand.path.clone(),
                source_occurrence_digest: self.source_occurrence_digest,
                resolution_stratum: self.resolution_stratum,
            })
        }
    }

    #[cfg(test)]
    mod native_formation_tests {
        use super::*;
        #[test]
        fn native_output_never_crosses_checked_package_scope() {
            let package = semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32]).unwrap();
            let other = semantic_vocabulary::PackageKeyIdentity::from_digest([8; 32]).unwrap();
            let mut formation = NativeBindingFormation {
                request: std::sync::Arc::new(ProductDeclarationRequest {
                    source_span: Default::default(),
                    operation: BuildOperation::NativeBinding,
                    operands: vec![ProductDeclarationOperand {
                        application: None,
                        path: "Raw::read".into(),
                        source_span: Default::default(),
                    }]
                    .into_boxed_slice(),
                }),
                author: Default::default(),
                author_package: Some(package),
                root_scope: true,
                declaration_path: "build".into(),
                callable_identity: "build(@mut Build)".into(),
                source_occurrence_digest: [9; 32],
                resolution_stratum: source_admission::SourceResolutionStratum::Base,
            };
            assert_eq!(formation.seal(package).unwrap().producer.package, package);
            formation.root_scope = false;
            assert!(formation.seal(package).is_err());
            formation.root_scope = true;
            assert!(formation.seal(other).is_err());
            formation.author_package = None;
            assert!(formation.seal(package).is_ok());
            formation.root_scope = false;
            assert!(formation.seal(package).is_err());
        }
    }
}
pub use build_selection_origin::BuildSelectionOrigin;
pub mod product_operands {

    use arena::{Arena, Handle, HandleSpan};
    use source_text::SourceSpan;

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct ProductOperandName {
        pub text: String,
        pub source_span: SourceSpan,
        pub declaration: Option<SourceSpan>,
        pub builtin_type: Option<symbols::BuiltinTypeAtom>,
        pub builtin_function: Option<symbols::BuiltinFunction>,
        pub authored_selection: Handle<ProductOperandSelection>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct ProductStaticOperandGraph {
        pub root: Handle<ProductStaticOperand>,
        pub names: Arena<ProductOperandName>,
        pub operands: Arena<ProductStaticOperand>,
        pub applications: Arena<ProductStaticApplication>,
        pub types: Arena<ProductOperandType>,
        pub expressions: Arena<ProductOperandExpression>,
        pub selections: Arena<ProductOperandSelection>,
        pub constraints: Arena<ProductOperandConstraint>,
        pub fields: Arena<ProductOperandField>,
        pub match_subjects: Arena<ProductMatchSubject>,
        pub match_arms: Arena<ProductMatchArm>,
        pub match_fields: Arena<ProductMatchField>,
        pub proof_selectors: Arena<ProductProofSelector>,
        pub type_arguments: Arena<Handle<ProductOperandType>>,
        pub static_arguments: Arena<Handle<ProductStaticOperand>>,
        pub expression_arguments: Arena<Handle<ProductOperandExpression>>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct ProductStaticOperand {
        pub authored_selection: Handle<ProductOperandSelection>,
        pub path: HandleSpan<ProductOperandName>,
        pub application: Handle<ProductStaticApplication>,
        pub structural_type: Handle<ProductOperandType>,
        pub integer: Option<numeric_literals::IntegerLiteral>,
        pub projection: Option<(Handle<ProductOperandName>, Handle<ProductOperandName>)>,
        pub declaration: Option<SourceSpan>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct ProductStaticApplication {
        pub lifetimes: HandleSpan<ProductOperandName>,
        pub arguments: HandleSpan<Handle<ProductStaticOperand>>,
        pub member: Handle<ProductStaticOperand>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub enum ProductOperandType {
        CanonicalConst(application_identity::const_value::CanonicalConstValue),
        #[default]
        Invalid,
        Unit,
        Named(Handle<ProductOperandName>),
        Generic {
            name: Handle<ProductOperandName>,
            lifetimes: HandleSpan<ProductOperandName>,
            arguments: HandleSpan<Handle<ProductOperandType>>,
        },
        Reference {
            referee: Handle<ProductOperandType>,
            access: language_core::ReferenceAccess,
            lifetime: Option<Handle<ProductOperandName>>,
        },
        Array {
            element: Handle<ProductOperandType>,
            length: ProductArrayLength,
        },
        Slice(Handle<ProductOperandType>),
        Constrained {
            base: Handle<ProductOperandType>,
            constraints: HandleSpan<ProductOperandConstraint>,
        },
        Dynamic {
            name: Handle<ProductOperandName>,
            arguments: HandleSpan<Handle<ProductOperandType>>,
        },
        ConstExpression(Handle<ProductOperandExpression>),
    }

    #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
    pub enum ProductArrayLength {
        Literal(usize),
        Expression(Handle<ProductOperandType>),
        Parameter(Handle<ProductOperandName>),
        Call(Handle<ProductOperandName>),
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub enum ProductOperandConstraint {
        #[default]
        Invalid,
        Named(Handle<ProductOperandName>),

        Arithmetic(numerics::arithmetic::ArithmeticDomain),
        Domain {
            name: Handle<ProductOperandName>,
            arguments: HandleSpan<Handle<ProductOperandType>>,
        },
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct ProductOperandExpression {
        pub source_span: SourceSpan,
        pub authored_selections: HandleSpan<ProductOperandSelection>,
        pub kind: ProductExpressionKind,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct ProductOperandSelection {
        pub source_span: SourceSpan,
        pub exposure: super::AuthoredDeclarationSelectionExposure,
        pub kind: Option<super::AuthoredDeclarationSelectionKind>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct ProductOperandField {
        pub name: Handle<ProductOperandName>,
        pub value: Handle<ProductOperandExpression>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub enum ProductExpressionKind {
        #[default]
        Invalid,
        Match {
            subject: Handle<ProductOperandExpression>,
            capture: Handle<ProductMatchSubject>,
            arms: HandleSpan<ProductMatchArm>,
        },
        SavedSubject {
            component: language_core::PatternComponent,
            capture: Handle<ProductMatchSubject>,
            pattern_head: Handle<ProductOperandExpression>,
        },
        Borrow {
            target: Handle<ProductOperandExpression>,
            access: language_core::ReferenceAccess,
        },
        Atomic {
            value: Handle<ProductOperandExpression>,
            result: Handle<ProductOperandExpression>,
            ordering: language_core::atomic::AtomicOrderingPlan,
            result_custody: language_core::atomic::AtomicExpressionResultCustody,
        },
        Range {
            start: Handle<ProductOperandExpression>,
            end: Handle<ProductOperandExpression>,
            inclusive: bool,
        },
        Membership {
            value: Handle<ProductOperandExpression>,
            domain: HandleSpan<ProductOperandName>,
            declaration: Option<SourceSpan>,
            case_type: Option<SourceSpan>,
            case: Option<SourceSpan>,
        },
        ZeroValue(Handle<ProductOperandType>),
        UndeterminedNaN(numeric_literals::FloatFormat),
        Integer(numeric_literals::IntegerLiteral),
        Boolean(bool),
        Float(numeric_literals::FloatLiteral),
        Text(Vec<u8>),
        Name(HandleSpan<ProductOperandName>),
        Unary {
            operator: ProductUnaryOperator,
            operand: Handle<ProductOperandExpression>,
        },
        Binary {
            left: Handle<ProductOperandExpression>,
            operator: ProductBinaryOperator,
            right: Handle<ProductOperandExpression>,
        },
        Call {
            receiver: Handle<ProductOperandExpression>,
            target: Handle<ProductOperandName>,
            owner: Handle<ProductOperandType>,
            static_arguments: HandleSpan<Handle<ProductStaticOperand>>,
            arguments: HandleSpan<Handle<ProductOperandExpression>>,
            evidence: HandleSpan<ProductOperandName>,
            acknowledgement: language_core::CallOperationalAcknowledgement,
        },
        Member {
            receiver: Handle<ProductOperandExpression>,
            member: Handle<ProductOperandName>,
        },
        Indexed {
            receiver: Handle<ProductOperandExpression>,
            index: Handle<ProductOperandExpression>,
        },
        Array(HandleSpan<Handle<ProductOperandExpression>>),
        Record {
            zero_omitted_fields: bool,
            name: Handle<ProductOperandName>,
            case: Option<Handle<ProductOperandName>>,
            fields: HandleSpan<ProductOperandField>,
        },
        Cast {
            value: Handle<ProductOperandExpression>,
            target: Handle<ProductOperandType>,
            policy: numerics::arithmetic::ArithmeticDomain,
            domain: HandleSpan<ProductOperandName>,
            domain_arguments: HandleSpan<Handle<ProductOperandType>>,
            form: language_core::cast_form::CastForm,
        },
        TypeExpression(Handle<ProductOperandType>),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
    pub enum ProductUnaryOperator {
        BitwiseNot,
        LogicalNot,
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
    pub enum ProductBinaryOperator {
        CaseMembership,
        Add,
        And,
        BitwiseAnd,
        BitwiseOr,
        BitwiseXor,
        Divide,
        Equal,
        Greater,
        GreaterOrEqual,
        Less,
        LessOrEqual,
        Modulo,
        Multiply,
        NotEqual,
        Or,
        ShiftLeft,
        ShiftRight,
        Subtract,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct ProductMatchSubject {
        pub subject: Handle<ProductOperandExpression>,
    }
    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct ProductMatchArm {
        pub pattern: ProductMatchPattern,
        pub value: Handle<ProductOperandExpression>,
        pub source_span: SourceSpan,
    }
    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub enum ProductMatchPattern {
        #[default]
        Wildcard,
        Value(Handle<ProductOperandExpression>),
        Destructure {
            head: Handle<ProductOperandExpression>,
            fields: HandleSpan<ProductMatchField>,
            has_rest: bool,
            guard: Handle<ProductOperandExpression>,
            condition: Handle<ProductOperandExpression>,
            proof_selectors: HandleSpan<ProductProofSelector>,
        },
    }
    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct ProductMatchField {
        pub component: language_core::PatternComponent,
        pub member: Handle<ProductOperandName>,
        pub action: ProductMatchFieldAction,
    }
    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub enum ProductMatchFieldAction {
        Bind(Handle<ProductOperandName>),
        Equal(Handle<ProductOperandExpression>),
        #[default]
        Waive,
    }
    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct ProductProofSelector {
        pub output: Handle<ProductOperandName>,
        pub binding: Handle<ProductOperandName>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct ProductStaticEnvironment {
        pub identity: Vec<u8>,
        pub template: Option<SourceSpan>,
        pub lexical_owner: Option<SourceSpan>,
        pub graph: ProductStaticOperandGraph,
        pub bindings: Arena<ProductStaticEnvironmentBinding>,
        pub lifetimes: Arena<ProductStaticEnvironmentLifetime>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct ProductStaticEnvironmentBinding {
        pub parameter: SourceSpan,
        pub argument: Handle<ProductStaticOperand>,
        pub lexical_owner: SourceSpan,
        pub declared_type: Handle<ProductOperandType>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct ProductStaticEnvironmentLifetime {
        pub parameter_owner: SourceSpan,
        pub parameter_ordinal: u32,
        pub argument: Handle<ProductOperandName>,
        pub lexical_owner: SourceSpan,
    }

    impl ProductStaticOperandGraph {
        pub fn remap_source_spans(&mut self, mut remap: impl FnMut(SourceSpan) -> SourceSpan) {
            self.names.for_each_mut(|_, name| {
                name.source_span = remap(name.source_span);
                name.declaration = name.declaration.map(&mut remap);
            });
            self.operands.for_each_mut(|_, operand| {
                operand.declaration = operand.declaration.map(&mut remap);
            });
            self.expressions.for_each_mut(|_, expression| {
                expression.source_span = remap(expression.source_span);
                if let ProductExpressionKind::Membership {
                    declaration,
                    case_type,
                    case,
                    ..
                } = &mut expression.kind
                {
                    *declaration = declaration.map(&mut remap);
                    *case_type = case_type.map(&mut remap);
                    *case = case.map(&mut remap);
                }
            });
            self.selections
                .for_each_mut(|_, selection| selection.source_span = remap(selection.source_span));
            self.match_arms
                .for_each_mut(|_, arm| arm.source_span = remap(arm.source_span));
        }

        pub fn authored_operand_span(&self, leading: SourceSpan) -> SourceSpan {
            let mut end = leading.span.end;
            let mut include = |span: SourceSpan| {
                if span.source_id == leading.source_id && span.span.start >= leading.span.start {
                    end = end.max(span.span.end);
                }
            };
            for (_, name) in self.names.iter() {
                include(name.source_span);
            }
            for (_, expression) in self.expressions.iter() {
                include(expression.source_span);
            }
            for (_, arm) in self.match_arms.iter() {
                include(arm.source_span);
            }
            SourceSpan::new(
                leading.source_id,
                source_text::Span::new(leading.span.start, end),
            )
        }
    }
}

pub use product::{
    BuildSelectionRequest, DescribedProductEntry, DescribedProductProvider, DescribedProductSchema,
    EnumExclusionRequest, ExclusionVocabulary, ExecutedBuildSelection, ExecutedEnumExclusion,
    ExecutedNativeBinding, ExecutedProductSelection, ExecutedRootBinding,
    ProductDeclarationOperand, ProductDeclarationRequest, ProductOperandRole,
    ProviderReadinessRequest, RootBindingOperand, RootBindingRequest, RootBindingSelection,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, record_storage::Record)]
pub enum AuthoredDeclarationSelectionExposure {
    #[default]
    PrivateImplementation,
    PublicInterface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, record_storage::Record)]
pub enum AuthoredDeclarationSelectionKind {
    TypeReference,
    StaticPathSegment,
    MemberAccess,
    StructLiteralType,
    StructLiteralCase,
    StructLiteralField,
    CaseReference,
    CaseMembership,
    DomainMembership,
    DomainIssuerAuthorization,
    DomainCleanupSelection,
    Call,
    StaticArgument,
    Operator,
    Conformance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub enum AuthoredDeclarationSelectionLateBinding {
    CheckedStaticPathSegment,
    CheckedMember,
    CheckedStructLiteralType,
    CheckedStructLiteralCase,
    CheckedStructLiteralField,
    CheckedCaseMembership,
    CheckedDomainMembership,
    CheckedCall,
    CheckedStaticArgument,
    CheckedOperator,
    CheckedConformance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub struct ResolvedAuthoredDeclarationSelection {
    selected_symbol: SymbolHandle,
}

impl ResolvedAuthoredDeclarationSelection {
    pub fn new(selected_symbol: SymbolHandle) -> Option<Self> {
        selected_symbol
            .is_valid()
            .then_some(Self { selected_symbol })
    }

    pub fn selected_symbol(self) -> SymbolHandle {
        self.selected_symbol
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub enum AuthoredDeclarationSelectionTarget {
    Resolved(ResolvedAuthoredDeclarationSelection),
    Intrinsic(AuthoredDeclarationSelectionIntrinsic),
    LateBound(AuthoredDeclarationSelectionLateBinding),
}

impl AuthoredDeclarationSelectionTarget {
    pub fn as_resolved(&self) -> Option<&ResolvedAuthoredDeclarationSelection> {
        let AuthoredDeclarationSelectionTarget::Resolved(resolved) = self else {
            return None;
        };
        Some(resolved)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub enum CollectionMeasure {
    Length,
    Capacity,
}

impl CollectionMeasure {
    pub const ALL: [Self; 2] = [Self::Length, Self::Capacity];

    pub const fn authored_spelling(self) -> &'static str {
        match self {
            Self::Length => "len",
            Self::Capacity => "capacity",
        }
    }

    pub fn from_authored_spelling(spelling: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|measure| measure.authored_spelling() == spelling)
    }

    pub const fn intrinsic(self) -> AuthoredDeclarationSelectionIntrinsic {
        match self {
            Self::Length => AuthoredDeclarationSelectionIntrinsic::CollectionLength,
            Self::Capacity => AuthoredDeclarationSelectionIntrinsic::CollectionCapacity,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub enum BuildOperation {
    ProviderReadiness,
    ProviderSelection,
    AssumedBindingContract,
    RepresentationSelection,
    ServiceExclusion,
    ServiceAudit,
    OptimizationSelection,
    OptimizationReportRequest,
    BoundaryAcceptance,
    WireCompatibilityRequest,
    IncludedSourceHandoff,
    LogWriteLine,
    NativeBinding,
    // `builder.tests.group<Requirement>()` (wiki/spec/build/testing.md): one
    // exact trait machine requirement registered as a test group.
    TestGroupRegistration,
    // `builder.tests.service<Group, Slot>()` (wiki/spec/build/testing.md): a
    // registered group's runner-supplied backing for one exact service slot.
    TestGroupService,
    ParameterDescription,
    FieldDescription,
}
pub fn is_unresolved_provider_selection_call(
    target: &str,
    target_symbol: symbols::SymbolHandle,
) -> bool {
    BuildOperation::from_call_target(target) == Some(BuildOperation::ProviderSelection)
        && !target_symbol.is_valid()
}

impl BuildOperation {
    /// The operations authored on the Build's `tests` facet
    /// (`builder.tests.group<…>()`, `builder.tests.service<…>()`), which that
    /// facet, not the Build value, authorizes.
    pub const fn uses_tests_facet(self) -> bool {
        matches!(self, Self::TestGroupRegistration | Self::TestGroupService)
    }
    pub const fn has_designated_operands(self) -> bool {
        self.selects_product_declarations() || matches!(self, Self::ProviderReadiness)
    }
    pub const fn selects_product_declarations(self) -> bool {
        matches!(
            self,
            Self::ProviderSelection
                | Self::NativeBinding
                | Self::ServiceExclusion
                | Self::ServiceAudit
                | Self::BoundaryAcceptance
                | Self::WireCompatibilityRequest
                | Self::RepresentationSelection
                | Self::AssumedBindingContract
                | Self::TestGroupRegistration
                | Self::TestGroupService
        )
    }

    pub const ALL: [Self; 15] = [
        Self::ProviderReadiness,
        Self::ProviderSelection,
        Self::AssumedBindingContract,
        Self::RepresentationSelection,
        Self::ServiceExclusion,
        Self::ServiceAudit,
        Self::OptimizationSelection,
        Self::OptimizationReportRequest,
        Self::BoundaryAcceptance,
        Self::WireCompatibilityRequest,
        Self::IncludedSourceHandoff,
        Self::LogWriteLine,
        Self::NativeBinding,
        Self::TestGroupRegistration,
        Self::TestGroupService,
    ];

    pub const fn authored_spelling(self) -> &'static str {
        match self {
            Self::ParameterDescription => "reflect::parameter",
            Self::FieldDescription => "reflect::field",
            Self::NativeBinding => "bind",
            Self::ProviderReadiness => "await_provider",
            Self::ProviderSelection => "select_provider",
            Self::AssumedBindingContract => "assume_binding_contract",
            Self::RepresentationSelection => "select_representation",
            Self::ServiceExclusion => "exclude_service",
            Self::ServiceAudit => "audit_service",
            Self::OptimizationSelection => "enable",
            Self::OptimizationReportRequest => "emit_report",
            Self::BoundaryAcceptance => "accept_boundary",
            Self::WireCompatibilityRequest => "require_wire_compatibility",
            Self::IncludedSourceHandoff => "include_source",
            Self::LogWriteLine => "write_line",
            Self::TestGroupRegistration => "group",
            Self::TestGroupService => "service",
        }
    }

    pub fn from_call_target(target: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|operation| operation.authored_spelling() == target)
    }

    pub const fn intrinsic(self) -> AuthoredDeclarationSelectionIntrinsic {
        match self {
            Self::ParameterDescription => {
                AuthoredDeclarationSelectionIntrinsic::BuildParameterDescription
            }
            Self::FieldDescription => AuthoredDeclarationSelectionIntrinsic::BuildFieldDescription,
            Self::NativeBinding => AuthoredDeclarationSelectionIntrinsic::BuildNativeBinding,
            Self::ProviderReadiness => {
                AuthoredDeclarationSelectionIntrinsic::BuildProviderReadiness
            }
            Self::ProviderSelection => {
                AuthoredDeclarationSelectionIntrinsic::BuildProviderSelection
            }
            Self::AssumedBindingContract => {
                AuthoredDeclarationSelectionIntrinsic::BuildAssumedBindingContract
            }
            Self::RepresentationSelection => {
                AuthoredDeclarationSelectionIntrinsic::BuildRepresentationSelection
            }
            Self::ServiceExclusion => AuthoredDeclarationSelectionIntrinsic::BuildServiceExclusion,
            Self::ServiceAudit => AuthoredDeclarationSelectionIntrinsic::BuildServiceAudit,
            Self::OptimizationSelection => {
                AuthoredDeclarationSelectionIntrinsic::BuildOptimizationSelection
            }
            Self::OptimizationReportRequest => {
                AuthoredDeclarationSelectionIntrinsic::BuildOptimizationReportRequest
            }
            Self::BoundaryAcceptance => {
                AuthoredDeclarationSelectionIntrinsic::BuildBoundaryAcceptance
            }
            Self::WireCompatibilityRequest => {
                AuthoredDeclarationSelectionIntrinsic::BuildWireCompatibilityRequest
            }
            Self::IncludedSourceHandoff => {
                AuthoredDeclarationSelectionIntrinsic::BuildIncludedSourceHandoff
            }
            Self::LogWriteLine => AuthoredDeclarationSelectionIntrinsic::BuildLogWriteLine,
            Self::TestGroupRegistration => {
                AuthoredDeclarationSelectionIntrinsic::BuildTestGroupRegistration
            }
            Self::TestGroupService => AuthoredDeclarationSelectionIntrinsic::BuildTestGroupService,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub enum AuthoredDeclarationSelectionIntrinsic {
    BuildNativeBinding,
    BuildProviderReadiness,
    BuiltinOperator,
    CarryPermission(language_core::CarryPermission),
    CollectionLength,
    CollectionCapacity,
    ByteSequencePredicate(declaration_contracts::byte_predicates::ByteSequencePredicate),
    StorageAddress,
    BuildProductOperand {
        operation: BuildOperation,
        position: usize,
        role: ProductOperandRole,
    },
    BuildProviderSelection,
    BuildAssumedBindingContract,
    BuildRepresentationSelection,
    BuildServiceExclusion,
    BuildServiceAudit,
    BuildTestGroupRegistration,
    BuildTestGroupService,
    BuildOptimizationSelection,
    BuildOptimizationReportRequest,
    BuildBoundaryAcceptance,
    BuildWireCompatibilityRequest,
    WireCompatibilityFact(declaration_contracts::wire::WireCompatibilityFact),
    BuildIncludedSourceHandoff,
    BuildLogWriteLine,
    WireEncode,
    WireDecode,
    InlineAssemblyOperation,
    ProofView,
    QuotientDefine,
    QuotientLift,
    BuildParameterDescription,
    BuildFieldDescription,
    CoreIntegerOperator(declaration_contracts::numeric_intrinsics::CoreIntegerOperatorSignature),
    AtomicCellOperation(language_core::atomic::AtomicCellMember),
}

pub use product::{CapturedNativeIntent, NativeBindingFormation, NativeBindingSource};

#[cfg(test)]
mod tests {
    use super::AuthoredDeclarationSelectionIntrinsic;
    use super::{BuildOperation, CollectionMeasure};
    use source_text::SourceSpan;
    use source_text::{SourceId, Span};

    fn source_span(start: usize, end: usize) -> SourceSpan {
        SourceSpan::new(SourceId(7), Span::new(start, end))
    }

    #[test]
    fn wire_request_formation_names_unknown_and_repeated_facts() {
        let request = |facts: &[&str]| super::ProductDeclarationRequest {
            source_span: source_span(1, 30),
            operation: BuildOperation::WireCompatibilityRequest,
            operands: ["Edge", "Lineage", "Local", "Peer"]
                .into_iter()
                .chain(facts.iter().copied())
                .map(|name| super::ProductDeclarationOperand {
                    path: name.to_owned(),
                    source_span: source_span(2, 5),
                    application: None,
                })
                .collect(),
        };
        assert_eq!(request(&["Readable", "Writable"]).validate_shape(), Ok(()));
        assert_eq!(
            request(&["Imaginary"]).validate_shape(),
            Err("unknown wire compatibility fact `Imaginary`".to_owned())
        );
        assert_eq!(
            request(&["Readable", "Readable"]).validate_shape(),
            Err("wire compatibility fact `Readable` is requested twice".to_owned())
        );
    }

    #[test]
    fn collection_measure_spellings_round_trip() {
        for measure in CollectionMeasure::ALL {
            assert_eq!(
                CollectionMeasure::from_authored_spelling(measure.authored_spelling()),
                Some(measure),
                "{} must select the measure it spells",
                measure.authored_spelling()
            );
        }
    }

    #[test]
    fn collection_measure_spellings_are_the_exact_authored_vocabulary() {
        assert_eq!(
            CollectionMeasure::ALL.map(CollectionMeasure::authored_spelling),
            ["len", "capacity"]
        );
        for spelling in ["length", "Len", "LEN", "as_slice", "bytes", "cap", ""] {
            assert_eq!(
                CollectionMeasure::from_authored_spelling(spelling),
                None,
                "`{spelling}` names no compiler-owned measure"
            );
        }
    }

    #[test]
    fn collection_measures_map_onto_their_intrinsic_selection_targets() {
        assert_eq!(
            CollectionMeasure::Length.intrinsic(),
            AuthoredDeclarationSelectionIntrinsic::CollectionLength
        );
        assert_eq!(
            CollectionMeasure::Capacity.intrinsic(),
            AuthoredDeclarationSelectionIntrinsic::CollectionCapacity
        );
    }

    #[test]
    fn build_operation_spellings_round_trip() {
        for operation in BuildOperation::ALL {
            let target = operation.authored_spelling();
            assert_eq!(
                BuildOperation::from_call_target(target),
                Some(operation),
                "`{target}` must select the operation it spells"
            );
        }
    }

    #[test]
    fn build_operation_spellings_reject_non_operations() {
        assert_eq!(
            BuildOperation::from_call_target("accept_boundary"),
            Some(BuildOperation::BoundaryAcceptance)
        );
        assert_eq!(
            BuildOperation::from_call_target("require_wire_compatibility"),
            Some(BuildOperation::WireCompatibilityRequest)
        );
        for spelling in [
            "accept_boundary#pkg::Console",
            "wire_compatibility",
            "wire_compatibility#Edge#Lineage#Local#Peer#Readable",
            "accept_boundary::Console",
            "select_provider#Console",
            "assume_binding_contract#WritableMemory::claim",
            "enable#Fast",
            "exclude_crash",
            "dependency",
            "build_dependency",
            "as_slice",
            "len",
            "select_providers",
            "Select_Provider",
            "SELECT_PROVIDER",
            "select_provider ",
            "write",
            "write_error_line",
            "",
        ] {
            assert_eq!(
                BuildOperation::from_call_target(spelling),
                None,
                "`{spelling}` names no toolchain-owned build operation"
            );
        }
    }
}
