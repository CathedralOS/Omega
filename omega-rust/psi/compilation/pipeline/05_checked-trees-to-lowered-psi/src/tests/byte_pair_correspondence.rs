//! Two bounded owned byte fields compared with `==` retain checking's
//! content decomposition: the length equality, then one guarded position per
//! byte below the smaller capacity. Source correspondence replays that chain
//! against the operands' declared capacities and places, so a decomposition
//! that stops a position early or swaps the measured carriers cannot stand in
//! for the authored comparison.
use super::{CheckedTrees, lower_machine};
use crate::TerminalMachineSelection;
use checked_trees::{
    CheckedBooleanExpression as Boolean, CheckedCallScalarArgument, CheckedScalarExpression,
    CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan,
};

const SOURCE: &str = r#"
    domain [u8; 3]::Utf8 requires valid_utf8(self);
    domain [u8; 4]::Utf8 requires valid_utf8(self);
    data Main { short: [u8; 3] in Utf8; long: [u8; 4] in Utf8; }
    machine Main::compare(&mut self) {
        transition { _ -> check() }
        state check(&mut self) {
            let same: bool = self.short == self.long;
            transition same {
                true -> equal()
                _ -> different()
            }
        }
        state equal(&mut self) {}
        state different(&mut self) {}
    }
"#;

/// The local initializer's retained decomposition: its position among the
/// checked scalar expressions and the comparison itself.
fn decomposition(checked: &CheckedTrees) -> (usize, Boolean) {
    let mut found = checked
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter()
        .enumerate()
        .filter_map(
            |(position, located)| match (&located.role, &located.expression) {
                (
                    CheckedScalarExpressionRole::LocalInitializer { .. },
                    CheckedScalarExpression::Boolean(value),
                ) => Some((position, value.as_ref().clone())),
                _ => None,
            },
        );
    let found_one = found.next().expect("the decomposed field comparison");
    assert!(found.next().is_none(), "one Boolean local initializer");
    found_one
}

fn lowers(checked: &CheckedTrees) -> bool {
    lower_machine(checked, TerminalMachineSelection::Name("Main::compare")).is_ok()
}

/// The correspondence refusal a substituted decomposition must meet.
fn refused_by_correspondence(checked: &CheckedTrees) -> bool {
    lower_machine(checked, TerminalMachineSelection::Name("Main::compare")).is_err_and(|error| {
        format!("{error:?}").contains("scalar operand value differs from its authored expression")
    })
}

/// Substitute `value` for the decomposition in both places checking retains
/// it, the local's source row and its scalar-local plan, so the two stay
/// consistent and only source correspondence can tell the substitution apart.
fn replaced(checked: &CheckedTrees, value: Boolean) -> CheckedTrees {
    let (position, original) = decomposition(checked);
    let mut changed = checked.clone();
    let substituted = CheckedScalarExpression::Boolean(Box::new(value));
    changed.facts.values.scalar_expressions.expressions[position].expression = substituted.clone();
    let mut plans = 0;
    let retained_value = |value: &CheckedScalarExpression| matches!(value, CheckedScalarExpression::Boolean(retained) if retained.as_ref() == &original);
    for state in changed
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter_mut()
        .flat_map(|machine| &mut machine.states)
    {
        for initializer in &mut state.binding_initializers {
            if retained_value(initializer) {
                *initializer = substituted.clone();
                plans += 1;
            }
        }
        for operation in &mut state.operations {
            if let CheckedUnitEffectOperationPlan::EstablishScalarLocal {
                value: CheckedCallScalarArgument::Pure(value),
                ..
            } = operation
                && retained_value(value)
            {
                *value = substituted.clone();
                plans += 1;
            }
        }
    }
    assert!(plans > 0, "the state plan retains the decomposition");
    changed
}

/// The chain without its final position: the one before it keeps only its
/// own agreeing comparison.
fn without_last_position(position: &Boolean) -> Boolean {
    let Boolean::Or { left, right } = position else {
        panic!("a position: {position:?}");
    };
    let Boolean::And {
        left: agreeing,
        right: next,
    } = right.as_ref()
    else {
        panic!("the chain has more than one position: {right:?}");
    };
    let next_is_last = matches!(
        next.as_ref(),
        Boolean::Or { right, .. }
            if !matches!(right.as_ref(), Boolean::And { right: tail, .. }
                if matches!(tail.as_ref(), Boolean::Or { .. }))
    );
    Boolean::Or {
        left: left.clone(),
        right: if next_is_last {
            agreeing.clone()
        } else {
            Box::new(Boolean::And {
                left: agreeing.clone(),
                right: Box::new(without_last_position(next)),
            })
        },
    }
}

#[test]
fn byte_field_pair_correspondence_replays_every_position() {
    let checked = crate::front_end::checked_program(SOURCE);
    assert!(lowers(&checked), "checking's own decomposition lowers");
    let (_, decomposed) = decomposition(&checked);
    let Boolean::And {
        left: lengths,
        right: chain,
    } = &decomposed
    else {
        panic!("lengths then positions: {decomposed:?}");
    };

    let truncated = Boolean::And {
        left: lengths.clone(),
        right: Box::new(without_last_position(chain)),
    };
    assert!(
        refused_by_correspondence(&replaced(&checked, truncated)),
        "a chain that stops before the smaller capacity cannot compare equal content"
    );

    let Boolean::IntegerComparison { kind, left, right } = lengths.as_ref() else {
        panic!("the length equality: {lengths:?}");
    };
    let swapped = Boolean::And {
        left: Box::new(Boolean::IntegerComparison {
            kind: *kind,
            left: right.clone(),
            right: left.clone(),
        }),
        right: chain.clone(),
    };
    assert!(
        refused_by_correspondence(&replaced(&checked, swapped)),
        "each measured carrier keeps its authored side"
    );
}
