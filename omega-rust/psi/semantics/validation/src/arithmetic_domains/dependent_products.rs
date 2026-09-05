use super::dependent_relations::{
    dependent_maximum_of_type_reference, validation_state_preserves_field,
};
use super::*;

/// R3's ONE closed bounded-product rule: `a * self.Fb + c` where
/// `a <= self.Fa - 1` (a STRICT dependent atom), `c <= self.Fb - 1` (strict,
/// on the SAME field the product multiplies by), and a machine `requires`
/// couples `self.Fa * self.Fb <= K` -- then
/// `a*Fb + c <= (Fa-1)*Fb + (Fb-1) = Fa*Fb - 1 <= K - 1`, so the interval is
/// `[0, K-1]` (unsigned operands floor at 0; a signed floor keeps the naive
/// low). Needed exactly where operand ranges are NOT independently tight
/// (runtime dims bounded only by their product). Both fields must be
/// machine-wide preserved (the coupling speaks of machine entry; the atoms
/// of state entry).
pub(super) fn refine_dependent_product(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    left: ExpressionHandle,
    right: ExpressionHandle,
    naive: Interval,
) -> Interval {
    let Some(state) = state else {
        return naive;
    };
    // Left: `a * self.Fb` (either operand order).
    let ExpressionNode::Binary(product) = program.expression_table.expression(left) else {
        return naive;
    };
    if product.operator != BinaryOperator::Multiply {
        return naive;
    }
    let (a_expr, fb_expr) = {
        let left_is_field = typed_trees::dependent_ranges::symbolic_max_bound(
            &program.expression_table,
            product.left,
        )
        .is_some_and(|bound| bound.offset == 0);
        if left_is_field {
            (product.right, product.left)
        } else {
            (product.left, product.right)
        }
    };
    let Some(fb) =
        typed_trees::dependent_ranges::symbolic_max_bound(&program.expression_table, fb_expr)
            .filter(|bound| bound.offset == 0)
            .map(|bound| bound.field)
    else {
        return naive;
    };
    // a's STRICT dependent atom names Fa; c's STRICT atom names Fb (the
    // multiplier field).
    let Some(fa) = strict_dependent_atom_field(program, machine, state, a_expr) else {
        return naive;
    };
    let Some(c_field) = strict_dependent_atom_field(program, machine, state, right) else {
        return naive;
    };
    if c_field.as_str() != fb.as_str() {
        return naive;
    }
    // The coupling: `requires self.Fa * self.Fb <= K` (either multiply order).
    let Some(k) = requires_product_coupling(program, machine, &fa, &fb) else {
        return naive;
    };
    // Preservation of both fields, machine-wide.
    for field in [&fa, &fb] {
        let preserved = program
            .machine_states(machine)
            .iter()
            .all(|state| validation_state_preserves_field(program, machine, state, field));
        if !preserved {
            return naive;
        }
    }
    let Some(high) = k.checked_sub(1) else {
        return naive;
    };
    Interval {
        low: Some(naive.low.map_or(0, |low| low.max(0))),
        high: Some(naive.high.map_or(high, |naive_high| naive_high.min(high))),
    }
}

/// The field a STRICTLY-bounded dependent place names: the expression is a
/// place whose declared range maximum is `self.<field> - 1` (the exclusive
/// sugar's normalization -- `a < field` at entry).
fn strict_dependent_atom_field(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<typed_trees::name::Identifier> {
    let raw = crate::places::declared_place_type_raw(program, machine, Some(state), expression)?;
    let (field, offset) = dependent_maximum_of_type_reference(program, raw)?;
    (offset == -1).then_some(field)
}

/// A machine `requires` conjunct `self.Fa * self.Fb <= K` (either multiply
/// order; strict `<` tightens to `K - 1`).
fn requires_product_coupling(
    program: &TypedTrees,
    machine: &Machine,
    fa: &typed_trees::name::Identifier,
    fb: &typed_trees::name::Identifier,
) -> Option<i64> {
    let fa_label = format!("self.{}", fa.as_str());
    let fb_label = format!("self.{}", fb.as_str());
    for contract in program.machine_contracts(machine) {
        if contract.kind != typed_trees::signature::SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let typed_trees::domain::ProofFact::Expression(expression) = fact else {
                continue;
            };
            if let Some(k) =
                product_coupling_conjunct(program, machine, *expression, &fa_label, &fb_label)
            {
                return Some(k);
            }
        }
    }
    None
}

fn product_coupling_conjunct(
    program: &TypedTrees,
    machine: &Machine,
    guard: ExpressionHandle,
    fa_label: &str,
    fb_label: &str,
) -> Option<i64> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return None;
    };
    match binary.operator {
        BinaryOperator::And => {
            product_coupling_conjunct(program, machine, binary.left, fa_label, fb_label).or_else(
                || product_coupling_conjunct(program, machine, binary.right, fa_label, fb_label),
            )
        }
        BinaryOperator::LessOrEqual | BinaryOperator::Less => {
            let ExpressionNode::Binary(product) = program.expression_table.expression(binary.left)
            else {
                return None;
            };
            if product.operator != BinaryOperator::Multiply {
                return None;
            }
            let lhs = product_coupling_operand(program, machine, product.left)?;
            let rhs = product_coupling_operand(program, machine, product.right)?;
            let matches =
                (lhs == fa_label && rhs == fb_label) || (lhs == fb_label && rhs == fa_label);
            if !matches {
                return None;
            }
            let k = literal_i64(program, binary.right)?;
            if binary.operator == BinaryOperator::Less {
                k.checked_sub(1)
            } else {
                Some(k)
            }
        }
        _ => None,
    }
}

/// Return the source place spelling admitted by the bounded-product rule.
/// A direct place is unchanged. An Exact integer widening is transparent
/// because it preserves every source-carrier value while making the
/// specification product total. Narrowing, signedness-changing, semantic,
/// address, recast, and policy-bearing conversions remain visible and reject.
fn product_coupling_operand(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
) -> Option<String> {
    let ExpressionNode::Cast(cast) = program.expression_table.expression(expression) else {
        return Some(program.expression_table.display_name(expression));
    };
    if cast.form.is_recast() || !cast.semantic_domain.is_empty() {
        return None;
    }
    let source_type = declared_place_type_raw(
        program,
        machine,
        program.machine_states(machine).first(),
        cast.value,
    )?;
    let source = program.primitive_type_reference(source_type)?;
    let target = program.primitive_type_reference(cast.target_type)?;
    if source == PrimitiveType::Addr || target == PrimitiveType::Addr {
        return None;
    }
    let source_range = primitive_range(source)?;
    let target_range = primitive_range(target)?;
    if cast.domain != ArithmeticDomain::Exact
        || source == target
        || !target_range.contains(source_range)
    {
        return None;
    }
    Some(program.expression_table.display_name(cast.value))
}

/// The MULTIPLY half of R3's rule: `a * self.Fb` (either order) with
/// `a <= self.Fa - 1` strict and the coupling `Fa * Fb <= K` is bounded by
/// `(Fa-1)*Fb = Fa*Fb - Fb <= K` (Fb unsigned) -- interval `[0, K]`. The
/// enclosing Add then tightens to `K - 1`.
pub(super) fn refine_dependent_product_factor(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    left: ExpressionHandle,
    right: ExpressionHandle,
    naive: Interval,
) -> Interval {
    let Some(state) = state else {
        return naive;
    };
    let (a_expr, fb_expr) = {
        let left_is_field =
            typed_trees::dependent_ranges::symbolic_max_bound(&program.expression_table, left)
                .is_some_and(|bound| bound.offset == 0);
        if left_is_field {
            (right, left)
        } else {
            (left, right)
        }
    };
    let Some(fb) =
        typed_trees::dependent_ranges::symbolic_max_bound(&program.expression_table, fb_expr)
            .filter(|bound| bound.offset == 0)
            .map(|bound| bound.field)
    else {
        return naive;
    };
    let Some(fa) = strict_dependent_atom_field(program, machine, state, a_expr) else {
        return naive;
    };
    let Some(k) = requires_product_coupling(program, machine, &fa, &fb) else {
        return naive;
    };
    for field in [&fa, &fb] {
        let preserved = program
            .machine_states(machine)
            .iter()
            .all(|state| validation_state_preserves_field(program, machine, state, field));
        if !preserved {
            return naive;
        }
    }
    Interval {
        low: Some(naive.low.map_or(0, |low| low.max(0))),
        high: Some(naive.high.map_or(k, |naive_high| naive_high.min(k))),
    }
}
