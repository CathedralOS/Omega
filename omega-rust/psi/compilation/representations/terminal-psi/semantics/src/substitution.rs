//! Capture-free substitution for retained proposition, scalar, and content terms.

use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};

use terminal_psi::{CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard};

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ContentConservation, ContentStructuralPlace, ContentTerm,
    IntegerMathTerm, PlaceId, Proposition, ScalarTerm, ValueId,
};

pub fn substitute_proposition_values(
    proposition: &Proposition,
    substitutions: &BTreeMap<ValueId, ScalarTerm>,
) -> Proposition {
    match proposition {
        Proposition::Truth => Proposition::Truth,
        Proposition::Falsehood => Proposition::Falsehood,
        Proposition::Atom(atom) => Proposition::Atom(*atom),
        Proposition::Equal(left, right) => Proposition::Equal(
            substitute_scalar_term_values(left, substitutions),
            substitute_scalar_term_values(right, substitutions),
        ),
        Proposition::LessThan(left, right) => Proposition::LessThan(
            substitute_scalar_term_values(left, substitutions),
            substitute_scalar_term_values(right, substitutions),
        ),
        Proposition::LessOrEqual(left, right) => Proposition::LessOrEqual(
            substitute_scalar_term_values(left, substitutions),
            substitute_scalar_term_values(right, substitutions),
        ),
        Proposition::IntegerMathEqual(left, right) => Proposition::IntegerMathEqual(
            substitute_integer_math_term_values(left, substitutions),
            substitute_integer_math_term_values(right, substitutions),
        ),
        Proposition::IntegerMathLessThan(left, right) => Proposition::IntegerMathLessThan(
            substitute_integer_math_term_values(left, substitutions),
            substitute_integer_math_term_values(right, substitutions),
        ),
        Proposition::IntegerMathLessOrEqual(left, right) => Proposition::IntegerMathLessOrEqual(
            substitute_integer_math_term_values(left, substitutions),
            substitute_integer_math_term_values(right, substitutions),
        ),
        Proposition::IeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => Proposition::IeeeFloatComparison {
            kind: *kind,
            format: *format,
            left: left.clone(),
            right: right.clone(),
        },
        Proposition::ScalarIeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => {
            let mut left = substitute_scalar_term_values(left, substitutions);
            let mut right = substitute_scalar_term_values(right, substitutions);
            if kind.is_symmetric() && left > right {
                std::mem::swap(&mut left, &mut right);
            }
            Proposition::ScalarIeeeFloatComparison {
                kind: *kind,
                format: *format,
                left,
                right,
            }
        }
        Proposition::ByteSequenceEqual { left, right } => Proposition::ByteSequenceEqual {
            left: left.clone(),
            right: right.clone(),
        },
        Proposition::StructuralIdentity { left, right } => Proposition::StructuralIdentity {
            left: left.clone(),
            right: right.clone(),
        },
        Proposition::StructuralCaseMembership { subject, case } => {
            Proposition::StructuralCaseMembership {
                subject: subject.clone(),
                case: *case,
            }
        }
        Proposition::Conjunction(conjuncts) => Proposition::Conjunction(
            conjuncts
                .iter()
                .map(|conjunct| substitute_proposition_values(conjunct, substitutions))
                .collect(),
        ),
        Proposition::Disjunction(disjuncts) => Proposition::Disjunction(
            disjuncts
                .iter()
                .map(|disjunct| substitute_proposition_values(disjunct, substitutions))
                .collect(),
        ),
        Proposition::Implication {
            premise,
            conclusion,
        } => Proposition::Implication {
            premise: Box::new(substitute_proposition_values(premise, substitutions)),
            conclusion: Box::new(substitute_proposition_values(conclusion, substitutions)),
        },
        Proposition::ContentConservation(conservation) => {
            Proposition::ContentConservation(conservation.clone())
        }
    }
}

pub fn substitute_proposition_structural_places(
    proposition: &Proposition,
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> Proposition {
    match proposition {
        Proposition::Truth => Proposition::Truth,
        Proposition::Falsehood => Proposition::Falsehood,
        Proposition::Atom(atom) => Proposition::Atom(*atom),
        Proposition::Equal(left, right) => Proposition::Equal(
            substitute_scalar_term_places(left, substitutions),
            substitute_scalar_term_places(right, substitutions),
        ),
        Proposition::LessThan(left, right) => Proposition::LessThan(
            substitute_scalar_term_places(left, substitutions),
            substitute_scalar_term_places(right, substitutions),
        ),
        Proposition::LessOrEqual(left, right) => Proposition::LessOrEqual(
            substitute_scalar_term_places(left, substitutions),
            substitute_scalar_term_places(right, substitutions),
        ),
        Proposition::IntegerMathEqual(left, right) => {
            Proposition::IntegerMathEqual(left.clone(), right.clone())
        }
        Proposition::IntegerMathLessThan(left, right) => {
            Proposition::IntegerMathLessThan(left.clone(), right.clone())
        }
        Proposition::IntegerMathLessOrEqual(left, right) => {
            Proposition::IntegerMathLessOrEqual(left.clone(), right.clone())
        }
        Proposition::IeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => {
            let rebase = |field: &semantic_vocabulary::IeeeFloatStructuralField| {
                substitutions
                    .get(&field.root())
                    .map(|(root, prefix)| field.rebase(*root, prefix))
                    .unwrap_or_else(|| field.clone())
            };
            Proposition::IeeeFloatComparison {
                kind: *kind,
                format: *format,
                left: rebase(left),
                right: rebase(right),
            }
        }
        Proposition::ScalarIeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => {
            let mut left = substitute_scalar_term_places(left, substitutions);
            let mut right = substitute_scalar_term_places(right, substitutions);
            if kind.is_symmetric() && left > right {
                std::mem::swap(&mut left, &mut right);
            }
            Proposition::ScalarIeeeFloatComparison {
                kind: *kind,
                format: *format,
                left,
                right,
            }
        }
        Proposition::ByteSequenceEqual { left, right } => {
            let rebase = |field: &semantic_vocabulary::ByteSequenceStructuralField| {
                substitutions
                    .get(&field.root())
                    .map(|(root, prefix)| field.rebase(*root, prefix))
                    .unwrap_or_else(|| field.clone())
            };
            Proposition::ByteSequenceEqual {
                left: rebase(left),
                right: rebase(right),
            }
        }
        Proposition::StructuralCaseMembership { subject, case } => {
            Proposition::StructuralCaseMembership {
                subject: substitutions
                    .get(&subject.root())
                    .map(|(root, prefix)| subject.rebase(*root, prefix))
                    .unwrap_or_else(|| subject.clone()),
                case: *case,
            }
        }
        Proposition::StructuralIdentity { left, right } => {
            let rebase = |subject: &semantic_vocabulary::StructuralIdentitySubject| {
                substitutions
                    .get(&subject.root())
                    .map(|(root, prefix)| subject.rebase(*root, prefix))
                    .unwrap_or_else(|| subject.clone())
            };
            let mut left = rebase(left);
            let mut right = rebase(right);
            if left > right {
                std::mem::swap(&mut left, &mut right);
            }
            Proposition::StructuralIdentity { left, right }
        }
        Proposition::Conjunction(conjuncts) => Proposition::Conjunction(
            conjuncts
                .iter()
                .map(|conjunct| substitute_proposition_structural_places(conjunct, substitutions))
                .collect(),
        ),
        Proposition::Disjunction(disjuncts) => Proposition::Disjunction(
            disjuncts
                .iter()
                .map(|disjunct| substitute_proposition_structural_places(disjunct, substitutions))
                .collect(),
        ),
        Proposition::Implication {
            premise,
            conclusion,
        } => Proposition::Implication {
            premise: Box::new(substitute_proposition_structural_places(
                premise,
                substitutions,
            )),
            conclusion: Box::new(substitute_proposition_structural_places(
                conclusion,
                substitutions,
            )),
        },
        Proposition::ContentConservation(conservation) => {
            Proposition::ContentConservation(ContentConservation::new(
                conservation.algebra().clone(),
                substitute_content_term_places(conservation.left(), substitutions),
                substitute_content_term_places(conservation.right(), substitutions),
            ))
        }
    }
}

fn substitute_integer_math_term_values(
    term: &IntegerMathTerm,
    substitutions: &BTreeMap<ValueId, ScalarTerm>,
) -> IntegerMathTerm {
    match term {
        IntegerMathTerm::MathValue { source_type, value } => substitutions
            .get(value)
            .and_then(|replacement| match replacement {
                ScalarTerm::Value {
                    id,
                    scalar_type: semantic_vocabulary::ScalarType::Integer(actual),
                } if actual == source_type => Some(IntegerMathTerm::MathValue {
                    source_type: *source_type,
                    value: *id,
                }),
                ScalarTerm::Integer {
                    scalar_type: actual,
                    value,
                } if actual == source_type => Some(IntegerMathTerm::literal(*value)),
                _ => None,
            })
            .unwrap_or_else(|| term.clone()),
        IntegerMathTerm::IntegerLiteral(_) => term.clone(),
        IntegerMathTerm::Add(left, right) => IntegerMathTerm::Add(
            Box::new(substitute_integer_math_term_values(left, substitutions)),
            Box::new(substitute_integer_math_term_values(right, substitutions)),
        ),
        IntegerMathTerm::Subtract(left, right) => IntegerMathTerm::Subtract(
            Box::new(substitute_integer_math_term_values(left, substitutions)),
            Box::new(substitute_integer_math_term_values(right, substitutions)),
        ),
        IntegerMathTerm::Multiply(left, right) => IntegerMathTerm::Multiply(
            Box::new(substitute_integer_math_term_values(left, substitutions)),
            Box::new(substitute_integer_math_term_values(right, substitutions)),
        ),
        IntegerMathTerm::ShiftLeft { value, count } => IntegerMathTerm::ShiftLeft {
            value: Box::new(substitute_integer_math_term_values(value, substitutions)),
            count: Box::new(substitute_integer_math_term_values(count, substitutions)),
        },
    }
}

fn substitute_scalar_term_places(
    term: &ScalarTerm,
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> ScalarTerm {
    let mut term = term.clone();
    fn substitute(
        term: &mut ScalarTerm,
        substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) {
        match term {
            ScalarTerm::StructuralRead { root, path, .. } => {
                if let Some((replacement, prefix)) = substitutions.get(root) {
                    *root = *replacement;
                    let mut rebased = prefix
                        .iter()
                        .copied()
                        .map(semantic_vocabulary::ScalarReadProjection::Static)
                        .collect::<Vec<_>>();
                    rebased.append(path);
                    *path = rebased;
                }
                for projection in path {
                    if let semantic_vocabulary::ScalarReadProjection::Index(index) = projection {
                        substitute(index, substitutions);
                    }
                }
            }
            semantic_vocabulary::scalar_term_predicate_quint_pattern!(root, path, ..) => {
                if let Some((replacement, prefix)) = substitutions.get(root) {
                    *root = *replacement;
                    if !prefix.is_empty() {
                        let mut rebased = Vec::with_capacity(prefix.len() + path.len());
                        rebased.extend(prefix);
                        rebased.append(path);
                        *path = rebased;
                    }
                }
            }
            semantic_vocabulary::scalar_term_unary_operand!(operand) => {
                substitute(operand, substitutions)
            }
            semantic_vocabulary::scalar_term_binary_operands!(left, right) => {
                substitute(left, substitutions);
                substitute(right, substitutions);
            }
            ScalarTerm::Value { .. }
            | ScalarTerm::Boolean(_)
            | ScalarTerm::Integer { .. }
            | ScalarTerm::IeeeFloatLiteral(_) => {}
        }
    }
    substitute(&mut term, substitutions);
    term
}

fn substitute_content_term_places(
    term: &ContentTerm,
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> ContentTerm {
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => ContentTerm::Projection {
            projection: *projection,
            subject: ContentStructuralPlace {
                version: subject.version,
                root: substitutions
                    .get(&subject.root)
                    .map(|(root, _)| *root)
                    .unwrap_or(subject.root),
                segments: subject.segments.clone(),
            },
        },
        ContentTerm::Separate(terms) => ContentTerm::Separate(
            terms
                .iter()
                .map(|term| substitute_content_term_places(term, substitutions))
                .collect(),
        ),
    }
}

/// The position of `left = right` in the roster under either operand order.
/// `Equal` is symmetric, so the twin form is the same rostered fact.
pub fn roster_equation_position(
    axioms: &[Proposition],
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> Option<usize> {
    axioms.iter().position(|axiom| match axiom {
        Proposition::Equal(l, r) => (l == left && r == right) || (l == right && r == left),
        _ => false,
    })
}

/// Whether the roster already carries this fact. For `Equal(Value, Value)`
/// the symmetric twin also counts: the denotation checker's definition graph
/// is directed, so a roster storing both orders of one equation reads as a
/// cyclic pair and fails conversion outright, while semantically they are one
/// fact. The direction already written always wins.
pub fn roster_has_fact(axioms: &[Proposition], proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Equal(left, right)
            if matches!(left, ScalarTerm::Value { .. })
                && matches!(right, ScalarTerm::Value { .. }) =>
        {
            roster_equation_position(axioms, left, right).is_some()
        }
        _ => axioms.contains(proposition),
    }
}

/// Push `proposition` unless the roster already carries it under
/// [`roster_has_fact`].
pub fn push_unique_fact(axioms: &mut Vec<Proposition>, proposition: Proposition) {
    if !roster_has_fact(axioms, &proposition) {
        axioms.push(proposition);
    }
}

/// Read access to a proposition roster — either a materialized `Vec` or a
/// [`SharedRoster`] sharing its spine. Positions number the roster in
/// iteration order, so citation indexes mean the same thing under either
/// representation.
pub trait AxiomRoster {
    fn roster_len(&self) -> usize;
    fn roster_fact(&self, index: usize) -> &Proposition;
}

/// Append access to a proposition roster.
pub trait AxiomRosterMut: AxiomRoster {
    fn push_fact(&mut self, proposition: Proposition);
}

impl AxiomRoster for Vec<Proposition> {
    fn roster_len(&self) -> usize {
        self.len()
    }

    fn roster_fact(&self, index: usize) -> &Proposition {
        &self[index]
    }
}

impl AxiomRoster for [Proposition] {
    fn roster_len(&self) -> usize {
        self.len()
    }

    fn roster_fact(&self, index: usize) -> &Proposition {
        &self[index]
    }
}

impl AxiomRosterMut for Vec<Proposition> {
    fn push_fact(&mut self, proposition: Proposition) {
        self.push(proposition);
    }
}

/// One edge's proposition roster: the predecessor's facts shared by
/// reference plus this edge's own appended tail.
///
/// Every terminator successor starts from the same established roster — a
/// conditional's two arms and a case's arms each hold the parent's facts
/// plus their own bound pushes. Materializing that per edge deep-clones
/// every `Proposition` for every successor; keeping the established facts
/// behind an `Rc` and appending edge facts in a private tail shares the
/// spine at O(1) per arm. Roster order is the shared facts then the tail —
/// the same order a materialized `Vec` gives — so positions and first-match
/// semantics are unchanged. The view lives only inside reconstruction:
/// joins and exit rosters materialize to `Vec<Proposition>` at their
/// boundaries, so exported obligation types are untouched.
#[derive(Clone)]
pub struct SharedRoster {
    shared: std::rc::Rc<Vec<Proposition>>,
    tail: Vec<Proposition>,
}

impl SharedRoster {
    /// Wrap a materialized roster as the shared spine later edge views
    /// reference.
    pub fn whole(axioms: Vec<Proposition>) -> Self {
        Self {
            shared: std::rc::Rc::new(axioms),
            tail: Vec::new(),
        }
    }

    /// An empty-tailed edge view over this roster's shared spine.
    pub fn successor(&self) -> Self {
        Self {
            shared: self.shared.clone(),
            tail: Vec::new(),
        }
    }

    /// The roster's fact count.
    pub fn len(&self) -> usize {
        self.shared.len() + self.tail.len()
    }

    /// Iterate shared facts then tail facts — roster order.
    pub fn iter(&self) -> impl Iterator<Item = &Proposition> {
        self.shared.iter().chain(self.tail.iter())
    }

    /// Materialize the whole roster — the join and export boundary shape.
    pub fn to_vec(&self) -> Vec<Proposition> {
        self.iter().cloned().collect()
    }

    /// Append one fact to this edge's private tail.
    pub fn push(&mut self, proposition: Proposition) {
        self.tail.push(proposition);
    }
}

impl AxiomRoster for SharedRoster {
    fn roster_len(&self) -> usize {
        self.len()
    }

    fn roster_fact(&self, index: usize) -> &Proposition {
        self.shared
            .get(index)
            .unwrap_or_else(|| &self.tail[index - self.shared.len()])
    }
}

impl AxiomRosterMut for SharedRoster {
    fn push_fact(&mut self, proposition: Proposition) {
        self.push(proposition);
    }
}

/// Fingerprint-bucketed view of a proposition roster for repeated membership
/// and position queries.
///
/// Reconstruction edges each consult a roster hundreds of times — every edge
/// parameter binding, rewritten established fact, and discrete fact emission
/// asks whether the roster already carries the fact or where it sits — so a
/// call that fingerprints the roster once turns each of those roster scans
/// into a bucket lookup. `Equal` propositions index under an operand-order
/// fingerprint so one bucket answers both the exact match and the
/// symmetric-twin match [`roster_equation_position`] applies; every bucket
/// candidate is still resolved with full proposition equality, and bucket
/// positions stay in roster order so first-match semantics are unchanged.
/// The index owns no roster borrow: callers fingerprint the roster once and
/// record each later push, so reads and pushes interleave freely.
pub struct FactIndex {
    positions: HashMap<u64, Vec<usize>>,
}

impl FactIndex {
    /// Fingerprint `axioms` once. Index construction is the same single
    /// roster pass one linear membership scan costs.
    pub fn new(axioms: &(impl AxiomRoster + ?Sized)) -> Self {
        let mut positions = HashMap::<u64, Vec<usize>>::new();
        for position in 0..axioms.roster_len() {
            positions
                .entry(index_key(axioms.roster_fact(position)))
                .or_default()
                .push(position);
        }
        Self { positions }
    }

    /// The position of `left = right` in the roster under either operand
    /// order, with the same first-match semantics as
    /// [`roster_equation_position`].
    pub fn equation_position(
        &self,
        axioms: &(impl AxiomRoster + ?Sized),
        left: &ScalarTerm,
        right: &ScalarTerm,
    ) -> Option<usize> {
        self.positions
            .get(&equation_key(left, right))?
            .iter()
            .copied()
            .find(|&position| match axioms.roster_fact(position) {
                Proposition::Equal(l, r) => (l == left && r == right) || (l == right && r == left),
                _ => false,
            })
    }

    /// The first roster position holding `proposition` exactly.
    pub fn position_of(
        &self,
        axioms: &(impl AxiomRoster + ?Sized),
        proposition: &Proposition,
    ) -> Option<usize> {
        self.positions
            .get(&index_key(proposition))?
            .iter()
            .copied()
            .find(|&position| axioms.roster_fact(position) == proposition)
    }

    /// [`roster_has_fact`] answered through the index.
    pub fn has_fact(
        &self,
        axioms: &(impl AxiomRoster + ?Sized),
        proposition: &Proposition,
    ) -> bool {
        match proposition {
            Proposition::Equal(left, right)
                if matches!(left, ScalarTerm::Value { .. })
                    && matches!(right, ScalarTerm::Value { .. }) =>
            {
                self.equation_position(axioms, left, right).is_some()
            }
            _ => self.position_of(axioms, proposition).is_some(),
        }
    }

    /// Register `axioms[position] = proposition` after a push so later
    /// queries see it.
    pub fn record(&mut self, position: usize, proposition: &Proposition) {
        self.positions
            .entry(index_key(proposition))
            .or_default()
            .push(position);
    }

    /// [`push_unique_fact`] answered through the index.
    pub fn push_unique(&mut self, axioms: &mut impl AxiomRosterMut, proposition: Proposition) {
        if !self.has_fact(axioms, &proposition) {
            self.record(axioms.roster_len(), &proposition);
            axioms.push_fact(proposition);
        }
    }
}

/// Bucket key for a rostered proposition. `Equal` rows share one
/// operand-order bucket so a symmetric-twin lookup reads one bucket;
/// other propositions bucket on their own fingerprint.
fn index_key(proposition: &Proposition) -> u64 {
    match proposition {
        Proposition::Equal(left, right) => equation_key(left, right),
        _ => fact_cache_fingerprint(proposition),
    }
}

/// Operand-order-free bucket key for `left = right`: the same equation under
/// either written order lands in one bucket.
fn equation_key(left: &ScalarTerm, right: &ScalarTerm) -> u64 {
    let mut hasher = FactHasher::default();
    left.hash(&mut hasher);
    let left = hasher.finish();
    let mut hasher = FactHasher::default();
    right.hash(&mut hasher);
    let right = hasher.finish();
    0x9e37_79b9_7f4a_7c15_u64 ^ (left ^ right)
}

/// Fast, non-authoritative bucketing for exact proposition deduplication.
/// Collisions are always resolved with full proposition equality.
fn fact_cache_fingerprint(proposition: &Proposition) -> u64 {
    let mut hasher = FactHasher::default();
    proposition.hash(&mut hasher);
    hasher.finish()
}

#[derive(Default)]
struct FactHasher(u64);

impl FactHasher {
    fn mix(&mut self, value: u64) {
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

impl Hasher for FactHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        let (chunks, remainder) = bytes.as_chunks::<8>();
        for chunk in chunks {
            self.mix(u64::from_ne_bytes(*chunk));
        }
        if !remainder.is_empty() {
            let mut tail = [0_u8; 8];
            tail[..remainder.len()].copy_from_slice(remainder);
            self.mix(u64::from_ne_bytes(tail) ^ (remainder.len() as u64));
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.mix(value);
    }

    fn write_u128(&mut self, value: u128) {
        self.mix(value as u64);
        self.mix((value >> 64) as u64);
    }

    fn write_usize(&mut self, value: usize) {
        self.mix(value as u64);
    }

    fn write_isize(&mut self, value: isize) {
        self.mix(value as u64);
    }
}

pub(super) fn substitute_scalar_term_values(
    term: &ScalarTerm,
    substitutions: &BTreeMap<ValueId, ScalarTerm>,
) -> ScalarTerm {
    let recurse = |term: &ScalarTerm| substitute_scalar_term_values(term, substitutions);
    macro_rules! typed_binary_body {
        ($variant:ident, $scalar_type:ident, $left:ident, $right:ident) => {
            ScalarTerm::$variant {
                scalar_type: *$scalar_type,
                left: Box::new(recurse($left)),
                right: Box::new(recurse($right)),
            }
        };
    }
    macro_rules! typed_unary_body {
        ($variant:ident, $scalar_type:ident, $operand:ident) => {
            ScalarTerm::$variant {
                scalar_type: *$scalar_type,
                operand: Box::new(recurse($operand)),
            }
        };
    }
    macro_rules! shift_body {
        ($variant:ident, $value_type:ident, $count_type:ident, $value:ident, $count:ident) => {
            ScalarTerm::$variant {
                value_type: *$value_type,
                count_type: *$count_type,
                value: Box::new(recurse($value)),
                count: Box::new(recurse($count)),
            }
        };
    }
    macro_rules! cast_body {
        ($variant:ident, $source_type:ident, $target_type:ident, $operand:ident) => {
            ScalarTerm::$variant {
                source_type: *$source_type,
                target_type: *$target_type,
                operand: Box::new(recurse($operand)),
            }
        };
    }
    match term {
        ScalarTerm::Value { id, .. } => substitutions
            .get(id)
            .cloned()
            .unwrap_or_else(|| term.clone()),
        ScalarTerm::StructuralRead {
            root,
            path,
            scalar_type,
        } => ScalarTerm::StructuralRead {
            root: *root,
            path: path
                .iter()
                .map(|projection| match projection {
                    semantic_vocabulary::ScalarReadProjection::Static(segment) => {
                        semantic_vocabulary::ScalarReadProjection::Static(*segment)
                    }
                    semantic_vocabulary::ScalarReadProjection::Index(index) => {
                        semantic_vocabulary::ScalarReadProjection::Index(Box::new(recurse(index)))
                    }
                })
                .collect(),
            scalar_type: *scalar_type,
        },
        semantic_vocabulary::scalar_term_predicate_quint_pattern!(..)
        | ScalarTerm::Boolean(_)
        | ScalarTerm::Integer { .. }
        | ScalarTerm::IeeeFloatLiteral(_) => term.clone(),
        ScalarTerm::BooleanNot { operand } => ScalarTerm::BooleanNot {
            operand: Box::new(recurse(operand)),
        },
        ScalarTerm::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => typed_unary_body!(IntegerBitwiseNot, scalar_type, operand),
        ScalarTerm::IntegerWiden {
            source_type,
            target_type,
            operand,
        } => cast_body!(IntegerWiden, source_type, target_type, operand),
        ScalarTerm::IntegerExactCast {
            source_type,
            target_type,
            operand,
        } => cast_body!(IntegerExactCast, source_type, target_type, operand),
        ScalarTerm::BooleanEqual { left, right } => ScalarTerm::BooleanEqual {
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::IntegerEqual {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(IntegerEqual, scalar_type, left, right),
        ScalarTerm::IntegerLessThan {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(IntegerLessThan, scalar_type, left, right),
        ScalarTerm::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(IntegerLessOrEqual, scalar_type, left, right),
        ScalarTerm::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(IntegerBitwiseAnd, scalar_type, left, right),
        ScalarTerm::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(IntegerBitwiseOr, scalar_type, left, right),
        ScalarTerm::IntegerBitwiseXor {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(IntegerBitwiseXor, scalar_type, left, right),
        ScalarTerm::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => shift_body!(
            WrappingIntegerShiftLeft,
            value_type,
            count_type,
            value,
            count
        ),
        ScalarTerm::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => shift_body!(
            WrappingIntegerShiftRight,
            value_type,
            count_type,
            value,
            count
        ),
        ScalarTerm::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => shift_body!(ExactIntegerShiftRight, value_type, count_type, value, count),
        ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => shift_body!(ExactIntegerShiftLeft, value_type, count_type, value, count),
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(ExactIntegerAdd, scalar_type, left, right),
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(ExactIntegerSubtract, scalar_type, left, right),
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(ExactIntegerMultiply, scalar_type, left, right),
        ScalarTerm::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(ExactIntegerDivide, scalar_type, left, right),
        ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(ExactIntegerRemainder, scalar_type, left, right),
        ScalarTerm::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(WrappingIntegerDivide, scalar_type, left, right),
        ScalarTerm::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(WrappingIntegerRemainder, scalar_type, left, right),
        ScalarTerm::SaturatingIntegerDivide {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(SaturatingIntegerDivide, scalar_type, left, right),
        ScalarTerm::SaturatingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(SaturatingIntegerRemainder, scalar_type, left, right),
        ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(WrappingIntegerAdd, scalar_type, left, right),
        ScalarTerm::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(SaturatingIntegerAdd, scalar_type, left, right),
        ScalarTerm::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(WrappingIntegerSubtract, scalar_type, left, right),
        ScalarTerm::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(SaturatingIntegerSubtract, scalar_type, left, right),
        ScalarTerm::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(WrappingIntegerMultiply, scalar_type, left, right),
        ScalarTerm::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(SaturatingIntegerMultiply, scalar_type, left, right),
    }
}

/// Replace formal value identities inside published crash routes with the
/// invocation's actual scalar terms. This is the one substitution the
/// verifier reconstructs at boundary calls and operation-level contracts;
/// producers reuse it so an emitted continuation roster cannot drift from
/// what verification will recompute.
pub fn substitute_crash_routes(
    routes: &[CrashRouteBucket],
    substitutions: &BTreeMap<ValueId, ScalarTerm>,
) -> Vec<CrashRouteBucket> {
    routes
        .iter()
        .filter_map(|bucket| {
            let mut alternatives = bucket
                .alternatives
                .iter()
                .filter_map(|guard| match guard {
                    CrashRouteGuard::Truth => Some(CrashRouteGuard::Truth),
                    CrashRouteGuard::Predicate(predicate) => {
                        match substitute_proposition_values(predicate.proposition(), substitutions)
                        {
                            Proposition::Truth => Some(CrashRouteGuard::Truth),
                            Proposition::Falsehood => None,
                            proposition => Some(CrashRouteGuard::Predicate(
                                CrashPredicateTerm::new(proposition),
                            )),
                        }
                    }
                })
                .collect::<Vec<_>>();
            alternatives.sort();
            alternatives.dedup();
            if alternatives.contains(&CrashRouteGuard::Truth) {
                alternatives = vec![CrashRouteGuard::Truth];
            }
            (!alternatives.is_empty()).then_some(CrashRouteBucket {
                cause: bucket.cause,
                alternatives,
            })
        })
        .collect()
}
