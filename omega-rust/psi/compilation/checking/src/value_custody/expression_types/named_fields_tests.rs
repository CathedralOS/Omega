//! Declaration predicates use their exact field binder as the literal's
//! destination, including payload binders with the same spelling.

use super::named_value_type_reference;
use numerics::literals::FloatFormat;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::PrimitiveType;

fn comparison_operands(
    program: &TypedTrees,
    fact: &ProofFact,
) -> (ExpressionHandle, ExpressionHandle) {
    let ProofFact::Expression(expression) = fact else {
        panic!("expression predicate");
    };
    let ExpressionNode::Binary(binary) = program.expression_table.expression(*expression) else {
        panic!("comparison predicate");
    };
    (binary.left, binary.right)
}

#[test]
fn data_where_float_endpoints_land_from_exact_field_symbols() {
    let mut program = crate::front_end::typed_program(
        "data Narrow where value >= 1.0e-50 { value: f32; }
         data Wide where value >= 1.0e-50 { value: f64; }",
    );
    let comparisons: Vec<_> = program
        .data_definitions()
        .iter()
        .map(|definition| {
            comparison_operands(
                &program,
                &program.proof_facts.span_or_empty(definition.where_facts)[0],
            )
        })
        .collect();
    crate::land_float_literal_destinations(&mut program);
    for ((field, bound), (primitive, format)) in comparisons.into_iter().zip([
        (PrimitiveType::F32, FloatFormat::F32),
        (PrimitiveType::F64, FloatFormat::F64),
    ]) {
        let ExpressionNode::Name(path) = program.expression_table.expression(field) else {
            panic!("field binder");
        };
        assert_eq!(
            named_value_type_reference(&program, path)
                .and_then(|reference| program.primitive_type_reference(reference)),
            Some(primitive)
        );
        let ExpressionNode::Float(literal) = program.expression_table.expression(bound) else {
            panic!("landed float endpoint");
        };
        assert_eq!(literal.landing(), Some(format));
        // Same decimal, different actual values: proving the f32 predicate
        // from the unrounded rational would classify its zero incorrectly.
        assert_eq!(literal.landed_f64() == 0.0, format == FloatFormat::F32);
        let mut unresolved = path.clone();
        unresolved.symbol = symbols::SymbolHandle::invalid();
        unresolved.head_symbol = symbols::SymbolHandle::invalid();
        assert_eq!(named_value_type_reference(&program, &unresolved), None);
    }
}

#[test]
fn case_where_endpoints_follow_the_selected_payload_binder() {
    let mut program = crate::front_end::typed_program(
        "data Choice {
            case Narrow(value: f32) where value <= 0.1;
            case Wide(value: f64) where value <= 0.1;
        }",
    );
    let definition = &program.data_definitions()[0];
    let comparisons: Vec<_> = program
        .data_members(definition)
        .iter()
        .map(|member| {
            let typed_trees::data::DataMember::Variant(variant) = member else {
                panic!("case");
            };
            comparison_operands(
                &program,
                &program.proof_facts.span_or_empty(variant.where_facts)[0],
            )
        })
        .collect();
    crate::land_float_literal_destinations(&mut program);
    for ((field, bound), format) in comparisons
        .into_iter()
        .zip([FloatFormat::F32, FloatFormat::F64])
    {
        let ExpressionNode::Name(path) = program.expression_table.expression(field) else {
            panic!("payload binder");
        };
        assert!(named_value_type_reference(&program, path).is_some());
        let ExpressionNode::Float(literal) = program.expression_table.expression(bound) else {
            panic!("landed endpoint");
        };
        assert_eq!(literal.landing(), Some(format));
    }
}
