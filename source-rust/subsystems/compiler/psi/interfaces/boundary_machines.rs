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
        declaration: Option<&crate::NominalDeclarationIdentity>,
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
                                            required.establishment_routes.is_none()
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
            (Self::Structural(_), crate::TerminalMachineResult::Structural(result)) => {
                self.matches_structural_result(result)
            }
            _ => false,
        }
    }

    pub fn matches_structural_result(
        &self,
        result: &crate::StructuralResultDeclaration,
    ) -> bool {
        let Self::Structural(expected) = self else {
            return false;
        };
        expected.structural_type == result.structural_type
            && expected.multiplicity == result.multiplicity
            && expected.qualifications == result.qualifications
            && expected.projected_qualifications == result.projected_qualifications
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

    pub const fn structural(&self) -> Option<StructuralTypeId> {
        match self {
            Self::Structural(result) => Some(result.structural_type),
            Self::Unit | Self::Scalar(_) => None,
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
    pub numeric_intrinsic: Option<numerics::intrinsics::NumericIntrinsic>,
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
    /// Declared logical lifetime membership, independent of spatial
    /// containment in an input. `None` leaves the frontier unavailable;
    /// an empty input group promises a caller-chosen provider result.
    pub result_lifetimes: Option<Vec<crate::DeclaredResultLifetime>>,
    pub requires: Vec<StructuralDomainRequirement>,
    pub normal_qualifications: Vec<StructuralDomainRequirement>,
    pub scalar_domain_requirements: Vec<ScalarDomainRequirement>,
    pub membership_ensures: Vec<crate::RuntimeDomainMembership>,
    pub scalar_requires: Vec<Proposition>,
    pub scalar_ensures: Vec<Proposition>,
    pub reference_outputs: Vec<BoundaryReferenceOutput>,
    pub program_local_root_introductions: Vec<ProgramLocalRootIntroductionSchema>,
    pub content_guarantees: Vec<BoundaryContentGuarantee>,
    pub fixed_service_reach: Vec<ServiceId>,
    /// Installation substitutes this requirement's exact selected row;
    /// the published ceiling bounds that row rather than contributing it.
    pub service_reach_is_installation_bound: bool,
    pub owning_service: Option<ServiceId>,
    pub published_service_ceiling: Vec<ServiceId>,
    /// The entries every admitted implementation may arrive at before
    /// returning: the requirement's written `invokes`, where omission
    /// means none. Strictly increasing, no parent closure.
    pub published_synchronous_entries: Vec<ServiceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedBoundaryCallable {
    pub service_declaration: [u8; 32],
    pub source: crate::CheckedMachineDeclaration,
    pub execution: CheckedBoundaryExecution,
    pub method_arguments: Vec<crate::ClosedReachArgument>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckedBoundaryExecution {
    Provider,
    /// Exact sealed numeric declaration, realized by the selected target table.
    NumericPrimitive,
    /// The actual primitive remains distinct from the public requirement.
    HostedPrimitive {
        operation: language_core::sealed_intrinsics::HostedOperation,
        requirement: semantic_vocabulary::BoundaryMachineId,
    },
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
            | Self::NumericPrimitive
            | Self::HostedPrimitive { .. }
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
            | Self::NumericPrimitive
            | Self::HostedPrimitive { .. }
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
    /// A closed requirement keeps its source application while physical
    /// calling uses the original declaration and the same static tuple.
    /// Neither a common template nor a common runtime shape is sufficient.
    pub fn matches_calling_abi(
        &self,
        identities: &crate::TerminalIdentityTable,
        abi: &semantic_vocabulary::CallingAbiIdentity,
    ) -> Result<bool, String> {
        let text = |index| {
            identities
                .text(index)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    "boundary calling correspondence lost a source identity".to_owned()
                })
        };
        let Some(application) = &self.source.specialization else {
            return Ok(self.method_arguments.is_empty()
                && abi.requirement_identity == text(self.source.callable)?);
        };
        if application.capture.is_some()
            || application.template != application.template_entry
            || abi.requirement_identity != text(application.template_callable)?
            || !application_identity::type_identity::closed_requirement_matches_calling_abi(
                text(self.source.semantic_declaration)?,
                text(application.template_semantic_declaration)?,
                text(application.template.identity())?,
                text(self.source.callable)?,
                abi,
            )
            .map_err(|reason| reason.to_string())?
        {
            return Ok(false);
        }
        if self.method_arguments.len() != abi.operands.len()
            || self
                .method_arguments
                .iter()
                .zip(&abi.operands)
                .any(|(argument, operand)| {
                    use semantic_vocabulary::CallingOperandKind;
                    let index = match (argument, operand.kind) {
                        (crate::ClosedReachArgument::Type(index), CallingOperandKind::Type)
                        | (
                            crate::ClosedReachArgument::Const(index),
                            CallingOperandKind::Const | CallingOperandKind::Value,
                        ) => *index,
                        _ => return true,
                    };
                    identities.text(index) != Some(operand.identity.as_str())
                })
        {
            return Ok(false);
        }
        let arguments = self
            .method_arguments
            .iter()
            .map(|argument| {
                use application_identity::conformance_identity::MachineOperandSelectionIdentity;
                match argument {
                    crate::ClosedReachArgument::Type(index) => Ok(
                        MachineOperandSelectionIdentity::Type(text(*index)?.to_owned()),
                    ),
                    crate::ClosedReachArgument::Const(index) => Ok(
                        MachineOperandSelectionIdentity::Const(text(*index)?.to_owned()),
                    ),
                    _ => {
                        Err("boundary calling needs a complete type or value tuple".to_owned())
                    }
                }
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(
            application_identity::conformance_identity::requirement_application_commitment(
                text(application.template_callable)?,
                &arguments,
            )? == application.commitment,
        )
    }

    pub fn same_identity_as(
        &self,
        module: &crate::TerminalModule,
        other: &Self,
        other_module: &crate::TerminalModule,
    ) -> bool {
        let identities = &module.identities;
        let other_identities = &other_module.identities;
        let same_execution = match (self.execution, other.execution) {
            (
                CheckedBoundaryExecution::HostedPrimitive {
                    operation,
                    requirement,
                },
                CheckedBoundaryExecution::HostedPrimitive {
                    operation: other_operation,
                    requirement: other_requirement,
                },
            ) => {
                operation == other_operation
                    && module
                        .boundary_machine(requirement)
                        .zip(other_module.boundary_machine(other_requirement))
                        .is_some_and(|(left, right)| {
                            left.kind == crate::BoundaryRequirementKind::Trait
                                && right.kind == crate::BoundaryRequirementKind::Trait
                                && left
                                    .declaration
                                    .as_ref()
                                    .zip(right.declaration.as_ref())
                                    .is_some_and(|(left, right)| {
                                        left.execution == CheckedBoundaryExecution::Provider
                                            && right.execution
                                                == CheckedBoundaryExecution::Provider
                                            && left.source.same_identity_as(
                                                identities,
                                                &right.source,
                                                other_identities,
                                            )
                                    })
                        })
            }
            (CheckedBoundaryExecution::HostedPrimitive { .. }, _)
            | (_, CheckedBoundaryExecution::HostedPrimitive { .. }) => false,
            (left, right) => left == right,
        };
        self.source
            .same_identity_as(identities, &other.source, other_identities)
            && self.service_declaration == other.service_declaration
            && same_execution
            && self.method_arguments.len() == other.method_arguments.len()
            && self
                .method_arguments
                .iter()
                .zip(&other.method_arguments)
                .all(|(left, right)| left.same_identity_as(identities, right, other_identities))
    }
}

impl BoundaryMachineDeclaration {
    /// The public contract observed for this actual external operation.
    /// Admission validates any primitive link before execution consumes it.
    pub fn external_completion_requirement(&self) -> semantic_vocabulary::BoundaryMachineId {
        match self
            .declaration
            .as_ref()
            .map(|declaration| declaration.execution)
        {
            Some(CheckedBoundaryExecution::HostedPrimitive { requirement, .. }) => requirement,
            _ => self.id,
        }
    }
}

impl BoundaryMachineDeclaration {
    /// The declared reach (fixed reach and published ceiling) that a
    /// supply of this requirement's owning service needs its own
    /// permission for. The owning service is left out because that supply
    /// performs it. This is a host-permission projection, never a ceiling:
    /// the owner is catalogue ownership and stays outside the explicit
    /// ceiling unless authored there, so an empty result says nothing
    /// about whether the declared ceiling is empty.
    pub fn reach_beyond_owning_supply(&self) -> impl Iterator<Item = ServiceId> + '_ {
        self.fixed_service_reach
            .iter()
            .chain(&self.published_service_ceiling)
            .copied()
            .filter(move |service| Some(*service) != self.owning_service)
    }

    pub fn requires_checked_result(
        &self,
        domains: &[crate::StructuralDomainDeclaration],
        scalar_domains: &[crate::ScalarDomainDeclaration],
    ) -> bool {
        // A promise can forward an owned input claim or require a checked
        // realization. An opaque native call may only issue its own claims.
        if self.membership_ensures.iter().any(|row| {
            let crate::RuntimeDomainMembership::Scalar { domain, .. } = row else {
                return false;
            };
            scalar_domains
                .iter()
                .find(|row| row.id == *domain)
                .is_none_or(|domain| {
                    domain
                        .required_domain_closure(scalar_domains)
                        .is_none_or(|closure| {
                            closure.into_iter().any(|node| {
                                node.establishment_routes.is_some()
                                    && !node.authorizes_boundary_requirement(
                                        self.identity,
                                        self.declaration.as_ref().map(|row| &row.source.state),
                                    )
                            })
                        })
                })
        }) {
            return true;
        }
        let result_domains = (match &self.result {
            BoundaryMachineResult::Structural(result) => Some(result),
            _ => None,
        })
        .into_iter()
        .flat_map(|result| {
            result.qualifications.iter().chain(
                result
                    .projected_qualifications
                    .iter()
                    .map(|row| &row.domain),
            )
        })
        .chain(self.membership_ensures.iter().filter_map(|row| match row {
            crate::RuntimeDomainMembership::Structural { domain, .. } => Some(domain),
            _ => None,
        }));
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
        result_domains.into_iter().any(|domain| {
            domains
                .iter()
                .find(|declaration| declaration.id == *domain)
                .is_none_or(|declaration| {
                    declaration
                        .required_domain_closure(domains)
                        .is_none_or(|required| {
                            required.into_iter().any(|node| {
                                (node.id != declaration.id && node.content_projection.is_some())
                                    || ((node.content_projection.is_none()
                                        || !consumes_qualification(node.id))
                                        && node.establishment_routes.is_some()
                                        && !node.authorizes_boundary_requirement(
                                            self.identity,
                                            self.declaration
                                                .as_ref()
                                                .map(|callable| &callable.source.state),
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
