//! Copy proof operands without using one Rust stack frame per expression.
use crate::{IntegerMathTerm, ScalarReadProjection, ScalarTerm};

// Visit the last operand first. When rebuilding the parent, popping the
// completed copies then supplies operands in their original declaration order.
fn clone_tree<T>(
    root: &T,
    append_operands: impl for<'a> Fn(&'a T, &mut Vec<&'a T>),
    rebuild: impl Fn(&T, &mut Vec<T>) -> T,
) -> T {
    let mut pending = Vec::new();
    let mut children = Vec::new();
    let mut copies = Vec::new();
    let mut next = (root, false);
    loop {
        let (node, visited_children) = next;
        if !visited_children {
            append_operands(node, &mut children);
            if let Some(last) = children.pop() {
                pending.push((node, true));
                pending.extend(children.drain(..).map(|child| (child, false)));
                next = (last, false);
                continue;
            }
        }
        let copy = rebuild(node, &mut copies);
        let Some(item) = pending.pop() else {
            return copy;
        };
        copies.push(copy);
        next = item;
    }
}

fn take_operand<T>(copies: &mut Vec<T>) -> Box<T> {
    Box::new(
        copies
            .pop()
            .expect("each operand was visited before its parent"),
    )
}

impl Clone for IntegerMathTerm {
    fn clone(&self) -> Self {
        clone_tree(self, Self::append_operands, |node, copies| {
            use IntegerMathTerm::*;
            match node {
                IntegerLiteral(n) => IntegerLiteral(*n),
                MathValue { source_type, value } => MathValue {
                    source_type: *source_type,
                    value: *value,
                },
                Embed(scalar) => Embed(scalar.clone()),
                Add(..) => Add(take_operand(copies), take_operand(copies)),
                Subtract(..) => Subtract(take_operand(copies), take_operand(copies)),
                Multiply(..) => Multiply(take_operand(copies), take_operand(copies)),
                ShiftLeft { .. } => ShiftLeft {
                    value: take_operand(copies),
                    count: take_operand(copies),
                },
            }
        })
    }
}

impl Clone for ScalarTerm {
    fn clone(&self) -> Self {
        // Proof facts mostly copy values and literals. Copy those leaves
        // directly instead of entering the operand walk and its two dispatches.
        match self {
            Self::Value { id, scalar_type } => Self::Value {
                id: *id,
                scalar_type: *scalar_type,
            },
            Self::Integer { scalar_type, value } => Self::Integer {
                scalar_type: *scalar_type,
                value: *value,
            },
            Self::Boolean(value) => Self::Boolean(*value),
            Self::Application {
                application,
                scalar_type,
            } => Self::Application {
                application: *application,
                scalar_type: *scalar_type,
            },
            _ => clone_tree(self, Self::append_operands, Self::copy_with_operands),
        }
    }
}

impl ScalarTerm {
    fn copy_with_operands(&self, copies: &mut Vec<Self>) -> Self {
        // Each row names the fields copied as-is and then the boxed operands
        // supplied by the walk. An indexed read interleaves operands and selectors.
        macro_rules! variants {
            ($( $variant:ident { $( $field:ident ),* ; $( $child:ident ),* } ),* $(,)?) => {
                match self {
                    $(Self::$variant { $( $field, )* $( $child: _, )* } => Self::$variant {
                        $( $field: $field.clone(), )*
                        $( $child: take_operand(copies), )*
                    },)*
                    Self::DeclarationApplication { declaration, arguments, scalar_type } =>
                        Self::DeclarationApplication {
                            declaration: *declaration,
                            arguments: arguments.iter().map(|_| {
                                copies.pop().expect("each argument was visited before its application")
                            }).collect(),
                            scalar_type: *scalar_type,
                        },
                    Self::Boolean(value) => Self::Boolean(*value),
                    Self::IeeeFloatLiteral(value) => Self::IeeeFloatLiteral(*value),
                    Self::StructuralRead { root, path, scalar_type } => Self::StructuralRead {
                        root: *root,
                        path: path.iter().map(|segment| match segment {
                            ScalarReadProjection::Index(_) => ScalarReadProjection::Index(take_operand(copies)),
                            ScalarReadProjection::Extent => ScalarReadProjection::Extent,
                            ScalarReadProjection::Alignment => ScalarReadProjection::Alignment,
                            ScalarReadProjection::Referent => ScalarReadProjection::Referent,
                            ScalarReadProjection::Static(selector) => ScalarReadProjection::Static(*selector),
                        }).collect(),
                        scalar_type: *scalar_type,
                    },
                }
            }
        }
        variants! {
            Application { application, scalar_type; },
            Value { id, scalar_type; },
            BooleanField { version, root, path; },
            IntegerField { version, root, path, scalar_type; },
            IeeeFloatField { version, root, path, format; },
            ViewExtent { root, path, scalar_type; },
            Integer { scalar_type, value; },
            ByteSequencePredicate { predicate, root, path; },
            BooleanNot { ; operand },
            BooleanEqual { ; left, right },
            IntegerEqual { scalar_type; left, right },
            IntegerLessThan { scalar_type; left, right },
            IntegerLessOrEqual { scalar_type; left, right },
            IntegerBitwiseAnd { scalar_type; left, right },
            IntegerBitwiseNot { scalar_type; operand },
            IntegerWiden { source_type, target_type; operand },
            IntegerExactCast { source_type, target_type; operand },
            IntegerBitwiseOr { scalar_type; left, right },
            IntegerBitwiseXor { scalar_type; left, right },
            WrappingIntegerShiftLeft { value_type, count_type; value, count },
            WrappingIntegerShiftRight { value_type, count_type; value, count },
            ExactIntegerShiftLeft { value_type, count_type; value, count },
            ExactIntegerShiftRight { value_type, count_type; value, count },
            ExactIntegerAdd { scalar_type; left, right },
            ExactIntegerSubtract { scalar_type; left, right },
            ExactIntegerMultiply { scalar_type; left, right },
            ExactIntegerDivide { scalar_type; left, right },
            ExactIntegerRemainder { scalar_type; left, right },
            WrappingIntegerDivide { scalar_type; left, right },
            WrappingIntegerRemainder { scalar_type; left, right },
            SaturatingIntegerDivide { scalar_type; left, right },
            SaturatingIntegerRemainder { scalar_type; left, right },
            WrappingIntegerAdd { scalar_type; left, right },
            SaturatingIntegerAdd { scalar_type; left, right },
            WrappingIntegerSubtract { scalar_type; left, right },
            SaturatingIntegerSubtract { scalar_type; left, right },
            WrappingIntegerMultiply { scalar_type; left, right },
            SaturatingIntegerMultiply { scalar_type; left, right },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        IntegerSign, IntegerType, IntegerValue, ScalarApplicationId, ScalarTerm, ScalarType,
        ValueId,
    };

    fn integer() -> IntegerType {
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
    }

    fn value(id: u64) -> ScalarTerm {
        ScalarTerm::value(ValueId::new(id).unwrap(), ScalarType::Integer(integer()))
    }

    #[test]
    fn leaf_terms_and_their_parents_clone_to_equal_terms() {
        let leaves = [
            value(3),
            ScalarTerm::integer(integer(), IntegerValue::Unsigned(9)).unwrap(),
            ScalarTerm::Boolean(true),
            ScalarTerm::Application {
                application: ScalarApplicationId::new(4).unwrap(),
                scalar_type: ScalarType::Boolean,
            },
        ];
        for leaf in &leaves {
            assert_eq!(&leaf.clone(), leaf);
        }
        let difference = ScalarTerm::ExactIntegerSubtract {
            scalar_type: integer(),
            left: Box::new(value(1)),
            right: Box::new(leaves[1].clone()),
        };
        let ScalarTerm::ExactIntegerSubtract { left, right, .. } = difference.clone() else {
            panic!("expected subtraction")
        };
        assert_eq!((*left, *right), (value(1), leaves[1].clone()));
    }

    #[test]
    fn term_walks_visit_operands_in_declaration_order() {
        let sum = ScalarTerm::ExactIntegerAdd {
            scalar_type: integer(),
            left: Box::new(ScalarTerm::ExactIntegerSubtract {
                scalar_type: integer(),
                left: Box::new(value(1)),
                right: Box::new(value(2)),
            }),
            right: Box::new(value(3)),
        };
        let mut seen = Vec::new();
        sum.visit_terms(|term| {
            if let ScalarTerm::Value { id, .. } = term {
                seen.push(id.get());
            }
        });
        assert_eq!(seen, [1, 2, 3]);
        assert!(sum.validate().is_ok());
    }
}
