//! Formation and establishment obligations at the L5 boundary: `ZeroField`,
//! `EstablishData`, `EvaluateAccessPlan`, `PositiveWidth`,
//! `FormRecursiveApplication` and `RefineEvidence`.
//!
//! Each test locates the obligation in the prepared package by kind and source
//! site, lowers those same prepared trees through the shared helper, and compares
//! L5's stopping point with that site. No input carrying one of these kinds
//! reaches Psi yet, so every case pins a located refusal and reports it with
//! `known_gap`. A refusal is not evidence of output. Where a twin without the
//! obligation lowers, its Psi shows the operation at the same site that the
//! refused input lacks. Where both twins refuse, the stop does not depend on the
//! obligation at all.
//!
//! Earlier consumers handle part of these obligations and are tested there:
//! L3 rejects a closed zero width and an effect row that exceeds a restricted
//! trait. L4 freezes omitted constant fields to the typed zero and refuses
//! `EstablishData` in a constant without a checked establishment judgment.
//! When L5 lowers one of these inputs, replace its pin with a Psi comparison.
#[path = "support/known_gap.rs"]
mod known_gap;
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{LoweredPackage, LoweringError, MachineBody, OperationKind, PsiOperation, ScalarLiteral, ValueOperand, ValueOrigin};
use omega_preparation::{PreparedTrees, prepare_source_package};
use omega_typed::{ObligationKind, Site, TypingObligation};

/// Prepare one file, read the prepared package, then lower the same trees.
fn lower<T>(
    file: &str,
    source: &str,
    read: impl FnOnce(&PreparedTrees<'_>) -> T,
) -> (Vec<TypingObligation>, T, Result<LoweredPackage, LoweringError>) {
    let package = sources::resolve(&[(file, source)]);
    let prepared = prepare_source_package(&package).unwrap_or_else(|error| panic!("{file}: preparation failed: {error:?}\n{source}"));
    let obligations = prepared.typed().obligations.clone();
    let observed = read(&prepared);
    let lowered = sources::lower_prepared(&prepared, &[(file, source)]);
    (obligations, observed, lowered)
}

fn spelled<'s>(
    site: &Site,
    file: &str,
    source: &'s str,
) -> &'s str {
    assert_eq!((site.package.as_str(), site.file.as_str()), ("app", file));
    std::str::from_utf8(
        site.span
            .slice(source.as_bytes())
            .expect("site lies in the source"),
    )
    .unwrap()
}

/// Whether `site` lies in this file and spells `text`; never panics on another file's site.
fn spells(
    site: &Site,
    file: &str,
    source: &str,
    text: &str,
) -> bool {
    site.package == "app" && site.file == file && site.span.slice(source.as_bytes()) == Some(text.as_bytes())
}

fn encloses(
    outer: &Site,
    inner: &Site,
) -> bool {
    outer.file == inner.file && outer.span.get_start_offset() <= inner.span.get_start_offset() && inner.span.get_end_offset() <= outer.span.get_end_offset()
}

fn refused(
    lowered: Result<LoweredPackage, LoweringError>,
    source: &str,
) -> LoweringError {
    lowered.expect_err(&format!("L5 now lowers this input; replace the pin with a Psi comparison:\n{source}"))
}

/// The one operation of `machine` whose retained site spells `text`.
fn operation_spelled<'a>(
    package: &'a LoweredPackage,
    machine: &str,
    source: &str,
    text: &str,
) -> (&'a MachineBody, &'a PsiOperation) {
    let body = &package
        .machines
        .iter()
        .find(|m| m.declaration.path == machine)
        .unwrap_or_else(|| panic!("missing lowered machine {machine}"))
        .body;
    let found: Vec<_> = body
        .operations
        .iter()
        .filter(|op| {
            body.diagnostics.operations[op.id.index()]
                .span
                .slice(source.as_bytes())
                == Some(text.as_bytes())
        })
        .collect();
    let [operation] = found.as_slice() else {
        panic!("expected one operation at {text}, found {found:?}")
    };
    (body, *operation)
}

fn producer<'a>(
    body: &'a MachineBody,
    operand: &ValueOperand,
) -> &'a OperationKind {
    let ValueOperand::Scalar(value) = operand else {
        panic!("expected a scalar initializer, found {operand:?}")
    };
    let ValueOrigin::Operation(id) = body.values[value.index()].origin else {
        panic!("initializer {value:?} is not an operation result")
    };
    &body.operations[id.index()].kind
}

/// L3 hands the omitted `retries` field and its instantiated type to the checker
/// at the construction. In an executable body L5 stops at exactly that site. The
/// twin writes the zero and lowers, so the omission alone causes the stop. Its Psi
/// shows the record establishment with a zero in the `retries` slot that the
/// omitted form still needs.
#[test]
fn an_omitted_field_stops_at_its_construction_while_the_written_zero_lowers() {
    let file = "zero_field.omg";
    let omitted = "data Options {count:u8; retries:u16;} machine build(){let options=Options{count:1,..};}";
    let (obligations, (), lowered) = lower(file, omitted, |_| ());
    let zero: Vec<_> = obligations
        .iter()
        .filter_map(|o| match &o.kind {
            ObligationKind::ZeroField { field, ty } => Some((o, field, ty)),
            _ => None,
        })
        .collect();
    let [(obligation, field, ty)] = zero.as_slice() else {
        panic!("expected one omitted field, found {zero:?}")
    };
    assert_eq!(spelled(&obligation.site, file, omitted), "Options{count:1,..}");
    assert_eq!((field.path.as_str(), ty.to_string()), ("Options::retries", "u16".to_owned()));
    let error = refused(lowered, omitted);
    assert_eq!(error.site, obligation.site);
    assert_eq!(error.message, "construction requires additional establishment evidence");
    known_gap::report(
        "omitted field zero in executable construction",
        "The record construction needs the typed zero of Options::retries in its declared slot",
        omitted,
        &error,
    );

    let written = omitted.replace("count:1,..", "count:1,retries:0");
    let (obligations, (), lowered) = lower(file, &written, |_| ());
    assert!(
        !obligations
            .iter()
            .any(|o| matches!(o.kind, ObligationKind::ZeroField { .. }))
    );
    let package = lowered.unwrap_or_else(|error| panic!("the written zero lowers: {error:?}"));
    let (body, construction) = operation_spelled(&package, "build", &written, "Options{count:1,retries:0}");
    let OperationKind::EstablishRecord { fields } = &construction.kind else {
        panic!("expected a record establishment, found {construction:?}")
    };
    let [count, retries] = fields.as_slice() else {
        panic!("expected both declared slots, found {fields:?}")
    };
    assert_eq!([count.field.path.as_str(), retries.field.path.as_str()], ["Options::count", "Options::retries"]);
    assert_eq!(producer(body, &retries.operand), &OperationKind::Constant(ScalarLiteral::Integer(0.into())));
}

/// `EstablishData` sits on the construction of a predicate-bearing record and on a
/// bare case of a predicate-bearing sum. A value that satisfies the predicate and
/// one that violates it stop at the same site with the same message, so the stop is
/// not a verdict on the predicate. Removing the predicate removes the obligation and
/// the same construction lowers to its establishment operation.
#[test]
fn establishment_stops_at_its_construction_whether_or_not_the_value_satisfies_the_predicate() {
    let file = "establish.omg";
    let record = ("data Positive requires self.n > 0 {n:u8;}", "data Positive {n:u8;}", "Positive");
    let sum = ("data Mode requires true {case On; case Off;}", "data Mode {case On; case Off;}", "Mode");
    let mut reported = false;
    for ((declaration, plain, owner), construction) in [
        (record, "Positive{n:1}"),
        (record, "Positive{n:0}"),
        (sum, "Mode::On"),
    ] {
        let source = format!("{declaration} machine make(){{let value={construction};}}");
        let (obligations, (), lowered) = lower(file, &source, |_| ());
        let establish: Vec<_> = obligations
            .iter()
            .filter(|o| matches!(o.kind, ObligationKind::EstablishData(_)))
            .collect();
        let [obligation] = establish.as_slice() else {
            panic!("{construction}: expected one establishment, found {establish:?}")
        };
        assert!(
            matches!(&obligation.kind, ObligationKind::EstablishData(id) if id.path == owner),
            "{obligation:?}"
        );
        assert_eq!(spelled(&obligation.site, file, &source), construction);
        let error = refused(lowered, &source);
        assert_eq!(error.site, obligation.site, "{construction}");
        assert_eq!(error.message, "construction requires additional establishment evidence", "{construction}");
        if !reported {
            reported = true;
            known_gap::report(
                "establishment evidence in executable construction",
                "The construction needs the establishment of its declaration predicate for the checker",
                &source,
                &error,
            );
        }

        let source = source.replace(declaration, plain);
        let (obligations, (), lowered) = lower(file, &source, |_| ());
        assert!(
            !obligations
                .iter()
                .any(|o| matches!(o.kind, ObligationKind::EstablishData(_))),
            "{construction}"
        );
        let package = lowered.unwrap_or_else(|error| panic!("{construction}: the predicate-free twin lowers: {error:?}"));
        let (body, operation) = operation_spelled(&package, "make", &source, construction);
        match (&operation.kind, owner) {
            (OperationKind::EstablishRecord { fields }, "Positive") => {
                let [field] = fields.as_slice() else {
                    panic!("expected one slot, found {fields:?}")
                };
                assert_eq!(field.field.path, "Positive::n");
                let value = if construction.contains("n:1") { 1 } else { 0 };
                assert_eq!(producer(body, &field.operand), &OperationKind::Constant(ScalarLiteral::Integer(value.into())));
            }
            (OperationKind::EstablishCase { case, fields }, "Mode") => assert_eq!((case.path.as_str(), fields.len()), ("Mode::On", 0)),
            (other, _) => panic!("{construction}: unexpected establishment {other:?}"),
        }
    }
}

/// The domain's access clause carries the selected plan machine for evaluation.
/// L5 stops at that clause, the obligation's own site, before any declaration
/// could retain the domain, policy or plan machine.
#[test]
fn an_access_plan_stops_at_its_own_policy_clause() {
    let file = "access_plan.omg";
    let source = "use omega\\language\\core;
                  use omega\\language\\core\\reflect;
                  use omega\\language\\core\\access;
                  domain i32::Locked access Rule;
                  Rule satisfies AccessPolicy {
                      machine plan<'t>(schema:TypeSchema<'t>)->AccessPlan<'t> reaches none crashes none terminates {
                          new_plan::<'t>(@schema, Issuance::Unrestricted{})
                      }
                  }";
    let (obligations, (), lowered) = lower(file, source, |_| ());
    let plans: Vec<_> = obligations
        .iter()
        .filter(|o| matches!(o.kind, ObligationKind::EvaluateAccessPlan { .. }))
        .collect();
    let [obligation] = plans.as_slice() else {
        panic!("expected one access plan, found {plans:?}")
    };
    let ObligationKind::EvaluateAccessPlan { domain, machine } = &obligation.kind else {
        unreachable!()
    };
    assert_eq!((domain.path.as_str(), machine.path.as_str()), ("i32::Locked", "Rule::plan"));
    assert_eq!(spelled(&obligation.site, file, source), "access Rule");
    let error = refused(lowered, source);
    assert_eq!(error.site, obligation.site);
    assert_eq!(error.message, "unsupported domain member: Access");
    known_gap::report(
        "access plan evaluation at its domain",
        "The domain needs its policy, arguments and plan machine for evaluation",
        source,
        &error,
    );
}

/// L3 infers the closed application `sum<13>` and records positivity of the open
/// width at each `u<N>` occurrence in the template. L3 itself rejects a closed zero
/// width. L5 stops at the template's declaration while planning the instance, so
/// neither the closed application nor the open obligation reaches Psi.
#[test]
fn an_open_width_stops_at_its_closed_const_application() {
    let file = "width.omg";
    let source = "machine sum<const N:u64>(x:u<N>,y:u<N>)->u<N> requires N>0{x+%y} machine pair(x:u13,y:u13)->u13{sum(x,y)}";
    let (obligations, (template, arguments), lowered) = lower(file, source, |prepared| {
        let typed = prepared.typed();
        let template = typed
            .signatures
            .iter()
            .find(|signature| signature.machine.path == "sum")
            .expect("sum is typed")
            .site
            .clone();
        let call = typed
            .calls
            .iter()
            .find(|call| call.machine.path == "sum")
            .expect("pair calls sum");
        (
            template,
            call.type_arguments
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
        )
    });
    assert_eq!(arguments, ["13"]);
    let widths: Vec<_> = obligations
        .iter()
        .filter(|o| matches!(o.kind, ObligationKind::PositiveWidth(_)))
        .collect();
    assert!(
        widths
            .iter()
            .any(|o| spells(&o.site, file, source, "u<N>") && encloses(&template, &o.site)),
        "{widths:?}"
    );
    let error = refused(lowered, source);
    assert_eq!(error.site, template);
    assert_eq!(error.message, "open generic machine requires a closed application before lowering");
    known_gap::report(
        "open width of a closed const-integer machine application",
        "The call needs the closed instance sum<13> and the template's width requirement",
        source,
        &error,
    );
}

/// A recursive field that changes `Link<3>` to `Link<4>` hands the next application
/// to the verifier at the use. The twin keeps the same argument and carries no such
/// obligation. Both stop at the same `const` binder of the data declaration, before
/// either the use or the field type is read, so neither refusal is evidence about
/// the recursive formation or the field's width.
#[test]
fn recursive_formation_stops_at_the_const_binder_of_its_data() {
    let file = "recursive.omg";
    let changing = "data Link<const N:u64>{next:@Link<N+1>;value:u<N>;} machine caller(x:Link<3>){}";
    let same = changing.replace("@Link<N+1>", "@Link<N>");
    let binder = |error: &LoweringError, source: &str| {
        assert_eq!(error.message, "generic data requires a closed application before lowering");
        assert_eq!(spelled(&error.site, file, source), "<const N:u64>");
    };

    let (obligations, (), lowered) = lower(file, changing, |_| ());
    assert!(
        obligations
            .iter()
            .any(|o| matches!(&o.kind, ObligationKind::FormRecursiveApplication { declaration, arguments }
                if declaration.path == "Link" && arguments.iter().map(ToString::to_string).eq(["4"]))
                && spells(&o.site, file, changing, "Link<3>")),
        "{obligations:?}"
    );
    assert!(
        obligations
            .iter()
            .any(|o| matches!(o.kind, ObligationKind::PositiveWidth(_)) && spells(&o.site, file, changing, "u<N>")),
        "{obligations:?}"
    );
    let error = refused(lowered, changing);
    binder(&error, changing);
    known_gap::report(
        "recursive formation of const-indexed data",
        "The use needs Link<3> and the next application Link<4> handed to the verifier",
        changing,
        &error,
    );

    let (obligations, (), lowered) = lower(file, &same, |_| ());
    assert!(
        !obligations
            .iter()
            .any(|o| matches!(o.kind, ObligationKind::FormRecursiveApplication { .. })),
        "{obligations:?}"
    );
    binder(&refused(lowered, &same), &same);
}

/// Selecting `Quiet` for a binder bounded by the restricted alias `Local` records
/// `RefineEvidence` at the call. L3 already checked Quiet's rows against the
/// restriction's effect ceilings. L5 stops earlier, at the alias declaration that
/// names the restricted trait, so the call carrying the evidence is never lowered.
#[test]
fn refined_evidence_stops_at_the_restricted_trait_alias() {
    let file = "refine.omg";
    let source = "trait Log{machine put() blocks;} trait Local=Log{machine * blocks false reaches none terminates;} Quiet satisfies Log{machine put(){}} Busy satisfies Log{machine put() blocks {}} machine use_it<const E:Local>(){E::put()} machine f(){use_it::<Quiet>()}";
    let (obligations, (), lowered) = lower(file, source, |_| ());
    let refinements: Vec<_> = obligations
        .iter()
        .filter(|o| matches!(o.kind, ObligationKind::RefineEvidence { .. }))
        .collect();
    let [obligation] = refinements.as_slice() else {
        panic!("expected one evidence refinement, found {refinements:?}")
    };
    let ObligationKind::RefineEvidence { binder, implementation } = &obligation.kind else {
        unreachable!()
    };
    assert_eq!((binder.declaration.path.as_str(), implementation.path.as_str()), ("Local", "Quiet"));
    assert_eq!(spelled(&obligation.site, file, source), "use_it::<Quiet>()");
    let error = refused(lowered, source);
    assert_eq!(error.message, "trait binders, inheritance and restrictions require an applied trait interface");
    assert_eq!(spelled(&error.site, file, source), "Log");
    assert_eq!(error.site.span.get_start_offset(), source.find("=Log").unwrap() + 1);
    known_gap::report(
        "refined evidence through a restricted trait alias",
        "The call needs the restricted trait Local and the evidence that Quiet refines it",
        source,
        &error,
    );
}
