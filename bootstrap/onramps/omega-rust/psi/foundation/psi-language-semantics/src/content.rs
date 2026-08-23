//! Compiler-owned resource-content algebra and normalized projection plans.
//!
//! These records are semantic facts, not runtime values. Field symbols remain
//! available to checked consumers, while fingerprints fold stable field names
//! and normalized type identities rather than arena-local handles.

use crate::SemanticDomainId;
use psi_numerics::bignum::BigInt;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentAlgebraIdentity {
    IntervalSet { coordinate_space: String },
    CountedQuantity { unit: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentFieldSegment {
    pub symbol: SymbolHandle,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentCaseSegment {
    pub symbol: SymbolHandle,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentArithmeticOperator {
    Add,
    Subtract,
    Multiply,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentScalarExpression {
    SubjectField(Vec<ContentFieldSegment>),
    RuntimeScalarEmbedding(Vec<ContentFieldSegment>),
    Natural(String),
    Successor(Box<ContentScalarExpression>),
    Arithmetic {
        operator: ContentArithmeticOperator,
        left: Box<ContentScalarExpression>,
        right: Box<ContentScalarExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentProjectionExpression {
    IntervalSet {
        members: Vec<ContentIntervalExpression>,
    },
    CountedQuantity {
        magnitude: ContentScalarExpression,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentIntervalExpression {
    start: ContentScalarExpression,
    end: ContentScalarExpression,
}

impl ContentIntervalExpression {
    pub fn new(start: ContentScalarExpression, end: ContentScalarExpression) -> Self {
        Self { start, end }
    }

    pub const fn start(&self) -> &ContentScalarExpression {
        &self.start
    }

    pub const fn end(&self) -> &ContentScalarExpression {
        &self.end
    }
}

/// One exact proof-natural half-open interval.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NaturalInterval {
    start: BigInt,
    end: BigInt,
}

impl NaturalInterval {
    pub fn new(start: BigInt, end: BigInt) -> Result<Self, IntervalSetError> {
        if start.is_negative() || end.is_negative() {
            return Err(IntervalSetError::NegativeBound);
        }
        if start > end {
            return Err(IntervalSetError::ReversedBounds);
        }
        Ok(Self { start, end })
    }

    pub const fn start(&self) -> &BigInt {
        &self.start
    }

    pub const fn end(&self) -> &BigInt {
        &self.end
    }

    fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Canonical finite set of sorted, disjoint proof-natural intervals.
///
/// Empty members disappear and adjacent members merge. Overlap is rejected,
/// including by `separate`, because separated composition is a partial
/// authority operation rather than ordinary set union.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CanonicalIntervalSet {
    members: Vec<NaturalInterval>,
}

impl CanonicalIntervalSet {
    pub fn new(
        members: impl IntoIterator<Item = NaturalInterval>,
    ) -> Result<Self, IntervalSetError> {
        let mut members = members
            .into_iter()
            .filter(|member| !member.is_empty())
            .collect::<Vec<_>>();
        members.sort();

        let mut normalized: Vec<NaturalInterval> = Vec::with_capacity(members.len());
        for member in members {
            let Some(previous) = normalized.last_mut() else {
                normalized.push(member);
                continue;
            };
            if member.start < previous.end {
                return Err(IntervalSetError::OverlappingMembers);
            }
            if member.start == previous.end {
                previous.end = member.end;
            } else {
                normalized.push(member);
            }
        }
        Ok(Self {
            members: normalized,
        })
    }

    pub fn singleton(start: BigInt, end: BigInt) -> Result<Self, IntervalSetError> {
        Self::new([NaturalInterval::new(start, end)?])
    }

    pub fn members(&self) -> &[NaturalInterval] {
        &self.members
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub fn separate<'a>(
        sets: impl IntoIterator<Item = &'a Self>,
    ) -> Result<Self, IntervalSetError> {
        Self::new(sets.into_iter().flat_map(|set| set.members.iter().cloned()))
    }

    pub fn contains(&self, kept: &Self) -> bool {
        let mut whole_index = 0;
        for member in &kept.members {
            while whole_index < self.members.len() && self.members[whole_index].end <= member.start
            {
                whole_index += 1;
            }
            let Some(whole) = self.members.get(whole_index) else {
                return false;
            };
            if member.start < whole.start || member.end > whole.end {
                return false;
            }
        }
        true
    }

    pub fn residual(&self, kept: &Self) -> Result<Self, IntervalSetError> {
        if !self.contains(kept) {
            return Err(IntervalSetError::NotContained);
        }
        let mut residual = Vec::new();
        for whole in &self.members {
            let mut cursor = whole.start.clone();
            for member in kept
                .members
                .iter()
                .filter(|member| member.start >= whole.start && member.end <= whole.end)
            {
                if cursor < member.start {
                    residual.push(NaturalInterval {
                        start: cursor,
                        end: member.start.clone(),
                    });
                }
                cursor = member.end.clone();
            }
            if cursor < whole.end {
                residual.push(NaturalInterval {
                    start: cursor,
                    end: whole.end.clone(),
                });
            }
        }
        Self::new(residual)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntervalSetError {
    NegativeBound,
    ReversedBounds,
    OverlappingMembers,
    NotContained,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentProjectionPlan {
    pub domain: SymbolHandle,
    pub semantic_domain: SemanticDomainId,
    pub carrier_identity: String,
    pub machine: SymbolHandle,
    pub algebra: ContentAlgebraIdentity,
    pub expression: ContentProjectionExpression,
    pub fingerprint: u64,
}

/// Which declared callable owns an authored content-conservation equation.
/// The symbols are retained separately on [`ContentConservationPlan`]; this
/// closed tag keeps artifacts and later terminal-Psi lowering explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentConservationOwnerKind {
    Machine,
    TraitRequirement,
}

/// The version of a structural place observed by a content projection.
/// `Entry` is available only through proof-only `entry(place)`; there is no
/// general historical-expression modality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentPlaceVersion {
    Entry,
    Current,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentPlaceRoot {
    Parameter {
        position: u32,
        symbol: SymbolHandle,
        name: String,
        is_self: bool,
    },
    Result,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentPlaceSegment {
    Case(ContentCaseSegment),
    Field(ContentFieldSegment),
    FixedIndex(u64),
}

/// A parameter, `self`, or result structural place in a callable contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentStructuralPlace {
    pub version: ContentPlaceVersion,
    pub root: ContentPlaceRoot,
    pub segments: Vec<ContentPlaceSegment>,
}

/// Closed content expression admitted in an authored conservation equation.
/// Projection identity is exact and `Separate` children are flattened and
/// canonically sorted, so package spelling order cannot perturb semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentConservationTerm {
    Projection {
        domain: SymbolHandle,
        semantic_domain: SemanticDomainId,
        projection_machine: SymbolHandle,
        projection_fingerprint: u64,
        subject: ContentStructuralPlace,
    },
    Separate(Vec<ContentConservationTerm>),
}

impl ContentConservationTerm {
    pub fn separate(terms: impl IntoIterator<Item = Self>) -> Self {
        let mut flattened = Vec::new();
        for term in terms {
            match term {
                Self::Separate(children) => flattened.extend(children),
                other => flattened.push(other),
            }
        }
        flattened.sort_by_key(content_conservation_term_bytes);
        Self::Separate(flattened)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentConservationEquation {
    left: ContentConservationTerm,
    right: ContentConservationTerm,
}

impl ContentConservationEquation {
    pub fn new(left: ContentConservationTerm, right: ContentConservationTerm) -> Self {
        if content_conservation_term_bytes(&left) <= content_conservation_term_bytes(&right) {
            Self { left, right }
        } else {
            Self {
                left: right,
                right: left,
            }
        }
    }

    pub const fn left(&self) -> &ContentConservationTerm {
        &self.left
    }

    pub const fn right(&self) -> &ContentConservationTerm {
        &self.right
    }
}

/// One normalized authored equation for one callable outcome and one closed
/// content algebra. Proof derivations do not enter this semantic carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentConservationPlan {
    pub owner_kind: ContentConservationOwnerKind,
    pub owner: SymbolHandle,
    pub callable: SymbolHandle,
    pub algebra: ContentAlgebraIdentity,
    pub equation: ContentConservationEquation,
    pub fingerprint: u64,
}

pub fn conservation_fingerprint(
    algebra: &ContentAlgebraIdentity,
    equation: &ContentConservationEquation,
) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"content-conservation-v1");
    encode_algebra(algebra, &mut bytes);
    encode_content_conservation_term(equation.left(), &mut bytes);
    encode_content_conservation_term(equation.right(), &mut bytes);
    bytes.into_iter().fold(OFFSET, |mut hash, byte| {
        hash ^= u64::from(byte);
        hash.wrapping_mul(PRIME)
    })
}

pub fn content_conservation_plan_bytes(plan: &ContentConservationPlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"content-conservation-plan-v1");
    bytes.push(match plan.owner_kind {
        ContentConservationOwnerKind::Machine => 1,
        ContentConservationOwnerKind::TraitRequirement => 2,
    });
    encode_algebra(&plan.algebra, &mut bytes);
    encode_content_conservation_term(plan.equation.left(), &mut bytes);
    encode_content_conservation_term(plan.equation.right(), &mut bytes);
    bytes
}

fn content_conservation_term_bytes(term: &ContentConservationTerm) -> Vec<u8> {
    let mut bytes = Vec::new();
    encode_content_conservation_term(term, &mut bytes);
    bytes
}

fn encode_content_conservation_term(term: &ContentConservationTerm, output: &mut Vec<u8>) {
    match term {
        ContentConservationTerm::Projection {
            semantic_domain,
            projection_fingerprint,
            subject,
            ..
        } => {
            output.push(1);
            output.extend_from_slice(&semantic_domain.0.to_le_bytes());
            output.extend_from_slice(&projection_fingerprint.to_le_bytes());
            output.push(match subject.version {
                ContentPlaceVersion::Entry => 1,
                ContentPlaceVersion::Current => 2,
            });
            match &subject.root {
                ContentPlaceRoot::Parameter {
                    position, is_self, ..
                } => {
                    output.push(if *is_self { 2 } else { 1 });
                    output.extend_from_slice(&position.to_le_bytes());
                }
                ContentPlaceRoot::Result => output.push(3),
            }
            output.extend_from_slice(&(subject.segments.len() as u64).to_le_bytes());
            for segment in &subject.segments {
                match segment {
                    ContentPlaceSegment::Case(case) => {
                        output.push(3);
                        encode_string(&case.name, output);
                    }
                    ContentPlaceSegment::Field(field) => {
                        output.push(1);
                        encode_string(&field.name, output);
                    }
                    ContentPlaceSegment::FixedIndex(index) => {
                        output.push(2);
                        output.extend_from_slice(&index.to_le_bytes());
                    }
                }
            }
        }
        ContentConservationTerm::Separate(terms) => {
            output.push(2);
            output.extend_from_slice(&(terms.len() as u64).to_le_bytes());
            for term in terms {
                encode_content_conservation_term(term, output);
            }
        }
    }
}

pub fn projection_fingerprint(
    algebra: &ContentAlgebraIdentity,
    expression: &ContentProjectionExpression,
) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut bytes = Vec::new();
    encode_algebra(algebra, &mut bytes);
    encode_projection(expression, &mut bytes);
    bytes.into_iter().fold(OFFSET, |mut hash, byte| {
        hash ^= u64::from(byte);
        hash.wrapping_mul(PRIME)
    })
}

/// Recompute the same projection identity after frontend symbols have erased.
/// This is the verifier bridge for portable program-local root schemas.
pub fn terminal_projection_fingerprint(
    algebra: &psi_core::ContentAlgebra,
    expression: &psi_core::ProgramLocalCapacityExpression,
) -> u64 {
    fn encode_terminal_scalar(
        expression: &psi_core::ProgramLocalCapacityScalar,
        output: &mut Vec<u8>,
    ) {
        use psi_core::ProgramLocalCapacityScalar as Scalar;
        match expression {
            Scalar::SubjectField(path) | Scalar::RuntimeScalarEmbedding(path) => {
                output.push(if matches!(expression, Scalar::SubjectField(_)) {
                    1
                } else {
                    2
                });
                output.extend_from_slice(&(path.len() as u64).to_le_bytes());
                for segment in path {
                    encode_string(segment, output);
                }
            }
            Scalar::Natural(value) => {
                output.push(3);
                encode_string(value, output);
            }
            Scalar::Successor(value) => {
                output.push(4);
                encode_terminal_scalar(value, output);
            }
            Scalar::Add(left, right)
            | Scalar::Subtract(left, right)
            | Scalar::Multiply(left, right) => {
                output.push(5);
                output.push(match expression {
                    Scalar::Add(_, _) => 1,
                    Scalar::Subtract(_, _) => 2,
                    Scalar::Multiply(_, _) => 3,
                    _ => unreachable!(),
                });
                encode_terminal_scalar(left, output);
                encode_terminal_scalar(right, output);
            }
        }
    }
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut bytes = Vec::new();
    match algebra.kind {
        psi_core::ContentAlgebraKind::IntervalSet => {
            bytes.push(3);
            encode_string(&algebra.parameter, &mut bytes);
        }
        psi_core::ContentAlgebraKind::CountedQuantity => {
            bytes.push(2);
            encode_string(&algebra.parameter, &mut bytes);
        }
    }
    match expression {
        psi_core::ProgramLocalCapacityExpression::IntervalSet(members) => {
            bytes.push(3);
            bytes.extend_from_slice(&(members.len() as u64).to_le_bytes());
            for (start, end) in members {
                encode_terminal_scalar(start, &mut bytes);
                encode_terminal_scalar(end, &mut bytes);
            }
        }
        psi_core::ProgramLocalCapacityExpression::CountedQuantity(magnitude) => {
            bytes.push(2);
            encode_terminal_scalar(magnitude, &mut bytes);
        }
    }
    bytes.into_iter().fold(OFFSET, |mut hash, byte| {
        hash ^= u64::from(byte);
        hash.wrapping_mul(PRIME)
    })
}

fn encode_algebra(algebra: &ContentAlgebraIdentity, output: &mut Vec<u8>) {
    match algebra {
        ContentAlgebraIdentity::IntervalSet { coordinate_space } => {
            output.push(3);
            encode_string(coordinate_space, output);
        }
        ContentAlgebraIdentity::CountedQuantity { unit } => {
            output.push(2);
            encode_string(unit, output);
        }
    }
}

fn encode_projection(expression: &ContentProjectionExpression, output: &mut Vec<u8>) {
    match expression {
        ContentProjectionExpression::IntervalSet { members } => {
            output.push(3);
            output.extend_from_slice(&(members.len() as u64).to_le_bytes());
            for member in members {
                encode_scalar(member.start(), output);
                encode_scalar(member.end(), output);
            }
        }
        ContentProjectionExpression::CountedQuantity { magnitude } => {
            output.push(2);
            encode_scalar(magnitude, output);
        }
    }
}

fn encode_scalar(expression: &ContentScalarExpression, output: &mut Vec<u8>) {
    match expression {
        ContentScalarExpression::SubjectField(path) => {
            output.push(1);
            output.extend_from_slice(&(path.len() as u64).to_le_bytes());
            for segment in path {
                encode_string(&segment.name, output);
            }
        }
        ContentScalarExpression::RuntimeScalarEmbedding(path) => {
            output.push(2);
            output.extend_from_slice(&(path.len() as u64).to_le_bytes());
            for segment in path {
                encode_string(&segment.name, output);
            }
        }
        ContentScalarExpression::Natural(value) => {
            output.push(3);
            encode_string(value, output);
        }
        ContentScalarExpression::Successor(value) => {
            output.push(4);
            encode_scalar(value, output);
        }
        ContentScalarExpression::Arithmetic {
            operator,
            left,
            right,
        } => {
            output.push(5);
            output.push(match operator {
                ContentArithmeticOperator::Add => 1,
                ContentArithmeticOperator::Subtract => 2,
                ContentArithmeticOperator::Multiply => 3,
            });
            encode_scalar(left, output);
            encode_scalar(right, output);
        }
    }
}

fn encode_string(value: &str, output: &mut Vec<u8>) {
    output.extend_from_slice(&(value.len() as u64).to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn natural(value: u64) -> BigInt {
        BigInt::from_u64(value)
    }

    fn interval(start: u64, end: u64) -> NaturalInterval {
        NaturalInterval::new(natural(start), natural(end)).unwrap()
    }

    #[test]
    fn interval_sets_sort_drop_empty_and_merge_adjacency() {
        let set = CanonicalIntervalSet::new([
            interval(8, 10),
            interval(4, 4),
            interval(0, 3),
            interval(3, 8),
        ])
        .unwrap();

        assert_eq!(set.members(), [interval(0, 10)]);
        assert_eq!(
            CanonicalIntervalSet::new([interval(0, 4), interval(3, 5)]),
            Err(IntervalSetError::OverlappingMembers)
        );
        assert!(
            CanonicalIntervalSet::singleton(natural(7), natural(7))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn separate_is_partial_and_canonical() {
        let left = CanonicalIntervalSet::new([interval(8, 10), interval(0, 2)]).unwrap();
        let middle = CanonicalIntervalSet::singleton(natural(2), natural(8)).unwrap();
        assert_eq!(
            CanonicalIntervalSet::separate([&left, &middle])
                .unwrap()
                .members(),
            [interval(0, 10)]
        );

        let overlap = CanonicalIntervalSet::singleton(natural(1), natural(3)).unwrap();
        assert_eq!(
            CanonicalIntervalSet::separate([&left, &overlap]),
            Err(IntervalSetError::OverlappingMembers)
        );
    }

    #[test]
    fn residual_derives_fragmented_canonical_difference_after_containment() {
        let whole = CanonicalIntervalSet::singleton(natural(0), natural(10)).unwrap();
        let kept = CanonicalIntervalSet::new([interval(2, 4), interval(6, 8)]).unwrap();
        assert!(whole.contains(&kept));
        assert_eq!(
            whole.residual(&kept).unwrap().members(),
            [interval(0, 2), interval(4, 6), interval(8, 10)]
        );

        let outside = CanonicalIntervalSet::singleton(natural(9), natural(11)).unwrap();
        assert_eq!(
            whole.residual(&outside),
            Err(IntervalSetError::NotContained)
        );
    }

    #[test]
    fn projection_fingerprint_ignores_arena_local_field_symbols() {
        let expression = |index| ContentProjectionExpression::CountedQuantity {
            magnitude: ContentScalarExpression::SubjectField(vec![ContentFieldSegment {
                symbol: SymbolHandle::from_arena_index(index),
                name: "remaining".to_owned(),
            }]),
        };
        let algebra = ContentAlgebraIdentity::CountedQuantity {
            unit: "named(name(Byte))".to_owned(),
        };

        assert_eq!(
            projection_fingerprint(&algebra, &expression(7)),
            projection_fingerprint(&algebra, &expression(91))
        );
    }

    #[test]
    fn embedded_scalar_fingerprint_ignores_arena_local_field_symbols() {
        let expression = |index| ContentProjectionExpression::CountedQuantity {
            magnitude: ContentScalarExpression::RuntimeScalarEmbedding(vec![ContentFieldSegment {
                symbol: SymbolHandle::from_arena_index(index),
                name: "remaining".to_owned(),
            }]),
        };
        let algebra = ContentAlgebraIdentity::CountedQuantity {
            unit: "named(name(Byte))".to_owned(),
        };

        assert_eq!(
            projection_fingerprint(&algebra, &expression(7)),
            projection_fingerprint(&algebra, &expression(91))
        );
    }

    #[test]
    fn interval_set_projection_has_a_stable_schema_distinct_fingerprint() {
        let algebra = ContentAlgebraIdentity::IntervalSet {
            coordinate_space: "named(name(PhysicalMemory))".to_owned(),
        };
        let expression = ContentProjectionExpression::IntervalSet {
            members: vec![ContentIntervalExpression::new(
                ContentScalarExpression::Natural("0".to_owned()),
                ContentScalarExpression::Natural("4096".to_owned()),
            )],
        };

        assert_eq!(
            projection_fingerprint(&algebra, &expression),
            0x0042_e73e_1d08_fd01
        );
    }

    #[test]
    fn conservation_fingerprint_normalizes_equation_and_separate_order() {
        let algebra = ContentAlgebraIdentity::CountedQuantity {
            unit: "named(name(ByteUnit))".to_owned(),
        };
        let projection = |subject| ContentConservationTerm::Projection {
            domain: SymbolHandle::from_arena_index(71),
            semantic_domain: SemanticDomainId(9),
            projection_machine: SymbolHandle::from_arena_index(72),
            projection_fingerprint: 0x1122_3344_5566_7788,
            subject,
        };
        let entry = |name: &str, symbol_index| ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: ContentPlaceRoot::Parameter {
                position: 0,
                symbol: SymbolHandle::from_arena_index(symbol_index),
                name: name.to_owned(),
                is_self: false,
            },
            segments: Vec::new(),
        };
        let output = |name: &str, symbol_index| ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root: ContentPlaceRoot::Result,
            segments: vec![ContentPlaceSegment::Field(ContentFieldSegment {
                symbol: SymbolHandle::from_arena_index(symbol_index),
                name: name.to_owned(),
            })],
        };

        let first = ContentConservationEquation::new(
            projection(entry("whole", 1)),
            ContentConservationTerm::separate([
                projection(output("right", 3)),
                projection(output("left", 2)),
            ]),
        );
        let renamed_and_reordered = ContentConservationEquation::new(
            ContentConservationTerm::separate([
                projection(output("left", 200)),
                projection(output("right", 300)),
            ]),
            projection(entry("renamed", 100)),
        );

        let fingerprint = conservation_fingerprint(&algebra, &first);
        assert_eq!(
            fingerprint,
            conservation_fingerprint(&algebra, &renamed_and_reordered)
        );
        assert_eq!(fingerprint, 0xbca0_a611_d59c_b3c1);
    }
}
