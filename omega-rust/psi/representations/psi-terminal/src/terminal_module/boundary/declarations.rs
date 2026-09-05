use crate::{
    RetainedBorrowCustody, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration,
};
use psi_core::{
    BoundaryMachineId, ContentAlgebra, ContentConservation, ContentProjectionExpression,
    ContentProjectionIdentity, ContentProjectionScalar, ScalarType, ServiceId, StructuralDomainId,
    StructuralTypeId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// One qualification-only boundary call-admission check. The corresponding
/// structural argument must already carry `domain`; this row does not create a
/// proposition or mint an obligation identity.
pub struct StructuralDomainRequirement {
    pub argument_index: u32,
    pub domain: StructuralDomainId,
}

/// Exact structural signature returned by one bodyless boundary declaration.
/// Unlike a machine result declaration this has no proof-visible place: the
/// successful call operation creates the caller-local result place.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundaryStructuralResultDeclaration {
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    pub qualifications: Vec<StructuralDomainId>,
}

/// Closed result role of one bodyless boundary declaration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoundaryMachineResult {
    Unit,
    Scalar(ScalarType),
    Structural(BoundaryStructuralResultDeclaration),
}

impl BoundaryMachineResult {
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundaryMachineDeclaration {
    pub id: BoundaryMachineId,
    pub identity: String,
    pub attachment: Option<StructuralTypeId>,
    /// Ordered primitive scalar parameters. Boundary calls bind their scalar
    /// arguments positionally and preserve this authored order exactly.
    pub scalar_parameters: Vec<ScalarType>,
    /// Ordered runtime structural parameters, independently positional from
    /// the scalar lane.
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    pub result: BoundaryMachineResult,
    /// Strictly ordered qualification checks by `(argument_index, domain)`.
    /// Admission consumes qualifications already carried by the arguments;
    /// these rows are not proof propositions.
    pub requires: Vec<StructuralDomainRequirement>,
    /// Exact portable schemas authorized by this requirement's domain routes.
    /// These rows describe per-occurrence capacity but introduce no authority;
    /// installation must still bind a concrete occurrence and cardinality.
    pub program_local_root_introductions: Vec<ProgramLocalRootIntroductionSchema>,
    /// Authored content guarantees of this exact boundary requirement. These
    /// are provider assumptions, not executable proof terms; a caller may use
    /// one only through the successful `BoundaryCall` operation that selected
    /// this declaration.
    pub content_guarantees: Vec<BoundaryContentGuarantee>,
    /// Strictly ordered normalized published ceiling.
    pub published_service_ceiling: Vec<ServiceId>,
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
            psi_core::ContentAlgebraKind::IntervalSet => 1,
            psi_core::ContentAlgebraKind::CountedQuantity => 2,
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
