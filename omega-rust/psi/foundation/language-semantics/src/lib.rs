/*
We define the target-neutral semantic identities, declaration selection ledgers,
type identities, constant values, content conservation rules, numeric
intrinsics, and boundary/service vocabularies shared by symbol-resolved, typed,
and checked Psi representations.
*/

/*
Resolved and typed Psi stages need one canonical representation for interned
semantic identities (`semantic_identities`), build-selected declarations
(`declaration_selection`), structural and nominal type identities
(`type_identity`), compile-time constant values (`const_value`), and operational
service reach (`service_reach`, `service_surface`) without pulling in checking
passes or Omega backend policy. Centralizing these tables and plans keeps
cross-stage handoffs lossless and deterministic.
*/

/*
`lib.rs` hosts the core identity, permission, termination, domain, service-reach,
and boundary modules directly, while `const_value.rs`, `content.rs`,
`declaration_selection.rs`, `numeric_intrinsics.rs`, `service_surface.rs`, and
`type_identity.rs` own the larger domain vocabularies.
*/

#![forbid(unsafe_code)]

pub(crate) use optimization::codec_bytes;
pub mod boundary_values {
    use arena::{Arena, Handle, HandleSpan};

    #[derive(Default)]
    pub struct BoundaryValueRecipes {
        pub values: Arena<BoundaryValueRecipe>,
        pub fields: Arena<BoundaryFieldRecipe>,
        pub targets: Arena<BoundaryTargetRecipe>,
    }

    #[derive(Default)]
    pub enum BoundaryValueRecipe {
        #[default]
        Absent,
        Rejected(String),
        Integer {
            byte_size: u16,
        },
        Float {
            byte_size: u16,
        },
        Reference {
            descriptor: bool,
        },
        FixedArray {
            element: Handle<Self>,
            length: u64,
        },
        Record {
            name: String,
            fields: HandleSpan<BoundaryFieldRecipe>,
            storage: Option<BoundaryRecordStorage>,
        },
        TargetLayouts {
            name: String,
            alternatives: HandleSpan<BoundaryTargetRecipe>,
        },
        Opaque {
            name: String,
            carrier_name: String,
            selection_ordinal: usize,
            carrier: Handle<Self>,
        },
    }

    #[derive(Default)]
    pub struct BoundaryFieldRecipe {
        pub value: Handle<BoundaryValueRecipe>,
        pub byte_offset: Option<u64>,
    }

    pub struct BoundaryRecordStorage {
        pub byte_size: u64,
        pub byte_alignment: u64,
    }

    #[derive(Default)]
    pub struct BoundaryTargetRecipe {
        pub target: String,
        pub value: Handle<BoundaryValueRecipe>,
    }
}
pub mod callback_declarations {
    use semantic_vocabulary::{CallbackRequirementId, NativeParameterId, StaticMachineBinderId};

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct PrivateCallbackSlotSelection {
        pub declaration_identity: String,
        pub trait_identity: String,
        pub semantic_identity: String,
        pub layout_identity: String,
        pub requirement_identity: String,
        pub application_report_fingerprint: u64,
        pub application_commitment: [u8; 32],
        pub application_encoding: std::sync::Arc<[u8]>,
    }

    impl PrivateCallbackSlotSelection {
        pub fn has_complete_identity(&self) -> bool {
            !self.declaration_identity.is_empty()
                && !self.trait_identity.is_empty()
                && !self.semantic_identity.is_empty()
                && !self.layout_identity.is_empty()
                && !self.requirement_identity.is_empty()
                && self.application_commitment != [0; 32]
                && !self.application_encoding.is_empty()
        }

        pub fn validate_identity_encoding(&self) -> Result<(), String> {
            if !self.has_complete_identity() {
                return Err(
                    "private callback slot lost its exact selected application identity".into(),
                );
            }
            let (report, commitment) = crate::conformance_identity::source_identity_commitments(
                &self.application_encoding,
            );
            if report != self.application_report_fingerprint
                || commitment != self.application_commitment
            {
                return Err("private callback slot changed its Source-v4 identity preimage".into());
            }
            Ok(())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum CallbackRegistrationDeclaration {
        CheckedSource,
        External(String),
    }

    pub struct CallbackDeclaration<'usage> {
        pub registration: Result<CallbackRegistrationDeclaration, String>,
        pub satisfaction_is_boundary: bool,
        pub has_checked_entry: bool,
        pub requirement_overload: &'usage str,
    }

    pub const BOUNDARY_NATIVE_PARAMETER_CAPACITY: usize = 32;

    pub const BOUNDARY_CALLBACK_CAPACITY: usize = 64;

    #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
    pub struct BoundaryTelescope {
        pub parameters: Vec<BoundaryRuntimeFormal>,
        pub binders: Vec<BoundaryCallbackBinderDeclaration>,
        pub direct_callbacks: Vec<BoundaryDirectCallbackDeclaration>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct BoundaryRuntimeFormal {
        pub identity: semantic_vocabulary::NativeParameterId,
        pub native_ordinal: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct BoundaryCallbackBinderDeclaration {
        pub binder: semantic_vocabulary::StaticMachineBinderId,
        pub requirement: semantic_vocabulary::CallbackRequirementId,
        pub static_machine_ordinal: u32,
        pub requirement_identity: String,
        pub invokes_synchronously: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct BoundaryDirectCallbackDeclaration {
        pub name: String,
        pub identity: semantic_vocabulary::NativeParameterId,
        pub native_ordinal: u32,
        pub binder: semantic_vocabulary::StaticMachineBinderId,
        pub requirement: semantic_vocabulary::CallbackRequirementId,
    }

    impl BoundaryDirectCallbackDeclaration {
        pub fn name(&self) -> &str {
            &self.name
        }

        pub const fn identity(&self) -> NativeParameterId {
            self.identity
        }

        pub const fn native_ordinal(&self) -> u32 {
            self.native_ordinal
        }

        pub const fn binder(&self) -> StaticMachineBinderId {
            self.binder
        }

        pub const fn requirement(&self) -> CallbackRequirementId {
            self.requirement
        }
    }
}
pub mod opaque_representations {
    use optimization::digest_encoder::DomainDigestEncoder;

    pub const OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION: u16 = 1;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum OpaqueRepresentationLifecycleDisposition {
        Inert,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum OpaqueRepresentationCopyDisposition {
        PlacementOnly,
        CheckedSemanticCopy,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum OpaqueRepresentationApplicationOrigin {
        NamedConformance,
    }

    pub fn selected_application_commitment(
        conformance_application_commitment: [u8; 32],
        lifecycle: OpaqueRepresentationLifecycleDisposition,
        copy_disposition: OpaqueRepresentationCopyDisposition,
        origin: OpaqueRepresentationApplicationOrigin,
    ) -> [u8; 32] {
        let mut digest = DomainDigestEncoder::new(b"omega.opaque-representation-application.v1");
        digest.u16(OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION);
        digest.byte(match origin {
            OpaqueRepresentationApplicationOrigin::NamedConformance => 1,
        });
        digest.byte(match lifecycle {
            OpaqueRepresentationLifecycleDisposition::Inert => 1,
        });
        digest.byte(match copy_disposition {
            OpaqueRepresentationCopyDisposition::PlacementOnly => 1,
            OpaqueRepresentationCopyDisposition::CheckedSemanticCopy => 2,
        });
        digest.raw(&conformance_application_commitment);
        digest.finish()
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct SelectedOpaqueApplication {
        pub opaque_identity: String,
        pub carrier_identity: String,
        pub conformance_identity: String,
        pub application_report_fingerprint: u64,
        pub conformance_application_commitment: [u8; 32],
        pub identity_encoding: Vec<u8>,
        pub schema_version: u16,
        pub origin: OpaqueRepresentationApplicationOrigin,
        pub lifecycle: OpaqueRepresentationLifecycleDisposition,
        pub copy_disposition: OpaqueRepresentationCopyDisposition,
        pub selected_application_commitment: [u8; 32],
    }

    impl SelectedOpaqueApplication {
        pub fn validate_identity_encoding(&self) -> Result<(), String> {
            if self.opaque_identity.is_empty()
                || self.carrier_identity.is_empty()
                || self.conformance_identity.is_empty()
                || self.identity_encoding.is_empty()
                || self.schema_version != OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION
            {
                return Err("opaque representation lost its complete selected application".into());
            }
            let (report, commitment) =
                crate::conformance_identity::source_identity_commitments(&self.identity_encoding);
            if report != self.application_report_fingerprint
                || commitment != self.conformance_application_commitment
            {
                return Err("opaque representation changed its Source-v4 identity preimage".into());
            }
            if selected_application_commitment(
                commitment,
                self.lifecycle,
                self.copy_disposition,
                self.origin,
            ) != self.selected_application_commitment
            {
                return Err(
                    "opaque representation changed its complete selected-application commitment"
                        .into(),
                );
            }
            Ok(())
        }
    }
}

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
    use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum TerminalBuildRequestOperation {
        Package,
        Application,
        Member,
        ArtifactOnly,
        PathDependency,
        GitDependency,
        LogLine,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalBuildRequestCall {
        pub machine: MachineId,
        pub operation: OperationId,
        pub boundary: BoundaryMachineId,
        pub action: TerminalBuildRequestOperation,
    }

    pub fn is_project_request_protocol(
        symbols: &symbols::SymbolTable,
        declaration: symbols::SymbolHandle,
    ) -> bool {
        is_build_protocol(symbols, declaration, "BuildRequestOperations")
    }

    /// The toolchain protocol whose operations carry a Build selection's
    /// designated operands (`Build::bind`, `Build::select_provider`, ...).
    pub const SELECTION_PROTOCOL: &str = "BuildBindingOperations";

    pub fn is_build_protocol(
        symbols: &symbols::SymbolTable,
        declaration: symbols::SymbolHandle,
        name: &str,
    ) -> bool {
        if !declaration.is_valid() || symbols.name(declaration) != name {
            return false;
        }
        symbols
            .symbol_source_span(declaration)
            .and_then(|span| symbols.source_file(span))
            .is_some_and(|source| {
                source.origin == source::SourceOrigin::Toolchain
                    && source.path == std::path::Path::new("<build-prelude>")
            })
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
pub mod conformance_identity {
    use optimization::digest_encoder::DomainDigestEncoder;
    use semantic_vocabulary::Fnv1a;

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct ConformanceIdentityDescription<'identity> {
        pub declaration: &'identity str,
        pub trait_name: &'identity str,
        pub lifetime_arguments: &'identity [String],
        pub type_arguments: &'identity [String],
        pub trait_lifetime_arguments: &'identity [String],
        pub trait_arguments: &'identity [String],
        pub const_arguments: Vec<ConformanceConstIdentity<'identity>>,
        pub machine_arguments: Vec<String>,
        pub machine_operands: Vec<MachineOperandIdentity>,
        pub rows: Vec<ConformanceRowIdentity<'identity>>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ConformanceConstIdentity<'identity> {
        Evaluated {
            parameter_carrier: String,
            declared_carrier: String,
            value_type: &'identity str,
            value_encoding: &'identity str,
        },
        CallerBinder {
            parameter_carrier: String,
            binder_carrier: String,
            binder_identity: String,
        },
        CallerExpression {
            parameter_carrier: String,
            expression_identity: String,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MachineOperandIdentity {
        pub selection: MachineOperandSelectionIdentity,
        pub application: Option<MachineOperandApplicationIdentity>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum MachineOperandSelectionIdentity {
        Const(String),
        Type(String),
        Conformance(String),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MachineOperandApplicationIdentity {
        pub lifetime_arguments: Vec<String>,
        pub arguments: Vec<MachineOperandIdentity>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ConformanceRowIdentity<'identity> {
        pub declaring_trait: &'identity str,
        pub requirement: &'identity str,
        pub realization: &'identity str,
        pub requirement_signature: Option<Vec<u8>>,
    }

    impl ConformanceIdentityDescription<'_> {
        pub fn encoded_bytes(&self) -> Vec<u8> {
            let mut bytes = Vec::new();
            for singleton in [self.declaration, self.trait_name] {
                bytes.extend(1u64.to_le_bytes());
                append_text(&mut bytes, singleton);
                bytes.push(0xff);
            }
            for lane in [
                self.lifetime_arguments,
                self.type_arguments,
                self.trait_lifetime_arguments,
                self.trait_arguments,
            ] {
                bytes.extend((lane.len() as u64).to_le_bytes());
                for item in lane {
                    append_text(&mut bytes, item);
                }
                bytes.push(0xff);
            }
            bytes.extend((self.const_arguments.len() as u64).to_le_bytes());
            for argument in &self.const_arguments {
                match argument {
                    ConformanceConstIdentity::Evaluated {
                        parameter_carrier,
                        declared_carrier,
                        value_type,
                        value_encoding,
                    } => {
                        bytes.push(0);
                        for identity in [
                            parameter_carrier.as_str(),
                            declared_carrier.as_str(),
                            value_type,
                            value_encoding,
                        ] {
                            append_text(&mut bytes, identity);
                        }
                    }
                    ConformanceConstIdentity::CallerBinder {
                        parameter_carrier,
                        binder_carrier,
                        binder_identity,
                    } => {
                        bytes.push(1);
                        for identity in [parameter_carrier, binder_carrier, binder_identity] {
                            append_text(&mut bytes, identity);
                        }
                    }
                    ConformanceConstIdentity::CallerExpression {
                        parameter_carrier,
                        expression_identity,
                    } => {
                        bytes.push(2);
                        append_text(&mut bytes, parameter_carrier);
                        append_text(&mut bytes, expression_identity);
                    }
                }
                bytes.push(0xfd);
            }
            for machine in &self.machine_arguments {
                append_text(&mut bytes, machine);
            }
            bytes.extend((self.machine_operands.len() as u64).to_le_bytes());
            for operand in &self.machine_operands {
                operand.encode(&mut bytes);
            }
            for row in &self.rows {
                for identity in [row.declaring_trait, row.requirement, row.realization] {
                    append_text(&mut bytes, identity);
                }
                match &row.requirement_signature {
                    Some(signature) => {
                        bytes.push(1);
                        bytes.extend((signature.len() as u64).to_le_bytes());
                        bytes.extend(signature);
                    }
                    None => bytes.push(0),
                }
                bytes.push(0xfe);
            }
            bytes
        }

        pub fn fingerprint_and_commitment(&self) -> (u64, [u8; 32]) {
            let bytes = self.encoded_bytes();
            source_identity_commitments(&bytes)
        }
    }

    pub fn source_identity_commitments(bytes: &[u8]) -> (u64, [u8; 32]) {
        let mut report = Fnv1a::new();
        report.bytes(bytes);
        let mut commitment =
            DomainDigestEncoder::new(b"omega.psi.closed-conformance-application.v4\0");
        commitment.raw(bytes);
        (report.finish(), commitment.finish())
    }

    impl MachineOperandIdentity {
        fn encode(&self, bytes: &mut Vec<u8>) {
            let (tag, identity) = match &self.selection {
                MachineOperandSelectionIdentity::Const(identity) => (1, identity),
                MachineOperandSelectionIdentity::Type(identity) => (2, identity),
                MachineOperandSelectionIdentity::Conformance(identity) => (3, identity),
            };
            bytes.push(tag);
            append_text(bytes, identity);
            match &self.application {
                Some(application) => {
                    bytes.push(1);
                    bytes.extend((application.lifetime_arguments.len() as u64).to_le_bytes());
                    for lifetime in &application.lifetime_arguments {
                        append_text(bytes, lifetime);
                    }
                    bytes.extend((application.arguments.len() as u64).to_le_bytes());
                    for argument in &application.arguments {
                        argument.encode(bytes);
                    }
                }
                None => bytes.push(0),
            }
        }
    }

    fn append_text(bytes: &mut Vec<u8>, text: &str) {
        bytes.extend((text.len() as u64).to_le_bytes());
        bytes.extend(text.as_bytes());
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn description<'identity>(
            lanes: &'identity [Vec<String>; 4],
        ) -> ConformanceIdentityDescription<'identity> {
            ConformanceIdentityDescription {
                declaration: "Pick",
                trait_name: "Slot",
                lifetime_arguments: &lanes[0],
                type_arguments: &lanes[1],
                trait_lifetime_arguments: &lanes[2],
                trait_arguments: &lanes[3],
                const_arguments: vec![
                    ConformanceConstIdentity::Evaluated {
                        parameter_carrier: "u64".into(),
                        declared_carrier: "u32".into(),
                        value_type: "u32",
                        value_encoding: "7",
                    },
                    ConformanceConstIdentity::CallerBinder {
                        parameter_carrier: "u64".into(),
                        binder_carrier: "u64".into(),
                        binder_identity: "Owner::N".into(),
                    },
                    ConformanceConstIdentity::CallerExpression {
                        parameter_carrier: "u64".into(),
                        expression_identity: "add(N,1)".into(),
                    },
                ],
                machine_arguments: vec!["Handler::call".into(), "Other::entry".into()],
                machine_operands: vec![MachineOperandIdentity {
                    selection: MachineOperandSelectionIdentity::Conformance("pkg::Dispatch".into()),
                    application: Some(MachineOperandApplicationIdentity {
                        lifetime_arguments: vec!["'a".into()],
                        arguments: vec![
                            MachineOperandIdentity {
                                selection: MachineOperandSelectionIdentity::Type("u8".into()),
                                application: None,
                            },
                            MachineOperandIdentity {
                                selection: MachineOperandSelectionIdentity::Const("u64:7".into()),
                                application: None,
                            },
                            MachineOperandIdentity {
                                selection: MachineOperandSelectionIdentity::Conformance(
                                    "pkg::Inner".into(),
                                ),
                                application: Some(MachineOperandApplicationIdentity {
                                    lifetime_arguments: Vec::new(),
                                    arguments: Vec::new(),
                                }),
                            },
                        ],
                    }),
                }],
                rows: vec![
                    ConformanceRowIdentity {
                        declaring_trait: "Slot",
                        requirement: "invoke",
                        realization: "handler",
                        requirement_signature: Some(vec![0, 255, 254]),
                    },
                    ConformanceRowIdentity {
                        declaring_trait: "Slot",
                        requirement: "empty",
                        realization: "none",
                        requirement_signature: None,
                    },
                ],
            }
        }

        fn lanes() -> [Vec<String>; 4] {
            [
                vec!["'a".into(), "'b".into()],
                vec!["Pair<u8>".into()],
                vec!["'b".into()],
                vec!["Pair<u8>".into(), "Layout".into(), "Callback".into()],
            ]
        }

        #[test]
        fn source_v4_snapshot_keeps_all_lanes_and_historical_framing() {
            let lanes = lanes();
            let description = description(&lanes);
            assert_eq!(description.encoded_bytes().len(), 545);
            assert_eq!(
                description.fingerprint_and_commitment(),
                (
                    0xb83bfa9b72729933,
                    [
                        0x93, 0x93, 0x25, 0xb5, 0xfa, 0xe8, 0x93, 0x96, 0xcc, 0x8e, 0xe3, 0x13,
                        0x43, 0x71, 0xf2, 0xd0, 0xd5, 0x14, 0xc6, 0x63, 0x1a, 0x00, 0x69, 0x52,
                        0x19, 0xb7, 0x23, 0x62, 0x3f, 0x30, 0x7d, 0x88,
                    ]
                )
            );
        }

        #[test]
        fn role_carriers_recursive_selection_and_application_presence_are_distinct() {
            let lanes = lanes();
            let original = description(&lanes);
            let expected = original.fingerprint_and_commitment();
            let changed = |candidate: ConformanceIdentityDescription<'_>| {
                let observed = candidate.fingerprint_and_commitment();
                assert_ne!(observed.0, expected.0);
                assert_ne!(observed.1, expected.1);
            };
            let mut candidate = original.clone();
            let ConformanceConstIdentity::Evaluated {
                declared_carrier, ..
            } = &mut candidate.const_arguments[0]
            else {
                panic!("fixture must contain an evaluated const");
            };
            *declared_carrier = "u64".into();
            changed(candidate);
            let mut candidate = original.clone();
            let application = candidate.machine_operands[0].application.as_mut().unwrap();
            application.arguments[0].selection =
                MachineOperandSelectionIdentity::Const("u8".into());
            changed(candidate);
            let mut candidate = original.clone();
            candidate.machine_operands[0]
                .application
                .as_mut()
                .unwrap()
                .arguments[2]
                .application = None;
            changed(candidate);
            let mut candidate = original.clone();
            candidate.rows[1].requirement_signature = Some(Vec::new());
            changed(candidate);
            let mut candidate = original.clone();
            candidate.machine_arguments.reverse();
            changed(candidate);

            let mut terminal_domain =
                DomainDigestEncoder::new(b"omega.psi.terminal.closed-conformance-application.v7\0");
            terminal_domain.raw(&original.encoded_bytes());
            assert_ne!(expected.1, terminal_domain.finish());
        }

        #[test]
        fn selected_slot_record_preserves_roles_and_source_preimage() {
            use crate::callback_declarations::PrivateCallbackSlotSelection;
            let lanes = lanes();
            let description = description(&lanes);
            let application_encoding: std::sync::Arc<[u8]> = description.encoded_bytes().into();
            let (application_report_fingerprint, application_commitment) =
                source_identity_commitments(&application_encoding);
            let selection = PrivateCallbackSlotSelection {
                declaration_identity: "package::Pick".into(),
                trait_identity: "core::layout::PrivateCallbackSlot".into(),
                semantic_identity: "package::Pair<u8>".into(),
                layout_identity: "package::Layout".into(),
                requirement_identity: "package::Callback::call".into(),
                application_report_fingerprint,
                application_commitment,
                application_encoding: application_encoding.clone(),
            };
            selection.validate_identity_encoding().unwrap();
            let bytes = compiler_storage::encode(&selection, Default::default()).unwrap();
            let restored: PrivateCallbackSlotSelection =
                compiler_storage::decode(&bytes, Default::default()).unwrap();
            assert_eq!(restored, selection);
            restored.validate_identity_encoding().unwrap();
            let cloned = selection.clone();
            assert!(std::sync::Arc::ptr_eq(
                &cloned.application_encoding,
                &application_encoding
            ));
            let mut changed = selection.clone();
            std::sync::Arc::make_mut(&mut changed.application_encoding)[0] ^= 1;
            assert!(
                changed
                    .validate_identity_encoding()
                    .unwrap_err()
                    .contains("Source-v4 identity preimage")
            );
            let mut changed = selection;
            changed.requirement_identity.clear();
            assert!(
                changed
                    .validate_identity_encoding()
                    .unwrap_err()
                    .contains("exact selected application identity")
            );
        }
    }
}
pub mod const_value;
pub mod content;
pub mod declaration_selection;
pub mod direct_calls {
    pub use source::DependencyScope;

    pub struct DirectCallDeclarations<Declaration> {
        pub scope: DependencyScope,
        pub machines: Vec<DirectCallMachine<Declaration>>,
        pub families: Vec<DirectCallFamily>,
        machine_positions: std::collections::HashMap<Declaration, usize>,
    }

    pub struct DirectCallMachine<Declaration> {
        pub declaration: Declaration,
        pub name: String,
        pub in_scope: bool,
        pub family_interface: bool,
        pub authored_target: Option<String>,
        pub source_target_context: Option<String>,
        pub target: Option<String>,
        pub states: Vec<DirectCallState<Declaration>>,
    }

    pub struct DirectCallState<Declaration> {
        pub declaration: Declaration,
        pub name: String,
        pub calls: Vec<DirectCall<Declaration>>,
    }

    pub struct DirectCall<Declaration> {
        pub declaration: Declaration,
        pub machine: Option<usize>,
    }

    pub struct DirectCallFamily {
        pub canonical: Option<usize>,
        pub members: Vec<DirectCallFamilyMember>,
    }

    pub struct DirectCallFamilyMember {
        pub target: String,
        pub machine: Option<usize>,
    }

    impl<Declaration: Eq + std::hash::Hash> DirectCallDeclarations<Declaration> {
        pub fn new(
            scope: DependencyScope,
            machines: Vec<DirectCallMachine<Declaration>>,
            families: Vec<DirectCallFamily>,
            machine_positions: Vec<(Declaration, usize)>,
        ) -> Self {
            let mut positions = std::collections::HashMap::with_capacity(machine_positions.len());
            for (declaration, position) in machine_positions {
                positions.entry(declaration).or_insert(position);
            }
            Self {
                scope,
                machines,
                families,
                machine_positions: positions,
            }
        }

        pub fn called_machine_position(&self, declaration: &Declaration) -> Option<usize> {
            self.machine_positions.get(declaration).copied()
        }
    }
}
mod external_bindings {
    use crate::ExternalBindingId;

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

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ExternalBindingTable {
        identities: Vec<ExternalBindingIdentity>,
    }

    impl ExternalBindingTable {
        pub fn intern(&mut self, identity: ExternalBindingIdentity) -> ExternalBindingId {
            if let Some(index) = self
                .identities
                .iter()
                .position(|existing| existing == &identity)
            {
                return ExternalBindingId(index as u32 + 1);
            }
            self.identities.push(identity);
            ExternalBindingId(self.identities.len() as u32)
        }

        pub fn identity(&self, binding: ExternalBindingId) -> Option<&ExternalBindingIdentity> {
            let index = usize::try_from(binding.0).ok()?.checked_sub(1)?;
            self.identities.get(index)
        }

        pub fn identities(
            &self,
        ) -> impl ExactSizeIterator<Item = (ExternalBindingId, &ExternalBindingIdentity)> {
            self.identities
                .iter()
                .enumerate()
                .map(|(index, identity)| (ExternalBindingId(index as u32 + 1), identity))
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
    }

    impl MachineSupplyMode {
        pub const fn is_checked_body(self) -> bool {
            matches!(self, Self::CheckedBody)
        }

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
            match self {
                Self::NAT_DESCENDING => Some(RankingViewDeclaration {
                    view: self,
                    source: RANKING_VIEW_CORE_SOURCE,
                    namespace: "Nat",
                    name: "Descending",
                    parameter: "u64",
                    result: "u64",
                }),
                _ => None,
            }
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
    pub struct QuotientCallableIdentity {
        pub declaration: String,
        pub overload: String,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum QuotientCorrespondenceOperationKind {
        Lift,
        LiftWithForwardPreconditionTransport,
        Define,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub struct QuotientStaticApplication {
        pub bindings: Vec<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub struct QuotientMachineApplication {
        pub callable: QuotientCallableIdentity,
        pub static_application: QuotientStaticApplication,
    }

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

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub enum QuotientRuntimeArgument {
        PublicParameter {
            public_position: u32,
        },
        ClosedLiteral {
            value_identity: String,
            representative_type: String,
        },
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum QuotientContractOwner {
        Machine,
        State,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub struct QuotientContractFactCoordinate {
        pub owner: QuotientContractOwner,
        pub contract_position: u32,
        pub fact_position: u32,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum QuotientTheoremApplicationSide {
        Left,
        Right,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub struct QuotientForwardPreconditionTransportFact {
        pub application: QuotientTheoremApplicationSide,
        pub source: QuotientContractFactCoordinate,
        pub actual: QuotientContractFactCoordinate,
    }

    pub type QuotientTheoremLegalityFact = QuotientForwardPreconditionTransportFact;

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum QuotientTheoremParameterRole {
        QuotientLeft { input_position: u32 },
        QuotientRight { input_position: u32 },
        Shared { input_position: u32 },
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub struct QuotientTheoremParameter {
        pub theorem_position: u32,
        pub role: QuotientTheoremParameterRole,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub struct QuotientTheoremRelationPremise {
        pub expected_position: u32,
        pub actual: QuotientContractFactCoordinate,
        pub relation: String,
        pub left_parameter: u32,
        pub right_parameter: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub struct QuotientRepresentativeApplication {
        pub arguments: Vec<u32>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub enum QuotientResultRelation {
        Quotient(String),
        ExactEquality(String),
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub struct QuotientTheoremConclusion {
        pub actual: QuotientContractFactCoordinate,
        pub relation: QuotientResultRelation,
        pub left: QuotientRepresentativeApplication,
        pub right: QuotientRepresentativeApplication,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub struct QuotientCongruenceCorrespondence {
        pub parameters: Vec<QuotientTheoremParameter>,
        pub relation_premises: Vec<QuotientTheoremRelationPremise>,
        pub legality_premises: Vec<QuotientTheoremLegalityFact>,
        pub conclusion: QuotientTheoremConclusion,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum QuotientTheoremRole {
        Congruence,
        ForwardPreconditionTransport,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub struct QuotientForwardPreconditionTransportCorrespondence {
        pub public_premises: Vec<QuotientForwardPreconditionTransportFact>,
        pub representative_conclusions: Vec<QuotientForwardPreconditionTransportFact>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub enum QuotientTheoremCorrespondence {
        Congruence(QuotientCongruenceCorrespondence),
        ForwardPreconditionTransport(QuotientForwardPreconditionTransportCorrespondence),
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum QuotientPurityCertificate {
        PureClosure,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum QuotientTerminationCertificate {
        Unconditional,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum QuotientCrashCertificate {
        CrashFree,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub struct QuotientRepresentativeEligibility {
        pub purity: QuotientPurityCertificate,
        pub termination: QuotientTerminationCertificate,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub struct QuotientTheoremEligibility {
        pub purity: QuotientPurityCertificate,
        pub termination: QuotientTerminationCertificate,
        pub crash: QuotientCrashCertificate,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub struct QuotientTheoremEvidence {
        pub role: QuotientTheoremRole,
        pub selected_application: QuotientMachineApplication,
        pub correspondence: QuotientTheoremCorrespondence,
        pub eligibility: QuotientTheoremEligibility,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub struct QuotientStateForwarding {
        pub source_position: u32,
        pub target_position: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub enum QuotientResultFlow {
        Direct {
            statement_position: u32,
            immutable_alias_count: u32,
        },
        Forwarded {
            machine_state_count: u32,
            result_state_position: u32,
            statement_position: u32,
            immutable_alias_count: u32,
            forwarding: Vec<QuotientStateForwarding>,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub struct QuotientCorrespondenceJudgment {
        pub operation_kind: QuotientCorrespondenceOperationKind,
        pub public_operation: QuotientCallableIdentity,
        pub representative: QuotientMachineApplication,
        pub input_relations: Vec<QuotientPositionalRelation>,
        pub result_relation: QuotientPositionalRelation,
        pub runtime_arguments: Vec<QuotientRuntimeArgument>,
        pub theorem_evidence: Vec<QuotientTheoremEvidence>,
        pub representative_eligibility: QuotientRepresentativeEligibility,
        pub result_flow: QuotientResultFlow,
    }
}
mod semantic_domains {
    use crate::SemanticDomainId;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct SemanticDomainTable {
        names: Vec<String>,
    }

    impl Default for SemanticDomainTable {
        fn default() -> Self {
            Self {
                names: vec![
                    "Wrapping".to_owned(),
                    "Saturating".to_owned(),
                    "Trapping".to_owned(),
                ],
            }
        }
    }

    impl SemanticDomainTable {
        pub fn len(&self) -> usize {
            self.names.len()
        }

        pub fn is_empty(&self) -> bool {
            self.names.is_empty()
        }

        pub const WRAPPING: SemanticDomainId = SemanticDomainId(1);
        pub const SATURATING: SemanticDomainId = SemanticDomainId(2);
        pub const TRAPPING: SemanticDomainId = SemanticDomainId(3);

        pub fn intern(&mut self, name: &str) -> SemanticDomainId {
            if let Some(position) = self.names.iter().position(|candidate| candidate == name) {
                return SemanticDomainId(
                    u32::try_from(position + 1).expect("domain table fits u32"),
                );
            }
            self.names.push(name.to_owned());
            SemanticDomainId(u32::try_from(self.names.len()).expect("domain table fits u32"))
        }

        pub fn name(&self, id: SemanticDomainId) -> Option<&str> {
            id.0.checked_sub(1)
                .and_then(|index| self.names.get(index as usize))
                .map(String::as_str)
        }

        pub fn lookup(&self, name: &str) -> Option<SemanticDomainId> {
            self.names
                .iter()
                .position(|candidate| candidate == name)
                .map(|position| {
                    SemanticDomainId(u32::try_from(position + 1).expect("domain table fits u32"))
                })
        }
    }

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
    use crate::{ServiceReachId, ServiceReachRowId};

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

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub(crate) enum ServiceReachRow {
        Concrete(Vec<ServiceReachId>),
        AbstractBounded(ServiceReachRowId),
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ServiceReachRowTable {
        pub(crate) rows: Vec<ServiceReachRow>,
    }

    impl ServiceReachRowTable {
        pub const EMPTY_ROW: ServiceReachRowId = ServiceReachRowId(1);

        pub fn len(&self) -> usize {
            self.rows.len()
        }

        pub fn is_empty(&self) -> bool {
            self.rows.is_empty()
        }

        pub fn starts_with(&self, retained: &Self) -> bool {
            self.rows.starts_with(&retained.rows)
        }

        pub fn intern(&mut self, mut services: Vec<ServiceReachId>) -> ServiceReachRowId {
            services.sort_by_key(|service| service.0);
            services.dedup();
            if self.rows.is_empty() {
                self.rows.push(ServiceReachRow::Concrete(Vec::new()));
            }
            if let Some(position) = self
                .rows
                .iter()
                .position(|row| matches!(row, ServiceReachRow::Concrete(row) if *row == services))
            {
                return ServiceReachRowId(u32::try_from(position + 1).expect("row table fits u32"));
            }
            self.rows.push(ServiceReachRow::Concrete(services));
            ServiceReachRowId(u32::try_from(self.rows.len()).expect("row table fits u32"))
        }

        pub fn intern_abstract(&mut self, bound: ServiceReachRowId) -> ServiceReachRowId {
            if self.rows.is_empty() {
                self.rows.push(ServiceReachRow::Concrete(Vec::new()));
            }
            if let Some(position) = self.rows.iter().position(
                |row| matches!(row, ServiceReachRow::AbstractBounded(existing) if *existing == bound),
            ) {
                return ServiceReachRowId(u32::try_from(position + 1).expect("row table fits u32"));
            }
            self.rows.push(ServiceReachRow::AbstractBounded(bound));
            ServiceReachRowId(u32::try_from(self.rows.len()).expect("row table fits u32"))
        }

        pub fn services(&self, row: ServiceReachRowId) -> &[ServiceReachId] {
            match row
                .0
                .checked_sub(1)
                .and_then(|index| self.rows.get(index as usize))
            {
                Some(ServiceReachRow::Concrete(services)) => services.as_slice(),
                Some(ServiceReachRow::AbstractBounded(bound)) => self.services(*bound),
                None => &[],
            }
        }

        pub fn is_abstract(&self, row: ServiceReachRowId) -> bool {
            matches!(self.entry(row), Some(ServiceReachRow::AbstractBounded(_)))
        }

        pub fn abstract_bound(&self, row: ServiceReachRowId) -> Option<ServiceReachRowId> {
            match self.entry(row) {
                Some(ServiceReachRow::AbstractBounded(bound)) => Some(*bound),
                _ => None,
            }
        }

        fn entry(&self, row: ServiceReachRowId) -> Option<&ServiceReachRow> {
            row.0
                .checked_sub(1)
                .and_then(|index| self.rows.get(index as usize))
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ServiceReachDefinition {
        pub symbol: symbols::SymbolHandle,
        pub name: String,
        pub parents: Vec<ServiceReachId>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ServiceReachTable {
        definitions: Vec<ServiceReachDefinition>,
    }

    impl ServiceReachTable {
        pub fn intern(&mut self, symbol: symbols::SymbolHandle, name: &str) -> ServiceReachId {
            if let Some((index, _)) = self
                .definitions
                .iter()
                .enumerate()
                .find(|(_, definition)| definition.symbol == symbol)
            {
                return ServiceReachId(u32::try_from(index + 1).expect("service table fits u32"));
            }
            self.definitions.push(ServiceReachDefinition {
                symbol,
                name: name.to_owned(),
                parents: Vec::new(),
            });
            ServiceReachId(u32::try_from(self.definitions.len()).expect("service table fits u32"))
        }

        pub fn id_for_symbol(&self, symbol: symbols::SymbolHandle) -> Option<ServiceReachId> {
            self.definitions
                .iter()
                .position(|definition| definition.symbol == symbol)
                .map(|index| {
                    ServiceReachId(u32::try_from(index + 1).expect("service table fits u32"))
                })
        }

        pub fn id_for_name(&self, name: &str) -> Option<ServiceReachId> {
            self.definitions
                .iter()
                .position(|definition| definition.name == name)
                .map(|index| {
                    ServiceReachId(u32::try_from(index + 1).expect("service table fits u32"))
                })
        }

        pub fn definition(&self, id: ServiceReachId) -> Option<&ServiceReachDefinition> {
            id.0.checked_sub(1)
                .and_then(|index| self.definitions.get(index as usize))
        }

        pub fn definitions(&self) -> &[ServiceReachDefinition] {
            &self.definitions
        }

        pub fn set_parents(&mut self, id: ServiceReachId, mut parents: Vec<ServiceReachId>) {
            parents.sort_by_key(|parent| parent.0);
            parents.dedup();
            if let Some(definition) =
                id.0.checked_sub(1)
                    .and_then(|index| self.definitions.get_mut(index as usize))
            {
                definition.parents = parents;
            }
        }

        pub fn extend_closure(&self, service: ServiceReachId, services: &mut Vec<ServiceReachId>) {
            if services.contains(&service) {
                return;
            }
            services.push(service);
            if let Some(definition) = self.definition(service) {
                for parent in &definition.parents {
                    self.extend_closure(*parent, services);
                }
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
        pub name: String,
        pub template: Result<Option<AcceptedTemplateDeclaration>, TrustDeclarationCardinality>,
        pub contract: Result<AcceptedContractDeclaration, TrustDeclarationCardinality>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct TrustDeclarations {
        pub machines: Box<[AcceptedMachineDeclaration]>,
    }
}
pub mod type_identity;
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

        pub fn proof_name(self) -> &'static str {
            match self {
                Self::Finite => "finite",
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

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct WireScalarRange {
        pub minimum: i64,
        pub maximum: i64,
        pub signed: bool,
    }
}

pub use external_bindings::{
    ExternalBindingIdentity, ExternalBindingMechanism, ExternalBindingTable,
};
pub use language_core::{
    CallOperationalAcknowledgement, CallOperationalAcknowledgementOrigin, CarryAddress, CarryCpu,
    CarryHostThread, CarryPermission, CarryPolicy, CarrySuspension, DataSupplyMode,
    DomainClassification, DomainPredicatePresence, Multiplicity, ReferenceAccess,
};
pub use machine_termination::{
    MachineSupplyMode, MachineTerminationPlan, ProgressPremise, ProgressSubject,
    RANKING_VIEW_CORE_SOURCE, RankRange, RankingViewDeclaration, RankingWitness,
    TerminationGuarantee, TerminationInterface,
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
    QualificationEvidenceOrigin, SemanticDomainTable,
};
pub use semantic_identities::{
    ExternalBindingId, RankingViewId, SemanticDomainId, ServiceReachId, ServiceReachRowId,
};
pub use service_reach::{
    BlockingInterface, BlockingPlan, BlockingSummary, ServiceReachDefinition,
    ServiceReachInterface, ServiceReachPlan, ServiceReachRowTable, ServiceReachSummary,
    ServiceReachTable, SuspensionInterface, SuspensionPlan, SuspensionSummary,
    SynchronousInvocationInterface, SynchronousInvocationPlan,
};

pub mod native_bindings {
    use build_time_values::BuildTimeValue;
    use semantic_vocabulary::{ForeignLocatorCandidate, NativeBindingLocator};

    pub trait NativeLocatorValue: Clone {
        fn case(&self) -> Result<NativeLocatorCase<'_, Self>, String>;
        fn integer(&self) -> Result<i128, String>;
        fn bytes(&self, width: u64, label: &str) -> Result<Vec<u8>, String>;
    }

    pub struct NativeLocatorCase<'value, Value> {
        pub name: &'value str,
        pub fields: Vec<(&'value str, Value)>,
    }

    pub fn decode_binding_value(
        value: &BuildTimeValue,
        widths: [u64; 3],
    ) -> Result<NativeBindingLocator, String> {
        decode_native_locator(value, widths)
    }

    pub fn decode_native_locator<Value: NativeLocatorValue>(
        value: Value,
        widths: [u64; 3],
    ) -> Result<NativeBindingLocator, String> {
        let binding = value.case()?;
        if binding.name == "Syscall" {
            require_unused_width(widths[0], "Syscall ObjectLength")?;
            require_unused_width(widths[1], "Syscall SymbolLength")?;
            require_unused_width(widths[2], "Syscall VersionLength")?;
            let [number] = exact_fields(binding, ["number"])?;
            let number = u64::try_from(number.integer()?)
                .map_err(|_| "ForeignBinding::Syscall number must fit u64".to_owned())?;
            return Ok(NativeBindingLocator::Syscall { number });
        }
        if binding.name != "DllImport" {
            return Err("native binding must evaluate to its exact ForeignBinding::DllImport or ForeignBinding::Syscall payload".to_owned());
        }
        let [import] = exact_fields(binding, ["import"])?;
        let case = import.case()?;
        let locator = match case.name {
            "PeByName" => {
                require_unused_width(widths[2], "PeByName VersionLength")?;
                let [library, export] = exact_fields(case, ["library", "export"])?;
                ForeignLocatorCandidate::PeByName {
                    library: library.bytes(widths[0], "PeByName library")?,
                    export: export.bytes(widths[1], "PeByName export")?,
                }
            }
            "PeByOrdinal" => {
                require_unused_width(widths[1], "PeByOrdinal SymbolLength")?;
                require_unused_width(widths[2], "PeByOrdinal VersionLength")?;
                let [library, ordinal] = exact_fields(case, ["library", "ordinal"])?;
                let ordinal = u16::try_from(ordinal.integer()?)
                    .map_err(|_| "PeByOrdinal ordinal must fit nonzero u16".to_owned())?;
                if ordinal == 0 {
                    return Err("PeByOrdinal ordinal must be nonzero".to_owned());
                }
                ForeignLocatorCandidate::PeByOrdinal {
                    library: library.bytes(widths[0], "PeByOrdinal library")?,
                    ordinal,
                }
            }
            "ElfVersioned" => {
                let [object, symbol, version] =
                    exact_fields(case, ["object", "symbol", "version"])?;
                ForeignLocatorCandidate::ElfVersioned {
                    object: object.bytes(widths[0], "ElfVersioned object")?,
                    symbol: symbol.bytes(widths[1], "ElfVersioned symbol")?,
                    version: version.bytes(widths[2], "ElfVersioned version")?,
                }
            }
            "MachODylibSymbol" => {
                require_unused_width(widths[2], "MachODylibSymbol VersionLength")?;
                let [install_name, symbol] = exact_fields(case, ["install_name", "symbol"])?;
                ForeignLocatorCandidate::MachODylibSymbol {
                    install_name: install_name.bytes(widths[0], "MachODylibSymbol install_name")?,
                    symbol: symbol.bytes(widths[1], "MachODylibSymbol symbol")?,
                }
            }
            name => return Err(format!("unknown compiler-owned DllImport case `{name}`")),
        };
        Ok(NativeBindingLocator::Import(locator))
    }

    fn exact_fields<Value: NativeLocatorValue, const N: usize>(
        case: NativeLocatorCase<'_, Value>,
        names: [&str; N],
    ) -> Result<[Value; N], String> {
        if case.fields.len() != N
            || case
                .fields
                .iter()
                .zip(names)
                .any(|((actual, _), expected)| *actual != expected)
        {
            return Err(
                "evaluated foreign locator payload fields drifted from the compiler-owned declaration"
                    .to_owned(),
            );
        }
        Ok(std::array::from_fn(|index| case.fields[index].1.clone()))
    }

    impl NativeLocatorValue for &BuildTimeValue {
        fn case(&self) -> Result<NativeLocatorCase<'_, Self>, String> {
            let BuildTimeValue::Case { variant, payload } = *self else {
                return Err("native binding value has no exact selected core case".to_owned());
            };
            Ok(NativeLocatorCase {
                name: variant,
                fields: payload
                    .iter()
                    .map(|(name, value)| (name.as_str(), value))
                    .collect(),
            })
        }
        fn integer(&self) -> Result<i128, String> {
            match self {
                BuildTimeValue::Int(value) => Ok(i128::from(*value)),
                _ => Err("native locator field must evaluate as an integer".to_owned()),
            }
        }
        fn bytes(&self, width: u64, label: &str) -> Result<Vec<u8>, String> {
            let BuildTimeValue::Array(elements) = self else {
                return Err(format!("{label} must evaluate as a fixed byte array"));
            };
            let expected = usize::try_from(width)
                .map_err(|_| format!("{label} width does not fit the compiler host"))?;
            if elements.len() != expected {
                return Err(format!(
                    "{label} evaluated length does not match its const width"
                ));
            }
            elements
                .iter()
                .map(|element| match element {
                    BuildTimeValue::Int(byte) => u8::try_from(*byte)
                        .map_err(|_| format!("{label} contains a value outside u8")),
                    _ => Err(format!("{label} contains a non-integer byte")),
                })
                .collect()
        }
    }

    fn require_unused_width(width: u64, label: &str) -> Result<(), String> {
        (width == 0)
            .then_some(())
            .ok_or_else(|| format!("{label} must be zero for this locator case"))
    }
}

#[cfg(test)]
mod tests;
