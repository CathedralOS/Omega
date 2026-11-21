use semantic_vocabulary::{
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, PlaceId, Proposition,
    ScalarReadProjection, ScalarTerm, ScalarType,
};

const DEPTH: u64 = 32_768;

fn integer() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}

fn literal(n: u64) -> ScalarTerm {
    ScalarTerm::integer(integer(), IntegerValue::Unsigned(n.into())).unwrap()
}

fn on_small_stack(test: fn()) {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn cloning_a_deep_arithmetic_premise_preserves_every_operand() {
    on_small_stack(|| {
        let mut sum = literal(0);
        for n in 1..=DEPTH {
            sum = ScalarTerm::ExactIntegerAdd {
                scalar_type: integer(),
                left: Box::new(sum),
                right: Box::new(literal(n)),
            };
        }
        let original = Proposition::Equal(sum, literal(7));
        let copy = original.clone();
        // Consume one level at a time so this tests copying independently of
        // the Rust stack used to compare, format or drop a tree.
        for premise in [original, copy] {
            let Proposition::Equal(mut sum, other) = premise else {
                panic!("expected equality")
            };
            assert_eq!(other, literal(7));
            for n in (1..=DEPTH).rev() {
                let ScalarTerm::ExactIntegerAdd {
                    scalar_type,
                    left,
                    right,
                } = sum
                else {
                    panic!("expected addition")
                };
                assert_eq!(scalar_type, integer());
                assert_eq!(*right, literal(n));
                sum = *left;
            }
            assert_eq!(sum, literal(0));
        }
    });
}

#[test]
fn cloning_mathematical_terms_and_indexed_reads_preserves_nested_operands() {
    on_small_stack(|| {
        let root = PlaceId::new(1).unwrap();
        let mut index = literal(5);
        for _ in 0..DEPTH {
            index = ScalarTerm::StructuralRead {
                root,
                path: vec![
                    ScalarReadProjection::Index(Box::new(literal(2))),
                    ScalarReadProjection::Referent,
                    ScalarReadProjection::Index(Box::new(index)),
                ],
                scalar_type: ScalarType::Integer(integer()),
            };
        }
        let mut math = IntegerMathTerm::Embed(Box::new(index));
        let one = || IntegerMathTerm::literal(IntegerValue::Unsigned(1));
        for _ in 0..DEPTH {
            math = IntegerMathTerm::Subtract(Box::new(one()), Box::new(math));
        }
        let copy = math.clone();
        for mut term in [math, copy] {
            for _ in 0..DEPTH {
                let IntegerMathTerm::Subtract(left, right) = term else {
                    panic!("expected subtraction")
                };
                assert_eq!(*left, one());
                term = *right;
            }
            let IntegerMathTerm::Embed(mut read) = term else {
                panic!("expected an embedded scalar")
            };
            for _ in 0..DEPTH {
                let ScalarTerm::StructuralRead {
                    root: place,
                    mut path,
                    scalar_type,
                } = *read
                else {
                    panic!("expected an indexed read")
                };
                assert_eq!(place, root);
                assert_eq!(scalar_type, ScalarType::Integer(integer()));
                let Some(ScalarReadProjection::Index(child)) = path.pop() else {
                    panic!("expected the nested index")
                };
                assert_eq!(
                    path,
                    [
                        ScalarReadProjection::Index(Box::new(literal(2))),
                        ScalarReadProjection::Referent
                    ]
                );
                read = child;
            }
            assert_eq!(*read, literal(5));
        }
    });
}
