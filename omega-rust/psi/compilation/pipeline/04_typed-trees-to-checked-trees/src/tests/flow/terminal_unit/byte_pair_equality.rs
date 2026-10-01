//! Two bounded owned byte fields compared with `==` are content equality:
//! equal live lengths, then one position per byte below the smaller declared
//! capacity, each later position nested in the previous one's agreeing
//! branch. A position either ends at the left length or bounds both reads by
//! their own lengths and compares the bytes. Borrowed views have no static
//! extent, so a pair involving one keeps no content form.
use super::checked;
use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedScalarExpression,
    CheckedScalarExpressionRole,
};

/// The guard at `statement_ordinal` of `machine`'s first state.
fn guard(
    checked: &checked_trees::CheckedTrees,
    machine: &str,
    statement_ordinal: u32,
) -> Option<CheckedBooleanExpression> {
    let machine = checked
        .machines()
        .iter()
        .find(|candidate| candidate.name.as_str().ends_with(machine))
        .expect("the authored machine");
    let state = &checked.machine_states(machine)[0];
    match checked.facts.values.scalar_expressions.expression_at(
        state.symbol,
        statement_ordinal,
        CheckedScalarExpressionRole::Guard,
    )? {
        CheckedScalarExpression::Boolean(guard) => Some(guard.as_ref().clone()),
        other => panic!("a Boolean guard: {other:?}"),
    }
}

fn literal(expression: &CheckedScalarExpression) -> u64 {
    let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
        panic!("a literal operand: {expression:?}");
    };
    literal.value_u64().expect("a non-negative literal")
}

fn is_length(expression: &CheckedScalarExpression) -> bool {
    matches!(
        expression,
        CheckedScalarExpression::StructuralParameterByteLength { .. }
    )
}

fn read_index(expression: &CheckedScalarExpression) -> u64 {
    let CheckedScalarExpression::StructuralParameterIndexedRead { index, .. } = expression else {
        panic!("a byte read: {expression:?}");
    };
    literal(index)
}

/// The compared positions of one decomposed pair comparison, in order,
/// checking every position's shape and nesting on the way.
fn positions(guard: &CheckedBooleanExpression) -> Vec<u64> {
    let CheckedBooleanExpression::And { left, right } = guard else {
        panic!("lengths then positions: {guard:?}");
    };
    assert!(
        matches!(
            left.as_ref(),
            CheckedBooleanExpression::IntegerComparison {
                kind: CheckedIntegerComparisonKind::Equal,
                left,
                right,
            } if is_length(left) && is_length(right)
        ),
        "the leftmost comparison measures both carriers: {left:?}"
    );
    let mut compared = Vec::new();
    let mut remaining = right.as_ref();
    loop {
        let CheckedBooleanExpression::Or {
            left: ended,
            right: continued,
        } = remaining
        else {
            panic!("each position may end at the left length: {remaining:?}");
        };
        let CheckedBooleanExpression::IntegerComparison {
            kind: CheckedIntegerComparisonKind::LessOrEqual,
            left: length,
            right: index,
        } = ended.as_ref()
        else {
            panic!("the position ends at the left length: {ended:?}");
        };
        assert!(is_length(length));
        let index = literal(index);
        let (agreeing, next) = match continued.as_ref() {
            CheckedBooleanExpression::And { left, right }
                if matches!(right.as_ref(), CheckedBooleanExpression::Or { .. }) =>
            {
                (left.as_ref(), Some(right.as_ref()))
            }
            last => (last, None),
        };
        let CheckedBooleanExpression::And {
            left: bound,
            right: bytes,
        } = agreeing
        else {
            panic!("a position bounds the right read, then compares: {agreeing:?}");
        };
        assert!(matches!(
            bound.as_ref(),
            CheckedBooleanExpression::IntegerComparison {
                kind: CheckedIntegerComparisonKind::LessThan,
                left,
                right,
            } if literal(left) == index && is_length(right)
        ));
        let CheckedBooleanExpression::IntegerComparison {
            kind: CheckedIntegerComparisonKind::Equal,
            left,
            right,
        } = bytes.as_ref()
        else {
            panic!("the position compares one byte pair: {bytes:?}");
        };
        assert_eq!((read_index(left), read_index(right)), (index, index));
        compared.push(index);
        match next {
            Some(next) => remaining = next,
            None => return compared,
        }
    }
}

#[test]
fn bounded_text_fields_compare_every_position_below_the_smaller_capacity() {
    let checked = checked(
        r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        domain [u8; 6]::Utf8 requires valid_utf8(self);
        data Main { short: [u8; 4] in Utf8; long: [u8; 6] in Utf8; }
        machine Main::classify(&self) {
            transition self.short == self.long {
                true -> same()
                _ -> different()
            }
            state same(&self) {}
            state different(&self) {}
        }
        machine Main::negated(&self) {
            transition self.long != self.short {
                true -> same()
                _ -> different()
            }
            state same(&self) {}
            state different(&self) {}
        }
        "#,
    );
    let classify = guard(&checked, "::classify", 0).expect("the field pair guard is retained");
    assert_eq!(positions(&classify), [0, 1, 2, 3]);
    let negated = guard(&checked, "::negated", 0).expect("the negated guard is retained");
    let CheckedBooleanExpression::Not(negated) = negated else {
        panic!("`!=` negates the content comparison: {negated:?}");
    };
    assert_eq!(positions(&negated), [0, 1, 2, 3]);
}

#[test]
fn a_borrowed_view_pair_keeps_no_content_comparison() {
    let checked = checked(
        r#"
        data Main { }
        machine Main::classify(&self, left: &[u8], right: &[u8]) {
            transition left == right {
                true -> same()
                _ -> different()
            }
            state same(&self) {}
            state different(&self) {}
        }
        "#,
    );
    assert_eq!(guard(&checked, "::classify", 0), None);
}
