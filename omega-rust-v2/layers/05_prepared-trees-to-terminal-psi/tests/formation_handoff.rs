//! Formation and establishment obligations at the L5 boundary: `ZeroField`,
//! `EstablishData`, `EvaluateAccessPlan`, `PositiveWidth`,
//! `FormRecursiveApplication` and `RefineEvidence`.
//!
//! Each test locates the obligation in the prepared package by kind and source
//! site, lowers those same prepared trees through the shared helper, and compares
//! L5's output or stopping point with that site. A plain scalar `ZeroField`
//! supplies the SSA operand for the omitted declaration slot. Other formations
//! below still stop before Psi. A refusal is not evidence of output. Where a twin without the
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

use std::ops::Range;

/// Read the obligations and run the requested check on the same prepared trees.
fn with_prepared<T>(
    file: &str,
    source: &str,
    read: impl FnOnce(&PreparedTrees<'_>) -> T,
) -> (Vec<TypingObligation>, T) {
    let package = sources::resolve(&[(file, source)]);
    let prepared = prepare_source_package(&package).unwrap_or_else(|error| panic!("{file}: preparation failed: {error:?}\n{source}"));
    let obligations = prepared.typed().obligations.clone();
    (obligations, read(&prepared))
}

fn refusal(file: &str, source: &str, span: Range<usize>, message: &str) -> (Vec<TypingObligation>, LoweringError) {
    with_prepared(file, source, |prepared| sources::refuse_prepared(prepared, &[(file, source)], file, span, message))
}

fn lower(file: &str, source: &str) -> (Vec<TypingObligation>, Result<LoweredPackage, LoweringError>) {
    with_prepared(file, source, |prepared| sources::lower_prepared(prepared, &[(file, source)]))
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

#[test]
fn zero_field_supplies_the_exact_declared_slot_at_the_construction() {
    use omega_lowering::{OperationResult, ScalarType, TypeShape};
    let file = "zero_field.omg";
    let omitted = "data Options {count:u8; retries:u16;} machine build(){let options=Options{count:1,..};}";
    let other = omitted.replace("retries:u16", "timeout:u32");
    let same = omitted.replace("count:u8", "count:u16");
    let swapped = same.replace("count:1", "retries:1");
    for (source, names, widths, slot, literal) in [
        (omitted, ["Options::count", "Options::retries"], [8, 16], 1, "Options{count:1,..}"),
        (other.as_str(), ["Options::count", "Options::timeout"], [8, 32], 1, "Options{count:1,..}"),
        (same.as_str(), ["Options::count", "Options::retries"], [16, 16], 1, "Options{count:1,..}"),
        (swapped.as_str(), ["Options::count", "Options::retries"], [16, 16], 0, "Options{retries:1,..}"),
    ] {
        let (_, (site, lowered)) = with_prepared(file, source, |prepared| {
            let tree = prepared.typed().trees().next().unwrap();
            let zeros: Vec<_> = prepared.typed().obligations.iter().filter(|o| matches!(o.kind, ObligationKind::ZeroField { .. })).collect();
            let [obligation] = zeros.as_slice() else { panic!("one ZeroField expected, found {zeros:?}") };
            let node = obligation.node.node;
            let construction = tree.construction(node).expect("obligation on the constructor");
            let [written] = construction.initializers.as_slice() else { panic!("one authored operand") };
            assert_eq!(written.position, 1 - slot);
            assert_eq!(tree.resolved().file.span(written.expression).slice(source.as_bytes()), Some(b"1".as_slice()));
            let declaration = &prepared.typed().record_fields[&construction.owner][slot];
            assert!(matches!(&obligation.kind, ObligationKind::ZeroField {field, ty} if field == &declaration.declaration && ty == &declaration.ty
                && field.path == names[slot] && ty.to_string() == format!("u{}", widths[slot])));
            (obligation.site.clone(), sources::lower_prepared(prepared, &[(file, source)]).unwrap())
        });
        assert_eq!(spelled(&site, file, source), literal);
        if source == omitted || source == other {
            assert_eq!((site.span.get_start_offset(), site.span.get_end_offset()), (66, 85));
        }
        let body = &lowered.machines.iter().find(|m| m.declaration.path == "build").unwrap().body;
        let [one, zero, establish] = body.operations.as_slice() else { panic!("two operands followed by one construction") };
        assert_eq!(one.kind, OperationKind::Constant(ScalarLiteral::Integer(1.into())));
        assert_eq!(spelled(&body.diagnostics.operations[one.id.index()], file, source), "1");
        assert_eq!(zero.kind, OperationKind::Constant(ScalarLiteral::Integer(0.into())));
        assert_eq!(body.diagnostics.operations[zero.id.index()], site);
        assert_eq!(body.diagnostics.operations[establish.id.index()], site);
        let OperationKind::EstablishRecord {fields} = &establish.kind else { panic!("expected a record") };
        let OperationResult::Structural(home) = establish.result else { panic!("expected fresh storage") };
        let schema = lowered.data.iter().find(|d| d.carrier == body.places[home.index()].ty).unwrap();
        assert_eq!(schema.declaration.path, "Options");
        assert_eq!(fields.len(), 2);
        let OperationResult::Scalar(zero_value) = zero.result else { panic!("zero must define an SSA value") };
        if source == omitted {
            spec_corpus::observe_handoff!(
                psi "psi-output/omitted-field-zero",
                assert_eq!((fields[1].field.path.as_str(), &fields[1].operand), ("Options::retries", &ValueOperand::Scalar(zero_value))),
            );
        } else if source == other {
            spec_corpus::observe_handoff!(
                psi "psi-output/omitted-timeout-zero",
                assert_eq!(
                    (fields[1].field.path.as_str(), &fields[1].operand, lowered.types.get(body.values[zero_value.index()].ty)),
                    ("Options::timeout", &ValueOperand::Scalar(zero_value), Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 32 }))),
                ),
            );
        }
        for (i, f) in fields.iter().enumerate() {
            assert_eq!(f.field, schema.fields[i].declaration);
            assert_eq!(f.field.path, names[i]);
            let ValueOperand::Scalar(v) = f.operand else { panic!("expected a scalar initializer") };
            let backing = if i == slot { zero } else { one };
            assert_eq!(OperationResult::Scalar(v), backing.result);
            assert_eq!(body.values[v.index()].origin, ValueOrigin::Operation(backing.id));
            assert_eq!(body.values[v.index()].ty, schema.fields[i].carrier);
            assert_eq!(lowered.types.get(schema.fields[i].carrier), Some(&TypeShape::PrimitiveScalar(ScalarType::Integer {signed:false, width:widths[i]})));
        }
    }

    let written = omitted.replace("count:1,..", "count:1,retries:0");
    let (obligations, lowered) = lower(file, &written);
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
    assert_eq!(producer(body, &count.operand), &OperationKind::Constant(ScalarLiteral::Integer(1.into())));
    assert_eq!(producer(body, &retries.operand), &OperationKind::Constant(ScalarLiteral::Integer(0.into())));
    let (_, written_zero) = operation_spelled(&package, "build", &written, "0");
    assert!(matches!(written_zero.result, OperationResult::Scalar(v) if retries.operand == ValueOperand::Scalar(v)));
}

#[test]
fn omitted_payload_preserves_authored_calls_and_selects_only_the_active_case() {
    use omega_lowering::{CallTarget, OperationResult, ScalarType, TypeShape};
    let file = "case_zero.omg";
    let source = "data Choice{case First(x:u8,flag:bool,y:u8); case Other(flag:bool);}
        machine left()->u8{1} machine right()->u8{2}
        machine build(){let choice=Choice::First{y:right(),x:left(),..};}";
    let (obligations, lowered) = lower(file, source);
    let zeros: Vec<_> = obligations.iter().filter(|o| matches!(&o.kind, ObligationKind::ZeroField {..})).collect();
    let [zero] = zeros.as_slice() else { panic!("one omitted payload field") };
    assert!(matches!(&zero.kind, ObligationKind::ZeroField {field, ty: omega_typed::ValueType::Bool} if field.path == "Choice::First::flag"));
    assert_eq!(spelled(&zero.site, file, source), "Choice::First{y:right(),x:left(),..}");
    let package = lowered.unwrap();
    let machine = |path| package.machines.iter().find(|m| m.declaration.path == path).unwrap();
    let body = &machine("build").body;
    let [right, left, flag, construct] = body.operations.as_slice() else { panic!("two calls, one zero, one constructor") };
    for (op, target, text) in [(right, "right", "right()"), (left, "left", "left()")] {
        assert!(matches!(op.kind, OperationKind::Call {callee: CallTarget::Machine(id), ..} if id == machine(target).id));
        assert_eq!(spelled(&body.diagnostics.operations[op.id.index()], file, source), text);
    }
    assert_eq!(flag.kind, OperationKind::Constant(ScalarLiteral::Boolean(false)));
    assert_eq!(body.diagnostics.operations[flag.id.index()], zero.site);
    assert_eq!(body.diagnostics.operations[construct.id.index()], zero.site);
    let OperationKind::EstablishCase {case, fields} = &construct.kind else { panic!("expected case construction") };
    assert_eq!(case.path, "Choice::First");
    let OperationResult::Structural(home) = construct.result else { panic!("expected a constructed Choice") };
    let schema = package.data.iter().find(|d| d.declaration.path == "Choice").unwrap();
    assert_eq!(body.places[home.index()].ty, schema.carrier);
    let active = schema.cases.iter().find(|c| &c.declaration == case).expect("case belongs to Choice");
    assert_eq!((fields.len(), active.fields.len()), (3, 3));
    assert_eq!(package.types.get(active.fields[1].carrier), Some(&TypeShape::PrimitiveScalar(ScalarType::Boolean)));
    let expected = [("Choice::First::x", left), ("Choice::First::flag", flag), ("Choice::First::y", right)];
    for ((field, declared), (path, backing)) in fields.iter().zip(&active.fields).zip(expected) {
        assert_eq!(field.field.path, path);
        assert_eq!(field.field, declared.declaration);
        let ValueOperand::Scalar(v) = field.operand else { panic!("expected scalar input for {path}") };
        assert_eq!(OperationResult::Scalar(v), backing.result);
        assert_eq!(body.values[v.index()].ty, declared.carrier, "carrier for {path}");
    }
}

#[test]
fn zero_field_cannot_supply_a_reference_or_discharge_an_independent_predicate() {
    let file = "zero_field.omg";
    for (declaration, reason) in [
        ("data Options{count:u8;handle:@u8;}", "cannot lower implicit zero of type `@u8` for omitted field `Options::handle`"),
        ("data Options requires self.n>0 {count:u8;n:u8;}", "lowering cannot establish the data predicate for `Options` at construction"),
    ] {
        let literal = "Options{count:1,..}";
        let source = format!("{declaration} machine build(){{let options={literal};}}");
        let (obligations, error) = refusal(file, &source, sources::unique_span(&source, literal), reason);
        assert!(obligations.iter().any(|o| matches!(o.kind, ObligationKind::ZeroField {..}) && o.site == error.site));
        if declaration.contains("requires") {
            assert!(obligations.iter().any(|o| matches!(&o.kind, ObligationKind::EstablishData(id) if id.path == "Options") && o.site == error.site));
        }
    }
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
        let (obligations, error) = refusal(
            file, &source, sources::unique_span(&source, construction),
            &format!("lowering cannot establish the data predicate for `{owner}` at construction"),
        );
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
        assert_eq!(error.site, obligation.site, "{construction}");
        assert_eq!(error.message, format!("lowering cannot establish the data predicate for `{owner}` at construction"), "{construction}");
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
        let (obligations, lowered) = lower(file, &source);
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
    let (obligations, error) = refusal(file, source, sources::unique_span(source, "access Rule"), "unsupported domain member: Access");
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
    let (obligations, (template, arguments, error)) = with_prepared(file, source, |prepared| {
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
            sources::refuse_prepared(
                prepared, &[(file, source)], file,
                sources::unique_span(source, "machine sum<const N:u64>(x:u<N>,y:u<N>)->u<N> requires N>0{x+%y}"),
                "open generic machine requires a closed application before lowering",
            ),
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

    let at_binder = |source: &str| refusal(
        file, source, sources::unique_span(source, "<const N:u64>"),
        "generic data requires a closed application before lowering",
    );
    let (obligations, error) = at_binder(changing);
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
    binder(&error, changing);
    known_gap::report(
        "recursive formation of const-indexed data",
        "The use needs Link<3> and the next application Link<4> handed to the verifier",
        changing,
        &error,
    );

    let (obligations, error) = at_binder(&same);
    assert!(
        !obligations
            .iter()
            .any(|o| matches!(o.kind, ObligationKind::FormRecursiveApplication { .. })),
        "{obligations:?}"
    );
    binder(&error, &same);
}

/// Selecting `Quiet` for a binder bounded by the restricted alias `Local` records
/// `RefineEvidence` at the call. L3 already checked Quiet's rows against the
/// restriction's effect ceilings. L5 stops earlier, at the alias declaration that
/// names the restricted trait, so the call carrying the evidence is never lowered.
#[test]
fn refined_evidence_stops_at_the_restricted_trait_alias() {
    let file = "refine.omg";
    let source = "trait Log{machine put() blocks;} trait Local=Log{machine * blocks false reaches none terminates;} Quiet satisfies Log{machine put(){}} Busy satisfies Log{machine put() blocks {}} machine use_it<const E:Local>(){E::put()} machine f(){use_it::<Quiet>()}";
    let (obligations, error) = refusal(
        file, source, sources::span_within(source, "trait Local=Log", "Log"),
        "trait binders, inheritance and restrictions require an applied trait interface",
    );
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
