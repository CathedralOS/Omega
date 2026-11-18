//! Stable declaration, service, effect and selection records carried by Psi.
//! Source interning and declaration preparation belong to source-compiler.
#![forbid(unsafe_code)]

pub(crate) use ::wire::codec_bytes;

mod boundary_bodies {
    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct BoundaryBodySelection {
        pub requirement: String,
        pub requirement_package: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub callable: String,
        pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub provider: String,
        pub provider_package: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub provider_application: Option<String>,
        pub requirement_lifetime_partition: Vec<u32>,
        pub target: Option<String>,
        pub is_product: bool,
    }

    #[derive(Clone, Copy)]
    pub struct CheckedBodyCandidate<'candidate, Body> {
        pub body: Body,
        pub declaration: &'candidate BoundaryBodySelection,
        pub target: Option<&'candidate str>,
        pub is_product: bool,
    }
}
pub mod build_requests {
    // A Build request names the operation structural lowering actually emits.
    // These rows carry source meaning, not host authority; publication binds
    // them to one artifact before an activation may install its adapters.
    pub use semantic_vocabulary::source_selections::BuildCallSite;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum TerminalBuildRequestOperation {
        Package,
        Application,
        Member,
        ArtifactOnly,
        PathDependency,
        GitDependency,
        PrepareDependency,
        LogLine,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalBuildRequestCall {
        pub site: BuildCallSite,
        pub action: TerminalBuildRequestOperation,
    }

    /// The toolchain protocol whose operations carry a Build selection's
    /// designated operands (`Build::bind`, `Build::select_provider`, ...).
    pub const SELECTION_PROTOCOL: &str = "BuildBindingOperations";

    /// One requirement of the selection protocol, as typing chose it for a
    /// generated call. Typing decides the operation from the request it
    /// lowers and records it beside that request; publication and the
    /// Build host read the recorded operation, so the requirement's
    /// declared name is consulted once, here, to find its declaration.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum SelectionProtocolOperation {
        /// Lends the Build value to the activation and returns its ticket.
        Prepare,
        /// Lends the Build's `tests` facet for test-group registration.
        PrepareTests,
        AwaitProvider,
        /// `Build::bind(builder, slot, entry)` with an authored entry path.
        Bind,
        /// `Build::bind(builder, slot, description)` with an entry value.
        BindDescription,
        BindNative,
        Select,
        SelectProvider,
        Exclude,
        DescribeParameter,
        DescribeField,
    }

    impl SelectionProtocolOperation {
        /// The requirement of `BuildBindingOperations` this operation calls.
        pub const fn requirement_name(self) -> &'static str {
            match self {
                Self::Prepare => "prepare",
                Self::PrepareTests => "prepare_tests",
                Self::AwaitProvider => "await_provider",
                Self::Bind => "bind",
                Self::BindDescription => "bind_description",
                Self::BindNative => "bind_native",
                Self::DescribeParameter => "describe_parameter",
                Self::DescribeField => "describe_field",
                Self::Select => "select",
                Self::SelectProvider => "select_provider",
                Self::Exclude => "exclude",
            }
        }

        /// Whether this is the call that lends the Build before a commit.
        pub const fn prepares(self) -> bool {
            matches!(self, Self::Prepare | Self::PrepareTests)
        }
    }
}
pub use boundary_bodies::{BoundaryBodySelection, CheckedBodyCandidate};
pub mod boundary_provider_review {
    use crate::CheckedBodyCandidate;
    use semantic_vocabulary::PackageKeyIdentity;

    mod retained {
        use super::{
            BoundaryApplicationEvidenceError, BoundaryOperatorApplication,
            BoundaryOperatorDeclaration, BoundaryOperatorUse, BoundaryProviderReview,
            BoundaryRequirementReview, BoundaryRequirementUse,
        };
        use crate::{BoundaryBodySelection, CheckedBodyCandidate};
        use semantic_vocabulary::PackageKeyIdentity;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct RetainedBoundaryProviderReview {
            declarations: Vec<OperatorDeclaration>,
            operator_uses: Vec<OperatorUse>,
            requirement_uses: Vec<RequirementUse>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        struct Requirement {
            schema_identity: String,
            package_identity: Option<PackageKeyIdentity>,
            numeric_requirement: Result<String, &'static str>,
        }

        impl Requirement {
            fn retain(value: BoundaryRequirementReview<'_>) -> Self {
                Self {
                    schema_identity: value.schema_identity.to_owned(),
                    package_identity: value.package_identity,
                    numeric_requirement: value.numeric_requirement.map(str::to_owned),
                }
            }

            fn borrow(&self) -> BoundaryRequirementReview<'_> {
                BoundaryRequirementReview {
                    schema_identity: &self.schema_identity,
                    package_identity: self.package_identity,
                    numeric_requirement: self
                        .numeric_requirement
                        .as_deref()
                        .map_err(|reason| *reason),
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        struct Body {
            declaration: BoundaryBodySelection,
            target: Option<String>,
            is_product: bool,
        }

        impl Body {
            fn retain(value: CheckedBodyCandidate<'_, ()>) -> Self {
                Self {
                    declaration: value.declaration.clone(),
                    target: value.target.map(str::to_owned),
                    is_product: value.is_product,
                }
            }

            fn borrow(&self) -> CheckedBodyCandidate<'_, ()> {
                CheckedBodyCandidate {
                    body: (),
                    declaration: &self.declaration,
                    target: self.target.as_deref(),
                    is_product: self.is_product,
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        struct OperatorDeclaration {
            requirement: Requirement,
            bodies: Vec<Body>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        struct OperatorUse {
            requirement: Requirement,
            application: OperatorApplication,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        enum OperatorApplication {
            GenericTemplate,
            Closed(Result<Vec<Body>, BoundaryApplicationEvidenceError>),
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        struct RequirementUse {
            requirement: Requirement,
            diagnostic_name: String,
        }

        impl BoundaryProviderReview<'_> {
            pub fn retain(self) -> RetainedBoundaryProviderReview {
                RetainedBoundaryProviderReview {
                    declarations: self
                        .declarations
                        .into_iter()
                        .map(|value| OperatorDeclaration {
                            requirement: Requirement::retain(value.requirement),
                            bodies: value.bodies.into_iter().map(Body::retain).collect(),
                        })
                        .collect(),
                    operator_uses: self
                        .operator_uses
                        .into_iter()
                        .map(|value| OperatorUse {
                            requirement: Requirement::retain(value.requirement),
                            application: match value.application {
                                BoundaryOperatorApplication::GenericTemplate => {
                                    OperatorApplication::GenericTemplate
                                }
                                BoundaryOperatorApplication::Closed(bodies) => {
                                    OperatorApplication::Closed(bodies.map(|bodies| {
                                        bodies.into_iter().map(Body::retain).collect()
                                    }))
                                }
                            },
                        })
                        .collect(),
                    requirement_uses: self
                        .requirement_uses
                        .into_iter()
                        .map(|value| RequirementUse {
                            requirement: Requirement::retain(value.requirement),
                            diagnostic_name: value.diagnostic_name.to_owned(),
                        })
                        .collect(),
                }
            }
        }

        impl RetainedBoundaryProviderReview {
            pub fn borrow(&self) -> BoundaryProviderReview<'_> {
                BoundaryProviderReview {
                    declarations: self
                        .declarations
                        .iter()
                        .map(|value| BoundaryOperatorDeclaration {
                            requirement: value.requirement.borrow(),
                            bodies: value.bodies.iter().map(Body::borrow).collect(),
                        })
                        .collect(),
                    operator_uses: self
                        .operator_uses
                        .iter()
                        .map(|value| BoundaryOperatorUse {
                            requirement: value.requirement.borrow(),
                            application: match &value.application {
                                OperatorApplication::GenericTemplate => {
                                    BoundaryOperatorApplication::GenericTemplate
                                }
                                OperatorApplication::Closed(bodies) => {
                                    BoundaryOperatorApplication::Closed(
                                        bodies
                                            .as_ref()
                                            .map(|bodies| bodies.iter().map(Body::borrow).collect())
                                            .map_err(|reason| *reason),
                                    )
                                }
                            },
                        })
                        .collect(),
                    requirement_uses: self
                        .requirement_uses
                        .iter()
                        .map(|value| BoundaryRequirementUse {
                            requirement: value.requirement.borrow(),
                            diagnostic_name: &value.diagnostic_name,
                        })
                        .collect(),
                }
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn detached_review_preserves_ambiguous_and_unavailable_evidence() {
                let retained = {
                    let schema = String::from("package::schema");
                    let numeric = String::from("package::Number::add");
                    let diagnostic_name = String::from("ordinary requirement");
                    let declaration = BoundaryBodySelection {
                        requirement: "package::requirement".into(),
                        callable: "package::implementation".into(),
                        provider: "package::Provider".into(),
                        provider_application: Some("Provider<Argument>".into()),
                        requirement_lifetime_partition: vec![1, 0, 1],
                        target: Some("linux_x86_64".into()),
                        is_product: true,
                        ..Default::default()
                    };
                    let body = CheckedBodyCandidate {
                        body: (),
                        declaration: &declaration,
                        target: Some("windows_x86_64"),
                        is_product: false,
                    };
                    let requirement = BoundaryRequirementReview {
                        schema_identity: &schema,
                        package_identity: None,
                        numeric_requirement: Ok(&numeric),
                    };
                    BoundaryProviderReview {
                        declarations: vec![BoundaryOperatorDeclaration {
                            requirement,
                            bodies: vec![body, body],
                        }],
                        operator_uses: vec![
                            BoundaryOperatorUse {
                                requirement,
                                application: BoundaryOperatorApplication::Closed(Ok(vec![
                                    body, body,
                                ])),
                            },
                            BoundaryOperatorUse {
                                requirement,
                                application: BoundaryOperatorApplication::Closed(Err(
                                    BoundaryApplicationEvidenceError::ApplicationCount(2),
                                )),
                            },
                            BoundaryOperatorUse {
                                requirement: BoundaryRequirementReview {
                                    numeric_requirement: Err("numeric declaration unavailable"),
                                    ..requirement
                                },
                                application: BoundaryOperatorApplication::GenericTemplate,
                            },
                        ],
                        requirement_uses: vec![BoundaryRequirementUse {
                            requirement,
                            diagnostic_name: &diagnostic_name,
                        }],
                    }
                    .retain()
                };
                let review = retained.borrow();
                assert_eq!(review.declarations.len(), 1);
                assert_eq!(review.declarations[0].bodies.len(), 2);
                let body = &review.declarations[0].bodies[0];
                assert_eq!(
                    body.declaration.provider_application.as_deref(),
                    Some("Provider<Argument>")
                );
                assert_eq!(body.declaration.requirement_lifetime_partition, [1, 0, 1]);
                assert_eq!(body.declaration.target.as_deref(), Some("linux_x86_64"));
                assert!(body.declaration.is_product);
                assert_eq!(body.target, Some("windows_x86_64"));
                assert!(!body.is_product);
                assert_eq!(review.operator_uses.len(), 3);
                assert!(matches!(&review.operator_uses[0].application,
                    BoundaryOperatorApplication::Closed(Ok(bodies)) if bodies.len() == 2));
                assert!(matches!(
                    review.operator_uses[1].application,
                    BoundaryOperatorApplication::Closed(Err(
                        BoundaryApplicationEvidenceError::ApplicationCount(2)
                    ))
                ));
                assert!(matches!(
                    review.operator_uses[2].application,
                    BoundaryOperatorApplication::GenericTemplate
                ));
                assert_eq!(
                    review.operator_uses[2].requirement.numeric_requirement,
                    Err("numeric declaration unavailable")
                );
                assert_eq!(
                    review.requirement_uses[0].diagnostic_name,
                    "ordinary requirement"
                );
                assert_eq!(
                    review.requirement_uses[0].requirement.schema_identity,
                    "package::schema"
                );
                assert_eq!(
                    review.requirement_uses[0].requirement.numeric_requirement,
                    Ok("package::Number::add")
                );
            }
        }
    }
    pub use retained::RetainedBoundaryProviderReview;

    #[derive(Clone, Copy)]
    pub struct BoundaryRequirementReview<'facts> {
        pub schema_identity: &'facts str,
        pub package_identity: Option<PackageKeyIdentity>,
        pub numeric_requirement: Result<&'facts str, &'static str>,
    }

    pub struct BoundaryOperatorDeclaration<'facts> {
        pub requirement: BoundaryRequirementReview<'facts>,
        pub bodies: Vec<CheckedBodyCandidate<'facts, ()>>,
    }

    pub struct BoundaryOperatorUse<'facts> {
        pub requirement: BoundaryRequirementReview<'facts>,
        pub application: BoundaryOperatorApplication<'facts>,
    }

    pub enum BoundaryOperatorApplication<'facts> {
        GenericTemplate,
        Closed(Result<Vec<CheckedBodyCandidate<'facts, ()>>, BoundaryApplicationEvidenceError>),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum BoundaryApplicationEvidenceError {
        ApplicationCount(usize),
        MissingBodySpan,
        MissingDeclaration,
        DifferentRequirement,
    }

    impl std::fmt::Display for BoundaryApplicationEvidenceError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::ApplicationCount(count) => write!(
                    formatter,
                    "selected checked boundary-operator use retains {count} exact application demands; expected one"
                ),
                Self::MissingBodySpan => {
                    formatter.write_str("operator application lost its checked body span")
                }
                Self::MissingDeclaration => {
                    formatter.write_str("checked operator body lost its declaration")
                }
                Self::DifferentRequirement => {
                    formatter.write_str("checked operator body names another requirement")
                }
            }
        }
    }

    pub struct BoundaryRequirementUse<'facts> {
        pub requirement: BoundaryRequirementReview<'facts>,
        pub diagnostic_name: &'facts str,
    }

    pub struct BoundaryProviderReview<'facts> {
        pub declarations: Vec<BoundaryOperatorDeclaration<'facts>>,
        pub operator_uses: Vec<BoundaryOperatorUse<'facts>>,
        pub requirement_uses: Vec<BoundaryRequirementUse<'facts>>,
    }
}
pub mod byte_predicates {
    pub use semantic_vocabulary::byte_predicates::{
        ByteSequencePredicate, ByteSequencePredicateSet,
    };
}

pub mod content;
mod external_bindings {

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum ExternalBindingMechanism {
        Syscall,
        CompilerIntrinsic,
        VtableSlot,
        VtableField,
        TableFunction,
    }

    impl ExternalBindingMechanism {
        pub const fn as_str(self) -> &'static str {
            match self {
                Self::Syscall => "syscall",
                Self::CompilerIntrinsic => "compiler_intrinsic",
                Self::VtableSlot => "vtable_slot",
                Self::VtableField => "vtable_field",
                Self::TableFunction => "table_function",
            }
        }

        pub const fn identity_tag(self) -> u8 {
            match self {
                Self::Syscall => 2,
                Self::CompilerIntrinsic => 3,
                Self::VtableSlot => 4,
                Self::VtableField => 5,
                Self::TableFunction => 6,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub enum ExternalBindingIdentity {
        Syscall { number: i64 },
        CompilerIntrinsic,
        VtableSlot { index: i64 },
        VtableField { field: String },
        TableFunction { field: String },
    }

    impl ExternalBindingIdentity {
        pub const fn mechanism(&self) -> ExternalBindingMechanism {
            match self {
                Self::Syscall { .. } => ExternalBindingMechanism::Syscall,
                Self::CompilerIntrinsic => ExternalBindingMechanism::CompilerIntrinsic,
                Self::VtableSlot { .. } => ExternalBindingMechanism::VtableSlot,
                Self::VtableField { .. } => ExternalBindingMechanism::VtableField,
                Self::TableFunction { .. } => ExternalBindingMechanism::TableFunction,
            }
        }
    }
}
mod machine_termination {
    use crate::RankingViewId;
    use crate::{ExternalBindingId, ExternalBindingMechanism, SemanticDomainId};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
    pub enum MachineSupplyMode {
        #[default]
        CheckedBody,
        Requirement,
        TopLevelRequirement,
        Boundary,
        AdmissionClaim,
        ExternalRealization {
            binding: Option<ExternalBindingId>,
            mechanism: Option<ExternalBindingMechanism>,
        },
        /// A bodyless declaration supplied by a closed compiler catalog.
        /// Consumers still authenticate its exact custody and complete schema;
        /// this source role does not assert a checked body or authorize supply.
        CompilerProvided,
    }

    impl MachineSupplyMode {
        pub const fn is_boundary_declaration(self) -> bool {
            matches!(
                self,
                Self::TopLevelRequirement | Self::Boundary | Self::AdmissionClaim
            )
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
    pub enum TerminationGuarantee {
        #[default]
        NoGuarantee,
        Terminates {
            premises: Vec<ProgressPremise>,
        },
    }

    impl TerminationGuarantee {
        pub const fn promises_termination(&self) -> bool {
            matches!(self, Self::Terminates { .. })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProgressPremise {
        pub profile: SemanticDomainId,
        pub subject: ProgressSubject,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProgressSubject {
        pub root: symbols::SymbolHandle,
        pub projections: Vec<symbols::SymbolHandle>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
    pub struct RankingWitness {
        pub subjects: Vec<String>,
        pub ranking_view: RankingViewId,
        pub view_path: String,
        pub view_arguments: Vec<String>,
        pub rank_range: Option<RankRange>,
    }

    /// A ranking use retains what name resolution selected. A numeric view
    /// tag alone is normalization metadata, never declaration authority.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
    pub enum RankingViewSelection {
        #[default]
        Unresolved,
        Declaration(symbols::SymbolHandle),
        /// A closed view with no authored declaration, selected only after
        /// ordinary declaration lookup found no shadowing declaration.
        Builtin(RankingViewId),
        /// The subject's typed carrier selected its stable default.
        CanonicalDefault(RankingViewId),
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
    pub struct RankRange {
        pub floor: String,
        pub ceiling: String,
        pub ceiling_inclusive: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
    pub enum TerminationInterface {
        #[default]
        InternalDerived,
        Published(TerminationGuarantee),
    }

    impl TerminationInterface {
        pub fn published(&self) -> Option<&TerminationGuarantee> {
            match self {
                Self::InternalDerived => None,
                Self::Published(guarantee) => Some(guarantee),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
    pub struct MachineTerminationPlan {
        pub interface: TerminationInterface,
        pub checked_summary: TerminationGuarantee,
        pub implementation_witness: Option<RankingWitness>,
    }

    pub const RANKING_VIEW_CORE_SOURCE: &str = "nat.omg";

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct RankingViewDeclaration {
        pub view: RankingViewId,
        pub source: &'static str,
        pub namespace: &'static str,
        pub name: &'static str,
        pub parameter: &'static str,
        pub subject_count: usize,
        pub result: &'static str,
    }

    impl RankingViewDeclaration {
        pub fn path(&self) -> String {
            format!("{}::{}", self.namespace, self.name)
        }
    }

    impl RankingViewId {
        pub const NAT_DESCENDING: Self = Self(1);
        pub const NAT_BOUNDED_DISTANCE: Self = Self(2);
        pub const SLICE_LENGTH: Self = Self(3);
        pub const NAT_INCREASING_TO: Self = Self(4);

        pub fn canonical(path: &str) -> Option<Self> {
            match path {
                "Nat::Descending" => Some(Self::NAT_DESCENDING),
                "Nat::BoundedDistance" => Some(Self::NAT_BOUNDED_DISTANCE),
                "Slice::Length" => Some(Self::SLICE_LENGTH),
                "Nat::IncreasingTo" => Some(Self::NAT_INCREASING_TO),
                _ => None,
            }
        }

        pub fn canonical_path(self) -> Option<&'static str> {
            match self {
                Self::NAT_DESCENDING => Some("Nat::Descending"),
                Self::NAT_BOUNDED_DISTANCE => Some("Nat::BoundedDistance"),
                Self::SLICE_LENGTH => Some("Slice::Length"),
                Self::NAT_INCREASING_TO => Some("Nat::IncreasingTo"),
                _ => None,
            }
        }

        pub fn catalog_declaration(self) -> Option<RankingViewDeclaration> {
            let (name, subject_count) = match self {
                Self::NAT_DESCENDING => ("Descending", 1),
                Self::NAT_BOUNDED_DISTANCE => ("BoundedDistance", 2),
                _ => return None,
            };
            Some(RankingViewDeclaration {
                view: self,
                source: RANKING_VIEW_CORE_SOURCE,
                namespace: "Nat",
                name,
                parameter: "u64",
                subject_count,
                result: "u64",
            })
        }

        pub fn from_catalog_declaration(
            namespace: &str,
            name: &str,
        ) -> Option<RankingViewDeclaration> {
            [
                Self::NAT_DESCENDING,
                Self::NAT_BOUNDED_DISTANCE,
                Self::SLICE_LENGTH,
                Self::NAT_INCREASING_TO,
            ]
            .into_iter()
            .filter_map(Self::catalog_declaration)
            .find(|declaration| declaration.namespace == namespace && declaration.name == name)
        }
    }
}
pub mod numeric_intrinsics;
mod permissions {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, compiler_storage::Record)]
    pub enum PermissionEventKind {
        Establish,
        Restore,
        #[default]
        Transfer,
        Consume,
        AffineDrop,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, compiler_storage::Record)]
    pub enum PermissionAccess {
        #[default]
        Owned,
        Shared,
        Exclusive,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, compiler_storage::Record,
    )]
    pub enum PermissionEventSource<Identity = symbols::SymbolHandle> {
        #[default]
        StateEntry,
        Statement {
            statement_index: usize,
        },
        Call {
            statement_index: usize,
            call_ordinal: usize,
            target_symbol: Identity,
        },
        InstructionDisposition {
            statement_index: usize,
        },
        InstructionExitDisposition {
            statement_index: usize,
        },
        StateExit,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, compiler_storage::Record,
    )]
    pub enum PermissionProvenance<Identity = symbols::SymbolHandle> {
        #[default]
        Unknown,
        Established {
            machine_symbol: Identity,
            state_symbol: Identity,
            source: PermissionEventSource<Identity>,
        },
        Joined {
            machine_symbol: Identity,
            state_symbol: Identity,
            source: PermissionEventSource<Identity>,
            ordinal: u32,
        },
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, compiler_storage::Record,
    )]
    pub enum PermissionClaimIdentity<Identity = symbols::SymbolHandle> {
        #[default]
        Unknown,
        Established {
            machine_symbol: Identity,
            state_symbol: Identity,
            source: PermissionEventSource<Identity>,
            ordinal: u32,
        },
    }

    #[derive(
        Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, compiler_storage::Record,
    )]
    pub struct PermissionArrival<Identity> {
        pub claim_identity: PermissionClaimIdentity<Identity>,
        pub provenance: PermissionProvenance<Identity>,
    }

    impl<Identity> PermissionProvenance<Identity> {
        pub fn map_declarations<Output, Error>(
            self,
            mut map: impl FnMut(Identity) -> Result<Output, Error>,
        ) -> Result<PermissionProvenance<Output>, Error> {
            Ok(match self {
                Self::Unknown => PermissionProvenance::Unknown,
                Self::Established {
                    machine_symbol,
                    state_symbol,
                    source,
                } => PermissionProvenance::Established {
                    machine_symbol: map(machine_symbol)?,
                    state_symbol: map(state_symbol)?,
                    source: source.map_declarations(map)?,
                },
                Self::Joined {
                    machine_symbol,
                    state_symbol,
                    source,
                    ordinal,
                } => PermissionProvenance::Joined {
                    machine_symbol: map(machine_symbol)?,
                    state_symbol: map(state_symbol)?,
                    source: source.map_declarations(map)?,
                    ordinal,
                },
            })
        }
    }

    impl<Identity> PermissionArrival<Identity> {
        pub fn map_declarations<Output, Error>(
            self,
            mut map: impl FnMut(Identity) -> Result<Output, Error>,
        ) -> Result<PermissionArrival<Output>, Error> {
            Ok(PermissionArrival {
                claim_identity: self.claim_identity.map_declarations(&mut map)?,
                provenance: self.provenance.map_declarations(map)?,
            })
        }
    }

    impl<Identity> PermissionEventSource<Identity> {
        pub fn map_declarations<Output, Error>(
            self,
            mut map: impl FnMut(Identity) -> Result<Output, Error>,
        ) -> Result<PermissionEventSource<Output>, Error> {
            Ok(match self {
                Self::StateEntry => PermissionEventSource::StateEntry,
                Self::Statement { statement_index } => {
                    PermissionEventSource::Statement { statement_index }
                }
                Self::Call {
                    statement_index,
                    call_ordinal,
                    target_symbol,
                } => PermissionEventSource::Call {
                    statement_index,
                    call_ordinal,
                    target_symbol: map(target_symbol)?,
                },
                Self::InstructionDisposition { statement_index } => {
                    PermissionEventSource::InstructionDisposition { statement_index }
                }
                Self::InstructionExitDisposition { statement_index } => {
                    PermissionEventSource::InstructionExitDisposition { statement_index }
                }
                Self::StateExit => PermissionEventSource::StateExit,
            })
        }
    }

    impl<Identity> PermissionClaimIdentity<Identity> {
        pub fn map_declarations<Output, Error>(
            self,
            mut map: impl FnMut(Identity) -> Result<Output, Error>,
        ) -> Result<PermissionClaimIdentity<Output>, Error> {
            Ok(match self {
                Self::Unknown => PermissionClaimIdentity::Unknown,
                Self::Established {
                    machine_symbol,
                    state_symbol,
                    source,
                    ordinal,
                } => PermissionClaimIdentity::Established {
                    machine_symbol: map(machine_symbol)?,
                    state_symbol: map(state_symbol)?,
                    source: source.map_declarations(map)?,
                    ordinal,
                },
            })
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, compiler_storage::Record)]
pub enum SuspensionValueOrigin<Identity> {
    Parameter {
        symbol: Identity,
        position: usize,
    },
    Local {
        symbol: Identity,
        statement_index: usize,
        environment_position: usize,
    },
    CallArgument {
        position: usize,
    },
}

impl<Identity> SuspensionValueOrigin<Identity> {
    pub fn map_declarations<Output, Error>(
        self,
        mut map: impl FnMut(Identity) -> Result<Output, Error>,
    ) -> Result<SuspensionValueOrigin<Output>, Error> {
        Ok(match self {
            Self::Parameter { symbol, position } => SuspensionValueOrigin::Parameter {
                symbol: map(symbol)?,
                position,
            },
            Self::Local {
                symbol,
                statement_index,
                environment_position,
            } => SuspensionValueOrigin::Local {
                symbol: map(symbol)?,
                statement_index,
                environment_position,
            },
            Self::CallArgument { position } => SuspensionValueOrigin::CallArgument { position },
        })
    }
}
pub mod provider_declarations {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SelectedTargetMachineOrigin {
        pub machine: symbols::SymbolHandle,
        pub target: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProviderReachDeclaration {
        pub requirement_identity: String,
        pub realization_name: String,
        pub upper_bound: Vec<String>,
        pub concrete_reach: Vec<String>,
        pub dependencies: Vec<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProviderApplicationDeclarations {
        pub has_closed_application: bool,
        pub applications: Result<Vec<String>, String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProviderRowDeclarations {
        pub reach: Result<Option<ProviderReachDeclaration>, String>,
        pub leaf_applications: Result<Vec<ProviderRequirementApplication>, Vec<String>>,
        pub external_applications: ProviderApplicationDeclarations,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProviderDeclarations {
        pub rows: Vec<ProviderRowDeclarations>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProviderRequirementApplication {
        pub requirement: String,
        pub receiving_application: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ClosedProviderInstance<Identity> {
        pub requirement: Identity,
        pub realization: Identity,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProviderInstanceApplication<Identity> {
        pub declaration_application: Result<String, String>,
        pub receiving_application: Result<String, String>,
        pub candidates: Vec<Result<ClosedProviderInstance<Identity>, String>>,
    }
}
pub mod quotient_correspondence {
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub struct QuotientRelationIdentity {
        pub quotient_declaration: String,
        pub quotient_type: String,
        pub carrier_type: String,
        pub relation: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub enum QuotientPositionalRelation {
        Quotient(QuotientRelationIdentity),
        ExactEquality {
            public_type: String,
            representative_type: String,
        },
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum QuotientTheoremRole {
        Congruence,
        ForwardPreconditionTransport,
    }
}
mod semantic_domains {
    use crate::SemanticDomainId;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum DomainSemanticRole {
        DenotationDimension,
        ArithmeticPolicy,
    }

    impl DomainSemanticRole {
        pub const fn as_str(self) -> &'static str {
            match self {
                Self::DenotationDimension => "denotation_dimension",
                Self::ArithmeticPolicy => "arithmetic_policy",
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
    pub struct DomainSemanticRoles {
        pub denotation_dimension: Option<SemanticDomainId>,
        pub arithmetic_policy: Option<SemanticDomainId>,
    }

    impl DomainSemanticRoles {
        pub const fn is_empty(self) -> bool {
            self.denotation_dimension.is_none() && self.arithmetic_policy.is_none()
        }

        pub const fn contribution(self, role: DomainSemanticRole) -> Option<SemanticDomainId> {
            match role {
                DomainSemanticRole::DenotationDimension => self.denotation_dimension,
                DomainSemanticRole::ArithmeticPolicy => self.arithmetic_policy,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum DomainEstablishmentRoute {
        CheckedRequirement {
            trait_definition: symbols::SymbolHandle,
            requirement: symbols::SymbolHandle,
        },
        BoundaryRequirement {
            boundary_trait: symbols::SymbolHandle,
            requirement: symbols::SymbolHandle,
        },
        ExactMachine {
            machine: symbols::SymbolHandle,
        },
    }

    impl DomainEstablishmentRoute {
        pub const fn kind_name(self) -> &'static str {
            match self {
                Self::CheckedRequirement { .. } => "checked_requirement",
                Self::BoundaryRequirement { .. } => "boundary_requirement",
                Self::ExactMachine { .. } => "exact_machine",
            }
        }

        pub const fn source_symbol(self) -> symbols::SymbolHandle {
            match self {
                Self::CheckedRequirement {
                    trait_definition, ..
                } => trait_definition,
                Self::BoundaryRequirement { boundary_trait, .. } => boundary_trait,
                Self::ExactMachine { machine } => machine,
            }
        }

        pub const fn established_declaration(self) -> symbols::SymbolHandle {
            match self {
                Self::CheckedRequirement { requirement, .. }
                | Self::BoundaryRequirement { requirement, .. } => requirement,
                Self::ExactMachine { machine } => machine,
            }
        }

        pub const fn requirement_symbol(self) -> symbols::SymbolHandle {
            match self {
                Self::CheckedRequirement { requirement, .. }
                | Self::BoundaryRequirement { requirement, .. } => requirement,
                Self::ExactMachine { .. } => symbols::SymbolHandle::invalid(),
            }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct BoundaryQualificationIssuer {
        pub requirement_identity: String,
        pub boundary_path: String,
        pub boundary_package: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub is_product_owner: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
    pub enum QualificationEvidenceOrigin {
        #[default]
        None,
        Prover,
        CheckedValidation,
        AuthorizedRouteEstablishment,
        CheckedTransformation,
        AdmittedReceipt,
        Propagated,
        VacuousQualification,
    }

    impl QualificationEvidenceOrigin {
        pub const fn as_str(self) -> &'static str {
            match self {
                Self::None => "none",
                Self::Prover => "prover",
                Self::CheckedValidation => "checked_validation",
                Self::AuthorizedRouteEstablishment => "authorized_route_establishment",
                Self::CheckedTransformation => "checked_transformation",
                Self::AdmittedReceipt => "admitted_receipt",
                Self::Propagated => "propagated",
                Self::VacuousQualification => "vacuous_qualification",
            }
        }
    }
}
mod semantic_identities {
    macro_rules! semantic_id {
        ($(#[$doc:meta])* $name:ident) => {
            $(#[$doc])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, compiler_storage::Record)]
            pub struct $name(pub u32);

            impl $name {
                pub const NULL: Self = Self(0);

                pub fn is_valid(self) -> bool {
                    self.0 != 0
                }
            }
        };
    }

    semantic_id!(SemanticDomainId);
    semantic_id!(ServiceReachId);
    semantic_id!(ServiceReachRowId);
    semantic_id!(ExternalBindingId);
    semantic_id!(RankingViewId);
}
mod service_reach {
    use crate::ServiceReachRowId;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
    pub enum ServiceReachInterface {
        #[default]
        InternalInferred,
        PublishedCeiling(ServiceReachRowId),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
    pub struct ServiceReachPlan {
        pub interface: ServiceReachInterface,
        pub checked_inferred: ServiceReachRowId,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
    pub enum SynchronousInvocationInterface {
        #[default]
        InternalInferred,
        PublishedCeiling,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct SynchronousInvocationPlan {
        pub interface: SynchronousInvocationInterface,
        pub published: Vec<String>,
        pub checked_inferred: Vec<String>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
    pub enum SuspensionInterface {
        #[default]
        InternalInferred,
        PublishedMaySuspend(bool),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
    pub struct SuspensionPlan {
        pub interface: SuspensionInterface,
        pub checked_may_suspend: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
    pub enum BlockingInterface {
        #[default]
        InternalInferred,
        PublishedMayBlock(bool),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
    pub struct BlockingPlan {
        pub interface: BlockingInterface,
        pub checked_may_block: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
    pub struct ServiceReachSummary {
        pub direct: ServiceReachRowId,
        pub transitive: ServiceReachRowId,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
    pub struct SuspensionSummary {
        pub direct_may_suspend: bool,
        pub transitive_may_suspend: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
    pub struct BlockingSummary {
        pub direct_may_block: bool,
        pub transitive_may_block: bool,
    }
}
pub mod service_surface;
pub mod trust_declarations {
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TrustDeclarationCardinality {
        Missing,
        Duplicate,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AcceptedTemplateDeclaration {
        pub report_fingerprint: u64,
        pub commitment: [u8; 32],
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AcceptedContractDeclaration {
        pub report_fingerprint: u64,
        pub commitment: [u8; 32],
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AcceptedMachineDeclaration {
        pub machine: SymbolHandle,
        pub declaration_identity: String,
        pub package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub name: String,
        pub template: Result<Option<AcceptedTemplateDeclaration>, TrustDeclarationCardinality>,
        pub contract: Result<AcceptedContractDeclaration, TrustDeclarationCardinality>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct TrustDeclarations {
        pub machines: Box<[AcceptedMachineDeclaration]>,
    }
}
pub mod value_domain {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub enum ValueDomain {
        Finite,
    }

    impl ValueDomain {
        pub fn from_name(name: &str) -> Option<Self> {
            match name {
                "Finite" => Some(Self::Finite),
                _ => None,
            }
        }

        pub fn name(self) -> &'static str {
            match self {
                Self::Finite => "Finite",
            }
        }
    }
}
pub mod wire {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub enum WireCompatibilityFact {
        Readable,
        Writable,
        PreserveUnknown,
        Canonical,
        CompleteMigration,
    }

    impl WireCompatibilityFact {
        pub fn from_name(name: &str) -> Option<Self> {
            match name {
                "Readable" => Some(Self::Readable),
                "Writable" => Some(Self::Writable),
                "PreserveUnknown" => Some(Self::PreserveUnknown),
                "Canonical" => Some(Self::Canonical),
                "CompleteMigration" => Some(Self::CompleteMigration),
                _ => None,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct WireCompatibilityDemand {
        pub edge: String,
        pub lineage: String,
        pub local_schema: String,
        pub peer_schema: String,
        pub require_readable: bool,
        pub require_writable: bool,
        pub require_unknown_preservation: bool,
        pub require_canonical: bool,
        pub require_complete_migration: bool,
    }
}

pub use external_bindings::{ExternalBindingIdentity, ExternalBindingMechanism};
pub use language_core::{
    CallOperationalAcknowledgement, CallOperationalAcknowledgementOrigin, CarryAddress, CarryCpu,
    CarryHostThread, CarryPermission, CarryPolicy, CarrySuspension, DataSupplyMode,
    DomainClassification, DomainPredicatePresence, Multiplicity, ReferenceAccess,
};
pub use machine_termination::{
    MachineSupplyMode, MachineTerminationPlan, ProgressPremise, ProgressSubject,
    RANKING_VIEW_CORE_SOURCE, RankRange, RankingViewDeclaration, RankingViewSelection,
    RankingWitness, TerminationGuarantee, TerminationInterface,
};
pub use permissions::{
    PermissionAccess, PermissionArrival, PermissionClaimIdentity as ScopedPermissionClaimIdentity,
    PermissionEventKind, PermissionEventSource as ScopedPermissionEventSource,
    PermissionProvenance as ScopedPermissionProvenance,
};
pub type PermissionProvenance = ScopedPermissionProvenance<symbols::SymbolHandle>;
pub type PermissionClaimIdentity = ScopedPermissionClaimIdentity<symbols::SymbolHandle>;
pub type PermissionEventSource = ScopedPermissionEventSource<symbols::SymbolHandle>;
pub use semantic_domains::{
    BoundaryQualificationIssuer, DomainEstablishmentRoute, DomainSemanticRole, DomainSemanticRoles,
    QualificationEvidenceOrigin,
};
pub use semantic_identities::{
    ExternalBindingId, RankingViewId, SemanticDomainId, ServiceReachId, ServiceReachRowId,
};
pub use service_reach::{
    BlockingInterface, BlockingPlan, BlockingSummary, ServiceReachInterface, ServiceReachPlan,
    ServiceReachSummary, SuspensionInterface, SuspensionPlan, SuspensionSummary,
    SynchronousInvocationInterface, SynchronousInvocationPlan,
};

#[cfg(test)]
mod tests;

pub mod selection;

pub mod entry_contract_identity;
pub mod source_identity;

pub mod build_stage;
