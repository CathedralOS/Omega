//! `BindLifetime` has no Psi witness. L3 records it where a borrow lands in a
//! destination carrying an explicit region, and L4 prepares that package. The
//! first stop is L5 machine instancing: a machine whose only binder is a region
//! has no closed application, so it lowers neither as an instance nor as a
//! template. The region-free twin, with the same borrow occurrence, lowers and
//! returns that borrow. A locally constructed record with a region-bound
//! reference field also has no path: `machines/construction.rs` admits shared
//! reference fields only when the field has no lifetime.
#[path = "support/known_gap.rs"]
mod known_gap;
#[allow(dead_code)]
#[path = "obligation_outputs/pipeline.rs"]
mod pipeline;
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{OperationKind, OperationResult, ReturnEdge};
use omega_typed::{ObligationKind, ValueType};

#[test]
fn an_explicit_region_binding_stops_at_the_machine_that_owns_the_region() {
    let bound = "data ByteCell {value:u8;} machine forward<'a>(input:@'a ByteCell)->@'a u8 {@input.value}";
    let spelled = |site: &omega_lowering::Site| {
        std::str::from_utf8(site.span.slice(bound.as_bytes()).unwrap())
            .unwrap()
            .to_owned()
    };
    let resolved = sources::resolve(&[(pipeline::FILE, bound)]);
    let prepared = omega_preparation::prepare_source_package(&resolved).expect("typing and preparation accept the region binder");
    let bindings: Vec<_> = prepared
        .typed()
        .obligations
        .iter()
        .filter(|o| matches!(o.kind, ObligationKind::BindLifetime { .. }))
        .cloned()
        .collect();
    let [binding] = bindings.as_slice() else {
        panic!("expected one lifetime binding, found {bindings:?}")
    };
    assert_eq!(spelled(&binding.site), "@input.value");
    let ObligationKind::BindLifetime {
        reference,
        destination: ValueType::AtLifetime(inner, region),
    } = &binding.kind
    else {
        panic!("expected a binding into an explicit region, found {:?}", binding.kind)
    };
    assert_eq!(inner.as_ref(), reference);
    assert!(
        matches!(region.as_ref(), ValueType::Parameter { owner, index: 0 } if owner.path == "forward"),
        "{region:?}"
    );

    let error = sources::refuse_prepared(
        &prepared,
        &[(pipeline::FILE, bound)],
        pipeline::FILE,
        sources::unique_span(bound, "machine forward<'a>(input:@'a ByteCell)->@'a u8 {@input.value}"),
        "open generic machine requires a closed application before lowering",
    );
    assert!(
        error.site.span.get_start_offset() <= binding.site.span.get_start_offset() && binding.site.span.get_end_offset() <= error.site.span.get_end_offset(),
        "the refusal names the machine that encloses the binding: {}",
        spelled(&error.site)
    );
    known_gap::report(
        "returned borrow into an explicit machine region",
        "The machine needs its region binder in Psi and the BindLifetime at the returned borrow",
        bound,
        &error,
    );
    drop(prepared);
    drop(resolved);

    // Twin: the same borrow with the region elided owes no lifetime binding. It
    // lowers, and the borrow retained at that occurrence is the returned loan.
    let elided = bound.replace("<'a>", "").replace("@'a ", "@");
    let lowered = pipeline::lower(&elided);
    assert!(lowered.obligations("BindLifetime").is_empty(), "an elided region owes no binding");
    let loans = lowered.obligations("ReturnBorrow");
    let [returned] = loans.as_slice() else {
        panic!("expected one returned loan, found {loans:?}")
    };
    assert_eq!(lowered.spelling(&returned.site), "@input.value");
    let body = &lowered.machine("forward").body;
    let borrow = pipeline::operation_at(body, &returned.site);
    assert!(matches!(borrow.kind, OperationKind::BorrowPlace { .. }), "{:?}", borrow.kind);
    assert!(matches!(pipeline::returns(body).as_slice(), [ReturnEdge::Structural { source, .. }] if borrow.result == OperationResult::Structural(*source)));
}
