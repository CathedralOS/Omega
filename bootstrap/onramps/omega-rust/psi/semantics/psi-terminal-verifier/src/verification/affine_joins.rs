//! Correlated affine reduction strategies.
//!
//! These helpers are sufficient-form certificate producers during the
//! canonical-semantic-ledger migration. They may derive affine summaries, but
//! they do not own canonical terminal-Psi goal reconstruction.

use std::collections::BTreeSet;

use psi_core::{IntegerSign, IntegerValue, Proposition, ScalarTerm, ScalarType, ValueId};

use super::{
    ExactIntegerOffsetOperation, IntegerOffset, canonical_conjunction,
    exact_integer_signed_affine_interval_obligation, fixed_integer_type_interval,
    integer_value_as_i128, landed_integer_constant_value, signed_negative_magnitude,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactIntegerAffineForkBranch {
    root: ScalarTerm,
    coefficient: IntegerOffset,
    offset: IntegerOffset,
    definition_indices: BTreeSet<usize>,
}

fn exact_integer_affine_fork_branch(
    integer_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<ExactIntegerAffineForkBranch> {
    fixed_integer_type_interval(integer_type)?;
    let mut coefficient = IntegerOffset::Nonnegative(1);
    let mut offset = IntegerOffset::Nonnegative(0);
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut definition_indices = BTreeSet::new();
    for _ in 0..=prior_axiom_count {
        if !definition_indices.is_empty()
            && matches!(
                &variable,
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(root_type),
                } if *root_type == integer_type && machine_parameter_values.contains(id)
            )
        {
            return Some(ExactIntegerAffineForkBranch {
                root: variable,
                coefficient,
                offset,
                definition_indices,
            });
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &variable => Some((index, right)),
                _ => None,
            })?;
        let (left, right, nested_coefficient, nested_offset) = match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (
                left,
                right,
                IntegerOffset::Nonnegative(1),
                IntegerOffset::from_value(landed_integer_constant_value(
                    integer_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
            ),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (
                left,
                right,
                IntegerOffset::Nonnegative(1),
                IntegerOffset::from_subtrahend(landed_integer_constant_value(
                    integer_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
            ),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (
                left,
                right,
                IntegerOffset::from_value(landed_integer_constant_value(
                    integer_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
                IntegerOffset::Nonnegative(0),
            ),
            _ => return None,
        };
        if landed_integer_constant_value(integer_type, left, semantic_axioms, definition_index)
            .is_some()
            || landed_integer_constant_value(integer_type, right, semantic_axioms, definition_index)
                .is_none()
            || !definition_indices.insert(definition_index)
        {
            return None;
        }
        offset = nested_offset
            .checked_multiply_offset(coefficient)
            .and_then(|nested| nested.checked_add(offset))?;
        coefficient = coefficient.checked_multiply_offset(nested_coefficient)?;
        variable = (**left).clone();
        prior_axiom_count = definition_index;
    }
    None
}

pub(super) fn exact_integer_affine_fork_join_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    operation: ExactIntegerOffsetOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let left = exact_integer_affine_fork_branch(
        integer_type,
        left,
        semantic_axioms,
        definition_axiom_count,
        machine_parameter_values,
    )?;
    let mut right = exact_integer_affine_fork_branch(
        integer_type,
        right,
        semantic_axioms,
        definition_axiom_count,
        machine_parameter_values,
    )?;
    if left.root != right.root
        || !left
            .definition_indices
            .is_disjoint(&right.definition_indices)
        || left.definition_indices.iter().next_back()? >= right.definition_indices.iter().next()?
    {
        return None;
    }
    if operation == ExactIntegerOffsetOperation::Subtract {
        right.coefficient = right.coefficient.negated();
        right.offset = right.offset.negated();
    }
    let coefficient = left.coefficient.checked_add(right.coefficient)?;
    let offset = left.offset.checked_add(right.offset)?;
    exact_integer_signed_affine_interval_obligation(
        integer_type,
        left.root,
        coefficient,
        offset,
        fixed_integer_type_interval(integer_type)?,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactIntegerSignatureInterval {
    interval: (i128, i128),
    selected_bounds: Vec<Proposition>,
}

fn exact_integer_signature_interval(
    integer_type: psi_core::IntegerType,
    root: &ScalarTerm,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
) -> Option<ExactIntegerSignatureInterval> {
    let (mut minimum, mut maximum) = fixed_integer_type_interval(integer_type)?;
    let mut lower_bound = None;
    let mut upper_bound = None;
    let constant = |term: &ScalarTerm| {
        let (constant_type, value) = term.integer_value()?;
        (constant_type == integer_type)
            .then(|| integer_value_as_i128(value))
            .flatten()
    };
    for axiom in &semantic_axioms[definition_axiom_count.min(semantic_axioms.len())..] {
        let Proposition::LessOrEqual(left, right) = axiom else {
            continue;
        };
        if right == root
            && let Some(candidate) = constant(left)
            && candidate > minimum
        {
            minimum = candidate;
            lower_bound = Some(axiom.clone());
        }
        if left == root
            && let Some(candidate) = constant(right)
            && candidate < maximum
        {
            maximum = candidate;
            upper_bound = Some(axiom.clone());
        }
    }
    if minimum > maximum {
        return None;
    }
    let mut selected_bounds = Vec::with_capacity(2);
    selected_bounds.extend(lower_bound);
    selected_bounds.extend(upper_bound);
    Some(ExactIntegerSignatureInterval {
        interval: (minimum, maximum),
        selected_bounds,
    })
}

fn integer_offset_as_i128(value: IntegerOffset) -> Option<i128> {
    match value {
        IntegerOffset::Nonnegative(value) => i128::try_from(value).ok(),
        IntegerOffset::Negative(value) => signed_negative_magnitude(value),
    }
}

fn exact_integer_affine_forward_interval(
    coefficient: IntegerOffset,
    offset: IntegerOffset,
    interval: (i128, i128),
) -> Option<(i128, i128)> {
    let apply = |value| {
        IntegerOffset::from_value(IntegerValue::Signed(value))
            .checked_multiply_offset(coefficient)?
            .checked_add(offset)
            .and_then(integer_offset_as_i128)
    };
    let (lower_input, upper_input) = match coefficient {
        IntegerOffset::Negative(_) => (interval.1, interval.0),
        IntegerOffset::Nonnegative(_) => interval,
    };
    Some((apply(lower_input)?, apply(upper_input)?))
}

pub(super) fn exact_integer_affine_quadratic_range(
    left_coefficient: IntegerOffset,
    left_offset: IntegerOffset,
    right_coefficient: IntegerOffset,
    right_offset: IntegerOffset,
    interval: (i128, i128),
) -> Option<(i128, i128)> {
    let left_coefficient = integer_offset_as_i128(left_coefficient)?;
    let left_offset = integer_offset_as_i128(left_offset)?;
    let right_coefficient = integer_offset_as_i128(right_coefficient)?;
    let right_offset = integer_offset_as_i128(right_offset)?;
    if left_coefficient == 0 || right_coefficient == 0 {
        return None;
    }
    let quadratic = left_coefficient.checked_mul(right_coefficient)?;
    let linear = left_coefficient
        .checked_mul(right_offset)?
        .checked_add(right_coefficient.checked_mul(left_offset)?)?;
    let constant = left_offset.checked_mul(right_offset)?;
    let apply = |value: i128| {
        quadratic
            .checked_mul(value.checked_mul(value)?)?
            .checked_add(linear.checked_mul(value)?)?
            .checked_add(constant)
    };
    let vertex_denominator = quadratic.checked_abs()?.checked_mul(2)?;
    let vertex_numerator = if quadratic > 0 {
        linear.checked_neg()?
    } else {
        linear
    };
    let vertex_floor = vertex_numerator.div_euclid(vertex_denominator);
    let vertex_ceiling = if vertex_numerator.rem_euclid(vertex_denominator) == 0 {
        vertex_floor
    } else {
        vertex_floor.checked_add(1)?
    };
    let mut minimum = None;
    let mut maximum = None;
    for candidate in [interval.0, interval.1, vertex_floor, vertex_ceiling] {
        if candidate < interval.0 || candidate > interval.1 {
            continue;
        }
        let value = apply(candidate)?;
        minimum = Some(minimum.map_or(value, |minimum: i128| minimum.min(value)));
        maximum = Some(maximum.map_or(value, |maximum: i128| maximum.max(value)));
    }
    Some((minimum?, maximum?))
}

pub(super) fn exact_integer_same_root_affine_product_join_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if integer_type.sign() != IntegerSign::Signed {
        return None;
    }
    let left = exact_integer_affine_fork_branch(
        integer_type,
        left,
        semantic_axioms,
        definition_axiom_count,
        machine_parameter_values,
    )?;
    let right = exact_integer_affine_fork_branch(
        integer_type,
        right,
        semantic_axioms,
        definition_axiom_count,
        machine_parameter_values,
    )?;
    if left.root != right.root
        || matches!(left.coefficient, IntegerOffset::Nonnegative(0))
        || matches!(right.coefficient, IntegerOffset::Nonnegative(0))
        || !left
            .definition_indices
            .is_disjoint(&right.definition_indices)
        || left.definition_indices.iter().next_back()? >= right.definition_indices.iter().next()?
    {
        return None;
    }
    let signature = exact_integer_signature_interval(
        integer_type,
        &left.root,
        semantic_axioms,
        definition_axiom_count,
    )?;
    if signature.selected_bounds.len() != 2 {
        return None;
    }
    let joined = exact_integer_affine_quadratic_range(
        left.coefficient,
        left.offset,
        right.coefficient,
        right.offset,
        signature.interval,
    )?;
    let carrier = fixed_integer_type_interval(integer_type)?;
    if joined.1 < carrier.0 || joined.0 > carrier.1 {
        return Some(Proposition::Falsehood);
    }
    if joined.0 < carrier.0 || joined.1 > carrier.1 {
        return None;
    }
    Some(canonical_conjunction(signature.selected_bounds))
}

fn exact_integer_affine_equation_root(
    coefficient: i128,
    offset: i128,
    target: i128,
) -> Option<Option<i128>> {
    if coefficient == 0 {
        return None;
    }
    let numerator = target.checked_sub(offset)?;
    if numerator.checked_rem(coefficient)? != 0 {
        return Some(None);
    }
    Some(Some(numerator.checked_div(coefficient)?))
}

fn exact_integer_affine_value(coefficient: i128, offset: i128, root: i128) -> Option<i128> {
    coefficient.checked_mul(root)?.checked_add(offset)
}

pub(super) fn exact_integer_same_root_affine_divide_remainder_join_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if integer_type.sign() != IntegerSign::Signed {
        return None;
    }
    let left = exact_integer_affine_fork_branch(
        integer_type,
        left,
        semantic_axioms,
        definition_axiom_count,
        machine_parameter_values,
    )?;
    let right = exact_integer_affine_fork_branch(
        integer_type,
        right,
        semantic_axioms,
        definition_axiom_count,
        machine_parameter_values,
    )?;
    if left.root != right.root
        || matches!(left.coefficient, IntegerOffset::Nonnegative(0))
        || matches!(right.coefficient, IntegerOffset::Nonnegative(0))
        || !left
            .definition_indices
            .is_disjoint(&right.definition_indices)
        || left.definition_indices.iter().next_back()? >= right.definition_indices.iter().next()?
    {
        return None;
    }
    let signature = exact_integer_signature_interval(
        integer_type,
        &left.root,
        semantic_axioms,
        definition_axiom_count,
    )?;
    if signature.selected_bounds.len() != 2 {
        return None;
    }
    let left_coefficient = integer_offset_as_i128(left.coefficient)?;
    let left_offset = integer_offset_as_i128(left.offset)?;
    let right_coefficient = integer_offset_as_i128(right.coefficient)?;
    let right_offset = integer_offset_as_i128(right.offset)?;
    let mut forbidden_roots = BTreeSet::new();
    if let Some(root) = exact_integer_affine_equation_root(right_coefficient, right_offset, 0)?
        && root >= signature.interval.0
        && root <= signature.interval.1
    {
        forbidden_roots.insert(root);
    }
    if let Some(root) = exact_integer_affine_equation_root(right_coefficient, right_offset, -1)?
        && root >= signature.interval.0
        && root <= signature.interval.1
        && exact_integer_affine_value(left_coefficient, left_offset, root)?
            == integer_value_as_i128(integer_type.minimum_value())?
    {
        forbidden_roots.insert(root);
    }
    if forbidden_roots.is_empty() {
        return Some(canonical_conjunction(signature.selected_bounds));
    }
    let interval_size = signature
        .interval
        .1
        .checked_sub(signature.interval.0)?
        .checked_add(1)?;
    if interval_size == i128::try_from(forbidden_roots.len()).ok()? {
        Some(Proposition::Falsehood)
    } else {
        None
    }
}

pub(super) fn exact_integer_distinct_root_affine_fork_join_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    operation: ExactIntegerOffsetOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let left = exact_integer_affine_fork_branch(
        integer_type,
        left,
        semantic_axioms,
        definition_axiom_count,
        machine_parameter_values,
    )?;
    let right = exact_integer_affine_fork_branch(
        integer_type,
        right,
        semantic_axioms,
        definition_axiom_count,
        machine_parameter_values,
    )?;
    if left.root == right.root
        || !left
            .definition_indices
            .is_disjoint(&right.definition_indices)
        || left.definition_indices.iter().next_back()? >= right.definition_indices.iter().next()?
    {
        return None;
    }
    let mut left_signature = exact_integer_signature_interval(
        integer_type,
        &left.root,
        semantic_axioms,
        definition_axiom_count,
    )?;
    let right_signature = exact_integer_signature_interval(
        integer_type,
        &right.root,
        semantic_axioms,
        definition_axiom_count,
    )?;
    let left_interval = exact_integer_affine_forward_interval(
        left.coefficient,
        left.offset,
        left_signature.interval,
    )?;
    let right_interval = exact_integer_affine_forward_interval(
        right.coefficient,
        right.offset,
        right_signature.interval,
    )?;
    let joined = match operation {
        ExactIntegerOffsetOperation::Add => (
            left_interval.0.checked_add(right_interval.0)?,
            left_interval.1.checked_add(right_interval.1)?,
        ),
        ExactIntegerOffsetOperation::Subtract => (
            left_interval.0.checked_sub(right_interval.1)?,
            left_interval.1.checked_sub(right_interval.0)?,
        ),
    };
    let carrier = fixed_integer_type_interval(integer_type)?;
    if joined.1 < carrier.0 || joined.0 > carrier.1 {
        return Some(Proposition::Falsehood);
    }
    if joined.0 < carrier.0 || joined.1 > carrier.1 {
        return None;
    }
    left_signature
        .selected_bounds
        .extend(right_signature.selected_bounds);
    Some(canonical_conjunction(left_signature.selected_bounds))
}

pub(super) fn exact_integer_distinct_root_affine_product_join_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if integer_type.sign() != IntegerSign::Signed {
        return None;
    }
    let left = exact_integer_affine_fork_branch(
        integer_type,
        left,
        semantic_axioms,
        definition_axiom_count,
        machine_parameter_values,
    )?;
    let right = exact_integer_affine_fork_branch(
        integer_type,
        right,
        semantic_axioms,
        definition_axiom_count,
        machine_parameter_values,
    )?;
    if left.root == right.root
        || !left
            .definition_indices
            .is_disjoint(&right.definition_indices)
        || left.definition_indices.iter().next_back()? >= right.definition_indices.iter().next()?
    {
        return None;
    }
    let mut left_signature = exact_integer_signature_interval(
        integer_type,
        &left.root,
        semantic_axioms,
        definition_axiom_count,
    )?;
    let right_signature = exact_integer_signature_interval(
        integer_type,
        &right.root,
        semantic_axioms,
        definition_axiom_count,
    )?;
    if left_signature.selected_bounds.len() != 2 || right_signature.selected_bounds.len() != 2 {
        return None;
    }
    let left_interval = exact_integer_affine_forward_interval(
        left.coefficient,
        left.offset,
        left_signature.interval,
    )?;
    let right_interval = exact_integer_affine_forward_interval(
        right.coefficient,
        right.offset,
        right_signature.interval,
    )?;
    let corners = [
        left_interval.0.checked_mul(right_interval.0)?,
        left_interval.0.checked_mul(right_interval.1)?,
        left_interval.1.checked_mul(right_interval.0)?,
        left_interval.1.checked_mul(right_interval.1)?,
    ];
    let joined = (
        *corners.iter().min().expect("four product corners exist"),
        *corners.iter().max().expect("four product corners exist"),
    );
    let carrier = fixed_integer_type_interval(integer_type)?;
    if joined.1 < carrier.0 || joined.0 > carrier.1 {
        return Some(Proposition::Falsehood);
    }
    if joined.0 < carrier.0 || joined.1 > carrier.1 {
        return None;
    }
    left_signature
        .selected_bounds
        .extend(right_signature.selected_bounds);
    Some(canonical_conjunction(left_signature.selected_bounds))
}
