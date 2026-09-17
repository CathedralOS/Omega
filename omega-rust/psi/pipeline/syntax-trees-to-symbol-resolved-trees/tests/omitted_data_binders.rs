//! Omitted trailing data binders recover from `where Binder == <structure>`
//! equations matched against the supplied argument's retained canonical
//! range, and a complete tuple must satisfy every equation exactly
//! (generics.md, "Structural type equations and inference" and "Canonical
//! integer range matching").
//!
//! The typed range normalizer runs in the build-time evaluation service, so
//! these tests retain each literal interval's canonical observation by hand
//! before synthesis, exactly as that service would.

use numerics::bignum::BigInt;
use source_files_to_tokens::Lexer;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::item::{DataMember, Item};
use syntax_trees::types::{
    FixedArrayLength, IntegerRangeNormalization, TypeConstraintNode, TypeReferenceHandle,
    TypeReferenceNode,
};
use syntax_trees_to_symbol_resolved_trees::pre_resolution::{
    GenericDataRequest, normalize_generic_data,
};
use tokens_to_syntax_trees::parse_syntax_trees;

const TINY_BYTES: &str = r#"
    data TinyBytes<Length, const Capacity: u64>
    where
        Length == u64[0..=Capacity]
    {
        storage: [u8; Capacity];
        length: Length;
    }
"#;

fn parse(source: &str) -> SyntaxTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize fixture");
    parse_syntax_trees(&tokens).expect("parse fixture")
}

/// Every range-shell argument of an authored generic application.
fn range_arguments(syntax: &SyntaxTrees) -> Vec<TypeReferenceHandle> {
    let mut ranges = Vec::new();
    for generic in syntax.type_references.generic_nodes() {
        let TypeReferenceNode::Generic { arguments, .. } =
            syntax.type_references.type_reference(generic)
        else {
            continue;
        };
        for argument in syntax.type_references.type_reference_handles(*arguments) {
            if let TypeReferenceNode::Constrained { constraints, .. } =
                syntax.type_references.type_reference(*argument)
                && syntax
                    .type_references
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| matches!(constraint, TypeConstraintNode::Range { .. }))
            {
                ranges.push(*argument);
            }
        }
    }
    ranges
}

fn literal(syntax: &SyntaxTrees, expression: syntax_trees::expression::ExpressionHandle) -> BigInt {
    let ExpressionNode::Integer(literal) = syntax.expressions.expression(expression) else {
        panic!("fixture endpoints are integer literals")
    };
    literal.value_bignum().expect("literal value")
}

/// Retain the canonical inclusive interval of every literal range-shell
/// argument: an exclusive end lands as its proof-integer predecessor.
fn retain_literal_ranges(syntax: &mut SyntaxTrees) {
    for range in range_arguments(syntax) {
        let TypeReferenceNode::Constrained { constraints, .. } =
            *syntax.type_references.type_reference(range)
        else {
            unreachable!("range arguments are constrained")
        };
        let observations = syntax
            .type_references
            .constraints(constraints)
            .iter()
            .enumerate()
            .filter_map(|(ordinal, constraint)| {
                let TypeConstraintNode::Range {
                    minimum,
                    maximum,
                    end_inclusive,
                } = *constraint
                else {
                    return None;
                };
                let maximum = literal(syntax, maximum);
                let maximum = if end_inclusive {
                    maximum
                } else {
                    maximum.sub(&BigInt::from_u64(1))
                };
                Some((
                    ordinal,
                    IntegerRangeNormalization {
                        minimum: literal(syntax, minimum),
                        maximum,
                    },
                ))
            })
            .collect::<Vec<_>>();
        for (ordinal, observation) in observations {
            syntax
                .type_references
                .retain_integer_range_normalization(range, ordinal, observation);
        }
    }
}

fn normalized(source: &str) -> SyntaxTrees {
    let mut syntax = parse(source);
    retain_literal_ranges(&mut syntax);
    normalize_generic_data(GenericDataRequest::new(syntax)).expect("synthesize instances")
}

fn rejection(source: &str) -> String {
    let mut syntax = parse(source);
    retain_literal_ranges(&mut syntax);
    normalize_generic_data(GenericDataRequest::new(syntax))
        .expect_err("synthesis rejects")
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn instances(syntax: &SyntaxTrees) -> Vec<&syntax_trees::item::DataDefinition> {
    syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Data(definition) if definition.generic_instance.is_some() => Some(definition),
            _ => None,
        })
        .collect()
}

fn main_field_types(syntax: &SyntaxTrees) -> Vec<TypeReferenceHandle> {
    let main = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str() == "Main" => Some(definition),
            _ => None,
        })
        .expect("Main record");
    syntax
        .tables
        .items
        .data_members(main.members)
        .iter()
        .map(|member| match member {
            DataMember::Field(field) => field.type_reference,
            _ => panic!("Main holds only fields"),
        })
        .collect()
}

#[test]
fn omitted_capacity_binds_from_the_supplied_range_before_layout() {
    let syntax = normalized(&format!(
        "{TINY_BYTES}
        data Main {{
            inclusive: TinyBytes<u64[0..=256]>;
            exclusive: TinyBytes<u64[0..257]>;
            explicit: TinyBytes<u64[0..=256], 256>;
        }}"
    ));

    // One canonical interval, one instance: the omitted, the exclusive and
    // the explicit spellings share the closed identity `[0..=256]` + 256.
    let found = instances(&syntax);
    let [instance] = found.as_slice() else {
        panic!("exactly one TinyBytes instance");
    };
    assert_eq!(instance.name.as_str(), "TinyBytes<u64 in [0..=256], 256>");
    let [DataMember::Field(storage), DataMember::Field(length)] =
        syntax.tables.items.data_members(instance.members)
    else {
        panic!("storage and length fields");
    };
    assert!(matches!(
        syntax
            .type_references
            .type_reference(storage.type_reference),
        TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Literal(256),
            ..
        }
    ));
    assert!(matches!(
        syntax.type_references.type_reference(length.type_reference),
        TypeReferenceNode::Constrained { .. }
    ));
    // The decided equation is a discharged instantiation obligation, not a
    // standing runtime invariant on the instance.
    assert!(
        syntax
            .tables
            .items
            .proof_facts(instance.where_facts)
            .is_empty()
    );

    // Every use rewrites to the instance, and each retained application
    // origin carries the complete two-binder tuple.
    let fields = main_field_types(&syntax);
    assert_eq!(fields.len(), 3);
    for field in fields {
        assert!(matches!(
            syntax.type_references.type_reference(field),
            TypeReferenceNode::Named(name) if name.as_str() == instance.name.as_str()
        ));
        let origin = syntax.type_references.generic_application_origin(field);
        let TypeReferenceNode::Generic { arguments, .. } =
            syntax.type_references.type_reference(origin)
        else {
            panic!("retained application origin");
        };
        assert_eq!(
            syntax
                .type_references
                .type_reference_handles(*arguments)
                .len(),
            2
        );
    }
}

#[test]
fn full_width_exclusive_end_binds_in_proof_integers_without_carrier_overflow() {
    let syntax = normalized(&format!(
        "{TINY_BYTES}
        data Main {{
            bytes: TinyBytes<u64[0..18446744073709551616]>;
        }}"
    ));
    let found = instances(&syntax);
    let [instance] = found.as_slice() else {
        panic!("exactly one TinyBytes instance");
    };
    assert_eq!(
        instance.name.as_str(),
        "TinyBytes<u64 in [0..=18446744073709551615], 18446744073709551615>"
    );
}

#[test]
fn a_supplied_argument_without_a_retained_endpoint_rejects() {
    let syntax = parse(&format!(
        "{TINY_BYTES}
        data Main {{
            bytes: TinyBytes<u64[0..=256]>;
        }}"
    ));
    let diagnostics = normalize_generic_data(GenericDataRequest::new(syntax))
        .expect_err("no canonical endpoint to match");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .to_string()
            .contains("the supplied `Length` argument has no canonical range endpoint")),
        "{diagnostics:?}"
    );
}

#[test]
fn a_shell_with_two_range_constraints_is_ambiguous() {
    let mut syntax = parse(&format!(
        "{TINY_BYTES}
        data Main {{
            bytes: TinyBytes<u64[0..=256]>;
        }}"
    ));
    let ranges = range_arguments(&syntax);
    let [range] = ranges.as_slice() else {
        panic!("one range argument");
    };
    let range = *range;
    let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = *syntax.type_references.type_reference(range)
    else {
        unreachable!("range arguments are constrained")
    };
    let [constraint] = syntax.type_references.constraints(constraints) else {
        panic!("one authored constraint");
    };
    let constraint = constraint.clone();
    let doubled = syntax
        .type_references
        .insert_constraints([constraint.clone(), constraint]);
    syntax.type_references.replace_type_reference(
        range,
        TypeReferenceNode::Constrained {
            base_type,
            constraints: doubled,
        },
    );
    for ordinal in 0..2 {
        syntax.type_references.retain_integer_range_normalization(
            range,
            ordinal,
            IntegerRangeNormalization {
                minimum: BigInt::from_u64(0),
                maximum: BigInt::from_u64(256),
            },
        );
    }
    let diagnostics = normalize_generic_data(GenericDataRequest::new(syntax))
        .expect_err("two endpoints cannot select one capacity");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .to_string()
            .contains("carries 2 range constraints, so its endpoint is ambiguous")),
        "{diagnostics:?}"
    );
}

#[test]
fn explicit_and_repeated_bindings_must_agree_exactly() {
    // An explicit argument is fixed; a larger compatible capacity is not the
    // equation's endpoint.
    let message = rejection(&format!(
        "{TINY_BYTES}
        data Main {{ bytes: TinyBytes<u64[0..=256], 512>; }}"
    ));
    assert!(
        message.contains("where equation binds `Capacity` to 256 but its explicit argument is 512"),
        "{message}"
    );

    // The exact equation also refuses a larger supplied interval against an
    // explicit capacity, unlike a call's declared-range compatibility.
    let message = rejection(&format!(
        "{TINY_BYTES}
        data Main {{ bytes: TinyBytes<u64[0..=512], 256>; }}"
    ));
    assert!(
        message.contains("where equation binds `Capacity` to 512 but its explicit argument is 256"),
        "{message}"
    );

    // Repeated occurrences must agree under normalization: the inclusive and
    // exclusive equations derive 256 and 257 from one supplied interval.
    let message = rejection(
        "
        data TinyBytes<Length, const Capacity: u64>
        where
            Length == u64[0..=Capacity],
            Length == u64[0..Capacity]
        {
            storage: [u8; Capacity];
            length: Length;
        }
        data Main { bytes: TinyBytes<u64[0..=256]>; }
        ",
    );
    assert!(
        message.contains("where equations bind `Capacity` to both 256 and 257"),
        "{message}"
    );
}

#[test]
fn malformed_and_underdetermined_equations_reject_distinctly() {
    let cases = [
        (
            "Length == u64[0..=Capacity]",
            "TinyBytes<u64>",
            "the supplied `Length` argument lacks a declared range shell",
        ),
        (
            "Length == u64[0..=Capacity]",
            "TinyBytes<u32[0..=256]>",
            "the supplied range carrier is not `u64`",
        ),
        (
            "Length == u64[0..=Capacity]",
            "TinyBytes<u64[0..0]>",
            "where equation binds `Capacity` to -1, outside its declared `u64`",
        ),
        (
            "Length == Length[0..=Capacity]",
            "TinyBytes<u64[0..=256]>",
            "where equations define `Length` through itself",
        ),
        (
            "Length == Capacity",
            "TinyBytes<u64[0..=256]>",
            "mixes type and value kinds: type binder `Length` cannot equal value binder `Capacity`",
        ),
        (
            "Capacity == u64[0..=256]",
            "TinyBytes<u64[0..=256]>",
            "mixes type and value kinds: value binder `Capacity` cannot equal a range shell",
        ),
        (
            "Length == u64[0..=256]",
            "TinyBytes<u64[0..=256]>",
            "cannot determine `Capacity` from its where equations",
        ),
        (
            "Length == u64[0..=Capacity * 2]",
            "TinyBytes<u64[0..=256]>",
            "structural inference binds a bare const binder, not an expression over it",
        ),
    ];
    for (equation, application, fragment) in cases {
        let message = rejection(&format!(
            "
            data TinyBytes<Length, const Capacity: u64>
            where
                {equation}
            {{
                storage: [u8; Capacity];
                length: Length;
            }}
            data Main {{ bytes: {application}; }}
            "
        ));
        assert!(
            message.contains(fragment),
            "{equation} applied as {application}: {message}"
        );
    }
}

#[test]
fn a_closed_endpoint_expression_over_a_bound_binder_verifies() {
    // Verification of a supplied tuple evaluates the endpoint expression
    // over the explicit binder; recovery of an omitted binder never solves
    // it, as the case above pins.
    let syntax = normalized(
        "
        data TinyBytes<Length, const Capacity: u64>
        where
            Length == u64[0..=Capacity * 2]
        {
            storage: [u8; Capacity];
            length: Length;
        }
        data Main { bytes: TinyBytes<u64[0..=256], 128>; }
        ",
    );
    let found = instances(&syntax);
    let [instance] = found.as_slice() else {
        panic!("exactly one TinyBytes instance");
    };
    assert_eq!(instance.name.as_str(), "TinyBytes<u64 in [0..=256], 128>");
    assert!(
        syntax
            .tables
            .items
            .proof_facts(instance.where_facts)
            .is_empty()
    );
}
