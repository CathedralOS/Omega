#[allow(dead_code)]
#[path = "support/source.rs"]
mod support;

use omega_preparation::{PreparationFailure, prepare_source_package_diagnosed};
use omega_resolved::{NameRole, Symbol};
use omega_typer::{TypingProgress, type_source_package_for_preparation};

const NO_MATCH: &str = "no visible domain on the written carrier";

#[test]
fn ordinary_head_selects_on_the_written_carrier_not_the_signature_or_call() {
    for (domains, root, count, valid) in [
        ("domain u<8>::Ready;", "u8", 1, true),
        ("domain u<16>::Ready;", "u8", 1, false),
        ("domain u<8>::Ready; domain u<16>::Ready;", "u16", 2, true),
        ("domain u<8>::Ready; domain u<16>::Ready;", "u32", 2, false),
    ] {
        let qualifier = format!("{root}::Ready");
        let path = format!("{qualifier}::read");
        let source = format!("machine {path}()->u8{{1u8}} machine caller()->u8{{{path}()}}");
        let package = support::resolve(&[("defs.omg", domains), ("use.omg", &source)]);
        let tree = package.trees().find(|t| t.file.path == "use.omg").unwrap();
        let head = tree.occurrences().find(|name| name.role == NameRole::Qualifier && name.span.get_start_offset() == "machine ".len()).unwrap();
        assert_eq!(head.span.slice(source.as_bytes()), Some(qualifier.as_bytes()));
        let Symbol::DomainCandidates { candidates, .. } = &head.target else { panic!("lost the carrier evidence") };
        assert_eq!(candidates.len(), count);
        assert!(candidates.iter().all(|c| c.carrier_deferred));

        let result = prepare_source_package_diagnosed(&package);
        if valid {
            let prepared = result.unwrap();
            let typed = prepared.typed();
            let read = typed.signatures.iter().find(|s| s.machine.path == path).unwrap();
            assert!(read.operator.is_none());
            assert!(typed.calls.iter().any(|c| c.machine == read.machine));
            omega_lowering::lower_package(&prepared).unwrap();
        } else {
            let PreparationFailure::Typing(error) = result.unwrap_err() else { panic!("expected initial L3 refusal") };
            assert_eq!(error.message, NO_MATCH);
            assert_eq!(error.file, "use.omg");
            assert_eq!(error.span, head.span);
        }
    }
}

#[test]
fn computed_domain_carrier_defers_the_ordinary_signature_until_retyping() {
    for width in [8, 16] {
        let source = format!("machine size()->u64{{{width}u64}} const SIZE:u64=size(); domain u<SIZE>::Ready; \
            machine u8::Ready::read()->u8{{1u8}} machine caller()->u8{{u8::Ready::read()}}");
        let package = support::resolve(&[("late.omg", &source)]);
        let TypingProgress::Pending(first) = type_source_package_for_preparation(&package, &Default::default()).unwrap() else {
            panic!("the machine's qualifier needs the computed width")
        };
        let [demand] = first.demands() else { panic!("expected just the domain width") };
        assert_eq!(demand.request().expression.span.slice(source.as_bytes()), Some(b"SIZE".as_slice()));
        assert!(!first.trees().signatures.iter().any(|s| s.machine.path == "u8::Ready::read"));
        assert!(!first.trees().calls.iter().any(|c| c.machine.path == "u8::Ready::read"));
        let result = prepare_source_package_diagnosed(&package);
        if width == 8 {
            omega_lowering::lower_package(&result.unwrap()).unwrap();
        } else {
            let PreparationFailure::ResumedTyping(error) = result.unwrap_err() else { panic!("expected resumed L3 refusal") };
            assert_eq!(error.message, NO_MATCH);
            assert_eq!(error.file, "late.omg");
            assert_eq!(error.span.get_start_offset(), source.find("u8::Ready").unwrap());
            assert_eq!(error.span.slice(source.as_bytes()), Some(b"u8::Ready".as_slice()));
        }
    }
}

#[test]
fn a_matching_import_cannot_win_before_a_competing_width_is_known() {
    let source = "use east; use west; machine u8::Ready::read()->u8{1u8}";
    let package = support::resolve(&[
        ("main.omg", source),
        ("east/defs.omg", "public domain u<8>::Ready;"),
        ("west/defs.omg", "public machine size()->u64 reaches none crashes none terminates {8u64} public const SIZE:u64=size(); public domain u<SIZE>::Ready;"),
    ]);
    let failure = prepare_source_package_diagnosed(&package).unwrap_err();
    let PreparationFailure::ResumedTyping(error) = failure else {
        panic!("both candidates match once the width is known: {failure:?}")
    };
    assert_eq!(error.file, "main.omg");
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"u8::Ready".as_slice()));
    assert!(error.message.starts_with(r"competing declarations: app\east\u::Ready, app\west\u::Ready"), "{}", error.message);
}

#[test]
fn module_and_named_implementation_still_qualify_ordinary_machines() {
    let source = r"trait Logger { machine write(); } Sink satisfies Logger { Logger::write = Sink::write; }
        machine Sink::write() {} machine combat\score() {} machine caller() { combat\score() }";
    let package = support::resolve(&[("combat/other.omg", "machine other() {}"), ("main.omg", source)]);
    let prepared = prepare_source_package_diagnosed(&package).unwrap();
    omega_lowering::lower_package(&prepared).unwrap();
}
