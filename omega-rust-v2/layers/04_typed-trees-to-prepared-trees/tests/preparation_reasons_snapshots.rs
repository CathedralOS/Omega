//! Typed preparation causes for constant snapshots, static demands that stay
//! unanswered and owned aggregate values.
//!
//! Every refusal below is produced by the public diagnosed entry point for an
//! authored source. Each test checks the refusing phase, the typed cause, the
//! reason text, package, file, byte range and complete diagnostic against
//! values written here, and the ordinary entry point must report the same
//! diagnostic. Each refusal has a healthy twin that differs only in the refused
//! premise and whose frozen product is checked.
use omega_preparation::{
    FloatFormat, FloatOperation, FloatRealization, FloatSemantics, PreparationFailure, PreparationReason, PreparedTrees, PreparedValue, RefusalCause,
    TargetIdentity, TargetObservation, TargetSemantics, prepare_source_package_diagnosed, prepare_source_package_diagnosed_with_inputs,
    prepare_source_package_for_target,
};
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{SourcePackage, resolve_source_package};

const PACKAGE: &str = "app";
const FILE: &str = "main.omg";
const ADMISSION: &str = "static evaluation cannot admit this invocation: ";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Typing,
    ResumedTyping,
    Preparation,
    MissingTarget,
}

fn phase(failure: &PreparationFailure) -> Phase {
    match failure {
        PreparationFailure::Typing(_) => Phase::Typing,
        PreparationFailure::ResumedTyping(_) => Phase::ResumedTyping,
        PreparationFailure::Preparation(_) => Phase::Preparation,
        PreparationFailure::MissingTarget(_) => Phase::MissingTarget,
    }
}

fn source_package(source: &str) -> SourcePackage<'_> {
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let syntax = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    let files = [StageSource {
        package: PACKAGE,
        source: ParsedSource {
            logical_path: FILE,
            text: source,
            tokens: &tokens,
            tree: &syntax,
        },
    }];
    resolve_source_package(&files).unwrap()
}

/// The byte range of the last occurrence of `text` in `source`.
fn last(
    source: &str,
    text: &str,
) -> (usize, usize) {
    let start = source
        .rfind(text)
        .unwrap_or_else(|| panic!("{text:?} does not occur in the source"));
    (start, start + text.len())
}

/// The byte range of `text` written directly after the only `context` in `source`.
fn after(
    source: &str,
    context: &str,
    text: &str,
) -> (usize, usize) {
    assert_eq!(source.matches(context).count(), 1, "{context:?} must occur once");
    let start = source.find(context).unwrap() + context.len();
    assert!(source[start..].starts_with(text), "{text:?} does not follow {context:?}");
    (start, start + text.len())
}

fn ieee_target(operations: &[FloatOperation]) -> TargetSemantics {
    TargetSemantics::new(
        TargetIdentity {
            profile: "test".into(),
            semantics_version: 1,
            realization: (&b"ieee"[..]).into(),
        },
        [],
        operations.iter().map(|&operation| FloatRealization {
            format: FloatFormat::Binary32,
            operation,
            semantics: FloatSemantics::Ieee754NearestEvenV1,
        }),
    )
    .unwrap()
}

/// Prepare with the supplied target and check every field of the refusal.
/// The ordinary entry points must report the same diagnostic.
fn assert_refusal(
    source: &str,
    target: Option<&TargetSemantics>,
    expected_phase: Phase,
    kind: PreparationReason,
    reason: &str,
    range: (usize, usize),
    message: &str,
) {
    let package = source_package(source);
    let failure = prepare_source_package_diagnosed_with_inputs(&package, &[], target)
        .map(drop)
        .expect_err("the source must be refused");
    let ordinary = match target {
        Some(target) => prepare_source_package_for_target(&package, &[], target)
            .map(drop)
            .expect_err("the ordinary target entry point must refuse the source"),
        None => {
            let diagnosed = prepare_source_package_diagnosed(&package)
                .map(drop)
                .expect_err("the diagnosed entry point must refuse the source");
            assert_eq!(phase(&diagnosed), phase(&failure), "the diagnosed entry point reports another phase");
            diagnosed.into()
        }
    };
    assert_eq!(&ordinary, failure.diagnostic(), "the ordinary entry point reports another diagnostic");
    assert_eq!(phase(&failure), expected_phase, "{failure:?}");
    let cause = RefusalCause::Preparation(kind);
    assert_eq!(failure.cause(), Some(cause), "{failure:?}");
    assert_eq!(failure.reason(), Some(kind));
    assert_eq!(kind.text(), reason);
    spec_corpus::assert_preparation_refusal!(failure, cause, PACKAGE, FILE, range, message);
}

/// An evaluation refusal rendered after the admission prefix without a suffix.
fn admitted(
    source: &str,
    kind: PreparationReason,
    reason: &str,
    range: (usize, usize),
) {
    let message = format!("{ADMISSION}{reason}");
    assert_refusal(source, None, Phase::Preparation, kind, reason, range, &message);
}

/// The healthy twin prepares completely under the supplied target.
fn prepared<'text>(
    source: &'text str,
    target: Option<&TargetSemantics>,
) -> PreparedTrees<'text> {
    let package = source_package(source);
    prepare_source_package_diagnosed_with_inputs(&package, &[], target).unwrap_or_else(|failure| panic!("healthy twin refused: {failure:?}"))
}

fn frozen(
    prepared: &PreparedTrees<'_>,
    name: &str,
) -> PreparedValue {
    let declaration = &prepared
        .typed()
        .constants
        .iter()
        .find(|constant| constant.declaration.path == name)
        .unwrap_or_else(|| panic!("missing constant {name}"))
        .declaration;
    prepared
        .constant(declaration)
        .cloned()
        .unwrap_or_else(|| panic!("{name} is not frozen"))
}

fn integer(value: u8) -> PreparedValue {
    PreparedValue::Integer(value.into())
}

const CYCLE: &str = "cyclic constant initializer";

/// Freezing walks the constants in declaration order. FIRST reads SECOND, and
/// SECOND's initializer reads FIRST while FIRST is still being frozen, so the
/// refusal names SECOND's initializer for scalar, array and record carriers.
#[test]
fn constant_snapshots_that_read_each_other_are_cyclic_for_every_carrier() {
    let pair = "data Pair [copy]{first:u8; second:u8;} ";
    for (prefix, ty, value) in [
        ("", "u8", "7"),
        ("", "[bool;1]", "[true]"),
        (pair, "Pair", "Pair{first:3,second:4}"),
    ] {
        let healthy = format!("{prefix}const FIRST:{ty}=SECOND; const SECOND:{ty}={value};");
        let product = prepared(&healthy, None);
        let first = frozen(&product, "FIRST");
        assert_eq!(first, frozen(&product, "SECOND"), "{healthy}");
        match ty {
            "u8" => assert_eq!(first, integer(7)),
            "[bool;1]" => assert_eq!(first, PreparedValue::Array([PreparedValue::Bool(true)].into())),
            _ => assert!(
                matches!(&first, PreparedValue::Record { owner, fields } if owner.path == "Pair" && fields.as_ref() == [integer(3), integer(4)]),
                "{first:?}"
            ),
        }

        let cyclic = format!("{prefix}const FIRST:{ty}=SECOND; const SECOND:{ty}=FIRST;");
        admitted(&cyclic, PreparationReason::CyclicConstantInitializer, CYCLE, last(&cyclic, "FIRST"));
    }
}

/// A closed extent demands N before typing can finish. The demand freezes N's
/// snapshot, which reads M, whose initializer reads N again.
#[test]
fn an_extent_demand_cannot_be_answered_through_a_constant_cycle() {
    let healthy = "const N:u64=M; const M:u64=4; machine take(x:[u8; N])->[u8;4]{x}";
    let product = prepared(healthy, None);
    for name in ["N", "M"] {
        assert_eq!(frozen(&product, name), PreparedValue::Integer(4u8.into()), "{name}");
    }

    let cyclic = "const N:u64=M; const M:u64=N; machine take(x:[u8; N])->[u8;4]{x}";
    admitted(cyclic, PreparationReason::CyclicConstantInitializer, CYCLE, after(cyclic, "M:u64=", "N"));
}

const READY: &str = "const READY:bool = 1.5f32 < 2.5f32;
                     machine count(n:u64)->u64 requires READY {n}";
const DEMAND: &str = "machine consume(x:[u8; count(4)])->[u8;4]{x}";
const COMPARISON: &str = "1.5f32 < 2.5f32";

/// Without a demand, READY keeps its typed recipe for a later target. A closed
/// extent that runs count(4) needs READY's snapshot now, so the missing target
/// leaves the demand unanswered and is reported as missing target input.
#[test]
fn an_extent_demand_on_a_floating_snapshot_needs_a_selected_target() {
    let reason = "the floating operation needs a selected target realization";
    let message = format!("{ADMISSION}{reason}");

    let open = prepared(READY, None);
    let declaration = &open
        .typed()
        .constants
        .iter()
        .find(|constant| constant.declaration.path == "READY")
        .expect("READY is typed")
        .declaration;
    assert_eq!(open.constant(declaration), None);
    let recipe = open
        .open_target_constant(declaration)
        .expect("READY retains its open recipe");
    assert_eq!((recipe.package.as_str(), recipe.file.as_str()), (PACKAGE, FILE));
    assert_eq!((recipe.span.get_start_offset(), recipe.span.get_end_offset()), last(READY, COMPARISON));
    assert_eq!(recipe.message, message);

    let source = format!("{READY}\n{DEMAND}");
    let range = last(&source, COMPARISON);
    assert_refusal(
        &source,
        None,
        Phase::MissingTarget,
        PreparationReason::FloatNeedsTargetRealization,
        reason,
        range,
        &message,
    );

    // The selected target answers the snapshot, and the demand closes the extent.
    let target = ieee_target(&[FloatOperation::Less]);
    let product = prepared(&source, Some(&target));
    assert_eq!(frozen(&product, "READY"), PreparedValue::Bool(true));
    let less = target
        .float_realization(FloatFormat::Binary32, FloatOperation::Less)
        .unwrap();
    assert!(
        product
            .target_observations()
            .contains(&TargetObservation::FloatRealization {
                target: target.identity().clone(),
                realization: *less,
            })
    );

    // A selected target without the comparison refuses the same snapshot instead.
    let reason = "the selected target does not support this floating operation";
    let without_less = ieee_target(&[FloatOperation::Add]);
    assert_refusal(
        &source,
        Some(&without_less),
        Phase::Preparation,
        PreparationReason::UnsupportedFloatOperation,
        reason,
        range,
        &format!("{reason} (Binary32 Less on test)"),
    );
}

const PAIR: &str = "data Pair [copy]{first:u8; second:u8;}";
const PATTERN: &str = "the selected pattern has no supported owned-value selection";

/// L3 accepts a positional pattern for a record subject. The evaluator selects
/// owned record fields only by name, so the positional arm has no selection.
#[test]
fn a_positional_pattern_over_an_owned_record_has_no_selection() {
    let call = "total(Pair{first:3,second:4})";
    let program = |pattern: &str| format!("{PAIR} machine total(p:Pair)->u8 {{match p {{ {pattern} -> first +% second, }}}} const VALUE:u8={call};");
    let named = program("Pair{first, second}");
    assert_eq!(frozen(&prepared(&named, None), "VALUE"), integer(7));

    let positional = program("(first, second)");
    admitted(&positional, PreparationReason::PatternWithoutSelection, PATTERN, last(&positional, call));
}

/// A field pattern that compares the field with a value has no binding. The
/// evaluator does not select an owned field for it.
#[test]
fn a_projected_equality_field_pattern_has_no_selection() {
    let call = "pick(Pair{first:3,second:4})";
    let program = |arms: &str| format!("{PAIR} machine pick(p:Pair)->u8 {{match p {{ {arms} }}}} const VALUE:u8={call};");
    let binding = program("Pair{first, ..} -> first,");
    assert_eq!(frozen(&prepared(&binding, None), "VALUE"), integer(3));

    let equality = program("Pair{first: 3u8, ..} -> 1u8, _ -> 2u8,");
    admitted(&equality, PreparationReason::PatternWithoutSelection, PATTERN, last(&equality, call));
}
