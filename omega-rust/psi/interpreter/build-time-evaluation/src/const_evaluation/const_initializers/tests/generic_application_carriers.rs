//! Const initializers whose declared carriers are closed generic
//! applications (`const B: Box<u64> = ...`), including through module
//! namespaces. A closed application stands in for its synthesized instance:
//! leaf collection, placeholder construction, canonicalization, and const
//! index eligibility all resolve parameter carriers through the enclosing
//! application's argument bindings rather than re-resolving the synthesized
//! name as a header symbol.
use super::{constant, evaluate, evaluate_fully, integer_encoding, literal_encoding, parse_files};
use syntax_trees::expression::ExpressionNode;

fn evaluate_files(
    files: &[(&str, &str)],
) -> Result<syntax_trees::SyntaxTrees, Vec<diagnostics::Diagnostic>> {
    let (syntax, sources, _) = parse_files(files);
    super::super::evaluate(syntax, Some(sources), &[], None)
}

#[test]
fn copied_nested_generic_table_retains_transitive_origins() {
    let evaluated = evaluate_files(&[
        (
            "settings.omg",
            "module settings;
            pub data Cell<T [copy]> [copy] { value: T; }
            pub data Row<T [copy]> [copy] { cells: [Cell<T>; 1]; }
            pub const TABLE: [Row<u64>; 1] = [Row { cells: [Cell { value: 7 }] }];",
        ),
        (
            "main.omg",
            "use settings;
            const COPIED: [settings::Row<u64>; 1] = settings::TABLE;
            const SECOND: [settings::Row<u64>; 1] = COPIED;",
        ),
    ])
    .expect("transitive table copy");
    let copied = constant(&evaluated, "COPIED")
        .normalization
        .as_ref()
        .unwrap();
    let second = constant(&evaluated, "SECOND")
        .normalization
        .as_ref()
        .unwrap();
    assert_eq!(
        copied.canonical_result_encoding,
        second.canonical_result_encoding
    );
    assert!(
        copied
            .selections
            .iter()
            .all(|origin| second.selections.contains(origin))
    );
    assert_eq!(second.selections.len(), copied.selections.len() + 1);
}

#[test]
fn generic_dest_struct_literal_with_computed_leaves() {
    let declarations = "data Box<T> [copy] { value: T; }";
    let evaluated = evaluate(&format!(
        "{declarations} const B: Box<u64> = Box {{ value: 2 + 3 }};"
    ))
    .expect("computed leaf at a closed generic carrier");
    assert_eq!(
        constant(&evaluated, "B")
            .normalization
            .as_ref()
            .expect("evaluated declaration")
            .canonical_result_encoding,
        literal_encoding(
            &format!("{declarations} const B: Box<u64> = Box {{ value: 5 }};"),
            "B"
        )
    );
}

#[test]
fn generic_dest_variant_and_payloadless_case() {
    let declarations = "data Opt<T [copy]> [copy] { case None; case Some(v: T); }";
    let evaluated = evaluate(&format!(
        "{declarations}
        const O: Opt<u64> = Opt::Some {{ v: 4 + 2 }};
        const E: Opt<u64> = Opt::None;"
    ))
    .expect("variant leaves at a closed generic carrier");
    // The payload case collects a computed leaf and materializes a receipt.
    assert_eq!(
        constant(&evaluated, "O")
            .normalization
            .as_ref()
            .expect("evaluated declaration")
            .canonical_result_encoding,
        literal_encoding(
            &format!("{declarations} const O: Opt<u64> = Opt::Some {{ v: 6 }};"),
            "O"
        )
    );
    // The payloadless case introduces no probe leaf: it stays a name literal
    // owned by downstream literal validation, so no receipt is published here.
    let payloadless = constant(&evaluated, "E");
    assert!(payloadless.normalization.is_none());
    assert!(matches!(
        evaluated.expressions.expression(payloadless.value),
        ExpressionNode::Name(_)
    ));
}

#[test]
fn generic_dest_nested_member_application() {
    // `Outer<u64>` binds `T` so its `Inner<T>` member canonicalizes against
    // `Inner<u64>` — the bindings resolve one application level at a time.
    let declarations = "data Inner<T [copy]> [copy] { v: T; }
        data Outer<T [copy]> [copy] { inner: Inner<T>; }";
    let evaluated = evaluate(&format!(
        "{declarations}
        const O: Outer<u64> = Outer {{ inner: Inner {{ v: 2 + 5 }} }};"
    ))
    .expect("nested generic member through the outer application's bindings");
    assert_eq!(
        constant(&evaluated, "O")
            .normalization
            .as_ref()
            .expect("evaluated declaration")
            .canonical_result_encoding,
        literal_encoding(
            &format!(
                "{declarations}
        const O: Outer<u64> = Outer {{ inner: Inner {{ v: 7 }} }};"
            ),
            "O"
        )
    );
}

#[test]
fn generic_dest_const_length_array_member() {
    // `cells: [u8; N]` resolves its length through the application's `N` -> 3
    // binding rather than declining the open parameter.
    let declarations = "data Arr<const N: u64> [copy] { cells: [u8; N]; }";
    let evaluated = evaluate(&format!(
        "{declarations}
        const A: Arr<3> = Arr {{ cells: [1, 1 + 1, 3] }};"
    ))
    .expect("const-parameter array length through the application binding");
    assert_eq!(
        constant(&evaluated, "A")
            .normalization
            .as_ref()
            .expect("evaluated declaration")
            .canonical_result_encoding,
        literal_encoding(
            &format!("{declarations} const A: Arr<3> = Arr {{ cells: [1, 2, 3] }};"),
            "A"
        )
    );
}

#[test]
fn foreign_generic_template_const_with_computed_leaf() {
    // The closed instance is owned by geom's module path; the computed leaf
    // still binds through `geom::Box`'s parameter.
    let evaluated = evaluate_files(&[
        (
            "geom.omg",
            "module geom; pub data Box<T [copy]> [copy] { value: T; }",
        ),
        (
            "mine.omg",
            "module mine; const B: geom::Box<u64> = geom::Box { value: 1 + 1 };",
        ),
    ])
    .expect("foreign specialized template const with a computed leaf");
    let definition = constant(&evaluated, "B");
    assert!(definition.normalization.is_some());
    assert!(matches!(
        evaluated.expressions.expression(definition.value),
        ExpressionNode::StructLiteral(_)
    ));
}

#[test]
fn constrained_scalar_carrier_evaluates_at_base() {
    // `u64 in u64::Pos` keeps its domain constraint on the declared carrier;
    // the computed leaf evaluates at the `u64` base.
    let evaluated =
        evaluate("domain u64::Pos requires self > 0; const X: u64 in u64::Pos = 1 + 2;")
            .expect("computed leaf at a constrained scalar carrier");
    integer_encoding(&evaluated, "X", 3);
}

#[test]
fn module_scoped_computed_and_foreign_dependency_consts() {
    let evaluated = evaluate_files(&[
        ("m.omg", "module m; pub const SIZE: u64 = 4;"),
        (
            "mine.omg",
            "module mine; use m::SIZE; const DOUBLE: u64 = SIZE * 2;",
        ),
    ])
    .expect("module const on a foreign scalar const");
    integer_encoding(&evaluated, "DOUBLE", 8);
}

#[test]
fn module_scalar_computed_const() {
    let evaluated = evaluate_files(&[("m.omg", "module m; const X: u64 = 1 + 2;")])
        .expect("module scalar computed const");
    integer_encoding(&evaluated, "X", 3);
}

#[test]
fn module_record_literal_with_computed_leaves() {
    let evaluated = evaluate_files(&[(
        "m.omg",
        "module m; data Pair [copy] { first: u64; second: u64; } const P: Pair = Pair { first: 1 + 1, second: 3 };",
    )])
    .expect("module record with computed leaves");
    let definition = constant(&evaluated, "P");
    assert!(matches!(
        evaluated.expressions.expression(definition.value),
        ExpressionNode::StructLiteral(_)
    ));
}

#[test]
fn module_aggregate_call_initializer_full_pipeline() {
    // End to end: the structured leaf's checked interpreter supplies the
    // record the const materializes.
    let typed = evaluate_fully(
        &[(
            "m.omg",
            "module m; data Pair [copy] { first: u64; } machine make() -> Pair { Pair { first: 7 } } const P: Pair = make();",
        )],
        &[],
    );
    assert!(
        typed
            .const_declarations()
            .iter()
            .any(|declaration| declaration.canonical_value_encoding.is_some())
    );
}

#[test]
fn float_literal_module_const_stays_literal() {
    // A scoped float literal needs no probe leaf; it remains a literal for the
    // module scalar gate, which accepts the `f64` landing.
    let evaluated = evaluate_files(&[("f.omg", "module f; const F: f64 = 1.5;")])
        .expect("module float literal const");
    let definition = constant(&evaluated, "F");
    assert!(matches!(
        evaluated.expressions.expression(definition.value),
        ExpressionNode::Float(_)
    ));
}

#[test]
fn module_owned_constrained_const_discharges_at_declaration_site() {
    // A module-owned const with a constrained carrier publishes compatibility
    // identity once its value is proved against the selected domain's facts:
    // `3 > 0` discharges under `m`'s own `Pos` at declaration site. The float
    // sibling keeps the evaluation batch nonempty so module validation runs.
    let evaluated = evaluate_files(&[
        (
            "m.omg",
            "module m; domain u64::Pos requires self > 0; const X: u64 in u64::Pos = 3;",
        ),
        ("f.omg", "module f; const F: f64 = 1.5;"),
    ])
    .expect("module-owned constrained const discharges its domain facts");
    let definition = constant(&evaluated, "X");
    assert!(matches!(
        evaluated.expressions.expression(definition.value),
        ExpressionNode::Integer(_)
    ));
}

#[test]
fn module_owned_constrained_const_rejects_a_refuted_domain() {
    // The same gate rejects when the selected domain's facts refute the
    // declared value: `0 > 0` is false, so no identity may publish. The float
    // sibling again keeps the evaluation batch nonempty.
    let errors = evaluate_files(&[
        (
            "m.omg",
            "module m; domain u64::Pos requires self > 0; const X: u64 in u64::Pos = 0;",
        ),
        ("f.omg", "module f; const F: f64 = 1.5;"),
    ])
    .expect_err("a refuted constrained const still rejects");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("is false")),
        "unexpected diagnostics: {errors:?}"
    );
}

#[test]
fn generic_machine_call_in_const_argument_position() {
    // `Buffer<identity<u64>(4)>` — a call inside a const argument position
    // keeps the whole carrier open, so the const defers value admission to
    // lowering exactly like any unclosed generic carrier: the declaration
    // itself still selects its template and survives this stage unevaluated.
    let evaluated = evaluate(
        "machine identity<T>(value: T) -> T { value } data Buffer<const N: u64> { v: u64; } const B: Buffer<identity<u64>(4)> = Buffer { v: 0 };",
    )
    .expect("generic call as a const argument defers, not rejects");
    assert!(constant(&evaluated, "B").normalization.is_none());
}
#[test]
fn module_const_evaluated_reference_leaf() {
    // `B = A` is a const-reference leaf at a module-owned nominal carrier; the
    // evaluator substitutes the referenced declaration's value.
    let evaluated = evaluate_files(&[(
        "m.omg",
        "module m; data Pair [copy] { first: u64; } const A: Pair = Pair { first: 1 }; const B: Pair = A;",
    )])
    .expect("module const evaluated reference");
    let definition = constant(&evaluated, "B");
    assert!(matches!(
        evaluated.expressions.expression(definition.value),
        ExpressionNode::StructLiteral(_)
    ));
}

#[test]
fn module_const_match_aggregate_leaf() {
    // A match producing an aggregate is a structured leaf whose selected arm
    // materializes against the declared carrier.
    let evaluated = evaluate_files(&[(
        "m.omg",
        "module m; data Pair [copy] { first: u64; second: u64; } const M: Pair = match true { true -> Pair { first: 1, second: 2 }, false -> Pair { first: 9, second: 9 } };",
    )])
    .expect("module const match aggregate");
    let definition = constant(&evaluated, "M");
    assert!(matches!(
        evaluated.expressions.expression(definition.value),
        ExpressionNode::StructLiteral(_)
    ));
}

#[test]
fn foreign_aggregate_call_through_owner_spelling() {
    // The probe machine's return type records the const's use-site spelling
    // (`geom::Point`, or the bare leaf `Point` under a narrow import); the
    // resolved symbol is the selected-home identity admission compares, so a
    // qualified spelling of the exact selected carrier is consistent. The
    // checked interpreter's record materializes under the owning module's
    // path at the leaf's source.
    for (tag, declaring) in [
        (
            "qualified",
            "module mine; const P: geom::Point = geom::make();",
        ),
        (
            "imported carrier",
            "module mine; use geom::Point; const P: Point = geom::make();",
        ),
        (
            "imported call",
            "module mine; use geom::make; const P: geom::Point = make();",
        ),
    ] {
        let evaluated = evaluate_files(&[
            (
                "geom.omg",
                "module geom; pub data Point [copy] { x: u64; } pub machine make() -> Point { Point { x: 7 } }",
            ),
            ("mine.omg", declaring),
        ])
        .unwrap_or_else(|errors| panic!("{tag} foreign aggregate call: {errors:?}"));
        let definition = constant(&evaluated, "P");
        assert!(
            definition.normalization.is_some(),
            "{tag}: evaluated receipt"
        );
        assert!(
            matches!(
                evaluated.expressions.expression(definition.value),
                ExpressionNode::StructLiteral(_)
            ),
            "{tag}: the record materializes as a constructor literal"
        );
    }
}

#[test]
fn foreign_aggregate_call_replays_end_to_end() {
    // The receiving-side replay re-admits the retained probe under the same
    // qualified carrier spelling and re-encodes the interpreter result.
    let typed = evaluate_fully(
        &[
            (
                "geom.omg",
                "module geom; pub data Point [copy] { x: u64; } pub machine make() -> Point { Point { x: 7 } }",
            ),
            (
                "mine.omg",
                "module mine; const P: geom::Point = geom::make();",
            ),
        ],
        &[],
    );
    assert!(
        typed
            .const_declarations()
            .iter()
            .any(|declaration| declaration.canonical_value_encoding.is_some())
    );
}

#[test]
fn foreign_aggregate_call_rejection_coverage() {
    for (tag, sources, fragment) in [
        // The declared carrier is a different module's same-leaf `Point` than
        // the one the selected call returns: the qualified spelling selects
        // `decoys`' owner exactly, and ConstEvaluable admission rejects the
        // interpreted record whose fields do not match it.
        (
            "mismatched carrier",
            vec![
                (
                    "geom.omg",
                    "module geom; pub data Point [copy] { x: u64; } pub machine make() -> Point { Point { x: 7 } }",
                ),
                (
                    "decoys.omg",
                    "module decoys; pub data Point [copy] { x: u64; y: u64; }",
                ),
                (
                    "mine.omg",
                    "module mine; const P: decoys::Point = geom::make();",
                ),
            ],
            "is not ConstEvaluable",
        ),
        // Both foreign `Point`s are narrow-imported, so the bare leaf carrier
        // is contested and module name law declines to pick an owner.
        (
            "contested carrier leaf",
            vec![
                (
                    "geom.omg",
                    "module geom; pub data Point [copy] { x: u64; } pub machine make() -> Point { Point { x: 7 } }",
                ),
                (
                    "decoys.omg",
                    "module decoys; pub data Point [copy] { x: u64; }",
                ),
                (
                    "mine.omg",
                    "module mine; use geom::Point; use decoys::Point; const P: Point = geom::make();",
                ),
            ],
            "does not select one declared",
        ),
        // A private foreign carrier cannot be named by the consumer at all.
        (
            "private foreign carrier",
            vec![
                (
                    "geom.omg",
                    "module geom; data Point [copy] { x: u64; } pub machine make() -> Point { Point { x: 7 } }",
                ),
                (
                    "mine.omg",
                    "module mine; const P: geom::Point = geom::make();",
                ),
            ],
            "private data",
        ),
        // A non-copyable foreign record never reaches admission: the constant
        // carrier gate requires a copy-permitting type first.
        (
            "linear foreign carrier",
            vec![
                (
                    "geom.omg",
                    "module geom; pub data Secret { key: u64; } pub machine mint() -> Secret { Secret { key: 1 } }",
                ),
                (
                    "mine.omg",
                    "module mine; const S: geom::Secret = geom::mint();",
                ),
            ],
            "must permit copying",
        ),
    ] {
        let (syntax, sources, _) = parse_files(&sources);
        let errors = super::super::evaluate(syntax, Some(sources), &[], None)
            .expect_err("{tag} must reject");
        assert!(
            errors.iter().any(|error| error.message.contains(fragment)),
            "{tag}: unexpected diagnostics: {errors:?}"
        );
    }
}
