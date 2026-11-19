use omega_preparation::{
    FloatFormat, FloatOperation, FloatRealization, FloatSemantics, MaterializationComponent, PreparedFloat, PreparedTrees, PreparedValue, TargetIdentity,
    TargetObservation, TargetSemantics, prepare_source_package, prepare_source_package_for_target,
};
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{TypeError, resolve_source_package};

fn prepare<'text>(
    source: &'text str,
    target: Option<&TargetSemantics>,
) -> Result<PreparedTrees<'text>, TypeError> {
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    let package = resolve_source_package(&[StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "main.omg",
            text: source,
            tokens: &tokens,
            tree: &tree,
        },
    }])
    .unwrap();
    match target {
        Some(target) => prepare_source_package_for_target(&package, &[], target),
        None => prepare_source_package(&package),
    }
}

fn frozen<'a>(
    prepared: &'a PreparedTrees<'_>,
    path: &str,
) -> &'a PreparedValue {
    let constant = prepared
        .typed()
        .constants
        .iter()
        .find(|constant| constant.declaration.path == path)
        .unwrap();
    prepared.constant(&constant.declaration).unwrap()
}

fn target(operations: impl IntoIterator<Item = (FloatFormat, FloatOperation)>) -> TargetSemantics {
    TargetSemantics::new(
        TargetIdentity {
            profile: "test".into(),
            semantics_version: 1,
            realization: (&b"ieee"[..]).into(),
        },
        [],
        operations
            .into_iter()
            .map(|(format, operation)| FloatRealization {
                format,
                operation,
                semantics: FloatSemantics::Ieee754NearestEvenV1,
            }),
    )
    .unwrap()
}

#[test]
fn floating_landings_round_exact_rationals_to_the_written_format() {
    let source = "const TIE:f32 = 1.000000059604644775390625;
                  const ABOVE_TIE:f32 = 1.0000000596046447753906250000000001;
                  const TENTH:f64 = 1/10;
                  const MIN_SUBNORMAL32:f32 = 1.40129846432481707092372958328991613128e-45;
                  const MIN_SUBNORMAL64:f64 = 4.9406564584124654e-324;
                  const NEGATIVE_UNDERFLOW:f32 = -1.0e-100;
                  const OVERFLOW:f64 = 1.0e400;";
    let prepared = prepare(source, None).unwrap();
    for (name, expected) in [
        ("TIE", PreparedFloat::Binary32(0x3f80_0000)),
        ("ABOVE_TIE", PreparedFloat::Binary32(0x3f80_0001)),
        ("TENTH", PreparedFloat::Binary64(0x3fb9_9999_9999_999a)),
        ("MIN_SUBNORMAL32", PreparedFloat::Binary32(1)),
        ("MIN_SUBNORMAL64", PreparedFloat::Binary64(1)),
        ("NEGATIVE_UNDERFLOW", PreparedFloat::Binary32(0x8000_0000)),
        ("OVERFLOW", PreparedFloat::Binary64(0x7ff0_0000_0000_0000)),
    ] {
        let value = frozen(&prepared, name);
        assert_eq!(value.float(), Some(expected), "{name}");
        value.check_materializable().unwrap();
    }
}

#[test]
fn a_float_component_cannot_become_a_canonical_static_atom() {
    let source = "data Key [copy]{entry:f32;}
                  data With<const K:Key>{}
                  machine key()->Key {Key{entry:1.0f32}}
                  machine take(x:With<key()>)->u8 {7u8}";
    let error = prepare(source, None).unwrap_err().message;
    assert!(error.contains("no canonical static index identity"), "{error}");
    let bool_key = source
        .replace("entry:f32", "entry:bool")
        .replace("1.0f32", "true");
    prepare(&bool_key, None).unwrap();
}

#[test]
fn an_owned_ieee_coercion_preserves_its_exact_meaning() {
    let source = "const ROUNDED:f32 = 1.00000006 as f32;
                  const WIDE:f64 = 0.5f32 as f64;
                  const NARROW:f32 = WIDE as f32;
                  const EXACT:f32 = 16777218u32 as f32;
                  const INTEGER:u32 = EXACT as u32;";
    let prepared = prepare(source, None).unwrap();
    for (name, expected) in [
        ("ROUNDED", PreparedFloat::Binary32(0x3f80_0001)),
        ("WIDE", PreparedFloat::Binary64(0x3fe0_0000_0000_0000)),
        ("NARROW", PreparedFloat::Binary32(0x3f00_0000)),
        ("EXACT", PreparedFloat::Binary32(0x4b80_0001)),
    ] {
        assert_eq!(frozen(&prepared, name).float(), Some(expected), "{name}");
    }
    assert_eq!(frozen(&prepared, "INTEGER"), &PreparedValue::Integer(16777218u32.into()));
    for source in [
        "const INVALID:f32 = 0.1f64 as f32;",
        "const INVALID:f32 = 16777217u32 as f32;",
        "const INVALID:u32 = 1.25f32 as u32;",
        "const INVALID:u32 = (-1.0e-100 as f32) as u32;",
    ] {
        let error = prepare(source, None).unwrap_err().message;
        assert!(error.contains("does not preserve the exact value"), "{source} gave {error}");
    }
}

#[test]
fn float_operations_use_the_selected_target_and_keep_separate_rounding() {
    use FloatFormat::Binary32;
    use FloatOperation::{Add, Multiply, Negate};
    let source = "machine step(value:f32)->f32 {
                    let product = value * 1.00000011920928955078125f32;
                    product + -1.0000002384185791015625f32
                  }
                  const VALUE:f32 = step(1.00000011920928955078125f32);";
    let target = target([(Binary32, Add), (Binary32, Multiply), (Binary32, Negate)]);
    let prepared = prepare(source, Some(&target)).unwrap();
    assert_eq!(frozen(&prepared, "VALUE").float(), Some(PreparedFloat::Binary32(0)));
    for operation in [Add, Multiply, Negate] {
        let selection = target.float_realization(Binary32, operation).unwrap();
        assert!(
            prepared
                .target_observations()
                .contains(&TargetObservation::FloatRealization {
                    target: target.identity().clone(),
                    realization: *selection
                })
        );
    }
    let open = prepare(source, None).unwrap();
    let declaration = &open.typed().constants[0].declaration;
    assert!(open.constant(declaration).is_none());
    let error = open.open_target_constant(declaration).unwrap();
    assert!(error.message.contains("needs a selected target realization"), "{error:?}");
    let missing = self::target([(Binary32, Multiply), (Binary32, Negate)]);
    let error = prepare(source, Some(&missing)).unwrap_err().message;
    assert!(error.contains("does not support this floating operation"), "{error}");
}

#[test]
fn arithmetic_nan_has_semantic_uses_but_no_determined_materialization() {
    use FloatFormat::Binary32;
    use FloatOperation::{Divide, Equal, Less, NotEqual};
    let source = "data Entry [copy]{reading:f32;}
                  data Choice [copy]{case Some(entry:Entry); case Empty;}
                  const READINGS:[Choice;2] = [Choice::Empty, Choice::Some{entry:Entry{reading:NAN}}];
                  const NAN:f32 = 0.0f32/0.0f32;
                  const IS_NAN:bool = NAN != NAN;
                  const EQUAL:bool = NAN == NAN;
                  const LESS:bool = NAN < 1.0f32;
                  const ZEROS_EQUAL:bool = (-1.0e-100 as f32) == 0.0f32;";
    let target = target([
        (Binary32, Divide),
        (Binary32, Equal),
        (Binary32, Less),
        (Binary32, NotEqual),
    ]);
    let prepared = prepare(source, Some(&target)).unwrap();
    assert_eq!(frozen(&prepared, "NAN").float(), Some(PreparedFloat::UnspecifiedNaN(Binary32)));
    for (name, expected) in [
        ("IS_NAN", true),
        ("EQUAL", false),
        ("LESS", false),
        ("ZEROS_EQUAL", true),
    ] {
        assert_eq!(frozen(&prepared, name), &PreparedValue::Bool(expected), "{name}");
    }
    let failure = frozen(&prepared, "READINGS")
        .check_materializable()
        .unwrap_err();
    let [
        MaterializationComponent::Element(1),
        MaterializationComponent::Field { owner: case, index: 0 },
        MaterializationComponent::Field { owner: record, index: 0 },
    ] = failure.path.as_slice()
    else {
        panic!("unexpected component path: {:?}", failure.path)
    };
    assert_eq!(case.path, "Choice::Some");
    assert_eq!(record.path, "Entry");
    assert_eq!(failure.origin.span.slice(source.as_bytes()), Some(b"0.0f32/0.0f32".as_slice()));
}
