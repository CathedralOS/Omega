//! Caller-side consumption of a callee's `ensures` result bounds for an index
//! position. A checked `ensures` is discharged at every callee exit
//! (`checks::contracts` proves each `ensures` fact on every normal return), so
//! its literal `result` comparisons bound THIS call occurrence's return value
//! the same way `seed_boundary_call_ensures_facts` bounds written
//! out-arguments. Only the authored contract is read — the callee's body is
//! never replayed here, so an unbounded result keeps the ordinary rejection.
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

/// The interval a call index's target `ensures` proves for its `result`, as
/// inclusive `(low, high)` endpoints — each side `None` when no literal
/// conjunct bounds it. `None` overall for non-calls, dispatched requirement
/// calls (a private realization is not the public requirement's proof
/// interface), unresolved targets, a `result`-shadowed signature, or a
/// contract with no literal `result` comparison.
pub(in crate::checks::ranges) fn ensured_call_result_bounds(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<(Option<i64>, Option<i64>)> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    // Internal operator shapes are not ordinary machine calls; their target
    // must not borrow a machine contract. A dispatched requirement's private
    // realization is likewise not the public proof interface.
    if call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return None;
    }
    // Resolve the contract target the way `call_target_parameters` does: a
    // machine head names the machine and binds contracts on its entry state; a
    // state call names the state inside its machine.
    let (machine, target) = if let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == call.target_symbol)
    {
        (machine, program.machine_states(machine).first()?)
    } else {
        let target = crate::semantic_calls::find_state(program, call.target_symbol)?;
        let machine = program.machines().iter().find(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == call.target_symbol)
        })?;
        (machine, target)
    };
    // An authored parameter named `result` on the called signature shadows the
    // reserved binder; a `result` operand then denotes that parameter, not the
    // return value (the `is_result_reference` precedence rule in
    // contracts/return_values, applied to the same parameter source the
    // contract-label instantiation uses).
    let result_is_parameter = crate::semantic_calls::call_target_parameters(
        program,
        call.target_symbol,
    )
    .is_some_and(|parameters| {
        parameters.iter().any(|parameter| {
            parameter.name.as_str() == crate::checks::contracts::labels::calls::RESULT_BINDER
        })
    });
    if result_is_parameter {
        return None;
    }
    let mut bounds = EnsuredResultBounds::default();
    for contract in program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(target))
    {
        if contract.kind != typed_trees::signature::SignatureContractKind::Ensures {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let typed_trees::domain::ProofFact::Expression(conjunct) = fact {
                bounds.fold_conjunct(program, *conjunct);
            }
        }
    }
    bounds.finish()
}

#[derive(Default)]
struct EnsuredResultBounds {
    low: Option<i64>,
    high: Option<i64>,
}

impl EnsuredResultBounds {
    /// Meets one ensures conjunct into the running interval. `&&` splits into
    /// independent conjuncts; only the ensures house spelling `result <OP>
    /// <literal>` contributes (mirroring `seed_ensures_bound_conjunct`), and a
    /// conjunct that cannot name a literal bound is skipped rather than
    /// poisoning the interval its sibling conjuncts still prove.
    fn fold_conjunct(&mut self, program: &typed_trees::TypedTrees, conjunct: ExpressionHandle) {
        if let ExpressionNode::Atomic(atomic) = program.expression_table.expression(conjunct) {
            self.fold_conjunct(program, atomic.value);
            return;
        }
        let ExpressionNode::Binary(comparison) = program.expression_table.expression(conjunct)
        else {
            return;
        };
        if comparison.operator == BinaryOperator::And {
            self.fold_conjunct(program, comparison.left);
            self.fold_conjunct(program, comparison.right);
            return;
        }
        if !is_result_binder(program, comparison.left) {
            return;
        }
        let ExpressionNode::Integer(literal) =
            program.expression_table.expression(comparison.right)
        else {
            return;
        };
        let Some(value) = literal.value_i64() else {
            return;
        };
        match comparison.operator {
            BinaryOperator::Less => {
                if let Some(inclusive) = value.checked_sub(1) {
                    self.meet_high(inclusive);
                }
            }
            BinaryOperator::LessOrEqual => self.meet_high(value),
            BinaryOperator::Equal => {
                self.meet_low(value);
                self.meet_high(value);
            }
            BinaryOperator::GreaterOrEqual => self.meet_low(value),
            BinaryOperator::Greater => {
                if let Some(inclusive) = value.checked_add(1) {
                    self.meet_low(inclusive);
                }
            }
            _ => {}
        }
    }

    fn meet_low(&mut self, bound: i64) {
        self.low = Some(self.low.map_or(bound, |low| low.max(bound)));
    }

    fn meet_high(&mut self, bound: i64) {
        self.high = Some(self.high.map_or(bound, |high| high.min(bound)));
    }

    fn finish(self) -> Option<(Option<i64>, Option<i64>)> {
        (self.low.is_some() || self.high.is_some()).then_some((self.low, self.high))
    }
}

/// Whether `expression` is exactly the reserved `result` binder — a
/// single-member `result` name, mirroring the contract-label convention.
fn is_result_binder(program: &typed_trees::TypedTrees, expression: ExpressionHandle) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    matches!(
        program.expression_table.name_path_members(path.members),
        [name] if name.as_str() == crate::checks::contracts::labels::calls::RESULT_BINDER
    )
}
