//! Stable declaration, service, effect and selection records carried by Psi.
//! Source interning and declaration preparation belong to source-compiler.
#![forbid(unsafe_code)]

pub(crate) use ::wire::codec_bytes;

mod boundary_bodies {
    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
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

    #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
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

    #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
    pub struct TerminalBuildRequestCall {
        pub site: BuildCallSite,
        pub action: TerminalBuildRequestOperation,
    }

    pub use language_core::build_protocol::{SELECTION_PROTOCOL, SelectionProtocolOperation};
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
mod permissions {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, record_storage::Record)]
    pub enum PermissionEventKind {
        Establish,
        Restore,
        #[default]
        Transfer,
        Consume,
        AffineDrop,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, record_storage::Record)]
    pub enum PermissionAccess {
        #[default]
        Owned,
        Shared,
        Exclusive,
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
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record,
    )]
    pub enum QuotientTheoremRole {
        Congruence,
        ForwardPreconditionTransport,
    }
}
mod semantic_domains {
    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct BoundaryQualificationIssuer {
        pub requirement_identity: String,
        pub boundary_path: String,
        pub boundary_package: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub is_product_owner: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, record_storage::Record)]
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
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, record_storage::Record)]
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
    pub use semantic_vocabulary::source_selections::WireCompatibilityFact;

    #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
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

pub use language_core::{
    CallOperationalAcknowledgement, CallOperationalAcknowledgementOrigin, CarryAddress, CarryCpu,
    CarryHostThread, CarryPermission, CarryPolicy, CarrySuspension, DataSupplyMode,
    DomainClassification, DomainPredicatePresence, Multiplicity, ReferenceAccess,
};
pub use permissions::{PermissionAccess, PermissionEventKind};
pub use psi_effect_contracts::{PermissionArrival, SuspensionValueOrigin};
pub type ScopedPermissionEventSource<Identity = symbols::SymbolHandle> =
    psi_effect_contracts::ScopedPermissionEventSource<Identity>;
pub type ScopedPermissionProvenance<Identity = symbols::SymbolHandle> =
    psi_effect_contracts::ScopedPermissionProvenance<Identity>;
pub type ScopedPermissionClaimIdentity<Identity = symbols::SymbolHandle> =
    psi_effect_contracts::ScopedPermissionClaimIdentity<Identity>;
pub use semantic_vocabulary::{ExternalBindingIdentity, ExternalBindingMechanism, RANKING_VIEW_CORE_SOURCE, RankingViewDeclaration};
pub type PermissionProvenance = ScopedPermissionProvenance<symbols::SymbolHandle>;
pub type PermissionClaimIdentity = ScopedPermissionClaimIdentity<symbols::SymbolHandle>;
pub type PermissionEventSource = ScopedPermissionEventSource<symbols::SymbolHandle>;
pub use semantic_domains::{BoundaryQualificationIssuer, QualificationEvidenceOrigin};
pub use semantic_vocabulary::{
    BlockingInterface, BlockingPlan, BlockingSummary, ServiceReachInterface, ServiceReachPlan,
    ServiceReachSummary, SuspensionInterface, SuspensionPlan, SuspensionSummary,
    SynchronousInvocationInterface, SynchronousInvocationPlan,
};
pub use semantic_vocabulary::{
    ExternalBindingId, RankingViewId, SemanticDomainId, ServiceReachId, ServiceReachRowId,
};

#[cfg(test)]
mod tests;

pub mod selection;

pub mod entry_contract_identity {
    pub use source_provenance::entry_contract_digest as digest;
}
pub use source_provenance as source_identity;
