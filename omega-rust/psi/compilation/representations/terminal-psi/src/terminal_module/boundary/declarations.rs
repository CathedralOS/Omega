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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// One qualification-only boundary call-admission check. The corresponding
/// structural argument must already carry `domain`; this row does not create a
/// proposition or mint an obligation identity.
pub struct StructuralDomainRequirement {
    pub argument_index: u32,
    pub domain: StructuralDomainId,
}

/// Place-free structural result shared by boundary and dynamic call signatures.
/// Unlike a machine result declaration this has no proof-visible place: the
/// successful call operation creates the caller-local result place.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundaryStructuralResultDeclaration {
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    pub qualifications: Vec<StructuralDomainId>,
    /// Memberships promised by the declared descendant types, not inferred
    /// from the receiving annotation or a selected member's private guarantees.
    pub projected_qualifications: Vec<crate::StructuralPathQualification>,
}

/// Closed place-free result role of a boundary or dynamic call signature.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoundaryMachineResult {
    Unit,
    Scalar(ScalarType),
    Structural(BoundaryStructuralResultDeclaration),
}

impl BoundaryMachineResult {
    /// Join a checked provider's own return declaration to its requirement.
    /// Predicate guarantees and boundary-authorized root qualifications belong
    /// to the selected crossing. Routed provenance owned by another issuer must
    /// occur in the adapter's actual result. Descendant memberships still
    /// require the provider's returned carrier to supply them. This comparison
    /// grants neither qualifications nor claims; conformance and actual return
    /// custody establish those separately.
    pub fn accepts_provider_result(
        &self,
        result: &crate::TerminalMachineResult,
        requirement: TerminalIdentityIndex,
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
                        result.qualifications.contains(domain)
                            || domains.iter().any(|declaration| {
                                declaration.id == *domain
                                    && declaration.carrier == expected.structural_type
                                    && (declaration.establishment_routes.is_empty()
                                        || declaration.authorizes_boundary_requirement(requirement))
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

    /// Compare representation and qualifications without exposing a private
    /// home. Reference origins belong to the caller's separate custody contract.
    pub fn matches_machine_result(&self, result: &crate::TerminalMachineResult) -> bool {
        match (self, result) {
            (Self::Unit, crate::TerminalMachineResult::Unit) => true,
            (Self::Scalar(expected), crate::TerminalMachineResult::Scalar(result)) => {
                *expected == result.scalar_type
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

    /// Compare the caller-local result against the public return signature.
    pub fn matches_operation_result(&self, result: &crate::OperationResult) -> bool {
        match (self, result) {
            (Self::Unit, crate::OperationResult::Unit) => true,
            (Self::Scalar(expected), crate::OperationResult::Scalar(result)) => {
                *expected == result.scalar_type
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
            Self::Scalar(scalar) => Some(*scalar),
            Self::Unit | Self::Structural(_) => None,
        }
    }

    pub const fn is_unit(&self) -> bool {
        matches!(self, Self::Unit)
    }
}

/// One retained runtime formal in declaration order. Its lane-local index is
/// the number of preceding tags of the same kind; private ABI materializations
/// and erased formals are not semantic parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoundaryParameterKind {
    Scalar,
    Structural,
}

/// A callable signature and retained contract evidence have different custody.
/// Evidence-only rows retain an unused foreign contract's checked loan, but
/// cannot create an installation demand or an executable entrance. Empty
/// parameter lists and Unit results also occur on real calls, so shape is not
/// a substitute for this explicit semantic role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoundaryDeclarationRole {
    Callable,
    ContractEvidence,
}

/// A promised descriptor on normal completion of an admitted provider call.
/// Its dependencies limit lifetime, not where the opaque referent is located.
/// Coordinates use the packed structural parameter lane, not authored ordinals.
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

/// A borrowed formal's referent and a descriptor in its storage are distinct
/// lifetime anchors, including when the stored descriptor has an empty path.
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
    /// Exact checked callable behind an opaque signature, when one exists.
    /// Schema-only requirements and synthetic contract evidence have no body
    /// declaration. This is the head's provenance, never a provider donation.
    pub declaration: Option<CheckedBoundaryCallable>,
    pub role: BoundaryDeclarationRole,
    pub kind: semantic_vocabulary::BoundaryRequirementKind,
    /// Exact opaque completion contract; installation must authenticate it.
    pub completion: semantic_vocabulary::BoundaryCompletion,
    /// Exact projection of the authored termination contract: true only for
    /// a guarantee with no progress premises. False makes no finite promise;
    /// it neither erases nor discharges a conditional guarantee's subjects.
    pub unconditional_termination: bool,
    pub identity: TerminalIdentityIndex,
    /// Exact declaring schema owner. Whole-provider selection joins this
    /// portable identity, never a publication-time source-index companion.
    pub owner: crate::NominalDeclarationIdentity,
    pub attachment: Option<StructuralTypeId>,
    /// Exact interleave of the two lanes. Each parameter appears once, in its
    /// lane's order. This relationship is part of the portable signature.
    pub parameter_order: Vec<BoundaryParameterKind>,
    /// Ordered primitive scalar parameters. Boundary calls bind their scalar
    /// arguments positionally and preserve this authored order exactly.
    pub scalar_parameters: Vec<ScalarType>,
    /// Published routes in this declaration's scalar-formal namespace. Formal
    /// IDs are scalar argument ordinals plus one, independent of caller IDs;
    /// verification substitutes all actuals simultaneously. The signature is
    /// the sole source of formal types, so no duplicate ID/type roster is stored.
    /// A selected provider cannot narrow this opaque requirement's contract.
    pub crash_routes: Vec<CrashRouteBucket>,
    /// Ordered runtime structural parameters, independently positional from
    /// the scalar lane.
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    pub result: BoundaryMachineResult,
    /// Strictly ordered qualification checks by `(argument_index, domain)`.
    /// Admission consumes qualifications already carried by the arguments;
    /// these rows are not proof propositions.
    pub requires: Vec<StructuralDomainRequirement>,
    /// The requirement's scalar `requires` propositions, in canonical order
    /// and in this declaration's scalar-formal namespace (the crash routes'
    /// namespace). Every `BoundaryCall` owes one obligation per row at the
    /// same index, the row instantiated at that call's scalar actuals. A
    /// selected provider cannot weaken what callers must establish.
    pub scalar_requires: Vec<Proposition>,
    /// Normal-return guarantees in the scalar-formal namespace. A scalar result
    /// uses the position immediately following the input scalar parameters.
    pub scalar_ensures: Vec<Proposition>,
    pub reference_outputs: Vec<BoundaryReferenceOutput>,
    /// Exact portable schemas authorized by this requirement's domain routes.
    /// These rows describe per-occurrence capacity but introduce no authority;
    /// installation must still bind a concrete occurrence and cardinality.
    pub program_local_root_introductions: Vec<ProgramLocalRootIntroductionSchema>,
    /// Authored content guarantees of this exact boundary requirement. These
    /// are provider assumptions, not executable proof terms; a caller may use
    /// one only through the successful `BoundaryCall` operation that selected
    /// this declaration.
    pub content_guarantees: Vec<BoundaryContentGuarantee>,
    /// Fixed concrete contribution, unaffected by installation selection.
    /// Strictly ordered and parent-closed; its union with a selected requirement's
    /// unresolved bound forms the published ceiling, without erasing overlap.
    pub fixed_service_reach: Vec<ServiceId>,
    /// Exact declaring boundary trait, when this is a trait requirement.
    /// Invoking the requirement invokes this service and its parents even
    /// when a selected implementation has an empty body.
    pub owning_service: Option<ServiceId>,
    /// Strictly ordered normalized published ceiling.
    pub published_service_ceiling: Vec<ServiceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedBoundaryCallable {
    pub source: crate::CheckedMachineDeclaration,
    /// Independent invocation tuple for a specialized opaque callable. Its
    /// published reach remains on the boundary; no body graph is invented.
    pub method_arguments: Vec<crate::ClosedReachArgument>,
}

impl BoundaryMachineDeclaration {
    /// Opaque supply cannot assert non-content provenance owned by another
    /// issuer. It needs an actual checked return; equal bits and a matching
    /// host result annotation do not carry that history. Content domains retain
    /// their independent input conservation and issuance checks. This query
    /// does not admit supply, prove predicates, or manufacture custody.
    pub fn requires_checked_result(&self, domains: &[crate::StructuralDomainDeclaration]) -> bool {
        let BoundaryMachineResult::Structural(result) = &self.result else {
            return false;
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
                        declaration.content_projection.is_none()
                            && !declaration.establishment_routes.is_empty()
                            && !declaration.authorizes_boundary_requirement(self.identity)
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

    /// Runtime formal positions for a lane, before any private ABI parameters.
    pub fn parameter_positions(
        &self,
        kind: BoundaryParameterKind,
    ) -> impl Iterator<Item = usize> + '_ {
        self.parameter_order
            .iter()
            .enumerate()
            .filter_map(move |(position, found)| (*found == kind).then_some(position))
    }

    /// Result pseudo-value shares the contract telescope, after all inputs.
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

    /// Reconstruct declaration-local scalar formals for contract validation and
    /// substitution. These are proof coordinates, not executable module values.
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
    /// Non-authoritative compact coordinate for reports and cache joins. The
    /// exact retained conservation equation and structural-place replay carry
    /// theorem authority.
    pub report_fingerprint: u64,
    /// Guarantee-local structural roots, alpha-matched to the boundary
    /// signature by parameter position.
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    pub conservation: ContentConservation,
}

/// Closed content contract attached to one exact boundary callable.
///
/// Retained borrows deliberately share the declaration catalog with ordinary
/// conservation guarantees while remaining a distinct, non-executable row.
/// This prevents a structural protocol result from being mistaken for the
/// established scalar boundary-result ABI.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoundaryContentGuarantee {
    Conservation(ContentConservationGuarantee),
    RetainedBorrow(RetainedBorrowCustody),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProgramLocalRootIntroductionSchema {
    /// Dense index in this boundary declaration's structural argument lane.
    pub argument_index: u32,
    /// Authored semantic parameter position before scalar/structural lanes split.
    pub source_parameter_position: u32,
    pub qualification: StructuralDomainId,
    pub carrier: StructuralTypeId,
    pub projection: ContentProjectionIdentity,
    pub algebra: ContentAlgebra,
    pub capacity: ContentProjectionExpression,
    /// Non-authoritative compatibility report identity of all fields above
    /// plus the enclosing requirement. Exact schema fields and owner
    /// projection replay carry semantic authority.
    pub compatibility_report_identity: u64,
}

/// The boundary machine declaration keyed `id` in a boundary roster.
pub fn find_boundary_machine<'declarations>(
    declarations: &'declarations [BoundaryMachineDeclaration],
    id: BoundaryMachineId,
) -> Option<&'declarations BoundaryMachineDeclaration> {
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
