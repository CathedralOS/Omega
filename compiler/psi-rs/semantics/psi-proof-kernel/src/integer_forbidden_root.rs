//! Independently checked normalization for correlated affine divisor safety.
//!
//! This checker binds the complete same-root forbidden-lattice analysis to
//! prior semantic axioms. It accepts no proof authority and is not a proof
//! rule.

use std::collections::BTreeSet;

use psi_core::{
    IntegerCarrier, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType, ValueId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelatedAffineStepWitness {
    pub definition_axiom: usize,
    /// Exact prior equality that lands a non-closed right sibling.
    pub literal_axiom: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelatedAffineBranchWitness {
    pub root: ScalarTerm,
    pub target: ScalarTerm,
    pub steps: Vec<CorrelatedAffineStepWitness>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerCorrelatedForbiddenRootWitness {
    pub dividend: CorrelatedAffineBranchWitness,
    pub divisor: CorrelatedAffineBranchWitness,
    /// Separates prior operation definitions from retained signature facts.
    pub definition_axiom_count: usize,
    pub lower_bound_axiom: usize,
    pub upper_bound_axiom: usize,
    /// Exact reducer-facing sufficient proposition reconstructed by the check.
    pub conclusion: Proposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedIntegerCorrelatedForbiddenRoots {
    integer_type: IntegerType,
    root: ScalarTerm,
    dividend: ScalarTerm,
    divisor: ScalarTerm,
    dividend_coefficient: i128,
    dividend_offset: i128,
    divisor_coefficient: i128,
    divisor_offset: i128,
    dividend_definition_axioms: Vec<usize>,
    divisor_definition_axioms: Vec<usize>,
    lower_bound_axiom: usize,
    upper_bound_axiom: usize,
    interval: (i128, i128),
    forbidden_roots: BTreeSet<i128>,
    conclusion: Proposition,
}

impl CheckedIntegerCorrelatedForbiddenRoots {
    pub const fn integer_type(&self) -> IntegerType {
        self.integer_type
    }

    pub const fn root(&self) -> &ScalarTerm {
        &self.root
    }

    pub const fn dividend(&self) -> &ScalarTerm {
        &self.dividend
    }

    pub const fn divisor(&self) -> &ScalarTerm {
        &self.divisor
    }

    pub const fn dividend_form(&self) -> (i128, i128) {
        (self.dividend_coefficient, self.dividend_offset)
    }

    pub const fn divisor_form(&self) -> (i128, i128) {
        (self.divisor_coefficient, self.divisor_offset)
    }

    pub fn dividend_definition_axioms(&self) -> &[usize] {
        &self.dividend_definition_axioms
    }

    pub fn divisor_definition_axioms(&self) -> &[usize] {
        &self.divisor_definition_axioms
    }

    pub const fn bound_axioms(&self) -> (usize, usize) {
        (self.lower_bound_axiom, self.upper_bound_axiom)
    }

    pub const fn interval(&self) -> (i128, i128) {
        self.interval
    }

    pub const fn forbidden_roots(&self) -> &BTreeSet<i128> {
        &self.forbidden_roots
    }

    pub const fn conclusion(&self) -> &Proposition {
        &self.conclusion
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckedBranch {
    integer_type: IntegerType,
    coefficient: i128,
    offset: i128,
    definition_axioms: Vec<usize>,
}

pub fn check_integer_correlated_forbidden_root_witness(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
    witness: &IntegerCorrelatedForbiddenRootWitness,
) -> Result<CheckedIntegerCorrelatedForbiddenRoots, IntegerCorrelatedForbiddenRootWitnessError> {
    if witness.definition_axiom_count > semantic_axioms.len() {
        return Err(IntegerCorrelatedForbiddenRootWitnessError::InvalidDefinitionBoundary);
    }
    let dividend = check_branch(
        context,
        semantic_axioms,
        machine_parameter_values,
        witness.definition_axiom_count,
        &witness.dividend,
        CorrelatedAffineBranch::Dividend,
    )?;
    let divisor = check_branch(
        context,
        semantic_axioms,
        machine_parameter_values,
        witness.definition_axiom_count,
        &witness.divisor,
        CorrelatedAffineBranch::Divisor,
    )?;
    if dividend.integer_type != divisor.integer_type {
        return Err(IntegerCorrelatedForbiddenRootWitnessError::BranchTypeMismatch);
    }
    if witness.dividend.root != witness.divisor.root {
        return Err(IntegerCorrelatedForbiddenRootWitnessError::RootMismatch);
    }
    if dividend.coefficient == 0 || divisor.coefficient == 0 {
        return Err(IntegerCorrelatedForbiddenRootWitnessError::ConstantBranch);
    }
    let dividend_indices = &dividend.definition_axioms;
    let divisor_indices = &divisor.definition_axioms;
    if !dividend_indices
        .iter()
        .all(|index| !divisor_indices.contains(index))
        || dividend_indices.last().expect("nonempty branch")
            >= divisor_indices.first().expect("nonempty branch")
    {
        return Err(IntegerCorrelatedForbiddenRootWitnessError::BranchOrderMismatch);
    }

    let integer_type = dividend.integer_type;
    let bounds = selected_signature_bounds(
        context,
        semantic_axioms,
        witness.definition_axiom_count,
        &witness.dividend.root,
        integer_type,
    )?;
    if witness.lower_bound_axiom != bounds.lower_axiom
        || witness.upper_bound_axiom != bounds.upper_axiom
    {
        return Err(IntegerCorrelatedForbiddenRootWitnessError::BoundIdentityMismatch);
    }

    let mut forbidden_roots = BTreeSet::new();
    if let Some(root) = affine_equation_root(divisor.coefficient, divisor.offset, 0)?
        && in_interval(root, bounds.interval)
    {
        forbidden_roots.insert(root);
    }
    if let Some(root) = affine_equation_root(divisor.coefficient, divisor.offset, -1)?
        && in_interval(root, bounds.interval)
        && affine_value(dividend.coefficient, dividend.offset, root)?
            == signed_minimum(integer_type)?
    {
        forbidden_roots.insert(root);
    }
    let interval_size = bounds
        .interval
        .1
        .checked_sub(bounds.interval.0)
        .and_then(|span| span.checked_add(1))
        .ok_or(IntegerCorrelatedForbiddenRootWitnessError::ArithmeticOverflow)?;
    let expected = if forbidden_roots.is_empty() {
        Proposition::Conjunction(vec![bounds.lower.clone(), bounds.upper.clone()])
    } else if interval_size
        == i128::try_from(forbidden_roots.len())
            .map_err(|_| IntegerCorrelatedForbiddenRootWitnessError::ArithmeticOverflow)?
    {
        Proposition::Falsehood
    } else {
        return Err(IntegerCorrelatedForbiddenRootWitnessError::PartiallyUnsafeInterval);
    };
    if witness.conclusion != expected {
        return Err(IntegerCorrelatedForbiddenRootWitnessError::ConclusionMismatch);
    }
    context
        .validate(&witness.conclusion)
        .map_err(IntegerCorrelatedForbiddenRootWitnessError::MalformedProposition)?;

    Ok(CheckedIntegerCorrelatedForbiddenRoots {
        integer_type,
        root: witness.dividend.root.clone(),
        dividend: witness.dividend.target.clone(),
        divisor: witness.divisor.target.clone(),
        dividend_coefficient: dividend.coefficient,
        dividend_offset: dividend.offset,
        divisor_coefficient: divisor.coefficient,
        divisor_offset: divisor.offset,
        dividend_definition_axioms: dividend.definition_axioms,
        divisor_definition_axioms: divisor.definition_axioms,
        lower_bound_axiom: bounds.lower_axiom,
        upper_bound_axiom: bounds.upper_axiom,
        interval: bounds.interval,
        forbidden_roots,
        conclusion: expected,
    })
}

fn check_branch(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
    definition_axiom_count: usize,
    witness: &CorrelatedAffineBranchWitness,
    branch: CorrelatedAffineBranch,
) -> Result<CheckedBranch, IntegerCorrelatedForbiddenRootWitnessError> {
    let ScalarTerm::Value {
        id,
        scalar_type: ScalarType::Integer(integer_type),
    } = witness.root
    else {
        return Err(IntegerCorrelatedForbiddenRootWitnessError::RootNotSignedNative(branch));
    };
    if !machine_parameter_values.contains(&id)
        || integer_type.carrier() != IntegerCarrier::Fixed
        || integer_type.sign() != IntegerSign::Signed
        || !matches!(integer_type.bits(), 8 | 16 | 32 | 64)
        || witness.target.scalar_type() != ScalarType::Integer(integer_type)
    {
        return Err(IntegerCorrelatedForbiddenRootWitnessError::RootNotSignedNative(branch));
    }
    if witness.steps.is_empty() {
        return Err(IntegerCorrelatedForbiddenRootWitnessError::EmptyBranch(
            branch,
        ));
    }
    if witness
        .steps
        .windows(2)
        .any(|steps| steps[0].definition_axiom >= steps[1].definition_axiom)
    {
        return Err(IntegerCorrelatedForbiddenRootWitnessError::NonCanonicalBranchOrder(branch));
    }

    let mut current = witness.target.clone();
    let mut prior = definition_axiom_count;
    let mut coefficient = 1_i128;
    let mut offset = 0_i128;
    for step in witness.steps.iter().rev() {
        let index = step.definition_axiom;
        if index >= prior {
            return Err(
                IntegerCorrelatedForbiddenRootWitnessError::NonCanonicalBranchOrder(branch),
            );
        }
        let latest = semantic_axioms[..prior].iter().enumerate().rev().find_map(
            |(candidate, proposition)| match proposition {
                Proposition::Equal(left, _) if left == &current => Some(candidate),
                _ => None,
            },
        );
        if latest != Some(index) {
            return Err(
                IntegerCorrelatedForbiddenRootWitnessError::StaleDefinition { branch, index },
            );
        }
        let proposition = checked_axiom(context, semantic_axioms, index)?;
        let Proposition::Equal(target, expression) = proposition else {
            return Err(
                IntegerCorrelatedForbiddenRootWitnessError::DefinitionShape { branch, index },
            );
        };
        if target != &current {
            return Err(
                IntegerCorrelatedForbiddenRootWitnessError::DefinitionShape { branch, index },
            );
        }
        let (next, sibling, nested_coefficient, nested_offset) = match expression {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (left, right, 1_i128, false),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (left, right, 1_i128, true),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (left, right, 0_i128, false),
            _ => {
                return Err(
                    IntegerCorrelatedForbiddenRootWitnessError::DefinitionShape { branch, index },
                );
            }
        };
        if known_landed_literal(semantic_axioms, index, integer_type, next).is_some() {
            return Err(
                IntegerCorrelatedForbiddenRootWitnessError::ConstantBranchPath { branch, index },
            );
        }
        let literal = landed_literal(
            context,
            semantic_axioms,
            index,
            step.literal_axiom,
            integer_type,
            sibling,
        )?;
        let (step_coefficient, step_offset) =
            if matches!(expression, ScalarTerm::ExactIntegerMultiply { .. }) {
                (literal, 0)
            } else if nested_offset {
                (
                    nested_coefficient,
                    literal
                        .checked_neg()
                        .ok_or(IntegerCorrelatedForbiddenRootWitnessError::ArithmeticOverflow)?,
                )
            } else {
                (nested_coefficient, literal)
            };
        offset = step_offset
            .checked_mul(coefficient)
            .and_then(|nested| nested.checked_add(offset))
            .ok_or(IntegerCorrelatedForbiddenRootWitnessError::ArithmeticOverflow)?;
        coefficient = coefficient
            .checked_mul(step_coefficient)
            .ok_or(IntegerCorrelatedForbiddenRootWitnessError::ArithmeticOverflow)?;
        current = (**next).clone();
        prior = index;
    }
    if current != witness.root {
        return Err(IntegerCorrelatedForbiddenRootWitnessError::BranchRootDrift(
            branch,
        ));
    }
    Ok(CheckedBranch {
        integer_type,
        coefficient,
        offset,
        definition_axioms: witness
            .steps
            .iter()
            .map(|step| step.definition_axiom)
            .collect(),
    })
}

fn known_landed_literal(
    semantic_axioms: &[Proposition],
    prior_axiom_count: usize,
    integer_type: IntegerType,
    term: &ScalarTerm,
) -> Option<i128> {
    signed_literal(term, integer_type).or_else(|| {
        semantic_axioms[..prior_axiom_count].iter().rev().find_map(
            |proposition| match proposition {
                Proposition::Equal(left, right) if left == term => {
                    signed_literal(right, integer_type)
                }
                _ => None,
            },
        )
    })
}

fn landed_literal(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definition_axiom: usize,
    literal_axiom: Option<usize>,
    integer_type: IntegerType,
    term: &ScalarTerm,
) -> Result<i128, IntegerCorrelatedForbiddenRootWitnessError> {
    if let Some(value) = signed_literal(term, integer_type) {
        if literal_axiom.is_some() {
            return Err(
                IntegerCorrelatedForbiddenRootWitnessError::UnexpectedLiteralAxiom(
                    definition_axiom,
                ),
            );
        }
        return Ok(value);
    }
    let selected = literal_axiom
        .ok_or(IntegerCorrelatedForbiddenRootWitnessError::MissingLiteralAxiom(definition_axiom))?;
    if selected >= definition_axiom {
        return Err(
            IntegerCorrelatedForbiddenRootWitnessError::LiteralAxiomNotPrior {
                definition_axiom,
                literal_axiom: selected,
            },
        );
    }
    let latest = semantic_axioms[..definition_axiom]
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, proposition)| match proposition {
            Proposition::Equal(left, right) if left == term => {
                signed_literal(right, integer_type).map(|value| (index, value))
            }
            _ => None,
        });
    let Some((index, value)) = latest else {
        return Err(
            IntegerCorrelatedForbiddenRootWitnessError::MissingLiteralAxiom(definition_axiom),
        );
    };
    if index != selected {
        return Err(
            IntegerCorrelatedForbiddenRootWitnessError::LiteralIdentityMismatch(definition_axiom),
        );
    }
    checked_axiom(context, semantic_axioms, selected)?;
    Ok(value)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectedBounds {
    lower_axiom: usize,
    upper_axiom: usize,
    lower: Proposition,
    upper: Proposition,
    interval: (i128, i128),
}

fn selected_signature_bounds(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    root: &ScalarTerm,
    integer_type: IntegerType,
) -> Result<SelectedBounds, IntegerCorrelatedForbiddenRootWitnessError> {
    let (mut minimum, mut maximum) = carrier_interval(integer_type)?;
    let mut lower = None;
    let mut upper = None;
    for (index, proposition) in semantic_axioms
        .iter()
        .enumerate()
        .skip(definition_axiom_count)
    {
        let Proposition::LessOrEqual(left, right) = proposition else {
            continue;
        };
        if right == root
            && let Some(candidate) = signed_literal(left, integer_type)
            && candidate > minimum
        {
            context
                .validate(proposition)
                .map_err(IntegerCorrelatedForbiddenRootWitnessError::MalformedProposition)?;
            minimum = candidate;
            lower = Some((index, proposition.clone()));
        }
        if left == root
            && let Some(candidate) = signed_literal(right, integer_type)
            && candidate < maximum
        {
            context
                .validate(proposition)
                .map_err(IntegerCorrelatedForbiddenRootWitnessError::MalformedProposition)?;
            maximum = candidate;
            upper = Some((index, proposition.clone()));
        }
    }
    if minimum > maximum {
        return Err(IntegerCorrelatedForbiddenRootWitnessError::InconsistentBounds);
    }
    let (lower_axiom, lower) =
        lower.ok_or(IntegerCorrelatedForbiddenRootWitnessError::MissingTightBounds)?;
    let (upper_axiom, upper) =
        upper.ok_or(IntegerCorrelatedForbiddenRootWitnessError::MissingTightBounds)?;
    Ok(SelectedBounds {
        lower_axiom,
        upper_axiom,
        lower,
        upper,
        interval: (minimum, maximum),
    })
}

fn checked_axiom<'a>(
    context: &PropositionContext,
    semantic_axioms: &'a [Proposition],
    index: usize,
) -> Result<&'a Proposition, IntegerCorrelatedForbiddenRootWitnessError> {
    let proposition = semantic_axioms
        .get(index)
        .ok_or(IntegerCorrelatedForbiddenRootWitnessError::UnknownSemanticAxiom(index))?;
    context
        .validate(proposition)
        .map_err(IntegerCorrelatedForbiddenRootWitnessError::MalformedProposition)?;
    Ok(proposition)
}

fn signed_literal(term: &ScalarTerm, integer_type: IntegerType) -> Option<i128> {
    match term.integer_value()? {
        (actual_type, IntegerValue::Signed(value)) if actual_type == integer_type => Some(value),
        _ => None,
    }
}

fn carrier_interval(
    integer_type: IntegerType,
) -> Result<(i128, i128), IntegerCorrelatedForbiddenRootWitnessError> {
    if integer_type.carrier() != IntegerCarrier::Fixed
        || integer_type.sign() != IntegerSign::Signed
        || !matches!(integer_type.bits(), 8 | 16 | 32 | 64)
    {
        return Err(IntegerCorrelatedForbiddenRootWitnessError::UnsupportedCarrier(integer_type));
    }
    Ok((signed_minimum(integer_type)?, signed_maximum(integer_type)?))
}

fn signed_minimum(
    integer_type: IntegerType,
) -> Result<i128, IntegerCorrelatedForbiddenRootWitnessError> {
    match integer_type.minimum_value() {
        IntegerValue::Signed(value) => Ok(value),
        _ => Err(IntegerCorrelatedForbiddenRootWitnessError::UnsupportedCarrier(integer_type)),
    }
}

fn signed_maximum(
    integer_type: IntegerType,
) -> Result<i128, IntegerCorrelatedForbiddenRootWitnessError> {
    match integer_type.maximum_value() {
        IntegerValue::Signed(value) => Ok(value),
        _ => Err(IntegerCorrelatedForbiddenRootWitnessError::UnsupportedCarrier(integer_type)),
    }
}

fn affine_equation_root(
    coefficient: i128,
    offset: i128,
    target: i128,
) -> Result<Option<i128>, IntegerCorrelatedForbiddenRootWitnessError> {
    let numerator = target
        .checked_sub(offset)
        .ok_or(IntegerCorrelatedForbiddenRootWitnessError::ArithmeticOverflow)?;
    if numerator
        .checked_rem(coefficient)
        .ok_or(IntegerCorrelatedForbiddenRootWitnessError::ArithmeticOverflow)?
        != 0
    {
        return Ok(None);
    }
    numerator
        .checked_div(coefficient)
        .ok_or(IntegerCorrelatedForbiddenRootWitnessError::ArithmeticOverflow)
        .map(Some)
}

fn affine_value(
    coefficient: i128,
    offset: i128,
    root: i128,
) -> Result<i128, IntegerCorrelatedForbiddenRootWitnessError> {
    coefficient
        .checked_mul(root)
        .and_then(|value| value.checked_add(offset))
        .ok_or(IntegerCorrelatedForbiddenRootWitnessError::ArithmeticOverflow)
}

fn in_interval(value: i128, interval: (i128, i128)) -> bool {
    value >= interval.0 && value <= interval.1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrelatedAffineBranch {
    Dividend,
    Divisor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegerCorrelatedForbiddenRootWitnessError {
    InvalidDefinitionBoundary,
    RootNotSignedNative(CorrelatedAffineBranch),
    EmptyBranch(CorrelatedAffineBranch),
    NonCanonicalBranchOrder(CorrelatedAffineBranch),
    UnknownSemanticAxiom(usize),
    MalformedProposition(psi_core::PropositionError),
    StaleDefinition {
        branch: CorrelatedAffineBranch,
        index: usize,
    },
    DefinitionShape {
        branch: CorrelatedAffineBranch,
        index: usize,
    },
    ConstantBranchPath {
        branch: CorrelatedAffineBranch,
        index: usize,
    },
    MissingLiteralAxiom(usize),
    UnexpectedLiteralAxiom(usize),
    LiteralAxiomNotPrior {
        definition_axiom: usize,
        literal_axiom: usize,
    },
    LiteralIdentityMismatch(usize),
    BranchRootDrift(CorrelatedAffineBranch),
    BranchTypeMismatch,
    RootMismatch,
    ConstantBranch,
    BranchOrderMismatch,
    UnsupportedCarrier(IntegerType),
    MissingTightBounds,
    InconsistentBounds,
    BoundIdentityMismatch,
    ArithmeticOverflow,
    PartiallyUnsafeInterval,
    ConclusionMismatch,
}

impl std::fmt::Display for IntegerCorrelatedForbiddenRootWitnessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for IntegerCorrelatedForbiddenRootWitnessError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn i8_type() -> IntegerType {
        IntegerType::new(IntegerSign::Signed, 8).expect("i8")
    }

    fn value(id: u64, integer_type: IntegerType) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(integer_type),
        )
    }

    fn integer(integer_type: IntegerType, value: i128) -> ScalarTerm {
        ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).expect("integer")
    }

    fn add(
        target: ScalarTerm,
        integer_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Proposition {
        Proposition::Equal(
            target,
            ScalarTerm::exact_integer_add(integer_type, left, right).expect("add"),
        )
    }

    fn subtract(
        target: ScalarTerm,
        integer_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Proposition {
        Proposition::Equal(
            target,
            ScalarTerm::exact_integer_subtract(integer_type, left, right).expect("subtract"),
        )
    }

    fn multiply(
        target: ScalarTerm,
        integer_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Proposition {
        Proposition::Equal(
            target,
            ScalarTerm::exact_integer_multiply(integer_type, left, right).expect("multiply"),
        )
    }

    fn safe_fixture() -> (
        PropositionContext,
        Vec<Proposition>,
        BTreeSet<ValueId>,
        IntegerCorrelatedForbiddenRootWitness,
    ) {
        let integer_type = i8_type();
        let root = value(1, integer_type);
        let sixty_four = value(2, integer_type);
        let left_offset = value(3, integer_type);
        let negative_two = value(4, integer_type);
        let dividend = value(5, integer_type);
        let two = value(6, integer_type);
        let right_product = value(7, integer_type);
        let divisor = value(8, integer_type);
        let lower = Proposition::LessOrEqual(integer(integer_type, -1), root.clone());
        let upper = Proposition::LessOrEqual(root.clone(), integer(integer_type, 0));
        let axioms = vec![
            Proposition::Equal(sixty_four.clone(), integer(integer_type, 64)),
            add(left_offset.clone(), integer_type, root.clone(), sixty_four),
            Proposition::Equal(negative_two.clone(), integer(integer_type, -2)),
            multiply(dividend.clone(), integer_type, left_offset, negative_two),
            Proposition::Equal(two.clone(), integer(integer_type, 2)),
            multiply(right_product.clone(), integer_type, root.clone(), two),
            add(
                divisor.clone(),
                integer_type,
                right_product,
                integer(integer_type, 1),
            ),
            lower.clone(),
            upper.clone(),
        ];
        let context = PropositionContext::from_value_types((1..=8).map(|id| {
            (
                ValueId::new(id).expect("value id"),
                ScalarType::Integer(integer_type),
            )
        }))
        .expect("context");
        let witness = IntegerCorrelatedForbiddenRootWitness {
            dividend: CorrelatedAffineBranchWitness {
                root: root.clone(),
                target: dividend,
                steps: vec![
                    CorrelatedAffineStepWitness {
                        definition_axiom: 1,
                        literal_axiom: Some(0),
                    },
                    CorrelatedAffineStepWitness {
                        definition_axiom: 3,
                        literal_axiom: Some(2),
                    },
                ],
            },
            divisor: CorrelatedAffineBranchWitness {
                root: root.clone(),
                target: divisor,
                steps: vec![
                    CorrelatedAffineStepWitness {
                        definition_axiom: 5,
                        literal_axiom: Some(4),
                    },
                    CorrelatedAffineStepWitness {
                        definition_axiom: 6,
                        literal_axiom: None,
                    },
                ],
            },
            definition_axiom_count: 7,
            lower_bound_axiom: 7,
            upper_bound_axiom: 8,
            conclusion: Proposition::Conjunction(vec![lower, upper]),
        };
        (
            context,
            axioms,
            BTreeSet::from([ValueId::new(1).expect("root")]),
            witness,
        )
    }

    #[test]
    fn checks_complete_landed_affine_forbidden_root_family() {
        let (context, axioms, parameters, witness) = safe_fixture();
        let checked = check_integer_correlated_forbidden_root_witness(
            &context,
            &axioms,
            &parameters,
            &witness,
        )
        .expect("safe correlated witness");
        assert_eq!(checked.dividend_form(), (-2, -128));
        assert_eq!(checked.divisor_form(), (2, 1));
        assert_eq!(checked.interval(), (-1, 0));
        assert!(checked.forbidden_roots().is_empty());
        assert_eq!(checked.bound_axioms(), (7, 8));
        assert_eq!(checked.conclusion(), &witness.conclusion);
    }

    #[test]
    fn checks_when_forbidden_roots_cover_the_complete_interval() {
        let integer_type = i8_type();
        let root = value(20, integer_type);
        let left_offset = value(21, integer_type);
        let dividend = value(22, integer_type);
        let divisor = value(23, integer_type);
        let lower = Proposition::LessOrEqual(integer(integer_type, -1), root.clone());
        let upper = Proposition::LessOrEqual(root.clone(), integer(integer_type, 0));
        let axioms = vec![
            subtract(
                left_offset.clone(),
                integer_type,
                root.clone(),
                integer(integer_type, 63),
            ),
            multiply(
                dividend.clone(),
                integer_type,
                left_offset,
                integer(integer_type, 2),
            ),
            multiply(
                divisor.clone(),
                integer_type,
                root.clone(),
                integer(integer_type, 1),
            ),
            lower,
            upper,
        ];
        let context = PropositionContext::from_value_types((20..=23).map(|id| {
            (
                ValueId::new(id).expect("value id"),
                ScalarType::Integer(integer_type),
            )
        }))
        .expect("context");
        let witness = IntegerCorrelatedForbiddenRootWitness {
            dividend: CorrelatedAffineBranchWitness {
                root: root.clone(),
                target: dividend,
                steps: vec![
                    CorrelatedAffineStepWitness {
                        definition_axiom: 0,
                        literal_axiom: None,
                    },
                    CorrelatedAffineStepWitness {
                        definition_axiom: 1,
                        literal_axiom: None,
                    },
                ],
            },
            divisor: CorrelatedAffineBranchWitness {
                root,
                target: divisor,
                steps: vec![CorrelatedAffineStepWitness {
                    definition_axiom: 2,
                    literal_axiom: None,
                }],
            },
            definition_axiom_count: 3,
            lower_bound_axiom: 3,
            upper_bound_axiom: 4,
            conclusion: Proposition::Falsehood,
        };
        let checked = check_integer_correlated_forbidden_root_witness(
            &context,
            &axioms,
            &BTreeSet::from([ValueId::new(20).expect("root")]),
            &witness,
        )
        .expect("whole interval forbidden");
        assert_eq!(checked.forbidden_roots(), &BTreeSet::from([-1, 0]));
        assert_eq!(checked.conclusion(), &Proposition::Falsehood);
    }

    #[test]
    fn rejects_partial_safety_and_identity_order_root_or_type_drift() {
        let (context, axioms, parameters, witness) = safe_fixture();

        let mut bound_drift = witness.clone();
        bound_drift.lower_bound_axiom = 8;
        assert_eq!(
            check_integer_correlated_forbidden_root_witness(
                &context,
                &axioms,
                &parameters,
                &bound_drift,
            ),
            Err(IntegerCorrelatedForbiddenRootWitnessError::BoundIdentityMismatch),
        );

        let mut conclusion_drift = witness.clone();
        conclusion_drift.conclusion =
            Proposition::Conjunction(vec![axioms[8].clone(), axioms[7].clone()]);
        assert_eq!(
            check_integer_correlated_forbidden_root_witness(
                &context,
                &axioms,
                &parameters,
                &conclusion_drift,
            ),
            Err(IntegerCorrelatedForbiddenRootWitnessError::ConclusionMismatch),
        );

        let mut order_drift = witness.clone();
        std::mem::swap(&mut order_drift.dividend, &mut order_drift.divisor);
        assert_eq!(
            check_integer_correlated_forbidden_root_witness(
                &context,
                &axioms,
                &parameters,
                &order_drift,
            ),
            Err(IntegerCorrelatedForbiddenRootWitnessError::BranchOrderMismatch),
        );

        let mut root_drift = witness.clone();
        root_drift.divisor.root = value(9, i8_type());
        assert!(matches!(
            check_integer_correlated_forbidden_root_witness(
                &context,
                &axioms,
                &parameters,
                &root_drift,
            ),
            Err(
                IntegerCorrelatedForbiddenRootWitnessError::RootNotSignedNative(
                    CorrelatedAffineBranch::Divisor
                )
            ),
        ));

        let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned_root = value(1, unsigned);
        let mut type_drift = witness.clone();
        type_drift.dividend.root = unsigned_root;
        assert!(matches!(
            check_integer_correlated_forbidden_root_witness(
                &context,
                &axioms,
                &parameters,
                &type_drift,
            ),
            Err(
                IntegerCorrelatedForbiddenRootWitnessError::RootNotSignedNative(
                    CorrelatedAffineBranch::Dividend
                )
            ),
        ));

        let mut partial_axioms = axioms.clone();
        partial_axioms[6] = multiply(
            witness.divisor.target.clone(),
            i8_type(),
            value(7, i8_type()),
            integer(i8_type(), 1),
        );
        assert_eq!(
            check_integer_correlated_forbidden_root_witness(
                &context,
                &partial_axioms,
                &parameters,
                &witness,
            ),
            Err(IntegerCorrelatedForbiddenRootWitnessError::PartiallyUnsafeInterval),
        );
    }
}
