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

#[test]
fn fixed_array_equation_recovers_element_and_count_without_changing_identity() {
    let syntax = normalized(
        r#"
        data Buffer<Backing, Element, const Count: u64>
        where Backing == [Element; Count]
        {
            storage: Backing;
            same_shape: [Element; Count];
        }
        data Main {
            inferred: Buffer<[u8; 4]>;
            explicit: Buffer<[u8; 4], u8, 4>;
        }
        "#,
    );
    let found = instances(&syntax);
    assert_eq!(found.len(), 1);
    for field in main_field_types(&syntax) {
        assert!(matches!(syntax.type_references.type_reference(field),
            TypeReferenceNode::Named(name) if name.as_str() == found[0].name.as_str()));
    }
}

#[test]
fn fixed_array_equations_recurse_and_construct_the_authored_identity() {
    for (parameters, equation, inferred, explicit) in [
        (
            "Backing, Element, const Rows: u64, const Columns: u64",
            "([ [Element; Columns]; Rows]) == Backing",
            "[[u8; 3]; 2]",
            "[[u8; 3]; 2], u8, 2, 3",
        ),
        (
            "Element, const Count: u64, Backing",
            "Backing == [Element; Count]",
            "u8, 4",
            "u8, 4, [u8; 4]",
        ),
        (
            "Element, const Rows: u64, const Columns: u64, Backing",
            "Backing == [[Element; Columns]; Rows]",
            "u8, 2, 3",
            "u8, 2, 3, [[u8; 3]; 2]",
        ),
        (
            "Backing, Element, const Count: u64",
            "Backing == [[Element; Count]; Count]",
            "[[u8; 4]; 4]",
            "[[u8; 4]; 4], u8, 4",
        ),
    ] {
        let syntax = normalized(&format!(
            r#"
            data Buffer<{parameters}> where {equation} {{ storage: Backing; }}
            data Main {{ inferred: Buffer<{inferred}>; explicit: Buffer<{explicit}>; }}
        "#
        ));
        let found = instances(&syntax);
        assert_eq!(found.len(), 1, "{equation}: {:?}", instance_names(&syntax));
        for field in main_field_types(&syntax) {
            assert!(matches!(syntax.type_references.type_reference(field),
                TypeReferenceNode::Named(name) if name.as_str() == found[0].name.as_str()));
        }
        assert!(syntax.items.proof_facts(found[0].where_facts).is_empty());
    }
}

#[test]
fn fixed_array_equations_reject_conflicts_cycles_and_wrong_roles() {
    for (parameters, equations, arguments, expected) in [
        (
            "Backing, Element, const Count: u64",
            "Backing == [Element; Count]",
            "[u8; 4], u16, 4",
            "conflicting element types",
        ),
        (
            "Backing, Element, const Count: u64",
            "Backing == [Element; Count]",
            "[u8; 4], u8, 5",
            "explicit argument is 5",
        ),
        (
            "Backing, Element, const Count: u64",
            "Backing == [[Element; Count]; Count]",
            "[[u8; 4]; 5]",
            "bind `Count` to both 4 and 5",
        ),
        (
            "Backing, Other, Element, const Count: u64",
            "Backing == [Element; Count], Other == [Element; Count]",
            "[u8; 4], [u16; 4]",
            "conflicting element types",
        ),
        (
            "Backing, const Count: u64",
            "Backing == [Backing; Count]",
            "[u8; 4]",
            "define `Backing` through itself",
        ),
        (
            "Backing, Element, const Count: u64",
            "Backing == [Count; 4]",
            "[u8; 4]",
            "mixes type and value kinds",
        ),
        (
            "Backing, Element",
            "Backing == [u8; Element]",
            "[u8; 4]",
            "mixes type and value kinds",
        ),
        (
            "Backing, const Count: u64",
            "Count == [u8; 4]",
            "[u8; 4]",
            "mixes type and value kinds",
        ),
        (
            "Backing, Element, const Count: u64",
            "Backing == [Element; 4]",
            "[u8; 4]",
            "cannot determine `Count`",
        ),
        (
            "Backing, Element, const Count: u64",
            "Backing == [Element; Count]",
            "u8",
            "same fixed-array constructor",
        ),
        (
            "Backing, const Count: u8",
            "Backing == [u8; Count]",
            "[u8; 256]",
            "outside its declared `u8`",
        ),
    ] {
        let error = rejection(&format!(
            r#"
            data Buffer<{parameters}> where {equations} {{ storage: Backing; }}
            data Main {{ value: Buffer<{arguments}>; }}
        "#
        ));
        assert!(
            error.contains(expected),
            "{equations} with {arguments}: {error}"
        );
    }
}

#[test]
fn fixed_array_equations_keep_nominal_element_identity() {
    let error = rejection(
        r#"
        data Left { value: u8; }
        data Right { value: u8; }
        data Buffer<Backing> where Backing == [Left; 4] { storage: Backing; }
        data Main { value: Buffer<[Right; 4]>; }
    "#,
    );
    assert!(error.contains("conflicting element types"), "{error}");
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

#[test]
fn fixed_array_equation_never_resolves_an_open_element_as_a_global() {
    let error = rejection(
        r#"
        data Element { value: u8; }
        data Box<T> { value: T; }
        data Buffer<Backing, Element>
        where Backing == [Box<Element>; 4]
        { storage: Backing; }
        data Main { value: Buffer<[Box<Element>; 4], u8>; }
        "#,
    );
    assert!(error.contains("open or unsupported"), "{error}");

    let error = rejection(
        r#"
        data Family<T> { value: T; }
        data Buffer<Backing, Family>
        where Backing == [Family<u8>; 4]
        { storage: Backing; }
        data Main { value: Buffer<[Family<u8>; 4], u8>; }
    "#,
    );
    assert!(error.contains("open or unsupported"), "{error}");

    let error = rejection(
        r#"
        data Element { value: u8; }
        data Box<T> { value: T; }
        data Buffer<Element, Backing>
        where Backing == [Box<Element>; 4]
        { storage: Backing; }
        data Main { value: Buffer<u8>; }
    "#,
    );
    assert!(error.contains("open or unsupported"), "{error}");
}

#[test]
fn fixed_array_equations_accept_closed_generic_elements_and_root_lengths() {
    let syntax = normalized(
        r#"
        const Count: u64 = 4;
        data Box<T> { value: T; }
        data Buffer<Backing> where Backing == [Box<u8>; Count] { storage: Backing; }
        data Main { value: Buffer<[Box<u8>; 4]>; }
    "#,
    );
    assert_eq!(instances(&syntax).len(), 2, "{:?}", instance_names(&syntax));
}

#[test]
fn fixed_array_equation_never_reads_a_root_constant_for_a_module_length() {
    let mut sources = source::SourceMap::default();
    let mut syntax = SyntaxTrees::default();
    for (path, text) in [
        ("root.omg", "const Count: u64 = 4;"),
        (
            "local.omg",
            r#"
            module local;
            const Count: u64 = 8;
            data Buffer<Backing> where Backing == [u8; Count] { storage: Backing; }
            data Main { value: Buffer<[u8; 4]>; }
        "#,
        ),
    ] {
        let id = sources
            .add(std::path::PathBuf::from(path), text.to_owned())
            .source_id;
        let tokens = Lexer::new(text).tokenize().expect("tokenize");
        tokens_to_syntax_trees::parse_syntax_trees_into_with_id(&mut syntax, id, &tokens)
            .expect("parse");
    }
    let errors = normalize_generic_data(GenericDataRequest {
        syntax,
        sources: Some(std::sync::Arc::new(sources)),
        top_level_bindings: Vec::new(),
        retained_base: None,
    })
    .expect_err("a root constant cannot satisfy the module equation");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("module constant `Count`")),
        "{errors:?}"
    );
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

/// `Bytes` states the same equation as `TinyBytes` with the binders ordered so
/// that supplying the capacity omits the *type* binder.
const BYTES: &str = r#"
    data Bytes<const Capacity: u64, Length>
    where
        Length == u64[0..=Capacity]
    {
        storage: [u8; Capacity];
        length: Length;
    }
"#;

/// Every instance name, sorted, so an assertion does not depend on the order
/// the fixpoint happened to synthesize them in.
fn instance_names(syntax: &SyntaxTrees) -> Vec<String> {
    let mut names = instances(syntax)
        .iter()
        .map(|definition| definition.name.as_str().to_owned())
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[test]
fn an_omitted_type_binder_is_built_from_its_range_shell_equation() {
    let syntax = normalized(&format!(
        "{BYTES}
        data Main {{
            omitted: Bytes<256>;
            explicit: Bytes<256, u64[0..=256]>;
            exclusive: Bytes<256, u64[0..257]>;
        }}"
    ));

    // The constructed shell is the authored shell: one canonical interval,
    // one instance, whatever the spelling.
    let found = instances(&syntax);
    let [instance] = found.as_slice() else {
        panic!(
            "exactly one Bytes instance, found {:?}",
            instance_names(&syntax)
        );
    };
    assert_eq!(instance.name.as_str(), "Bytes<256, u64 in [0..=256]>");

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
    // The recovered binder reaches the field as a real constrained type, not
    // a name standing in for one.
    let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = *syntax.type_references.type_reference(length.type_reference)
    else {
        panic!("the constructed length is a constrained type reference");
    };
    assert!(matches!(
        syntax.type_references.type_reference(base_type),
        TypeReferenceNode::Named(carrier) if carrier.as_str() == "u64"
    ));
    let [TypeConstraintNode::Range { end_inclusive, .. }] =
        syntax.type_references.constraints(constraints)
    else {
        panic!("one authored-shape range constraint");
    };
    assert!(*end_inclusive, "the equation's own boundary kind is kept");
    // Identity comes from the retained canonical interval, nothing else.
    assert_eq!(
        syntax
            .type_references
            .integer_range_normalization(length.type_reference, 0),
        Some(&IntegerRangeNormalization {
            minimum: BigInt::from_u64(0),
            maximum: BigInt::from_u64(256),
        })
    );

    // The decided equation is a discharged instantiation obligation.
    assert!(
        syntax
            .tables
            .items
            .proof_facts(instance.where_facts)
            .is_empty()
    );
}

#[test]
fn a_constructed_shell_normalizes_an_exclusive_equation_end() {
    // `0..Capacity` with Capacity = 257 and the authored `0..=256` are the
    // same canonical interval, so they share one instance.
    let syntax = normalized(
        "
        data Bytes<const Capacity: u64, Length>
        where
            Length == u64[0..Capacity]
        {
            storage: [u8; Capacity];
            length: Length;
        }
        data Main {
            omitted: Bytes<257>;
            explicit: Bytes<257, u64[0..=256]>;
        }
        ",
    );
    assert_eq!(
        instance_names(&syntax),
        vec!["Bytes<257, u64 in [0..=256]>".to_owned()]
    );
}

#[test]
fn a_constructed_shell_evaluates_a_closed_endpoint_expression() {
    // Verification already evaluates `Capacity * 2` against a supplied shell;
    // construction evaluates the same expression to land the endpoint.
    let syntax = normalized(
        "
        data Bytes<const Capacity: u64, Length>
        where
            Length == u64[0..=Capacity * 2]
        {
            storage: [u8; Capacity];
            length: Length;
        }
        data Main { bytes: Bytes<128>; }
        ",
    );
    assert_eq!(
        instance_names(&syntax),
        vec!["Bytes<128, u64 in [0..=256]>".to_owned()]
    );
}

#[test]
fn constructing_a_type_binder_keeps_every_rejection() {
    let cases = [
        // An endpoint naming an undetermined binder constructs nothing.
        (
            "data Bytes<const Capacity: u64, Length, const Other: u64>
             where Length == u64[0..=Other]",
            "Bytes<256>",
            "cannot construct type binder `Length` from its range-shell equation",
        ),
        // The occurs check fires before any construction is attempted.
        (
            "data Bytes<const Capacity: u64, Length>
             where Length == Length[0..=Capacity]",
            "Bytes<256>",
            "where equations define `Length` through itself",
        ),
        // Nothing converts a value to a type in either direction.
        (
            "data Bytes<const Capacity: u64, Length>
             where Length == Capacity",
            "Bytes<256>",
            "mixes type and value kinds: type binder `Length` cannot equal value binder `Capacity`",
        ),
        (
            "data Bytes<const Capacity: u64, Length>
             where Length == Capacity[0..=Capacity]",
            "Bytes<256>",
            "value binder `Capacity` cannot carry a range shell",
        ),
        // An explicitly supplied argument is fixed: construction never
        // overwrites it, and a containing interval is not the endpoint.
        (
            "data Bytes<const Capacity: u64, Length>
             where Length == u64[0..=Capacity]",
            "Bytes<256, u64[0..=512]>",
            "where equation binds `Capacity` to 512 but its explicit argument is 256",
        ),
        // Repeated occurrences must agree: the constructed inclusive shell
        // derives 257 from the exclusive equation.
        (
            "data Bytes<const Capacity: u64, Length>
             where Length == u64[0..=Capacity], Length == u64[0..Capacity]",
            "Bytes<256>",
            "where equation binds `Capacity` to 257 but its explicit argument is 256",
        ),
    ];
    for (template, application, fragment) in cases {
        let message = rejection(&format!(
            "
            {template}
            {{
                storage: [u8; Capacity];
                length: Length;
            }}
            data Main {{ bytes: {application}; }}
            "
        ));
        assert!(
            message.contains(fragment),
            "{template} applied as {application}: {message}"
        );
    }
}

#[test]
fn an_omitted_binder_application_nested_in_another_application_recovers() {
    // The enclosing application's argument is itself an application with an
    // omitted binder. Its own arguments stay whole through the parser, so the
    // nested recovery runs and the enclosing tuple sees the recovered instance.
    let syntax = normalized(&format!(
        "{TINY_BYTES}
        data Pair<Left, Right> {{ left: Left; right: Right; }}
        data Main {{ pair: Pair<TinyBytes<u64[0..=256]>, u64>; }}"
    ));
    assert_eq!(
        instance_names(&syntax),
        vec![
            "Pair<TinyBytes<u64 in [0..=256], 256>, u64>".to_owned(),
            "TinyBytes<u64 in [0..=256], 256>".to_owned(),
        ]
    );
}

#[test]
fn an_omitted_binder_application_inside_a_template_body_recovers() {
    // The omitted-binder spelling sits in another template's field. Template
    // bodies are not monomorphized in place, so the recovery happens on the
    // enclosing template's synthesized instance in the following round.
    let syntax = normalized(&format!(
        "{TINY_BYTES}
        data Wrapper<T> {{ bytes: TinyBytes<u64[0..=256]>; other: T; }}
        data Main {{ wrapper: Wrapper<u64>; }}"
    ));
    assert_eq!(
        instance_names(&syntax),
        vec![
            "TinyBytes<u64 in [0..=256], 256>".to_owned(),
            "Wrapper<u64>".to_owned(),
        ]
    );
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
