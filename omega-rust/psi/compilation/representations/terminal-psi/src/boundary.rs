/*
Target-neutral boundary declarations, external provider contracts, runtime callable rows, dynamic
dispatch families, effect ceilings, and suspension/carry obligations in `TerminalModule`.
*/

pub(crate) mod applications {

    use crate::terminal_module::TerminalIdentityIndex;
    use crate::{CheckedMachineDeclaration, NominalDeclarationIdentity};
    use semantic_vocabulary::OperationId;

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct BoundaryTypeIdentity(TerminalIdentityIndex);

    impl BoundaryTypeIdentity {
        pub fn new(identity: TerminalIdentityIndex) -> Self {
            Self(identity)
        }

        pub fn identity(&self) -> TerminalIdentityIndex {
            self.0
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct BoundaryOperatorRequirement {
        declaration: NominalDeclarationIdentity,
        overload: String,
        package: Option<semantic_vocabulary::PackageKeyIdentity>,
    }

    impl BoundaryOperatorRequirement {
        pub fn new(
            declaration: NominalDeclarationIdentity,
            overload: String,
            package: Option<semantic_vocabulary::PackageKeyIdentity>,
        ) -> Result<Self, &'static str> {
            if overload.is_empty() {
                return Err("boundary operator overload identity is empty");
            }
            Ok(Self {
                declaration,
                overload,
                package,
            })
        }

        pub const fn declaration(&self) -> &NominalDeclarationIdentity {
            &self.declaration
        }

        pub fn overload(&self) -> &str {
            &self.overload
        }

        pub const fn package(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
            self.package
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct BoundaryApplicationCandidate {
        pub requirement: BoundaryOperatorRequirement,
        pub application: BoundaryApplication,
        pub selection_identity: TerminalIdentityIndex,
        pub selection_package: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub target: Option<String>,
        pub body: CheckedMachineDeclaration,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum BoundaryApplication {
        Empty,
        Exact(Vec<BoundaryApplicationArgument>),
    }

    impl BoundaryApplication {
        pub fn exact(arguments: Vec<BoundaryApplicationArgument>) -> Result<Self, &'static str> {
            if arguments.is_empty() {
                return Err("nonempty boundary application has no arguments");
            }
            for (ordinal, argument) in arguments.iter().enumerate() {
                let expected = u32::try_from(ordinal)
                    .map_err(|_| "boundary application exceeds the supported ordinal range")?;
                if argument.binder_ordinal() != expected {
                    return Err("boundary application arguments are not in exact binder order");
                }
            }
            Ok(Self::Exact(arguments))
        }

        pub fn arguments(&self) -> &[BoundaryApplicationArgument] {
            match self {
                Self::Empty => &[],
                Self::Exact(arguments) => arguments,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum BoundaryApplicationArgument {
        Type {
            binder_ordinal: u32,
            type_identity: BoundaryTypeIdentity,
        },
        Const {
            binder_ordinal: u32,
            declared_carrier: BoundaryTypeIdentity,
            value_type: String,
            value_encoding: String,
        },
    }

    impl BoundaryApplicationArgument {
        pub fn type_argument(binder_ordinal: u32, type_identity: BoundaryTypeIdentity) -> Self {
            Self::Type {
                binder_ordinal,
                type_identity,
            }
        }

        pub fn const_argument(
            binder_ordinal: u32,
            declared_carrier: BoundaryTypeIdentity,
            value_type: String,
            value_encoding: String,
        ) -> Result<Self, &'static str> {
            if value_type.is_empty() || value_encoding.is_empty() {
                return Err("boundary const application has an empty canonical value identity");
            }
            Ok(Self::Const {
                binder_ordinal,
                declared_carrier,
                value_type,
                value_encoding,
            })
        }

        pub const fn binder_ordinal(&self) -> u32 {
            match self {
                Self::Type { binder_ordinal, .. } | Self::Const { binder_ordinal, .. } => {
                    *binder_ordinal
                }
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalBoundaryApplicationDemand {
        terminal_operation: OperationId,
        requirement: BoundaryOperatorRequirement,
        application: BoundaryApplication,
    }

    impl TerminalBoundaryApplicationDemand {
        pub fn new(
            terminal_operation: OperationId,
            requirement: BoundaryOperatorRequirement,
            application: BoundaryApplication,
        ) -> Self {
            Self {
                terminal_operation,
                requirement,
                application,
            }
        }

        pub const fn terminal_operation(&self) -> OperationId {
            self.terminal_operation
        }

        pub const fn requirement(&self) -> &BoundaryOperatorRequirement {
            &self.requirement
        }

        pub const fn application(&self) -> &BoundaryApplication {
            &self.application
        }
    }
}
pub(crate) mod conformances {
    use crate::terminal_module::{TerminalIdentityIndex, TerminalIdentityTable};
    use semantic_vocabulary::MachineId;

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ClosedConformanceApplication {
        pub owner: MachineId,
        pub declaration_identity: TerminalIdentityIndex,
        pub telescope: Vec<ClosedConformanceParameterBinding>,
        pub trait_identity: TerminalIdentityIndex,
        pub trait_lifetime_arguments: Vec<String>,
        pub trait_arguments: Vec<String>,
        pub realization_callables: Vec<ClosedConformanceRealizationCallable>,
        pub schemas: Vec<ClosedConformanceSchema>,
        pub static_applications: Vec<ClosedConformanceStaticApplication>,
        pub rows: Vec<ClosedConformanceRow>,
        pub report_fingerprint: u64,
        pub commitment: ClosedConformanceApplicationCommitment,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ClosedConformanceSchemaKind {
        Nongeneric,
        Finite,
        Open,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ClosedConformanceSchema {
        pub requirement_declaration: crate::NominalDeclarationIdentity,
        pub declaring_trait_identity: TerminalIdentityIndex,
        pub public_requirement_identity: TerminalIdentityIndex,
        pub requirement_identity: TerminalIdentityIndex,
        pub realization_identity: TerminalIdentityIndex,
        pub definition: Vec<u8>,
        pub kind: ClosedConformanceSchemaKind,
        pub lifetime_parameter_count: u32,
        pub parameter_kinds: Vec<u8>,
        pub finite_tuples: Option<Vec<Vec<TerminalIdentityIndex>>>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ClosedConformanceStaticApplication {
        pub public_requirement_identity: TerminalIdentityIndex,
        pub realization_identity: TerminalIdentityIndex,
        pub source_callable_identity: TerminalIdentityIndex,
        pub callable: crate::TargetMachineCallable,
        pub application_commitment: Option<[u8; 32]>,
        pub method_arguments: Vec<crate::ClosedReachArgument>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ClosedConformanceApplicationCommitment([u8; 32]);

    impl ClosedConformanceApplicationCommitment {
        pub const fn from_digest(digest: [u8; 32]) -> Self {
            Self(digest)
        }

        pub const fn as_bytes(self) -> [u8; 32] {
            self.0
        }

        pub fn is_zero(self) -> bool {
            self.0 == [0; 32]
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ClosedConformanceParameterKind {
        Lifetime,
        Type,
        Const,
        Machine,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ClosedConformanceParameterBinding {
        pub parameter: String,
        pub kind: ClosedConformanceParameterKind,
        pub argument: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ClosedConformanceRow {
        pub requirement_declaration: crate::NominalDeclarationIdentity,
        pub declaring_trait_identity: TerminalIdentityIndex,
        pub public_requirement_identity: TerminalIdentityIndex,
        pub family_tuple: Vec<crate::TerminalIdentityIndex>,
        pub requirement_identity: TerminalIdentityIndex,
        pub realization_identity: TerminalIdentityIndex,
        pub realization_callable_identity: Option<TerminalIdentityIndex>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ClosedConformanceRealizationCallable {
        pub source_callable_identity: TerminalIdentityIndex,
        pub callable: crate::TargetMachineCallable,
        pub result: ClosedConformanceCallableResult,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ClosedConformanceCallableResult {
        Unit,
        Scalar(semantic_vocabulary::ScalarType),
        Structural(semantic_vocabulary::StructuralTypeId),
    }

    fn closed_conformance_application_bytes(
        identities: &TerminalIdentityTable,
        application: &ClosedConformanceApplication,
    ) -> Vec<u8> {
        fn push(bytes: &mut Vec<u8>, value: &str) {
            bytes.extend((value.len() as u64).to_le_bytes());
            bytes.extend(value.as_bytes());
        }

        let mut bytes = Vec::new();
        macro_rules! identity {
            ($identity:expr) => {
                push(&mut bytes, identities.text($identity).unwrap_or_default())
            };
        }
        identity!(application.declaration_identity);
        identity!(application.trait_identity);
        bytes.extend((application.trait_lifetime_arguments.len() as u64).to_le_bytes());
        for argument in &application.trait_lifetime_arguments {
            push(&mut bytes, argument);
        }
        bytes.extend((application.telescope.len() as u64).to_le_bytes());
        for binding in &application.telescope {
            push(&mut bytes, &binding.parameter);
            bytes.push(match binding.kind {
                ClosedConformanceParameterKind::Lifetime => 1,
                ClosedConformanceParameterKind::Type => 2,
                ClosedConformanceParameterKind::Const => 3,
                ClosedConformanceParameterKind::Machine => 4,
            });
            push(&mut bytes, &binding.argument);
        }
        bytes.extend((application.trait_arguments.len() as u64).to_le_bytes());
        for argument in &application.trait_arguments {
            push(&mut bytes, argument);
        }
        bytes.extend((application.realization_callables.len() as u64).to_le_bytes());
        for callable in &application.realization_callables {
            identity!(callable.source_callable_identity);
            bytes.extend(callable_bytes(callable.callable));
            bytes.extend(callable_result_bytes(callable.result));
        }
        bytes.extend(schema_application_bytes(identities, application));
        bytes.extend((application.rows.len() as u64).to_le_bytes());
        for row in &application.rows {
            identity!(row.requirement_declaration.identity());
            identity!(row.declaring_trait_identity);
            identity!(row.public_requirement_identity);
            bytes.extend((row.family_tuple.len() as u64).to_le_bytes());
            for identity in &row.family_tuple {
                push(&mut bytes, identities.text(*identity).unwrap_or_default());
            }
            identity!(row.requirement_identity);
            identity!(row.realization_identity);
            bytes.push(u8::from(row.realization_callable_identity.is_some()));
            if let Some(identity) = &row.realization_callable_identity {
                push(&mut bytes, identities.text(*identity).unwrap_or_default());
            }
        }
        bytes
    }

    pub fn closed_conformance_application_report_fingerprint(
        identities: &TerminalIdentityTable,
        application: &ClosedConformanceApplication,
    ) -> u64 {
        let mut hash = semantic_vocabulary::Fnv1a::new();
        hash.bytes(&closed_conformance_application_bytes(
            identities,
            application,
        ));
        hash.finish()
    }

    fn callable_result_bytes(result: ClosedConformanceCallableResult) -> Vec<u8> {
        use semantic_vocabulary::{IeeeFloatFormat, IntegerCarrier, IntegerSign, ScalarType};
        match result {
            ClosedConformanceCallableResult::Unit => vec![1],
            ClosedConformanceCallableResult::Structural(identity) => {
                let mut bytes = vec![3];
                bytes.extend(identity.get().to_le_bytes());
                bytes
            }
            ClosedConformanceCallableResult::Scalar(ScalarType::Boolean) => vec![2, 1],
            ClosedConformanceCallableResult::Scalar(ScalarType::Callable(contract)) => {
                let mut bytes = vec![2, 4];
                bytes.extend(contract.get().to_le_bytes());
                bytes
            }
            ClosedConformanceCallableResult::Scalar(ScalarType::Integer(integer)) => {
                let mut bytes = vec![
                    2,
                    2,
                    match (integer.carrier(), integer.sign()) {
                        (IntegerCarrier::Fixed, IntegerSign::Signed) => 1,
                        (IntegerCarrier::Fixed, IntegerSign::Unsigned) => 2,
                        (IntegerCarrier::Address, IntegerSign::Unsigned) => 3,
                        (IntegerCarrier::Address, IntegerSign::Signed) => {
                            unreachable!("address carriers are unsigned")
                        }
                    },
                ];
                bytes.extend(integer.bits().to_le_bytes());
                bytes
            }
            ClosedConformanceCallableResult::Scalar(ScalarType::IeeeFloat(format)) => vec![
                2,
                3,
                match format {
                    IeeeFloatFormat::Binary32 => 1,
                    IeeeFloatFormat::Binary64 => 2,
                },
            ],
        }
    }

    fn callable_bytes(callable: crate::TargetMachineCallable) -> Vec<u8> {
        let (tag, identity) = match callable {
            crate::TargetMachineCallable::Machine(machine) => (1, machine.get()),
            crate::TargetMachineCallable::Boundary(boundary) => (2, boundary.get()),
        };
        let mut bytes = vec![tag];
        bytes.extend(identity.to_le_bytes());
        bytes
    }

    fn schema_application_bytes(
        identities: &TerminalIdentityTable,
        application: &ClosedConformanceApplication,
    ) -> Vec<u8> {
        fn text(
            bytes: &mut Vec<u8>,
            identities: &TerminalIdentityTable,
            identity: TerminalIdentityIndex,
        ) {
            let value = identities.text(identity).unwrap_or_default().as_bytes();
            bytes.extend((value.len() as u64).to_le_bytes());
            bytes.extend(value);
        }
        let mut bytes = Vec::new();
        bytes.extend((application.schemas.len() as u64).to_le_bytes());
        for schema in &application.schemas {
            for identity in [
                schema.requirement_declaration.identity(),
                schema.declaring_trait_identity,
                schema.public_requirement_identity,
                schema.requirement_identity,
                schema.realization_identity,
            ] {
                text(&mut bytes, identities, identity);
            }
            bytes.push(match schema.kind {
                ClosedConformanceSchemaKind::Nongeneric => 1,
                ClosedConformanceSchemaKind::Finite => 2,
                ClosedConformanceSchemaKind::Open => 3,
            });
            bytes.extend((schema.definition.len() as u64).to_le_bytes());
            bytes.extend(&schema.definition);
            bytes.extend(schema.lifetime_parameter_count.to_le_bytes());
            bytes.extend((schema.parameter_kinds.len() as u64).to_le_bytes());
            bytes.extend(&schema.parameter_kinds);
            bytes.push(u8::from(schema.finite_tuples.is_some()));
            if let Some(tuples) = &schema.finite_tuples {
                bytes.extend((tuples.len() as u64).to_le_bytes());
                for tuple in tuples {
                    bytes.extend((tuple.len() as u64).to_le_bytes());
                    for member in tuple {
                        text(&mut bytes, identities, *member);
                    }
                }
            }
        }
        bytes.extend((application.static_applications.len() as u64).to_le_bytes());
        for applied in &application.static_applications {
            for identity in [
                applied.public_requirement_identity,
                applied.realization_identity,
                applied.source_callable_identity,
            ] {
                text(&mut bytes, identities, identity);
            }
            bytes.extend(callable_bytes(applied.callable));
            bytes.push(u8::from(applied.application_commitment.is_some()));
            if let Some(commitment) = applied.application_commitment {
                bytes.extend(commitment);
            }
            bytes.extend((applied.method_arguments.len() as u64).to_le_bytes());
            for argument in &applied.method_arguments {
                match argument {
                    crate::ClosedReachArgument::Type(identity) => {
                        bytes.push(1);
                        text(&mut bytes, identities, *identity);
                    }
                    crate::ClosedReachArgument::Const(identity) => {
                        bytes.push(2);
                        text(&mut bytes, identities, *identity);
                    }
                    crate::ClosedReachArgument::Proposition(identity) => {
                        bytes.push(3);
                        text(&mut bytes, identities, *identity);
                    }
                    crate::ClosedReachArgument::Machine {
                        identity,
                        contract_commitment,
                    } => {
                        bytes.push(4);
                        text(&mut bytes, identities, *identity);
                        bytes.extend(contract_commitment);
                    }
                }
            }
        }
        bytes
    }

    pub fn closed_conformance_application_commitment(
        identities: &TerminalIdentityTable,
        application: &ClosedConformanceApplication,
    ) -> ClosedConformanceApplicationCommitment {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.psi.terminal.closed-conformance-application.v7\0",
        );
        digest.raw(&closed_conformance_application_bytes(
            identities,
            application,
        ));
        ClosedConformanceApplicationCommitment::from_digest(digest.finish())
    }

    #[cfg(test)]
    mod tests {
        use super::{
            ClosedConformanceApplication, ClosedConformanceRow,
            closed_conformance_application_commitment,
            closed_conformance_application_report_fingerprint,
        };

        fn application(
            identities: &mut crate::TerminalIdentityTableBuilder,
            family_tuple: Vec<crate::TerminalIdentityIndex>,
        ) -> ClosedConformanceApplication {
            ClosedConformanceApplication {
                owner: semantic_vocabulary::MachineId::new(1).unwrap(),
                declaration_identity: identities.intern("package::CarrierScanner"),
                telescope: Vec::new(),
                trait_identity: identities.intern("package::Scanner"),
                trait_lifetime_arguments: Vec::new(),
                trait_arguments: vec!["package::Carrier".into()],
                realization_callables: Vec::new(),
                schemas: Vec::new(),
                static_applications: Vec::new(),
                rows: vec![ClosedConformanceRow {
                    requirement_declaration: crate::NominalDeclarationIdentity::new(
                        identities.intern("package::Scanner::scan"),
                    ),
                    declaring_trait_identity: identities.intern("package::Scanner"),
                    public_requirement_identity: identities.intern("package::Scanner::scan()"),
                    family_tuple,
                    requirement_identity: identities.intern("package::Scanner::scan"),
                    realization_identity: identities.intern("package::Carrier::scan"),
                    realization_callable_identity: None,
                }],
                report_fingerprint: 0,
                commitment: Default::default(),
            }
        }

        fn width_lanes_tuple(
            identities: &mut crate::TerminalIdentityTableBuilder,
        ) -> Vec<crate::TerminalIdentityIndex> {
            vec![
                identities.intern("named(integer-const(16))"),
                identities.intern("named(integer-const(4))"),
            ]
        }

        #[test]
        fn family_tuple_enters_row_commitment_and_report_fingerprint() {
            let mut builder = crate::TerminalIdentityTableBuilder::default();
            let nongeneric = application(&mut builder, Vec::new());
            let tuple = width_lanes_tuple(&mut builder);
            let width_lanes = application(&mut builder, tuple);
            let mut reversed = width_lanes_tuple(&mut builder);
            reversed.reverse();
            let lanes_width = application(&mut builder, reversed);
            let identities = builder.snapshot();

            let commitments = [
                closed_conformance_application_commitment(&identities, &nongeneric),
                closed_conformance_application_commitment(&identities, &width_lanes),
                closed_conformance_application_commitment(&identities, &lanes_width),
            ];
            let fingerprints = [
                closed_conformance_application_report_fingerprint(&identities, &nongeneric),
                closed_conformance_application_report_fingerprint(&identities, &width_lanes),
                closed_conformance_application_report_fingerprint(&identities, &lanes_width),
            ];
            assert!(commitments.iter().all(|commitment| !commitment.is_zero()));
            assert_ne!(commitments[0], commitments[1]);
            assert_ne!(commitments[1], commitments[2]);
            assert_ne!(commitments[0], commitments[2]);
            assert_ne!(fingerprints[0], fingerprints[1]);
            assert_ne!(fingerprints[1], fingerprints[2]);
            assert_ne!(fingerprints[0], fingerprints[2]);
            assert_eq!(
                closed_conformance_application_commitment(&identities, &{
                    let tuple = width_lanes_tuple(&mut builder);
                    application(&mut builder, tuple)
                }),
                commitments[1]
            );
        }
    }
}
pub(crate) mod declarations {
    use crate::terminal_module::TerminalIdentityIndex;
    use crate::{
        CrashRouteBucket, RetainedBorrowCustody, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralPlaceDeclaration, ValueDeclaration,
    };
    use semantic_vocabulary::{
        BoundaryMachineId, ContentAlgebra, ContentConservation, ContentProjectionExpression,
        ContentProjectionIdentity, ContentProjectionScalar, Proposition, ScalarType, ServiceId,
        StructuralDomainId, StructuralTypeId, ValueId,
    };

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct StructuralDomainRequirement {
        pub argument_index: u32,
        pub path: Vec<crate::StructuralPathSegment>,
        pub domain: StructuralDomainId,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ScalarDomainRequirement {
        pub argument_index: u32,
        pub domain: semantic_vocabulary::ScalarDomainId,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct BoundaryStructuralResultDeclaration {
        pub structural_type: StructuralTypeId,
        pub multiplicity: StructuralMultiplicity,
        pub qualifications: Vec<StructuralDomainId>,
        pub projected_qualifications: Vec<crate::StructuralPathQualification>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum BoundaryMachineResult {
        Unit,
        Scalar(semantic_vocabulary::QualifiedScalarType),
        Structural(BoundaryStructuralResultDeclaration),
    }

    impl BoundaryMachineResult {
        pub fn accepts_provider_result(
            &self,
            result: &crate::TerminalMachineResult,
            requirement: TerminalIdentityIndex,
            declaration: Option<TerminalIdentityIndex>,
            domains: &[crate::StructuralDomainDeclaration],
        ) -> bool {
            match (self, result) {
                (Self::Structural(expected), crate::TerminalMachineResult::Structural(result)) => {
                    expected.structural_type == result.structural_type
                        && expected.multiplicity == result.multiplicity
                        && result
                            .qualifications
                            .iter()
                            .all(|domain| expected.qualifications.contains(domain))
                        && expected.qualifications.iter().all(|domain| {
                            domains.iter().any(|domain_declaration| {
                                domain_declaration.id == *domain
                                    && domain_declaration.carrier == expected.structural_type
                                    && domain_declaration
                                        .missing_membership_requirements(
                                            domains,
                                            &result.qualifications,
                                        )
                                        .is_some_and(|missing| {
                                            missing.into_iter().all(|required| {
                                                required.establishment_routes.is_empty()
                                                    || required.authorizes_boundary_requirement(
                                                        requirement,
                                                        declaration,
                                                    )
                                            })
                                        })
                            })
                        })
                        && expected
                            .projected_qualifications
                            .iter()
                            .all(|row| result.projected_qualifications.contains(row))
                }
                _ => self.matches_machine_result(result),
            }
        }

        pub fn matches_machine_result(&self, result: &crate::TerminalMachineResult) -> bool {
            match (self, result) {
                (Self::Unit, crate::TerminalMachineResult::Unit) => true,
                (Self::Scalar(expected), crate::TerminalMachineResult::Scalar(result)) => {
                    *expected == result.value_type()
                }
                (Self::Structural(expected), crate::TerminalMachineResult::Structural(result)) => {
                    expected.structural_type == result.structural_type
                        && expected.multiplicity == result.multiplicity
                        && expected.qualifications == result.qualifications
                        && expected.projected_qualifications == result.projected_qualifications
                }
                _ => false,
            }
        }

        pub fn matches_operation_result(&self, result: &crate::OperationResult) -> bool {
            match (self, result) {
                (Self::Unit, crate::OperationResult::Unit) => true,
                (Self::Scalar(expected), crate::OperationResult::Scalar(result)) => {
                    *expected == result.value_type()
                }
                (Self::Structural(expected), crate::OperationResult::Structural(result)) => {
                    expected.structural_type == result.structural_type
                        && expected.multiplicity == result.multiplicity
                        && expected.qualifications == result.qualifications
                        && expected.projected_qualifications == result.projected_qualifications
                }
                _ => false,
            }
        }

        pub const fn scalar(&self) -> Option<ScalarType> {
            match self {
                Self::Scalar(scalar) => Some(scalar.scalar_type),
                Self::Unit | Self::Structural(_) => None,
            }
        }

        pub const fn is_unit(&self) -> bool {
            matches!(self, Self::Unit)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum BoundaryParameterKind {
        Scalar,
        Structural,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum BoundaryDeclarationRole {
        Callable,
        ContractEvidence,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct BoundaryReferenceOutput {
        pub destination: BoundaryReferenceDestination,
        pub path: Vec<crate::StructuralPathSegment>,
        pub access: crate::StructuralAccess,
        pub dependencies: Vec<BoundaryLifetimeDependency>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum BoundaryReferenceDestination {
        Parameter(u32),
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum BoundaryLifetimeDependency {
        Referent {
            argument_index: u32,
            access: crate::StructuralAccess,
        },
        Stored {
            argument_index: u32,
            path: Vec<crate::StructuralPathSegment>,
            access: crate::StructuralAccess,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct BoundaryMachineDeclaration {
        pub id: BoundaryMachineId,
        pub declaration: Option<CheckedBoundaryCallable>,
        pub numeric_intrinsic: Option<language_semantics::numeric_intrinsics::NumericIntrinsic>,
        pub role: BoundaryDeclarationRole,
        pub kind: semantic_vocabulary::BoundaryRequirementKind,
        pub completion: semantic_vocabulary::BoundaryCompletion,
        pub unconditional_termination: bool,
        pub identity: TerminalIdentityIndex,
        pub owner: crate::NominalDeclarationIdentity,
        pub attachment: Option<StructuralTypeId>,
        pub parameter_order: Vec<BoundaryParameterKind>,
        pub scalar_parameters: Vec<ScalarType>,
        pub crash_routes: Vec<CrashRouteBucket>,
        pub structural_parameters: Vec<StructuralParameterDeclaration>,
        pub result: BoundaryMachineResult,
        pub result_reference_regions: Vec<crate::TerminalDynamicReferenceRegion>,
        pub requires: Vec<StructuralDomainRequirement>,
        pub normal_qualifications: Vec<StructuralDomainRequirement>,
        pub scalar_domain_requirements: Vec<ScalarDomainRequirement>,
        pub scalar_requires: Vec<Proposition>,
        pub scalar_ensures: Vec<Proposition>,
        pub reference_outputs: Vec<BoundaryReferenceOutput>,
        pub program_local_root_introductions: Vec<ProgramLocalRootIntroductionSchema>,
        pub content_guarantees: Vec<BoundaryContentGuarantee>,
        pub fixed_service_reach: Vec<ServiceId>,
        pub owning_service: Option<ServiceId>,
        pub published_service_ceiling: Vec<ServiceId>,
        /// The entries every admitted implementation may arrive at before
        /// returning: the requirement's written `invokes`, where omission
        /// means none. Strictly increasing, no parent closure.
        pub published_synchronous_entries: Vec<ServiceId>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct CheckedBoundaryCallable {
        pub source: crate::CheckedMachineDeclaration,
        pub execution: CheckedBoundaryExecution,
        pub method_arguments: Vec<crate::ClosedReachArgument>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CheckedBoundaryExecution {
        Provider,
        AccessInitialize,
        AccessMoveOut,
        AccessRelocate,
        AccessView,
        EmptyRunRelease,
        StaticInterruptInstall,
        StaticInterruptContext,
        StaticInterruptBeginCompletion,
        StaticInterruptFinishCompletion,
    }

    impl CheckedBoundaryExecution {
        pub const fn static_interrupt_operation(
            self,
        ) -> Option<language_core::static_interrupts::StaticInterruptOperation> {
            use language_core::static_interrupts::StaticInterruptOperation;
            match self {
                Self::StaticInterruptInstall => Some(StaticInterruptOperation::Install),
                Self::StaticInterruptContext => Some(StaticInterruptOperation::Context),
                Self::StaticInterruptBeginCompletion => {
                    Some(StaticInterruptOperation::BeginCompletion)
                }
                Self::StaticInterruptFinishCompletion => {
                    Some(StaticInterruptOperation::FinishCompletion)
                }
                Self::Provider
                | Self::EmptyRunRelease
                | Self::AccessInitialize
                | Self::AccessMoveOut
                | Self::AccessRelocate
                | Self::AccessView => None,
            }
        }

        pub const fn compiler_owned(self) -> bool {
            !matches!(self, Self::Provider)
        }

        pub const fn placement_operation(self) -> Option<crate::TerminalAccessLifecycleOperation> {
            match self {
                Self::Provider
                | Self::EmptyRunRelease
                | Self::StaticInterruptInstall
                | Self::StaticInterruptContext
                | Self::StaticInterruptBeginCompletion
                | Self::StaticInterruptFinishCompletion => None,
                Self::AccessInitialize => Some(crate::TerminalAccessLifecycleOperation::Initialize),
                Self::AccessMoveOut => Some(crate::TerminalAccessLifecycleOperation::MoveOut),
                Self::AccessRelocate => Some(crate::TerminalAccessLifecycleOperation::Relocate),
                Self::AccessView => Some(crate::TerminalAccessLifecycleOperation::View),
            }
        }
    }

    impl CheckedBoundaryCallable {
        pub fn same_identity_as(
            &self,
            identities: &crate::TerminalIdentityTable,
            other: &Self,
            other_identities: &crate::TerminalIdentityTable,
        ) -> bool {
            self.source
                .same_identity_as(identities, &other.source, other_identities)
                && self.execution == other.execution
                && self.method_arguments.len() == other.method_arguments.len()
                && self
                    .method_arguments
                    .iter()
                    .zip(&other.method_arguments)
                    .all(|(left, right)| left.same_identity_as(identities, right, other_identities))
        }
    }

    impl BoundaryMachineDeclaration {
        pub fn requires_checked_result(
            &self,
            domains: &[crate::StructuralDomainDeclaration],
        ) -> bool {
            let BoundaryMachineResult::Structural(result) = &self.result else {
                return false;
            };
            let consumes_qualification = |required: StructuralDomainId| {
                self.structural_parameters
                    .iter()
                    .enumerate()
                    .any(|(position, parameter)| {
                        parameter.access == crate::StructuralAccess::Owned
                            && parameter
                                .qualifications
                                .iter()
                                .chain(
                                    parameter
                                        .projected_qualifications
                                        .iter()
                                        .filter(|row| {
                                            !row.path
                                                .contains(&crate::StructuralPathSegment::Referent)
                                        })
                                        .map(|row| &row.domain),
                                )
                                .chain(
                                    self.requires
                                        .iter()
                                        .filter(|row| {
                                            usize::try_from(row.argument_index).ok()
                                                == Some(position)
                                                && !row.path.contains(
                                                    &crate::StructuralPathSegment::Referent,
                                                )
                                        })
                                        .map(|row| &row.domain),
                                )
                                .any(|domain| {
                                    domains
                                        .iter()
                                        .find(|declaration| declaration.id == *domain)
                                        .and_then(|declaration| {
                                            declaration.required_domain_closure(domains)
                                        })
                                        .is_some_and(|closure| {
                                            closure.iter().any(|node| node.id == required)
                                        })
                                })
                    })
            };
            result
                .qualifications
                .iter()
                .chain(
                    result
                        .projected_qualifications
                        .iter()
                        .map(|row| &row.domain),
                )
                .any(|domain| {
                    domains
                        .iter()
                        .find(|declaration| declaration.id == *domain)
                        .is_none_or(|declaration| {
                            declaration
                                .required_domain_closure(domains)
                                .is_none_or(|required| {
                                    required.into_iter().any(|node| {
                                        (node.id != declaration.id
                                            && node.content_projection.is_some())
                                            || ((node.content_projection.is_none()
                                                || !consumes_qualification(node.id))
                                                && !node.establishment_routes.is_empty()
                                                && !node.authorizes_boundary_requirement(
                                                    self.identity,
                                                    self.declaration.as_ref().map(|callable| {
                                                        callable.source.semantic_declaration
                                                    }),
                                                ))
                                    })
                                })
                        })
                })
        }

        pub fn has_valid_parameter_order(&self) -> bool {
            self.parameter_order.len()
                == self.scalar_parameters.len() + self.structural_parameters.len()
                && self
                    .parameter_positions(BoundaryParameterKind::Scalar)
                    .count()
                    == self.scalar_parameters.len()
        }

        pub fn parameter_positions(
            &self,
            kind: BoundaryParameterKind,
        ) -> impl Iterator<Item = usize> + '_ {
            self.parameter_order
                .iter()
                .enumerate()
                .filter_map(move |(position, found)| (*found == kind).then_some(position))
        }

        pub fn scalar_contract_result(&self) -> Option<ValueDeclaration> {
            Some(ValueDeclaration {
                id: ValueId::new(
                    u64::try_from(self.scalar_parameters.len())
                        .ok()?
                        .checked_add(1)?,
                )?,
                scalar_type: self.result.scalar()?,
                qualifications: Default::default(),
            })
        }

        pub fn scalar_contract_parameters(&self) -> Option<Vec<ValueDeclaration>> {
            self.scalar_parameters
                .iter()
                .enumerate()
                .map(|(position, scalar_type)| {
                    Some(ValueDeclaration {
                        id: ValueId::new(u64::try_from(position).ok()?.checked_add(1)?)?,
                        scalar_type: *scalar_type,
                        qualifications: Default::default(),
                    })
                })
                .collect()
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ContentConservationGuarantee {
        pub report_fingerprint: u64,
        pub structural_places: Vec<StructuralPlaceDeclaration>,
        pub conservation: ContentConservation,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum BoundaryContentGuarantee {
        Conservation(ContentConservationGuarantee),
        RetainedBorrow(RetainedBorrowCustody),
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ProgramLocalRootIntroductionSchema {
        pub argument_index: u32,
        pub source_parameter_position: u32,
        pub qualification: StructuralDomainId,
        pub carrier: StructuralTypeId,
        pub projection: ContentProjectionIdentity,
        pub algebra: ContentAlgebra,
        pub capacity: ContentProjectionExpression,
        pub compatibility_report_identity: u64,
    }

    pub fn find_boundary_machine(
        declarations: &[BoundaryMachineDeclaration],
        id: BoundaryMachineId,
    ) -> Option<&BoundaryMachineDeclaration> {
        declarations.iter().find(|declaration| declaration.id == id)
    }

    pub fn program_local_root_introduction_compatibility_report_identity(
        requirement_identity: &str,
        qualification_identity: &str,
        carrier_identity: &str,
        schema: &ProgramLocalRootIntroductionSchema,
    ) -> u64 {
        fn bytes(hash: &mut u64, value: &[u8]) {
            for byte in value {
                *hash ^= u64::from(*byte);
                *hash = hash.wrapping_mul(1_099_511_628_211);
            }
        }
        fn string(hash: &mut u64, value: &str) {
            bytes(hash, &(value.len() as u64).to_le_bytes());
            bytes(hash, value.as_bytes());
        }
        fn scalar(hash: &mut u64, value: &ContentProjectionScalar) {
            match value {
                ContentProjectionScalar::SubjectField(path)
                | ContentProjectionScalar::RuntimeScalarEmbedding(path) => {
                    bytes(
                        hash,
                        &[
                            if matches!(value, ContentProjectionScalar::SubjectField(_)) {
                                1
                            } else {
                                2
                            },
                        ],
                    );
                    bytes(hash, &(path.len() as u64).to_le_bytes());
                    for segment in path {
                        string(hash, segment);
                    }
                }
                ContentProjectionScalar::Natural(value) => {
                    bytes(hash, &[3]);
                    string(hash, value);
                }
                ContentProjectionScalar::Successor(inner) => {
                    bytes(hash, &[4]);
                    scalar(hash, inner);
                }
                ContentProjectionScalar::Add(left, right)
                | ContentProjectionScalar::Subtract(left, right)
                | ContentProjectionScalar::Multiply(left, right) => {
                    bytes(
                        hash,
                        &[match value {
                            ContentProjectionScalar::Add(_, _) => 5,
                            ContentProjectionScalar::Subtract(_, _) => 6,
                            ContentProjectionScalar::Multiply(_, _) => 7,
                            _ => unreachable!(),
                        }],
                    );
                    scalar(hash, left);
                    scalar(hash, right);
                }
            }
        }
        let mut hash = 14_695_981_039_346_656_037_u64;
        bytes(&mut hash, b"psi.program-local-root-introduction.v1");
        string(&mut hash, requirement_identity);
        string(&mut hash, qualification_identity);
        string(&mut hash, carrier_identity);
        bytes(&mut hash, &schema.argument_index.to_le_bytes());
        bytes(&mut hash, &schema.source_parameter_position.to_le_bytes());
        bytes(
            &mut hash,
            &schema
                .projection
                .projection_report_fingerprint
                .to_le_bytes(),
        );
        bytes(
            &mut hash,
            &[match schema.algebra.kind {
                semantic_vocabulary::ContentAlgebraKind::IntervalSet => 1,
                semantic_vocabulary::ContentAlgebraKind::CountedQuantity => 2,
            }],
        );
        string(&mut hash, &schema.algebra.parameter);
        match &schema.capacity {
            ContentProjectionExpression::IntervalSet(members) => {
                bytes(&mut hash, &[1]);
                bytes(&mut hash, &(members.len() as u64).to_le_bytes());
                for (start, end) in members {
                    scalar(&mut hash, start);
                    scalar(&mut hash, end);
                }
            }
            ContentProjectionExpression::CountedQuantity(magnitude) => {
                bytes(&mut hash, &[2]);
                scalar(&mut hash, magnitude);
            }
        }
        if hash == 0 { 1 } else { hash }
    }
}
pub(crate) mod external_providers {

    use crate::{
        CheckedMachineDeclaration, NominalDeclarationIdentity, TerminalIdentityIndex,
        TerminalIdentityTable,
    };
    use language_semantics::ExternalBindingIdentity;
    use semantic_vocabulary::PackageKeyIdentity;

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ExternalProviderOwner {
        pub declaration: NominalDeclarationIdentity,
        pub identity: TerminalIdentityIndex,
        pub package: Option<PackageKeyIdentity>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ExternalRequirementSchema {
        pub identity: TerminalIdentityIndex,
        pub method: TerminalIdentityIndex,
        pub owner: TerminalIdentityIndex,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ExternalProviderConformance {
        pub requirement: NominalDeclarationIdentity,
        pub requirement_identity: TerminalIdentityIndex,
        pub requirement_package: Option<PackageKeyIdentity>,
        pub numeric_intrinsic: Option<language_semantics::numeric_intrinsics::NumericIntrinsic>,
        pub sealed_intrinsic: Option<crate::TerminalSealedNativeIntrinsic>,
        pub requirement_schema: Option<ExternalRequirementSchema>,
        pub target: Option<String>,
        pub realization: CheckedMachineDeclaration,
        pub realization_package: Option<PackageKeyIdentity>,
        pub provider: Option<ExternalProviderOwner>,
        pub binding: ExternalBindingIdentity,
    }

    impl ExternalProviderConformance {
        pub fn validate(&self, identities: &TerminalIdentityTable) -> Result<(), &'static str> {
            let declaration = &self.realization;
            let present = |identity| {
                identities
                    .text(identity)
                    .is_some_and(|text| !text.is_empty())
            };
            if self.sealed_intrinsic.is_some()
                && (self.numeric_intrinsic.is_some()
                    || self.binding != ExternalBindingIdentity::CompilerIntrinsic)
            {
                return Err("sealed native meaning differs from its intrinsic binding");
            }
            if self.numeric_intrinsic.is_some() && self.requirement_schema.is_none() {
                return Err("numeric provider lost its exact requirement schema");
            }
            if self.requirement_schema.as_ref().is_some_and(|schema| {
                ![schema.identity, schema.method, schema.owner]
                    .into_iter()
                    .all(present)
            }) {
                return Err("external provider has an incomplete requirement schema");
            }
            if self.target.as_ref().is_some_and(String::is_empty)
                || self
                    .numeric_intrinsic
                    .is_some_and(|meaning| meaning.signature().is_none())
            {
                return Err("external provider has invalid target membership or numeric meaning");
            }
            if ![
                self.requirement.identity(),
                self.requirement_identity,
                declaration.machine.identity(),
                declaration.state.identity(),
                declaration.semantic_declaration,
                declaration.callable,
                declaration.result_type,
            ]
            .into_iter()
            .chain(declaration.parameter_types.iter().copied())
            .all(present)
                || declaration.contract == [0; 32]
            {
                return Err("external provider lost its exact checked declaration");
            }
            if let Some(specialization) = &declaration.specialization
                && (![
                    specialization.template.identity(),
                    specialization.template_semantic_declaration,
                    specialization.template_callable,
                ]
                .into_iter()
                .all(present)
                    || specialization.commitment == [0; 32]
                    || specialization.capture.as_ref().is_some_and(|capture| {
                        !present(capture.template_identity)
                            || capture.template_commitment == [0; 32]
                            || capture.specialization_commitment == [0; 32]
                    }))
            {
                return Err("external provider lost its checked specialization");
            }
            if self.provider.as_ref().is_some_and(|provider| {
                !present(provider.declaration.identity()) || !present(provider.identity)
            }) {
                return Err("external provider lost its selecting declaration");
            }
            match &self.binding {
                ExternalBindingIdentity::VtableField { field }
                | ExternalBindingIdentity::TableFunction { field }
                    if field.is_empty() =>
                {
                    Err("external provider has an empty binding field")
                }
                _ => Ok(()),
            }
        }
    }
}
pub use external_providers::{
    ExternalProviderConformance, ExternalProviderOwner, ExternalRequirementSchema,
};
pub(crate) mod non_observation {

    use super::BoundaryMachineDeclaration;
    use crate::{ByteSequenceCarrier, StructuralAccess, StructuralTypeDeclaration};
    use crate::{StructuralTypeShape, structural_children};
    use semantic_vocabulary::StructuralTypeId;
    use std::collections::BTreeSet;

    impl BoundaryMachineDeclaration {
        pub fn requires_checked_non_observation<'types>(
            &self,
            lookup: impl Fn(StructuralTypeId) -> Option<&'types StructuralTypeDeclaration>,
        ) -> bool {
            if self
                .structural_parameters
                .iter()
                .any(|parameter| parameter.access == StructuralAccess::WriteOnlyBorrow)
            {
                return true;
            }
            let mut pending: Vec<_> = self
                .structural_parameters
                .iter()
                .map(|parameter| parameter.structural_type)
                .collect();
            let mut visited = BTreeSet::new();
            while let Some(current) = pending.pop() {
                if !visited.insert(current) {
                    continue;
                }
                let Some(declaration) = lookup(current) else {
                    continue;
                };
                let mut fields_require = |fields: &[crate::StructuralFieldDeclaration]| {
                    for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                        pending.extend(structural_children(std::slice::from_ref(field)));
                        if field
                            .field_type
                            .byte_sequence_carrier()
                            .is_some_and(write_only_bytes)
                        {
                            return true;
                        }
                    }
                    false
                };
                match &declaration.shape {
                    StructuralTypeShape::Reference { referent, access } => {
                        if *access == StructuralAccess::WriteOnlyBorrow {
                            return true;
                        }
                        pending.push(*referent);
                    }
                    StructuralTypeShape::ElementView { element, access } => {
                        if *access == Some(StructuralAccess::WriteOnlyBorrow) {
                            return true;
                        }
                        pending.push(*element);
                    }
                    StructuralTypeShape::ByteSequence(carrier) if write_only_bytes(*carrier) => {
                        return true;
                    }
                    StructuralTypeShape::Record { fields, .. } if fields_require(fields) => {
                        return true;
                    }
                    StructuralTypeShape::Sum { cases }
                        if cases.iter().any(|case| fields_require(&case.fields)) =>
                    {
                        return true;
                    }
                    StructuralTypeShape::Mixed { fields, cases }
                        if fields_require(fields)
                            || cases.iter().any(|case| fields_require(&case.fields)) =>
                    {
                        return true;
                    }
                    StructuralTypeShape::FixedArray { element, length }
                        if *length != crate::ArrayExtent::Literal(0) =>
                    {
                        pending.push(*element);
                    }
                    _ => {}
                }
            }
            false
        }
    }

    fn write_only_bytes(carrier: ByteSequenceCarrier) -> bool {
        matches!(
            carrier,
            ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::WriteOnlyBorrow)
            } | ByteSequenceCarrier::BoundedView {
                access: Some(StructuralAccess::WriteOnlyBorrow),
                ..
            }
        )
    }
}
pub(crate) mod numeric_intrinsics {

    pub fn validate_numeric_intrinsic_signature(
        boundary: &crate::BoundaryMachineDeclaration,
        structural_types: &[crate::StructuralTypeDeclaration],
    ) -> Result<(), &'static str> {
        use language_semantics::numeric_intrinsics::NumericResult;
        let Some(meaning) = boundary.numeric_intrinsic else {
            return Ok(());
        };
        let signature = meaning
            .signature()
            .ok_or("unsupported numeric intrinsic meaning")?;
        if boundary.role != crate::BoundaryDeclarationRole::Callable
            || boundary.completion != crate::BoundaryCompletion::NormalReturn
            || boundary.attachment.is_some()
            || !boundary.structural_parameters.is_empty()
            || boundary.scalar_parameters.len() != signature.operand_count
            || boundary
                .scalar_parameters
                .iter()
                .any(|parameter| Some(*parameter) != signature.operand.scalar_type())
        {
            return Err("numeric intrinsic has a mismatched boundary parameter signature");
        }
        let result_matches = match (&boundary.result, signature.result) {
            (crate::BoundaryMachineResult::Scalar(result), NumericResult::Primitive(primitive)) => {
                Some(result.scalar_type) == primitive.scalar_type()
            }
            (crate::BoundaryMachineResult::Scalar(result), NumericResult::Boolean) => {
                result.scalar_type == semantic_vocabulary::ScalarType::Boolean
            }
            (crate::BoundaryMachineResult::Structural(result), NumericResult::FloatClass) => {
                structural_types
                    .iter()
                    .find(|declaration| declaration.id == result.structural_type)
                    .is_some_and(crate::StructuralTypeDeclaration::is_float_class_shape)
            }
            _ => false,
        };
        if !result_matches {
            return Err("numeric intrinsic has a mismatched boundary result signature");
        }
        Ok(())
    }
}
pub use numeric_intrinsics::validate_numeric_intrinsic_signature;
pub(crate) mod dynamic_dispatch {

    use crate::terminal_module::TerminalIdentityIndex;
    use semantic_vocabulary::{MachineId, ScalarType, StructuralDomainId, StructuralTypeId};

    use crate::{StructuralAccess, StructuralMultiplicity, StructuralPathSegment};

    #[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalDynamicDispatchCatalog {
        pub interfaces: Vec<TerminalDynamicInterface>,
        pub tables: Vec<TerminalDynamicTable>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalDynamicInterface {
        pub id: u32,
        pub identity: TerminalIdentityIndex,
        pub slots: Vec<TerminalDynamicSlot>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalDynamicSlot {
        pub slot: u32,
        pub declaring_trait_identity: TerminalIdentityIndex,
        pub requirement_identity: TerminalIdentityIndex,
        pub family_tuple: Vec<String>,
        pub scalar_parameters: Vec<ScalarType>,
        pub structural_parameters: Vec<TerminalDynamicStructuralFormal>,
        pub entry_claim_paths: Vec<TerminalDynamicClaimPath>,
        pub result: crate::BoundaryMachineResult,
        pub result_claim_paths: Vec<Vec<StructuralPathSegment>>,
        pub result_reference_regions: Vec<TerminalDynamicReferenceRegion>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalDynamicReferenceRegion {
        pub path: Vec<StructuralPathSegment>,
        pub origin: TerminalDynamicReferenceOrigin,
        pub source_path: Vec<StructuralPathSegment>,
        pub access: StructuralAccess,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalDynamicReferenceOrigin {
        Parameter { argument_index: u32 },
        Invocation { carrier_type: StructuralTypeId },
    }

    impl TerminalDynamicSlot {
        pub fn matches_operation_result(&self, result: &crate::OperationResult) -> bool {
            if !self.result.matches_operation_result(result) {
                return false;
            }
            let mut paths = result.structural().map_or_else(Vec::new, |result| {
                result
                    .claims
                    .iter()
                    .map(|claim| claim.path.as_slice())
                    .collect()
            });
            paths.sort();
            paths
                .iter()
                .copied()
                .eq(self.result_claim_paths.iter().map(Vec::as_slice))
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalDynamicClaimPath {
        pub argument_index: u32,
        pub path: Vec<StructuralPathSegment>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalDynamicStructuralFormal {
        pub position: u32,
        pub structural_type: StructuralTypeId,
        pub access: StructuralAccess,
        pub multiplicity: StructuralMultiplicity,
        pub qualifications: Vec<StructuralDomainId>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalDynamicTable {
        pub id: u32,
        pub interface: u32,
        pub implementation_identity: TerminalIdentityIndex,
        pub rows: Vec<Option<MachineId>>,
    }

    impl TerminalDynamicDispatchCatalog {
        pub fn interface(&self, id: u32) -> Option<&TerminalDynamicInterface> {
            self.interfaces.get(usize::try_from(id).ok()?)
        }

        pub fn table(&self, id: u32) -> Option<&TerminalDynamicTable> {
            self.tables.get(usize::try_from(id).ok()?)
        }

        pub fn row(&self, table: u32, slot: u32) -> Option<MachineId> {
            *self.table(table)?.rows.get(usize::try_from(slot).ok()?)?
        }
    }
}
pub(crate) mod providers {
    use crate::terminal_module::TerminalIdentityIndex;
    use crate::{
        StructuralAccess, StructuralDomainRequirement, StructuralMultiplicity,
        StructuralPathQualification,
    };
    use semantic_vocabulary::{
        BoundaryMachineId, MachineId, ServiceId, StructuralDomainId, StructuralTypeId,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProviderInstanceRequirements {
        pub template: TerminalIdentityIndex,
        pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub selecting_declaration: Option<crate::NominalDeclarationIdentity>,
        pub source_candidate_identity: TerminalIdentityIndex,
        pub required_instances: Vec<RequiredProviderInstance>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct RequiredProviderInstance {
        pub requirement_identity: TerminalIdentityIndex,
        pub candidate_identity: TerminalIdentityIndex,
        pub declaration_application: TerminalIdentityIndex,
        pub receiving_application: TerminalIdentityIndex,
        pub receiving_region_arguments: Vec<TerminalIdentityIndex>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ProviderCandidateConformance {
        pub boundary: BoundaryMachineId,
        pub requirement_identity: TerminalIdentityIndex,
        pub source_requirement_identity: TerminalIdentityIndex,
        pub declaration_application: TerminalIdentityIndex,
        pub receiving_application: TerminalIdentityIndex,
        pub receiving_region_arguments: Vec<TerminalIdentityIndex>,
        pub provider_identity: TerminalIdentityIndex,
        pub provider_declaration: crate::NominalDeclarationIdentity,
        pub candidate_identity: TerminalIdentityIndex,
        pub source_candidate_identity: TerminalIdentityIndex,
        pub source_candidate_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub candidate: MachineId,
        pub signature: ProviderSignature,
        pub refinement: ProviderRefinement,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ProviderSignature {
        pub parameters: Vec<ProviderSignatureParameter>,
    }

    impl ProviderSignature {
        pub fn empty() -> Self {
            Self {
                parameters: Vec::new(),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ProviderSignatureParameter {
        pub position: u32,
        pub structural_type: StructuralTypeId,
        pub multiplicity: StructuralMultiplicity,
        pub access: StructuralAccess,
        pub qualifications: Vec<StructuralDomainId>,
        pub projected_qualifications: Vec<StructuralPathQualification>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ProviderParameterRefinement {
        pub boundary_index: u32,
        pub candidate_index: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ProviderRefinement {
        pub positional_parameters: Vec<ProviderParameterRefinement>,
        pub required_domains: Vec<StructuralDomainRequirement>,
        pub realized_service_ceiling: Vec<ServiceId>,
    }
    impl ProviderRefinement {
        pub fn empty() -> Self {
            Self {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            }
        }
    }
}
pub(crate) mod reach_applications {

    use crate::terminal_module::TerminalIdentityIndex;
    use semantic_vocabulary::{MachineId, ServiceId};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ClosedReachApplication {
        pub template_identity: TerminalIdentityIndex,
        pub template_base_commitment: [u8; 32],
        pub template_commitment: [u8; 32],
        pub suspension_dependency: flow_effects::ContractDependency<bool, Vec<u32>>,
        pub blocking_dependency: flow_effects::ContractDependency<bool, Vec<u32>>,
        pub conditional_carry: Vec<flow_effects::ConditionalCarryRequirement>,
        pub specialization_commitment: [u8; 32],
        pub parameter_kinds: Vec<u8>,
        pub telescope: Vec<ClosedReachParameter>,
        pub capture: Option<ClosedReachCapture>,
        pub fixed: Vec<ServiceId>,
        pub dependencies: Vec<u32>,
        pub calls: Vec<ClosedReachCall>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ClosedReachCapture {
        pub template_identity: TerminalIdentityIndex,
        pub template_base_commitment: [u8; 32],
        pub template_commitment: [u8; 32],
        pub suspension_dependency: flow_effects::ContractDependency<bool, Vec<u32>>,
        pub blocking_dependency: flow_effects::ContractDependency<bool, Vec<u32>>,
        pub conditional_carry: Vec<flow_effects::ConditionalCarryRequirement>,
        pub specialization_commitment: [u8; 32],
        pub parameter_kinds: Vec<u8>,
        pub telescope: Vec<ClosedReachParameter>,
    }

    impl ClosedReachApplication {
        pub fn parameters(&self) -> impl Iterator<Item = &ClosedReachParameter> {
            self.capture
                .iter()
                .flat_map(|capture| &capture.telescope)
                .chain(&self.telescope)
        }

        pub fn parameter(&self, position: u32) -> Option<&ClosedReachParameter> {
            self.parameters().nth(position as usize)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ClosedReachParameter {
        Type { argument: String },
        Const { argument: String },
        Proposition { argument: String },
        Machine(ClosedReachMachineBinding),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ClosedReachMachineBinding {
        pub nominal_requirement: Option<String>,
        pub requirement_parameter_kinds: Vec<u8>,
        pub requirement_arguments: Vec<TerminalIdentityIndex>,
        pub upper_bound: Vec<ServiceId>,
        pub upper_bound_may_suspend: bool,
        pub upper_bound_may_block: bool,
        pub selected_may_suspend: bool,
        pub selected_may_block: bool,
        pub selected_identity: TerminalIdentityIndex,
        pub selected_contract_commitment: [u8; 32],
        pub selected_reach: Vec<ServiceId>,
        pub callee: Option<MachineId>,
        pub schema: Option<ClosedReachSchema>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ClosedReachSchema {
        pub template_identity: TerminalIdentityIndex,
        pub template_commitment: [u8; 32],
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ClosedReachCall {
        pub site: semantic_vocabulary::InvocationSite,
        pub binder: u32,
        pub application: Option<ClosedReachCallApplication>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ClosedReachCallApplication {
        pub callee: MachineId,
        pub specialization_commitment: [u8; 32],
        pub arguments: Vec<ClosedReachArgument>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ClosedReachArgument {
        Type(TerminalIdentityIndex),
        Const(TerminalIdentityIndex),
        Proposition(TerminalIdentityIndex),
        Machine {
            identity: TerminalIdentityIndex,
            contract_commitment: [u8; 32],
        },
    }

    impl ClosedReachArgument {
        pub fn same_identity_as(
            &self,
            identities: &crate::TerminalIdentityTable,
            other: &Self,
            other_identities: &crate::TerminalIdentityTable,
        ) -> bool {
            let (left, right) = match (self, other) {
                (Self::Type(left), Self::Type(right))
                | (Self::Const(left), Self::Const(right))
                | (Self::Proposition(left), Self::Proposition(right)) => (*left, *right),
                (
                    Self::Machine {
                        identity: left,
                        contract_commitment: left_contract,
                    },
                    Self::Machine {
                        identity: right,
                        contract_commitment: right_contract,
                    },
                ) if left_contract == right_contract => (*left, *right),
                _ => return false,
            };
            identities
                .text(left)
                .zip(other_identities.text(right))
                .is_some_and(|(left, right)| !left.is_empty() && left == right)
        }
    }
}
pub(crate) mod services {
    use crate::terminal_module::TerminalIdentityIndex;
    use semantic_vocabulary::ServiceId;

    #[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalRootServiceReach {
        pub concrete: Vec<ServiceId>,
        pub installation_dependencies: Vec<InstallationReachDependency>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct InstallationReachDependency {
        pub requirement_identity: TerminalIdentityIndex,
        pub upper_bound: Vec<ServiceId>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ServiceDeclaration {
        pub id: ServiceId,
        pub identity: TerminalIdentityIndex,
        pub parents: Vec<ServiceId>,
    }
}

pub use applications::{
    BoundaryApplication, BoundaryApplicationArgument, BoundaryApplicationCandidate,
    BoundaryOperatorRequirement, BoundaryTypeIdentity, TerminalBoundaryApplicationDemand,
};
pub use conformances::{
    ClosedConformanceApplication, ClosedConformanceApplicationCommitment,
    ClosedConformanceCallableResult, ClosedConformanceParameterBinding,
    ClosedConformanceParameterKind, ClosedConformanceRealizationCallable, ClosedConformanceRow,
    ClosedConformanceSchema, ClosedConformanceSchemaKind, ClosedConformanceStaticApplication,
    closed_conformance_application_commitment, closed_conformance_application_report_fingerprint,
};
pub use declarations::{
    BoundaryContentGuarantee, BoundaryDeclarationRole, BoundaryLifetimeDependency,
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind,
    BoundaryReferenceDestination, BoundaryReferenceOutput, BoundaryStructuralResultDeclaration,
    CheckedBoundaryCallable, CheckedBoundaryExecution, ContentConservationGuarantee,
    ProgramLocalRootIntroductionSchema, ScalarDomainRequirement, StructuralDomainRequirement,
    find_boundary_machine, program_local_root_introduction_compatibility_report_identity,
};
pub use dynamic_dispatch::{
    TerminalDynamicClaimPath, TerminalDynamicDispatchCatalog, TerminalDynamicInterface,
    TerminalDynamicReferenceOrigin, TerminalDynamicReferenceRegion, TerminalDynamicSlot,
    TerminalDynamicStructuralFormal, TerminalDynamicTable,
};
pub use providers::{
    ProviderCandidateConformance, ProviderInstanceRequirements, ProviderParameterRefinement,
    ProviderRefinement, ProviderSignature, ProviderSignatureParameter, RequiredProviderInstance,
};
pub use reach_applications::{
    ClosedReachApplication, ClosedReachArgument, ClosedReachCall, ClosedReachCallApplication,
    ClosedReachCapture, ClosedReachMachineBinding, ClosedReachParameter, ClosedReachSchema,
};
pub use services::{InstallationReachDependency, ServiceDeclaration, TerminalRootServiceReach};

pub(crate) mod result_regions {

    use crate::{
        BoundaryMachineDeclaration, BoundaryMachineResult, StructuralAccess,
        TerminalDynamicReferenceRegion, TerminalDynamicSlot,
    };
    use semantic_vocabulary::StructuralTypeId;

    pub trait ReferenceResultShape {
        fn result(&self) -> &BoundaryMachineResult;
        fn reference_regions(&self) -> &[TerminalDynamicReferenceRegion];
        fn structural_parameter_count(&self) -> usize;
        fn structural_parameter(
            &self,
            index: usize,
        ) -> Option<(StructuralTypeId, StructuralAccess)>;
    }

    impl ReferenceResultShape for BoundaryMachineDeclaration {
        fn result(&self) -> &BoundaryMachineResult {
            &self.result
        }
        fn reference_regions(&self) -> &[TerminalDynamicReferenceRegion] {
            &self.result_reference_regions
        }
        fn structural_parameter_count(&self) -> usize {
            self.structural_parameters.len()
        }
        fn structural_parameter(
            &self,
            index: usize,
        ) -> Option<(StructuralTypeId, StructuralAccess)> {
            self.structural_parameters
                .get(index)
                .map(|p| (p.structural_type, p.access))
        }
    }
    impl ReferenceResultShape for TerminalDynamicSlot {
        fn result(&self) -> &BoundaryMachineResult {
            &self.result
        }
        fn reference_regions(&self) -> &[TerminalDynamicReferenceRegion] {
            &self.result_reference_regions
        }
        fn structural_parameter_count(&self) -> usize {
            self.structural_parameters.len()
        }
        fn structural_parameter(
            &self,
            index: usize,
        ) -> Option<(StructuralTypeId, StructuralAccess)> {
            self.structural_parameters
                .get(index)
                .map(|p| (p.structural_type, p.access))
        }
    }
}
pub use result_regions::ReferenceResultShape;
