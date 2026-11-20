//! Owned-value refusals observed with their typed cause, site and message.
//!
//! Two kinds of evidence appear here and each test names its kind:
//! - authored: a source program goes through the public diagnosed entry point
//!   and the refusal is the real `PreparationFailure` of that stage.
//! - stage input: the typer's actual output for a source is evaluated by one
//!   `PreparationRound` with supplied checked judgments, so the refusal is the
//!   stage's own verification of that input, observed before the package entry
//!   point wraps it.
//!
//! Every refusal has a healthy twin that differs only in the refused premise.
//! Expected reason texts, sites and messages are written here, independent of
//! the compiler's inventory.
use super::{prove_literal_default_domain, resolve};
use crate::reasons::{PreparationReason, RefusalCause};
use crate::{
    CheckedPreparationEvidence, PreparationFailure, PreparationProgress, PreparationRound, PreparationStep, PreparedValue,
    prepare_source_package_diagnosed_with_inputs,
};
use omega_typer::TypingProgress;

const PACKAGE: &str = "app";
const FILE: &str = "main.omg";
const ADMISSION: &str = "static evaluation cannot admit this invocation: ";

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

/// Authored evidence: prepare through the public entry point and check every
/// field of the evaluation refusal.
fn authored_refusal(
    source: &str,
    reason: PreparationReason,
    text: &str,
    occurrence: &str,
) {
    assert_eq!(reason.text(), text, "{reason:?}");
    let package = resolve(source);
    let failure = prepare_source_package_diagnosed_with_inputs(&package, &[], None)
        .map(drop)
        .expect_err("the authored source must be refused");
    assert!(matches!(failure, PreparationFailure::Preparation(_)), "{failure:?}");
    assert_eq!(failure.reason(), Some(reason), "{failure:?}");
    spec_corpus::assert_preparation_refusal!(
        failure,
        RefusalCause::Preparation(reason),
        PACKAGE,
        FILE,
        last(source, occurrence),
        format!("{ADMISSION}{text}")
    );
}

/// Authored healthy control: the twin prepares and freezes `name`.
fn prepared_constant(
    source: &str,
    name: &str,
) -> PreparedValue {
    let package = resolve(source);
    let prepared = prepare_source_package_diagnosed_with_inputs(&package, &[], None).unwrap_or_else(|failure| panic!("healthy twin refused: {failure:?}"));
    let declaration = &prepared
        .typed()
        .constants
        .iter()
        .find(|constant| constant.declaration.path == name)
        .unwrap_or_else(|| panic!("missing constant {name}"))
        .declaration;
    prepared.constant(declaration).cloned().expect("frozen constant")
}

fn integer(value: u32) -> PreparedValue {
    PreparedValue::Integer(value.into())
}

#[test]
fn authored_omitted_fields_without_a_supported_zero_are_refused_at_the_construction() {
    let source = "data Phase [copy] {case Busy(amount:u8); case Empty;}
                  data Settings [copy] {enabled:bool; phase:Phase; fraction:f32; attempts:u8; empty:[u8;0]; samples:[[Phase;2];2];}
                  const VALUE:Settings=Settings{attempts:4,..};";
    let PreparedValue::Record { fields, .. } = prepared_constant(source, "VALUE") else {
        panic!("the healthy twin freezes a Settings record")
    };
    assert_eq!(fields.len(), 6);
    assert_eq!((&fields[0], &fields[3], &fields[4]), (&PreparedValue::Bool(false), &integer(4), &PreparedValue::Array([].into())));

    let gated = source.replace("data Phase [copy] {", "data Phase [copy] requires false {");
    authored_refusal(
        &gated,
        PreparationReason::OmittedFieldZeroNeedsProof,
        "the omitted field's zero value requires proof of the data predicate",
        "Settings{attempts:4,..}",
    );
    let oversized = source.replace("[u8;0]", "[u8;18446744073709551615]");
    authored_refusal(
        &oversized,
        PreparationReason::ZeroArrayNeedsStorage,
        "zero-filling an array requires checked preparation storage",
        "Settings{attempts:4,..}",
    );
}

#[test]
fn authored_inactive_payload_outside_the_fragment_refuses_the_constant_type() {
    let source = "data Choice [copy] {case Empty; case Used(view:u8);}
                  const VALUE:Choice=Choice::Empty;";
    assert!(matches!(prepared_constant(source, "VALUE"),
        PreparedValue::Sum { case, fields, .. } if case.path == "Choice::Empty" && fields.is_empty()));
    authored_refusal(
        &source.replace("view:u8", "view:@u8"),
        PreparationReason::ConstantTypeOutsideFragment,
        "the complete constant type is outside the prepared value fragment",
        "Choice::Empty",
    );
}

#[test]
fn authored_borrowed_array_elements_are_outside_the_fragment() {
    let machines = "machine identity<T>(x:T)->T{x}
        machine relay<T>(x:T)->T {let saved:T=x; identity::<T>(saved)}";
    let owned = format!("{machines}\nconst ARRAY:[u8;2]=relay::<[u8;2]>([3u8,7u8]);");
    assert_eq!(prepared_constant(&owned, "ARRAY"), PreparedValue::Array([integer(3), integer(7)].into()));
    let borrowed = format!("{machines}\nconst BORROWED:[@[u8];0]=relay::<[@[u8];0]>([]);");
    authored_refusal(
        &borrowed,
        PreparationReason::ArrayElementOutsideFragment,
        "the complete array element type is outside the prepared value fragment",
        "relay::<[@[u8];0]>([])",
    );
}

#[test]
fn authored_selected_array_read_past_the_end_is_refused_at_the_call() {
    let setup = "data Packet [copy] { byte:u8; }
        const ITEMS:[Packet;2]=[Packet{byte:4u8}, Packet{byte:9u8}];
        machine pick(items:[Packet;2], i:u64)->u8{items[i].byte}";
    let read = |index: u64| format!("{setup} const N:u8=pick(ITEMS,{index}u64);");
    assert_eq!(prepared_constant(&read(1), "N"), integer(9));
    authored_refusal(
        &read(2),
        PreparationReason::ArrayIndexOutOfBounds,
        "selected array read requires index < items.len",
        "pick(ITEMS,2u64)",
    );
}

/// The typer publishes the shape of a recursive sum once. Freezing must still
/// refuse it, even when the active case has no recursive payload.
#[test]
fn authored_recursive_by_value_sum_has_no_frozen_value() {
    let source = "data List [copy] {case Nil; case Cons(next:u8);}
                  const VALUE:List=List::Nil;";
    assert!(matches!(prepared_constant(source, "VALUE"),
        PreparedValue::Sum { case, fields, .. } if case.path == "List::Nil" && fields.is_empty()));
    authored_refusal(
        &source.replace("next:u8", "next:List"),
        PreparationReason::RecursiveByValueType,
        "a recursive by-value type has no supported frozen value",
        "List::Nil",
    );
}

/// A moved parameter is an owned record without a typed construction at the
/// moved occurrence. The refusal belongs to the invocation that returns it.
#[test]
fn authored_moved_record_has_no_supported_initializer() {
    let source = "data Key [copy]{count:u8;}
                  machine pass(value:Key)->Key{value}
                  const VALUE:Key=pass(Key{count:7});";
    assert!(matches!(prepared_constant(source, "VALUE"),
        PreparedValue::Record { owner, fields } if owner.path == "Key" && fields.as_ref() == [integer(7)]));
    authored_refusal(
        &source.replace("{value}", "{move value}"),
        PreparationReason::NominalWithoutInitializer,
        "the selected nominal value has no supported initializer",
        "pass(Key{count:7})",
    );
}

/// Stage input: the round checks a supplied establishment against the exact
/// fields its construction produced in this typing output.
#[test]
fn stage_input_establishment_for_other_fields_is_refused_at_the_construction() {
    let source = "data Checked [copy] requires true {amount:u32;}
                  const ALLOWED:Checked = Checked{amount:3};";
    let package = resolve(source);
    let round = PreparationRound::begin(&package, &[], None).unwrap();
    let TypingProgress::Complete(trees) = round.typing() else {
        panic!("no index demand keeps typing open")
    };
    let expected = RefusalCause::Preparation(PreparationReason::ConstructionNeedsProof);
    let text = "the selected constructor requires proof outside the prepared value fragment";
    assert_eq!(expected.text(), text);
    let message = format!("{ADMISSION}{text}");
    let site = last(source, "Checked{amount:3}");

    let missing = round
        .evaluate_with_cause(CheckedPreparationEvidence::default())
        .err()
        .expect("a constrained construction needs a supplied establishment");
    spec_corpus::assert_preparation_cause!(missing, expected, PACKAGE, FILE, site, message.clone());

    let other_fields = prove_literal_default_domain(trees, "ALLOWED", [integer(4)].into()).unwrap();
    let refused = round
        .evaluate_with_cause(CheckedPreparationEvidence::default().with_data_establishment(other_fields))
        .err()
        .expect("the establishment covers other field values");
    spec_corpus::assert_preparation_cause!(refused, expected, PACKAGE, FILE, site, message);

    let exact = prove_literal_default_domain(trees, "ALLOWED", [integer(3)].into()).unwrap();
    let evaluated = round
        .evaluate_with_cause(CheckedPreparationEvidence::default().with_data_establishment(exact))
        .unwrap();
    let PreparationStep::Finished(PreparationProgress::Complete(prepared)) = round.advance(evaluated).unwrap() else {
        panic!("the exact establishment completes preparation")
    };
    let declaration = &prepared.typed().constants[0].declaration;
    assert!(matches!(prepared.constant(declaration),
        Some(PreparedValue::Record { owner, fields }) if owner.path == "Checked" && fields.as_ref() == [integer(3)]));
}

#[test]
fn a_nested_omission_reports_the_literal_that_requested_the_zero() {
    let declarations = "data Part [copy] {value:u8;} data Inner [copy] {part:Part;} data Outer [copy] {inner:Inner;}";
    let source = format!("{declarations} const VALUE:Outer=Outer{{inner:Inner{{..}}}};");
    let PreparedValue::Record { fields, .. } = prepared_constant(&source, "VALUE") else { panic!("expected Outer") };
    let [PreparedValue::Record { fields, .. }] = fields.as_ref() else { panic!("expected Inner") };
    let [PreparedValue::Record { fields, .. }] = fields.as_ref() else { panic!("expected Part") };
    assert_eq!(fields.as_ref(), &[integer(0)]);
    authored_refusal(
        &source.replace("Part [copy] {", "Part [copy] requires false {"),
        PreparationReason::OmittedFieldZeroNeedsProof,
        "the omitted field's zero value requires proof of the data predicate",
        "Inner{..}",
    );
}
