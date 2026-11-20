//! Preparation refusals carry the typed rule that L4 rendered.
//!
//! `PreparationReason::ALL` is the declared inventory of rules this layer owns.
//! Each test below prepares a source through the public stage entry point and
//! checks the refusing phase, the typed cause, the reason text, the source
//! location and the complete rendered diagnostic against values written here.
//! A reason is reported as observed only after all of those checks pass.
//! Reasons forwarded from the target contract or the shared integer primitive
//! keep their owner's text and are checked but not reported as L4 reasons.
use std::sync::Arc;

use omega_preparation::{
    FloatFormat, FloatOperation, FloatRealization, FloatSemantics, PreparationFailure, PreparationReason, PreparedValue, RefusalCause,
    TargetIdentity, TargetSemantics, prepare_source_package_diagnosed, prepare_source_package_diagnosed_with_inputs, prepare_source_package_for_target,
};
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{AssemblyTypes, SourcePackage, resolve_source_package, resolve_source_package_with_assembly};

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

fn with_package<R>(
    source: &str,
    inspect: impl FnOnce(&SourcePackage<'_>) -> R,
) -> R {
    inspect(&source_package(source, None))
}

fn source_package(source: &str, assembly: Option<Arc<AssemblyTypes>>) -> SourcePackage<'_> {
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
    match assembly {
        Some(catalog) => resolve_source_package_with_assembly(&files, catalog),
        None => resolve_source_package(&files),
    }.unwrap()
}

#[test]
fn an_assembly_catalog_cannot_be_prepared_for_another_realization_of_the_same_target() {
    let identity = TargetIdentity { profile: "test-cpu".into(), semantics_version: 1, realization: vec![1].into() };
    let catalog = AssemblyTypes { target: identity.clone(), registers: Default::default(), instructions: Default::default() };
    let package = source_package("machine f() {}", Some(Arc::new(catalog)));
    let target = TargetSemantics::new(identity.clone(), [], []).unwrap();
    prepare_source_package_diagnosed_with_inputs(&package, &[], Some(&target)).unwrap();
    let other = TargetSemantics::new(TargetIdentity { realization: vec![2].into(), ..identity }, [], []).unwrap();
    let failure = prepare_source_package_diagnosed_with_inputs(&package, &[], Some(&other)).expect_err("different target realization");
    assert!(matches!(failure, PreparationFailure::Preparation(_)));
    spec_corpus::assert_preparation_refusal!(
        failure, RefusalCause::Preparation(PreparationReason::AssemblyTargetMismatch), "", "", (0, 0),
        "assembly catalog and preparation selected different targets",
    );
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

struct Expected<'a> {
    phase: Phase,
    cause: RefusalCause,
    /// The reason text, written independently of the inventory.
    reason: &'a str,
    range: (usize, usize),
    message: &'a str,
}

/// Prepare with the supplied target and check every field of
/// the refusal. The ordinary entry points must report the same diagnostic.
fn assert_refusal(
    source: &str,
    target: Option<&TargetSemantics>,
    expected: Expected<'_>,
) {
    let failure: PreparationFailure = with_package(source, |package| {
        let failure = prepare_source_package_diagnosed_with_inputs(package, &[], target)
            .map(drop)
            .expect_err("the source must be refused");
        let ordinary = match target {
            Some(target) => prepare_source_package_for_target(package, &[], target).map(drop),
            None => {
                let diagnosed = prepare_source_package_diagnosed(package)
                    .map(drop)
                    .expect_err("the diagnosed entry point must refuse the source");
                assert_eq!(phase(&diagnosed), phase(&failure), "the diagnosed entry point reports another phase");
                Err(diagnosed.into())
            }
        };
        assert_eq!(
            ordinary.as_ref().err(),
            Some(failure.diagnostic()),
            "the ordinary entry point reports another diagnostic"
        );
        failure
    });
    assert_eq!(phase(&failure), expected.phase, "{failure:?}");
    assert_eq!(failure.cause(), Some(expected.cause), "{failure:?}");
    assert_eq!(failure.reason(), expected.cause.reason());
    assert_eq!(expected.cause.text(), expected.reason);
    spec_corpus::assert_preparation_refusal!(failure, expected.cause, PACKAGE, FILE, expected.range, expected.message);
}

fn owned<'a>(
    phase: Phase,
    kind: PreparationReason,
    reason: &'a str,
    range: (usize, usize),
    message: &'a str,
) -> Expected<'a> {
    Expected {
        phase,
        cause: RefusalCause::Preparation(kind),
        reason,
        range,
        message,
    }
}

/// An owned reason rendered after the admission prefix, with an optional suffix.
fn admitted(
    source: &str,
    kind: PreparationReason,
    reason: &'static str,
    occurrence: &str,
    suffix: &str,
) {
    let message = format!("{ADMISSION}{reason}{suffix}");
    assert_refusal(source, None, owned(Phase::Preparation, kind, reason, last(source, occurrence), &message));
}

#[test]
fn declared_preparation_reasons() {
    let all = PreparationReason::ALL;
    assert!(!all.is_empty());
    let mut kinds = std::collections::HashSet::new();
    for kind in all {
        assert!(kinds.insert(*kind), "{kind:?} is declared twice");
        assert!(!kind.text().is_empty(), "{kind:?}");
        assert_eq!(RefusalCause::Preparation(*kind).reason(), Some(*kind));
    }
    if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
        let names: Vec<_> = all.iter().map(|kind| format!("{kind:?}")).collect();
        println!("OMEGA_PREPARATION_REASONS\t{}", names.join(","));
    }
}

#[test]
fn a_ranked_body_requires_its_checked_termination_evidence() {
    let source = "machine size(n:u64)->u64 terminates by n {n} const WIDTH:u64=size(3u64);";
    admitted(source, PreparationReason::RankedBodyNeedsProof, "the selected ranked body requires a checked termination proof", "size(3u64)", "");
}

#[test]
fn a_false_integer_precondition() {
    let source = "machine divide(n:u64,d:u64)->u64 requires d != 0 reaches none crashes none terminates {n / d}
                  const SIZE:u64=divide(12,0);
                  machine carry(x:[u8; SIZE])->[u8; 4]{x}";
    admitted(
        source,
        PreparationReason::IntegerPreconditionFalse,
        "the selected integer precondition is false for the supplied arguments",
        "divide(12,0)",
        "",
    );
}

#[test]
fn a_false_boolean_precondition() {
    let source = "machine predicate(value:bool)->bool reaches none crashes none terminates {value}
                  machine guarded(value:bool)->bool requires predicate(value) {value}
                  machine wrapped(value:bool)->bool {guarded(value)}
                  const VALUE:bool=guarded(false);";
    admitted(
        source,
        PreparationReason::BooleanPreconditionFalse,
        "the selected Boolean precondition is false for the supplied arguments",
        "guarded(false)",
        "",
    );
}

#[test]
fn exact_integer_operations_outside_their_carrier() {
    for (body, input, kind, reason) in [
        (
            "(n + 1) - 1",
            "127",
            PreparationReason::AdditionOutsideCarrier,
            "exact addition exceeds its selected integer carrier",
        ),
        (
            "(n - 1) + 1",
            "-128",
            PreparationReason::SubtractionOutsideCarrier,
            "exact subtraction exceeds its selected integer carrier",
        ),
        (
            "(n * 2) - 1",
            "64",
            PreparationReason::MultiplicationOutsideCarrier,
            "exact multiplication exceeds its selected integer carrier",
        ),
    ] {
        let call = format!("compute({input})");
        let source = format!("machine compute(n:i8)->i8{{{body}}} const RESULT:i8={call};");
        admitted(&source, kind, reason, &call, "");
    }
}

#[test]
fn exact_quotients_outside_their_carrier() {
    for (operator, kind, reason) in [
        (
            "/",
            PreparationReason::DivisionOutsideCarrier,
            "exact division exceeds its selected integer carrier",
        ),
        (
            "%",
            PreparationReason::RemainderQuotientOutsideCarrier,
            "exact remainder requires a representable integer quotient",
        ),
    ] {
        let source = format!("machine calc(n:i8,d:i8)->i8{{n {operator} d}} const VALUE:i8=calc(-128,-1);");
        admitted(&source, kind, reason, "calc(-128,-1)", "");
    }
}

#[test]
fn a_zero_divisor_in_a_tail_helper() {
    let source = "machine divide(n:u64,d:u64)->u64{n / d} machine inner(d:u64)->u64{(divide(12,d))}
                  machine width(d:u64)->u64{inner(d)} machine consume(x:[u8; width(0)])->[u8; 4]{x}";
    admitted(
        source,
        PreparationReason::ZeroDivisor,
        "integer division or remainder requires a nonzero divisor",
        "divide(12,d)",
        "\ncall chain: width -> inner -> divide",
    );
}

#[test]
fn a_public_helper_without_a_termination_promise() {
    let source = "public machine helper(n:u64)->u64 reaches none crashes none {n} machine width(n:u64)->u64{helper(n)} const SIZE:u64=width(4);";
    admitted(
        source,
        PreparationReason::PublicMachineWithoutTermination,
        "the selected public machine does not declare termination",
        "helper(n)",
        "\ncall chain: width -> helper",
    );
}

#[test]
fn a_cyclic_invocation_without_a_termination_proof() {
    let source = "machine a(n:u64)->u64{b(n)} machine b(n:u64)->u64{a(n)} const SIZE:u64=a(4);";
    admitted(
        source,
        PreparationReason::CyclicInvocation,
        "a cyclic static invocation requires a checked termination proof",
        "a(n)",
        "\nrecursive component: a -> b -> a",
    );
}

#[test]
fn crash_routes_that_remain_possible() {
    let program = |routes: &str, value: &str| format!("machine f(n:i8)->i8 {routes} {{ 12i8 /! n }} const SIZE:i8=f({value});");
    for (routes, value, kind, reason) in [
        (
            "crashes Trap true",
            "3",
            PreparationReason::TrapRoutePossible,
            "the selected Trap route remains possible for the supplied arguments",
        ),
        (
            "crashes Abort",
            "3",
            PreparationReason::AbortRoutePossible,
            "the selected Abort route remains possible for the supplied arguments",
        ),
        (
            "crashes Trap false",
            "0",
            PreparationReason::ZeroDivisorTrapPossible,
            "a zero divisor leaves the selected Trap route possible",
        ),
    ] {
        let source = program(routes, value);
        admitted(&source, kind, reason, &format!("f({value})"), "");
    }
}

#[test]
fn trapping_operations_outside_their_carrier() {
    let source = "machine div(n:i8,d:i8)->i8 crashes Trap (d == 0) {n /! d} const SIZE:i8=div(-128,-1);";
    admitted(
        source,
        PreparationReason::QuotientTrapPossible,
        "an unrepresentable quotient leaves the selected Trap route possible",
        "div(-128,-1)",
        "",
    );
    let source = "machine calc(n:i8)->i8 crashes Trap false {n +! 1} const RESULT:i8=calc(127);";
    admitted(
        source,
        PreparationReason::OverflowTrapPossible,
        "overflow leaves the selected Trap route possible",
        "calc(127)",
        "",
    );
}

#[test]
fn a_constructor_whose_predicate_needs_proof() {
    let source = "data Choice [copy] requires false {case Empty;}
                  const VALUE:Choice=Choice::Empty;";
    admitted(
        source,
        PreparationReason::ConstructionNeedsProof,
        "the selected constructor requires proof outside the prepared value fragment",
        "Choice::Empty",
        "",
    );
}

#[test]
fn constant_types_that_cannot_be_frozen() {
    let linear = "data Handle [linear] {}
                  data Choice [linear] {case Empty; case Used(handle:Handle);}
                  const VALUE:Choice=Choice::Empty;";
    admitted(
        linear,
        PreparationReason::ConstantTypeNotCopyable,
        "the complete constant type must permit copying",
        "Choice::Empty",
        "",
    );
    let cleanup = "data Handle [copy] dropped by dispose {}
                   machine dispose(handle:Handle) {}
                   data Choice<T [copy]> [copy] {case Empty; case Used(handle:T);}
                   const VALUE:Choice<Handle> = Choice::Empty;";
    admitted(
        cleanup,
        PreparationReason::ConstantTypeHasCleanup,
        "the complete constant type has a cleanup obligation",
        "Choice::Empty",
        "",
    );
    let elements = "data Handle [linear] {} const A:[Handle;0]=[];";
    admitted(
        elements,
        PreparationReason::ArrayElementNotCopyable,
        "the complete array element type must permit copying",
        "[]",
        "",
    );
}

#[test]
fn an_inexact_floating_cast() {
    let source = "const INVALID:f32 = 0.1f64 as f32;";
    admitted(
        source,
        PreparationReason::InexactFloatCast,
        "the floating carrier cast does not preserve the exact value",
        "0.1f64 as f32",
        "",
    );
}

#[test]
fn a_float_component_has_no_canonical_index_identity() {
    let source = "data Key [copy]{entry:f32;}
                  data With<const K:Key>{}
                  machine key()->Key {Key{entry:1.0f32}}
                  machine take(x:With<key()>)->u8 {7u8}";
    admitted(
        source,
        PreparationReason::NoCanonicalIndexIdentity,
        "the prepared value has no canonical static index identity",
        "key()",
        "",
    );
}

#[test]
fn a_selected_target_without_the_floating_operation() {
    let source = "machine step(value:f32)->f32 {
                    let product = value * 1.00000011920928955078125f32;
                    product + -1.0000002384185791015625f32
                  }
                  const VALUE:f32 = step(1.00000011920928955078125f32);";
    let target = TargetSemantics::new(
        TargetIdentity {
            profile: "test".into(),
            semantics_version: 1,
            realization: (&b"ieee"[..]).into(),
        },
        [],
        [FloatOperation::Multiply, FloatOperation::Negate].map(|operation| FloatRealization {
            format: FloatFormat::Binary32,
            operation,
            semantics: FloatSemantics::Ieee754NearestEvenV1,
        }),
    )
    .unwrap();
    let reason = "the selected target does not support this floating operation";
    let message = format!("{reason} (Binary32 Add on test)");
    assert_refusal(
        source,
        Some(&target),
        owned(
            Phase::Preparation,
            PreparationReason::UnsupportedFloatOperation,
            reason,
            last(source, "product + -1.0000002384185791015625f32"),
            &message,
        ),
    );
}

const LAYOUT: &str = r"use omega\language\core; const VALUE:u64=core\size_of::<u16>(); machine consume(bytes:[u8; VALUE]){}";
const LAYOUT_QUERY: &str = r"core\size_of::<u16>()";

#[test]
fn a_layout_query_without_a_selected_target_is_missing_target_input() {
    let reason = "the layout query requires a selected target";
    let message = format!("{ADMISSION}{reason}");
    assert_refusal(
        LAYOUT,
        None,
        owned(
            Phase::MissingTarget,
            PreparationReason::LayoutQueryNeedsTarget,
            reason,
            last(LAYOUT, LAYOUT_QUERY),
            &message,
        ),
    );
}

#[test]
fn a_target_contract_layout_refusal_is_forwarded_unchanged() {
    let target = TargetSemantics::new(
        TargetIdentity {
            profile: "selected".into(),
            semantics_version: 1,
            realization: Arc::from(b"test-abi-v1".as_slice()),
        },
        [],
        [],
    )
    .unwrap();
    let reason = "the selected target has no validated layout for the type";
    let message = format!("{ADMISSION}{reason}");
    assert_refusal(
        LAYOUT,
        Some(&target),
        Expected {
            phase: Phase::Preparation,
            cause: RefusalCause::TargetLayout(reason),
            reason,
            range: last(LAYOUT, LAYOUT_QUERY),
            message: &message,
        },
    );
}

#[test]
fn an_integer_primitive_refusal_is_forwarded_unchanged() {
    let source = "machine shift(n:u7,k:u64)->u7{n << k} const VALUE:u7=shift(7,7);";
    let reason = "static shift count outside carrier width";
    let message = format!("{ADMISSION}{reason}");
    assert_refusal(
        source,
        None,
        Expected {
            phase: Phase::Preparation,
            cause: RefusalCause::IntegerPrimitive(reason),
            reason,
            range: last(source, "shift(7,7)"),
            message: &message,
        },
    );
}

#[test]
fn typing_refusals_have_no_preparation_cause() {
    let source = "machine f()->u32 { true }";
    let failure = with_package(source, |package| {
        prepare_source_package_diagnosed_with_inputs(package, &[], None)
            .map(drop)
            .unwrap_err()
    });
    assert_eq!(phase(&failure), Phase::Typing);
    assert_eq!((failure.cause(), failure.reason()), (None, None));
    let resumed = "machine divide(n:u64,d:u64)->u64 requires d != 0 reaches none crashes none terminates {n / d}
                   const SIZE:u64=divide(12,2);
                   machine carry(x:[u8; SIZE])->[u8; 4]{x}";
    let failure = with_package(resumed, |package| {
        prepare_source_package_diagnosed_with_inputs(package, &[], None)
            .map(drop)
            .unwrap_err()
    });
    assert_eq!(phase(&failure), Phase::ResumedTyping);
    assert_eq!((failure.cause(), failure.reason()), (None, None));
    assert_eq!(failure.diagnostic().message, "expected [u8; 4], found [u8; 6]");
}

#[test]
fn a_valid_shift_count_can_still_exceed_the_selected_carrier() {
    let source = "machine shift(n:u7,k:u64)->u7{n << k} const VALUE:u7=shift(64,1);";
    assert_refusal(
        source,
        None,
        owned(
            Phase::Preparation,
            PreparationReason::OperationOutsideCarrier,
            "exact operation exceeds its selected integer carrier",
            (53, 64),
            "static evaluation cannot admit this invocation: exact operation exceeds its selected integer carrier",
        ),
    );
}

#[test]
fn a_precondition_on_an_array_result_rejects_the_false_call() {
    let source = "machine pair(n:u8)->[u8;2] requires n > 0u8 {[n,n]} const VALUE:[u8;2]=pair(0u8);";
    let valid = source.replace("pair(0u8)", "pair(7u8)");
    with_package(&valid, |package| {
        let prepared = omega_preparation::prepare_source_package(package).unwrap();
        assert_eq!(prepared.constant(&prepared.typed().constants[0].declaration),
            Some(&PreparedValue::Array([PreparedValue::Integer(7.into()), PreparedValue::Integer(7.into())].into())));
    });
    let start = source.find("pair(0u8)").unwrap();
    assert_refusal(source, None, owned(Phase::Preparation, PreparationReason::PreconditionFalse,
        "the selected precondition is false for the supplied arguments", (start, start + "pair(0u8)".len()),
        "static evaluation cannot admit this invocation: the selected precondition is false for the supplied arguments"));
}

#[test]
fn freezing_a_description_identifies_the_selected_query() {
    let source = r"use omega\language\core\reflect; machine update(){} const META=reflect\callable(update);";
    let query = r"reflect\callable(update)";
    let start = source.find(query).unwrap();
    assert_refusal(source, None, owned(Phase::Preparation, PreparationReason::DescriptorStorageUnavailable,
        "freezing a reflected declaration requires descriptor storage",
        (start, start + query.len()),
        "static evaluation cannot admit this invocation: freezing a reflected declaration requires descriptor storage"));
}

#[test]
fn a_boundary_requirement_selected_as_a_static_callback_reports_the_slot() {
    use omega_typer::{BoundarySlot, RequirementSlot, SlotDirection};
    let source = "boundary trait Host{machine read(n:u64)->u64 reaches none crashes none terminates;}
        data Step=machine(n:u64)->u64; machine identity(n:u64)->u64{n}
        machine apply<const F:Step>(n:u64)->u64{F(n)} const ANSWER:u64=apply::<Host::read>(42u64);";
    for (callback, succeeds) in [("identity", true), ("Host::read", false)] {
        let text = source.replace("apply::<Host::read>", &format!("apply::<{callback}>"));
        let package = source_package(&text, None);
        let declaration = package.declarations.iter().find(|id| id.path == "Host::read").unwrap().clone();
        let slot = BoundarySlot {
            requirement: RequirementSlot { declaration, trait_arguments: vec![] },
            arguments: vec![],
            direction: SlotDirection::ProgramToProvider,
        };
        let result = prepare_source_package_diagnosed_with_inputs(&package, &[slot], None);
        if succeeds {
            let prepared = result.unwrap();
            assert_eq!(prepared.constant(&prepared.typed().constants[0].declaration), Some(&PreparedValue::Integer(42.into())));
        } else {
            let failure = result.expect_err("static preparation cannot execute this boundary callback");
            spec_corpus::assert_preparation_refusal!(
                failure, RefusalCause::Preparation(PreparationReason::RuntimeBoundaryCallback), PACKAGE, FILE,
                last(&text, "apply::<Host::read>(42u64)"),
                "static evaluation cannot admit this invocation: a runtime boundary selection cannot execute as a static callback"
            );
        }
    }
}
