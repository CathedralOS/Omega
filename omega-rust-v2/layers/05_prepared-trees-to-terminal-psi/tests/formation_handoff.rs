//! Formation and establishment obligations at the L5 boundary: `ZeroField`,
//! `EstablishData`, `EvaluateAccessPlan`, `PositiveWidth`,
//! `FormRecursiveApplication` and `RefineEvidence`.
//!
//! Each test locates the obligation in the prepared package by kind and source
//! site, lowers those same prepared trees through the shared helper, and compares
//! L5's output or stopping point with that site. A plain scalar `ZeroField`
//! supplies the SSA operand for the omitted declaration slot; `EstablishData`
//! joins the constructor operands to the result schema's goals. A closed machine
//! retains the integer argument that each direct width formation must establish.
//! The indexed record preserves its open and closed formation questions, and a
//! closed restricted evidence argument preserves the bound and dispatch. Other
//! inputs below still stop before Psi. A refusal is not evidence of output. Where a twin without the
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

/// `..` supplies no borrowed reference, so an omitted `@u8` field makes the
/// construction invalid no matter what a checker could prove: no zero of that
/// type borrows nothing. Changing only the field type to `u8` lets the same
/// literal lower with the zero in the `handle` slot.
#[test]
fn zero_field_cannot_supply_a_reference() {
    let file = "zero_field.omg";
    let literal = "Options{count:1,..}";
    let source = "data Options{count:u8;handle:@u8;} machine build(){let options=Options{count:1,..};}";
    let (obligations, error) = refusal(file, source, sources::unique_span(source, literal),
        "omitted field `Options::handle` of type `@u8` needs a borrowed reference, which `..` never supplies");
    assert!(obligations.iter().any(|o| matches!(o.kind, ObligationKind::ZeroField {..}) && o.site == error.site));
    assert!(
        obligations
            .iter()
            .any(|o| matches!(&o.kind, ObligationKind::ZeroField {field, ty: omega_typed::ValueType::Reference(..)} if field.path == "Options::handle"))
    );
    let scalar = source.replace("handle:@u8", "handle:u8");
    let (_, lowered) = lower(file, &scalar);
    let package = lowered.unwrap_or_else(|error| panic!("the scalar twin lowers: {error:?}"));
    let body = &package
        .machines
        .iter()
        .find(|m| m.declaration.path == "build")
        .unwrap()
        .body;
    // The zero and the record both report the literal's site; select the record.
    let records: Vec<_> = body
        .operations
        .iter()
        .filter_map(|op| match &op.kind {
            OperationKind::EstablishRecord { fields } if spells(&body.diagnostics.operations[op.id.index()], file, &scalar, literal) => Some(fields),
            _ => None,
        })
        .collect();
    let [fields] = records.as_slice() else {
        panic!("expected one record establishment at {literal}, found {records:?}")
    };
    let [count, handle] = fields.as_slice() else {
        panic!("expected both declared slots, found {fields:?}")
    };
    assert_eq!([count.field.path.as_str(), handle.field.path.as_str()], ["Options::count", "Options::handle"]);
    assert_eq!(producer(body, &handle.operand), &OperationKind::Constant(ScalarLiteral::Integer(0.into())));
}

fn result_schema<'a>(package: &'a LoweredPackage, body: &MachineBody, op: &PsiOperation) -> &'a omega_lowering::LoweredData {
    let omega_lowering::OperationResult::Structural(home) = op.result else { panic!("expected structural result") };
    let carrier = body.places[home.index()].ty;
    let Some(omega_lowering::TypeShape::Nominal(id)) = package.types.get(carrier) else { panic!("expected nominal result") };
    let schema = &package.data[id.get() - 1];
    assert_eq!(schema.carrier, carrier);
    schema
}

#[test]
fn omitted_operand_is_the_subject_of_the_independent_declaration_goal() {
    use omega_lowering::{PredicateOperator, PredicateStep};
    let source = "data Options requires self.n>0 {count:u8;n:u8;} machine build(){let options=Options{count:1,..};}";
    // Same constructor, operands and carriers; change only which slot `self` selects.
    for (source, slot) in [(source.to_owned(), 1), (source.replace("self.n", "self.count"), 0)] {
        let (obligations, product) = lower("zero_field.omg", &source);
        let package = product.unwrap();
        let body = &package.machines[0].body;
        let [one, zero, op] = body.operations.as_slice() else { panic!("written, omitted and constructed") };
        let site = &body.diagnostics.operations[op.id.index()];
        assert_eq!(spelled(site, "zero_field.omg", &source), "Options{count:1,..}");
        assert!(obligations.iter().any(|o| &o.site == site && matches!(&o.kind, ObligationKind::EstablishData(id) if id.path == "Options")));
        assert!(obligations.iter().any(|o| &o.site == site && matches!(&o.kind, ObligationKind::ZeroField {field,..} if field.path == "Options::n")));
        let schema = result_schema(&package, body, op);
        let OperationKind::EstablishRecord { fields } = &op.kind else { panic!("expected record") };
        let [count, n] = fields.as_slice() else { panic!("expected two slots") };
        assert_eq!((&count.field, &n.field), (&schema.fields[0].declaration, &schema.fields[1].declaration));
        assert_eq!((count.field.path.as_str(), n.field.path.as_str()), ("Options::count", "Options::n"));
        assert_eq!(producer(body, &count.operand), &one.kind);
        assert_eq!(producer(body, &n.operand), &zero.kind);
        assert_eq!(one.kind, OperationKind::Constant(ScalarLiteral::Integer(1.into())));
        assert_eq!(zero.kind, OperationKind::Constant(ScalarLiteral::Integer(0.into())));
        let [goal] = schema.requires.as_slice() else { panic!("expected one goal") };
        let expected = [PredicateStep::SelfValue, PredicateStep::Field(fields[slot].field.clone()), PredicateStep::Integer(0.into()), PredicateStep::Apply(PredicateOperator::GreaterThan)];
        if slot == 1 {
            spec_corpus::observe_handoff!(psi "psi-output/omitted-establishment-goal",
                assert_eq!((&goal.predicate[..], producer(body, &fields[slot].operand)), (&expected[..], &OperationKind::Constant(ScalarLiteral::Integer(0.into())))),
            );
        } else {
            assert_eq!(goal.predicate, expected);
        }
    }
}

/// Both literal values owe the same goal; the checker receives different actuals.
#[test]
fn executable_constructions_select_their_own_default_domain_goals() {
    use omega_lowering::{PredicateOperator, PredicateStep};
    let file = "establish.omg";
    let record = ("data Positive requires self.n > 0 {n:u8;}", "data Positive {n:u8;}", "Positive");
    let sum = ("data Mode requires true {case On; case Off;}", "data Mode {case On; case Off;}", "Mode");
    for ((declaration, plain, owner), construction) in [
        (record, "Positive{n:1}"),
        (record, "Positive{n:0}"),
        (sum, "Mode::On"),
        (sum, "Mode::Off"),
    ] {
        let source = format!("{declaration} machine make(){{let value={construction};}}");
        let (obligations, product) = lower(file, &source);
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
        let package = product.unwrap();
        let (body, op) = operation_spelled(&package, "make", &source, construction);
        assert_eq!(body.diagnostics.operations[op.id.index()], obligation.site);
        let schema = result_schema(&package, body, op);
        assert_eq!(schema.declaration.path, owner);
        let [goal] = schema.requires.as_slice() else { panic!("expected one establishment goal") };
        match &op.kind {
            OperationKind::EstablishRecord { fields } => {
                let [field] = fields.as_slice() else { panic!("one field operand") };
                assert_eq!(field.field, schema.fields[0].declaration);
                assert_eq!(field.field.path, "Positive::n");
                let predicate = [PredicateStep::SelfValue, PredicateStep::Field(field.field.clone()), PredicateStep::Integer(0.into()), PredicateStep::Apply(PredicateOperator::GreaterThan)];
                if construction == "Positive{n:1}" {
                    spec_corpus::observe_handoff!(psi "psi-output/establish-positive-one",
                        assert_eq!((&goal.predicate[..], producer(body, &field.operand)), (&predicate[..], &OperationKind::Constant(ScalarLiteral::Integer(1.into())))),
                    );
                } else {
                    spec_corpus::observe_handoff!(psi "psi-output/establish-positive-zero",
                        assert_eq!((&goal.predicate[..], producer(body, &field.operand)), (&predicate[..], &OperationKind::Constant(ScalarLiteral::Integer(0.into())))),
                    );
                }
            }
            OperationKind::EstablishCase { case, fields } => {
                assert!(fields.is_empty());
                assert_eq!(case.path, construction);
                if construction == "Mode::On" {
                    spec_corpus::observe_handoff!(psi "psi-output/establish-mode-on",
                        assert_eq!((case, &goal.predicate[..]), (&schema.cases[0].declaration, &[PredicateStep::Boolean(true)][..])),
                    );
                } else {
                    assert_eq!(case, &schema.cases[1].declaration);
                    assert_eq!(goal.predicate, [PredicateStep::Boolean(true)]);
                }
            }
            other => panic!("unexpected constructor {other:?}"),
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
        assert!(result_schema(&package, body, operation).requires.is_empty());
        match (&operation.kind, owner) {
            (OperationKind::EstablishRecord { fields }, "Positive") => {
                let [field] = fields.as_slice() else {
                    panic!("expected one slot, found {fields:?}")
                };
                assert_eq!(field.field.path, "Positive::n");
                let value = if construction.contains("n:1") { 1 } else { 0 };
                assert_eq!(producer(body, &field.operand), &OperationKind::Constant(ScalarLiteral::Integer(value.into())));
            }
            (OperationKind::EstablishCase { case, fields }, "Mode") => assert_eq!((case.path.as_str(), fields.len()), (construction, 0)),
            (other, _) => panic!("{construction}: unexpected establishment {other:?}"),
        }
    }
}

const ACCESS_PLAN: &str = "use omega\\language\\core;
                  use omega\\language\\core\\reflect;
                  use omega\\language\\core\\access;
                  domain i32::Locked access Rule;
                  Rule satisfies AccessPolicy {
                      machine plan<'t>(schema:TypeSchema<'t>)->AccessPlan<'t> reaches none crashes none terminates {
                          new_plan::<'t>(@schema, Issuance::Unrestricted{})
                      }
                  }";

/// The domain's access clause carries the selected plan machine for evaluation.
/// L5 stops at that clause before it publishes a governed domain.
#[test]
fn an_access_plan_stops_at_its_own_policy_clause() {
    let file = "access_plan.omg";
    let source = ACCESS_PLAN;
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
        "Typing selects Rule::plan, but preparation has not supplied its evaluated result on the domain schema",
        source,
        &error,
    );
}

/// The same evaluation request can have two different issuance operands. Neither
/// the request nor the imported constructor's postconditions are its result.
#[test]
fn issuance_is_an_operand_of_the_library_call_not_a_fact_of_policy_selection() {
    let only = ACCESS_PLAN.replace("Unrestricted{}", "Only{..}");
    // The exact bytes of target/l5owner/policy/empty/access_plan.omg.
    let empty = ACCESS_PLAN.replace("Unrestricted{}", "Only{routes: []}");
    for (source, case) in [
        (ACCESS_PLAN, "Issuance::Unrestricted"),
        (only.as_str(), "Issuance::Only"),
        (empty.as_str(), "Issuance::Only"),
    ] {
        with_prepared("policy.omg", source, |prepared| {
            let typed = prepared.typed();
            let [selection] = typed.access_policies.as_slice() else {
                panic!("expected one access selection")
            };
            assert_eq!(spelled(&selection.site, "policy.omg", source), "access Rule");
            assert_eq!(
                (selection.domain.path.as_str(), selection.policy.path.as_str(), selection.plan_machine.path.as_str()),
                ("i32::Locked", "Rule", "Rule::plan")
            );
            assert!(selection.arguments.is_empty());
            let core = omega_toolchain::core_interface();
            let row = typed
                .conformance_rows
                .iter()
                .find(|row| row.conformance.as_ref() == Some(&selection.policy) && &row.requirement.declaration == core.access_plan_requirement())
                .unwrap();
            assert_eq!(row.implementation, selection.plan_machine);
            let calls: Vec<_> = typed.calls.iter().filter(|call| call.caller.as_ref() == Some(&selection.plan_machine)).collect();
            let [call] = calls.as_slice() else {
                panic!("expected one plan constructor call")
            };
            assert_eq!(&call.machine, core.id("access\\new_plan"));
            let [schema, issuance] = call.operands.as_slice() else {
                panic!("expected schema and issuance arguments")
            };
            assert_eq!(spelled(&schema.site, "policy.omg", source), "@schema");
            let tree = typed.trees().nth(issuance.node.file).unwrap();
            let construction = tree.construction(issuance.node.node).unwrap();
            assert_eq!(&construction.owner, core.id("Issuance"));
            assert_eq!(construction.case.as_ref(), Some(core.id(case)));
            // Only the explicit spelling writes `routes`. Its `[]` is the declared
            // owned `[CallableInfo<'t>]` field at the constructed issuance's own
            // `'t`, with no elements; it is not a view and not a `[T; 0]` value.
            let written: Vec<_> = construction
                .initializers
                .iter()
                .map(|field| field.expression)
                .collect();
            if source == empty {
                use omega_typed::ValueType::{Applied, Slice};
                let [routes] = written.as_slice() else {
                    panic!("expected the written routes field, found {written:?}")
                };
                assert_eq!(tree.resolved().file.span(*routes).slice(source.as_bytes()), Some(&b"[]"[..]));
                assert!(tree.resolved().file.nodes[*routes].children.is_empty());
                let Applied(_, lifetime) = &construction.carrier else {
                    panic!("Issuance is applied to its lifetime")
                };
                let published: Vec<_> = tree
                    .semantics_at(*routes)
                    .filter_map(|record| match record {
                        omega_typed::NodeSemantics::Expression(expression) => Some(expression.ty.clone()),
                        _ => None,
                    })
                    .collect();
                let element = Applied(Box::new(core.id(r"reflect\CallableInfo").clone()), lifetime.clone());
                assert_eq!(published, [Slice(Box::new(element))]);
            } else {
                assert!(written.is_empty(), "{case} writes no field: {written:?}");
            }

            let library = typed.library.unwrap();
            let signature = library.signatures.iter().find(|s| s.machine == call.machine).unwrap();
            let declaration = library.trees().nth(signature.node.file).unwrap();
            assert!(declaration.resolved().file.child(signature.node.node, omega_parser::NodeKind::Body).is_none());
            assert!(typed.obligations.iter().any(|o| {
                o.site == signature.site && matches!(&o.kind, ObligationKind::VerifyLibraryImplementation(id) if id == &call.machine)
            }));
        });
    }
}

/// With the selection removed, lowering still has to package the declared plan
/// template. Its call does not name a local machine in this package.
#[test]
fn an_unselected_policy_template_still_calls_the_library_constructor() {
    let source = ACCESS_PLAN.replace(" access Rule;", ";");
    let constructor = "new_plan::<'t>(@schema, Issuance::Unrestricted{})";
    let (obligations, error) = refusal(
        "unused.omg", &source, sources::unique_span(&source, constructor), "template call needs its packaged declaration",
    );
    assert!(!obligations.iter().any(|o| matches!(o.kind, ObligationKind::EvaluateAccessPlan { .. })));
    assert_eq!(spelled(&error.site, "unused.omg", &source), constructor);
}

/// Positive Psi controls only for the policy-free carrier. `Unrestricted`
/// would still require an evaluated policy; it is not this declaration.
#[test]
fn removing_the_policy_and_its_implementation_leaves_the_exact_plain_carrier() {
    let (prefix, _) = ACCESS_PLAN.split_once(" access Rule;").unwrap();
    let plain = format!("{prefix};");
    let domains = [plain.clone(), plain.replace("i32::Locked", "u32::Locked")].map(|source| {
        let (obligations, product) = lower("plain.omg", &source);
        assert!(!obligations.iter().any(|o| matches!(o.kind, ObligationKind::EvaluateAccessPlan { .. })));
        let package = product.unwrap();
        let [domain] = package.domains.as_slice() else {
            panic!("expected one plain domain")
        };
        assert_eq!(domain.declaration.package, "app");
        assert!(domain.requires.is_empty());
        (domain.declaration.path.clone(), package.types.get(domain.carrier).cloned().unwrap())
    });
    use omega_lowering::{ScalarType::Integer, TypeShape::PrimitiveScalar};
    spec_corpus::observe_handoff!(
        psi "psi-output/policy-free-domain-controls",
        assert_eq!(domains, [
            ("i32::Locked".to_owned(), PrimitiveScalar(Integer { signed: true, width: 32 })),
            ("u32::Locked".to_owned(), PrimitiveScalar(Integer { signed: false, width: 32 }))
        ]),
    );
}

/// Follow the actual call edge rather than looking up a specialization by name.
fn called_sum<'a>(pkg: &'a LoweredPackage, source: &str, caller: &str, text: &str) -> &'a omega_lowering::LoweredMachine {
    use omega_lowering::{CallRequirement, CallSite, CallTarget, Parameter};
    let (body, op) = operation_spelled(pkg, caller, source, text);
    let OperationKind::Call { callee: CallTarget::Machine(id), arguments, requirements } = &op.kind else { panic!("expected direct call") };
    let [Parameter::Scalar(x), Parameter::Scalar(y)] = body.parameters[..] else { panic!("two formals") };
    assert_eq!(arguments, &[ValueOperand::Scalar(x), ValueOperand::Scalar(y)]);
    assert_eq!(requirements, &[CallRequirement { site: CallSite::Operation(op.id), clause: 0 }]);
    let sum = pkg.machines.iter().find(|m| m.id == *id).unwrap();
    assert_eq!((sum.declaration.package.as_str(), sum.declaration.path.as_str()), ("app", "sum"));
    assert_eq!(sum.contract.requires.len(), 1);
    sum
}

fn unsigned(width: u64) -> omega_lowering::TypeShape {
    omega_lowering::TypeShape::PrimitiveScalar(omega_lowering::ScalarType::Integer { signed: false, width })
}

/// The declared `u64` carrier of a static integer binder; it is not a runtime type row.
const U64: omega_lowering::ScalarType = omega_lowering::ScalarType::Integer { signed: false, width: 64 };

#[test]
fn inferred_width_reaches_the_selected_sum_and_its_formation_sites() {
    use omega_lowering::{ArithmeticPolicy, BlockTerminator, IntegerOperation, IntegerPrimitive, OperationResult, Parameter, PositiveWidth,
        PredicateOperator, PredicateStep, ReturnEdge, StaticArgument};
    let file = "width.omg";
    let source = "machine sum<const N:u64>(x:u<N>,y:u<N>)->u<N> requires N>0{x+%y} machine pair(x:u13,y:u13)->u13{sum(x,y)}";
    let (obligations, pkg) = with_prepared(file, source, |prepared| {
        let call = prepared.typed().calls.iter().find(|c| c.machine.path == "sum").unwrap();
        assert_eq!(call.type_arguments, [omega_typed::ValueType::static_size(13)]);
        sources::lower_prepared(prepared, &[(file, source)]).unwrap()
    });
    let sum = called_sum(&pkg, source, "pair", "sum(x,y)");
    let [StaticArgument::Integer { carrier, value }] = &sum.arguments[..] else { panic!("one static integer") };
    let mut expected: Vec<_> = obligations.iter().filter_map(|o| match &o.kind {
        ObligationKind::PositiveWidth(omega_typed::ValueType::Parameter { owner, index }) if owner.path == "sum" => {
            assert_eq!(spelled(&o.site, file, source), "u<N>");
            Some(PositiveWidth { argument: *index, site: o.site.clone() })
        }
        _ => None,
    }).collect();
    expected.sort_by_key(|goal| goal.site.span.get_start_offset());
    assert_eq!(expected.iter().map(|g| (g.argument, g.site.span.get_start_offset())).collect::<Vec<_>>(), [(0,27), (0,34), (0,41)]);
    spec_corpus::observe_handoff!(psi "psi-output/closed-width-sites",
        assert_eq!((&sum.positive_widths, carrier, value), (&expected, &U64, &13.into())));
    let (_, call) = operation_spelled(&pkg, "pair", source, "sum(x,y)");
    let OperationKind::Call { callee, requirements, .. } = &call.kind else { panic!("direct call") };
    spec_corpus::observe_handoff!(psi "psi-output/closed-width-call",
        assert_eq!((callee, requirements.as_slice()), (&omega_lowering::CallTarget::Machine(sum.id),
            [omega_lowering::CallRequirement { site: omega_lowering::CallSite::Operation(call.id), clause: 0 }].as_slice())));
    let width = sum.result_type.unwrap();
    assert_eq!(sum.parameter_types, [width, width]);
    let req = &sum.contract.requires[0];
    assert_eq!(spelled(&req.site, file, source), "requires N>0");
    spec_corpus::observe_handoff!(psi "psi-output/closed-width-requirement",
        assert_eq!((pkg.types.get(width), req.predicate.as_slice()), (Some(&unsigned(13)),
            [PredicateStep::Integer(13.into()), PredicateStep::Integer(0.into()), PredicateStep::Apply(PredicateOperator::GreaterThan)].as_slice())));
    let (body, add) = operation_spelled(&pkg, "sum", source, "x+%y");
    let [Parameter::Scalar(x), Parameter::Scalar(y)] = body.parameters[..] else { panic!("two operands") };
    spec_corpus::observe_handoff!(psi "psi-output/closed-width-operation",
        assert_eq!(add.kind, OperationKind::Integer {
            operation: IntegerOperation { primitive: IntegerPrimitive::Add, policy: ArithmeticPolicy::Wrapping, obligations: vec![] },
            operands: vec![x, y],
        }));
    let OperationResult::Scalar(v) = add.result else { panic!("scalar result") };
    assert_eq!(body.values[v.index()].ty, width);
    assert!(matches!(body.blocks[0].terminator, BlockTerminator::Return {result: ReturnEdge::Scalar(result), ..} if result == v));
}

/// Both calls have the very same u17 runtime signature. The width comes from
/// slot 1; slot 0 changes the premise, not the operation's carrier. L5 must
/// hand even a false premise to the checker, not decide admission itself.
#[test]
fn same_width_distinct_static_arguments_keep_their_own_requirements() {
    use omega_lowering::{PredicateOperator, PredicateStep, StaticArgument};
    let source = "machine sum<const K:u64,const N:u64>(x:u<N>,y:u<N>)->u<N> requires K>0{x+%y} machine pair(x:u17,y:u17)->u17{sum::<0,17>(x,y)} machine other(x:u17,y:u17)->u17{sum::<5,17>(x,y)}";
    let pkg = lower("slots.omg", source).1.unwrap();
    let first = called_sum(&pkg, source, "pair", "sum::<0,17>(x,y)");
    let second = called_sum(&pkg, source, "other", "sum::<5,17>(x,y)");
    assert_eq!(first.declaration, second.declaration);
    assert_ne!(first.id, second.id);
    for (sum, k) in [(first, 0), (second, 5)] {
        let [StaticArgument::Integer { carrier, .. }, _] = &sum.arguments[..] else { panic!("two arguments") };
        assert_eq!(*carrier, U64);
        assert_eq!(sum.arguments, [StaticArgument::Integer { carrier: *carrier, value: k.into() }, StaticArgument::Integer { carrier: *carrier, value: 17.into() }]);
        let width = sum.result_type.unwrap();
        assert_eq!(sum.parameter_types, [width, width]);
        assert_eq!(pkg.types.get(width), Some(&unsigned(17)));
        assert_eq!(sum.positive_widths.iter().map(|w| w.argument).collect::<Vec<_>>(), [1,1,1]);
    }
    let premise = |k: u64| [PredicateStep::Integer(k.into()), PredicateStep::Integer(0.into()), PredicateStep::Apply(PredicateOperator::GreaterThan)];
    spec_corpus::observe_handoff!(psi "psi-output/same-width-distinct-premises",
        assert_eq!((&first.contract.requires[0].predicate[..], &second.contract.requires[0].predicate[..]), (&premise(0)[..], &premise(5)[..])));
}

/// A recursive field that changes `Link<3>` to `Link<4>` hands the next application
/// to the verifier at the use. The twin keeps the same argument and carries no such
/// obligation. A shared field names its referent without materializing a recursive layout.
#[test]
fn recursive_formation_carries_the_next_application_not_an_unfolded_layout() {
    use omega_psi::{DataArgument, DataFormation, ScalarType, TypeExpression, TypeExpressionStep as Step, TypeShape};
    let file = "recursive.omg";
    let changing = "data Link<const N:u64>{next:@Link<N+1>;value:u<N>;} machine caller(x:Link<3>){}";
    let same = changing.replace("@Link<N+1>", "@Link<N>");
    let (obligations, psi) = lower(file, changing);
    assert!(
        obligations
            .iter()
            .any(|o| matches!(&o.kind, ObligationKind::FormRecursiveApplication { declaration, active_arguments, arguments }
                if active_arguments.iter().map(ToString::to_string).eq(["3"]) && declaration.path == "Link" && arguments.iter().map(ToString::to_string).eq(["4"]))
                && spells(&o.site, file, changing, "Link<3>")),
        "{obligations:?}"
    );
    assert!(
        obligations
            .iter()
            .any(|o| matches!(o.kind, ObligationKind::PositiveWidth(_)) && spells(&o.site, file, changing, "u<N>")),
        "{obligations:?}"
    );
    let (obligations, twin) = lower(file, &same);
    assert!(
        !obligations
            .iter()
            .any(|o| matches!(o.kind, ObligationKind::FormRecursiveApplication { .. })),
        "{obligations:?}"
    );
    let carrier = ScalarType::Integer { signed: false, width: 64 };
    for (source, next, psi) in [(changing, 4, psi.unwrap()), (same.as_str(), 3, twin.unwrap())] {
        let [template] = psi.data_templates.as_slice() else { panic!("one Link template") };
        let [link] = psi.data.as_slice() else { panic!("one closed Link schema") };
        assert_eq!(link.declaration.path, "Link");
        assert_eq!(link.declaration, template.declaration);
        assert_eq!(psi.machines[0].parameter_types, [link.carrier]);
        assert_eq!(psi.types.get(link.carrier), Some(&TypeShape::Nominal(link.id)));
        assert_eq!(link.type_arguments, [DataArgument::Integer {carrier,value:3.into()}]);
        let [edge, width] = link.fields.as_slice() else {panic!("next, value")};
        assert_eq!((edge.declaration.path.as_str(), width.declaration.path.as_str()), ("Link::next", "Link::value"));
        assert_eq!(psi.types.get(width.carrier), Some(&TypeShape::PrimitiveScalar(ScalarType::Integer {signed:false,width:3})));
        let Some(TypeShape::IndexedSharedReference(id)) = psi.types.get(edge.carrier) else { panic!("shared recursion") };
        assert_eq!(psi.types.indexed_referent(*id), Some((&link.declaration, [DataArgument::Integer { carrier, value: next.into() }].as_slice())));
        let parameter = Step::GenericParameter { owner: link.declaration.clone(), index:0 };
        let integer = |n: i32| Step::StaticInteger { carrier, value: n.into() };
        let mut referred = vec![parameter.clone()];
        if next == 4 {
            referred.extend([integer(1), Step::IndexSum { carrier, operands: 2 }]);
        }
        referred.extend([
            Step::Named { declaration: link.declaration.clone(), argument_count: 1 },
            Step::Reference(omega_psi::TemplateAccess::Shared),
        ]);
        assert_eq!(template.fields[0].ty.0, referred);
        assert_eq!(template.fields[1].ty.0, [parameter.clone(), Step::IndexedInteger { signed: false }]);
        let width_questions: Vec<_> = template.formation.iter().filter_map(|(site,q)| match q {
            DataFormation::PositiveWidth(ty) => Some((spelled(site,file,source), ty)), _ => None,
        }).collect();
        if next == 4 {
            spec_corpus::observe_handoff!(psi "psi-output/indexed-field-width", assert_eq!(width_questions, [("u<N>", &TypeExpression(vec![parameter.clone()]))]));
        } else { assert_eq!(width_questions, [("u<N>", &TypeExpression(vec![parameter.clone()]))]); }
        let recursive: Vec<_> = template.formation.iter().filter_map(|(site,q)| match q {
            DataFormation::RecursiveApplication {active_arguments, arguments} => Some((spelled(site,file,source), active_arguments.as_slice(), arguments.as_slice())), _ => None,
        }).collect();
        if next == 4 {
            let addition = DataFormation::IntegerOperation {
                operation: omega_psi::IntegerOperation {
                    primitive: omega_psi::IntegerPrimitive::Add,
                    policy: omega_psi::ArithmeticPolicy::Exact,
                    obligations: vec![omega_psi::IntegerObligation::ResultFitsCarrier],
                },
                carrier,
                operands: [TypeExpression(vec![parameter.clone()]), TypeExpression(vec![integer(1)])],
            };
            assert!(template.formation.iter().any(|(site, q)| spelled(site, file, source) == "N+1" && q == &addition));
            let prior = [TypeExpression(vec![parameter.clone(), integer(1), Step::IndexSum {carrier,operands:2}])];
            let open = [TypeExpression(vec![parameter, integer(1), integer(1), Step::IndexSum {carrier,operands:3}])];
            let closed = [TypeExpression(vec![integer(4)])];
            let actual = [TypeExpression(vec![integer(3)])];
            spec_corpus::observe_handoff!(psi "psi-output/recursive-index-formation", assert_eq!(recursive, [
                ("@Link<N+1>", prior.as_slice(), open.as_slice()),
                ("Link<3>", actual.as_slice(), closed.as_slice()),
            ]));
        } else { assert!(recursive.is_empty()); }
    }
}

/// Returning `x.next` from an owned `Link<3>` returns a loan into the storage
/// that `x.next` already borrows, not a borrow of `x` itself. Psi forms the
/// result by reborrowing through the stored `@Link<4>`. That stored loan is the
/// parent, and the public result type is the very row of the `Link::next`
/// field, naming `Link<4>` without its layout. The plain `Node`/`Leaf` twin
/// uses the same mechanism. A Psi-only reader recovers the formal, field and
/// referent. Without the crossing it refuses, because the result would then
/// point into `x`'s own carrier storage. L6 judges the lifetime relation.
#[test]
fn a_returned_recursive_field_reborrows_through_its_stored_loan() {
    use omega_psi::{DataArgument, PlaceOrigin, PlaceProjection, ReferenceAccess, ReturnEdge, ScalarType, TypeShape};
    use psi_handoff_consumer::{ReturnedStoredLoan, returned_stored_loan};
    let file = "load_next.omg";
    let source = "data Link<const N:u64>{next:@Link<N+1>;value:u<N>;} machine read(x:Link<3>)->@Link<4>{x.next}";
    let (obligations, psi) = lower(file, source);
    let mut psi = psi.unwrap_or_else(|error| panic!("the original program must lower: {error:?}"));
    // The typer's return obligation sits at the field read that forms the loan.
    let returned: Vec<_> = obligations.iter().filter(|o| matches!(&o.kind, ObligationKind::ReturnBorrow { machine } if machine.path == "read")).collect();
    let [returned] = returned.as_slice() else { panic!("one ReturnBorrow: {obligations:?}") };
    assert_eq!(spelled(&returned.site, file, source), "x.next");
    let carrier = ScalarType::Integer { signed: false, width: 64 };
    let [link] = psi.data.as_slice() else { panic!("one closed Link schema") };
    let next = link.fields.iter().find(|f| f.declaration.path == "Link::next").expect("Link::next");
    let read = psi.machines.iter().find(|m| m.declaration.path == "read").expect("read");
    assert_eq!(read.parameter_types, [link.carrier]);
    assert_eq!(read.result_type, Some(next.carrier));
    let Some(TypeShape::IndexedSharedReference(id)) = psi.types.get(next.carrier) else { panic!("indexed shared result") };
    assert_eq!(psi.types.indexed_referent(*id), Some((&link.declaration, [DataArgument::Integer { carrier, value: 4.into() }].as_slice())));
    let (body, borrow) = operation_spelled(&psi, "read", source, "x.next");
    let [omega_lowering::Parameter::Structural(formal)] = body.parameters[..] else { panic!("one owned formal") };
    let expected = omega_lowering::ProjectedPlace {
        root: formal,
        path: vec![PlaceProjection::Field(next.declaration.clone()), PlaceProjection::Referent],
    };
    assert_eq!(borrow.kind, OperationKind::BorrowPlace { source: expected, access: ReferenceAccess::Shared });
    let omega_lowering::OperationResult::Structural(descriptor) = borrow.result else { panic!("a descriptor place") };
    assert_eq!((body.places[descriptor.index()].ty, body.places[descriptor.index()].origin), (next.carrier, PlaceOrigin::Operation(borrow.id)));
    let [block] = body.blocks.as_slice() else { panic!("one block") };
    assert!(matches!(&block.terminator, omega_lowering::BlockTerminator::Return { result: ReturnEdge::Structural { source, .. }, .. } if *source == descriptor));
    spec_corpus::observe_handoff!(psi "psi-output/returned-indexed-field-loan", assert_eq!(
        returned_stored_loan(&psi, "read"),
        Ok(ReturnedStoredLoan {
            formal: 0,
            fields: vec!["Link::next".into()],
            referent: "Link".into(),
            arguments: vec![DataArgument::Integer { carrier, value: 4.into() }],
        })
    ));

    // The plain record is the same read with a materialized referent schema.
    let plain = "data Leaf{v:u8;} data Node{next:@Leaf;} machine read(x:Node)->@Leaf{x.next}";
    let (_, twin) = lower(file, plain);
    let twin = twin.unwrap_or_else(|error| panic!("the plain twin must lower: {error:?}"));
    assert_eq!(
        returned_stored_loan(&twin, "read"),
        Ok(ReturnedStoredLoan { formal: 0, fields: vec!["Node::next".into()], referent: "Leaf".into(), arguments: vec![] })
    );

    // Dropping the crossing would borrow the field slot inside x.
    let read = psi.machines.iter_mut().find(|m| m.declaration.path == "read").unwrap();
    let OperationKind::BorrowPlace { source, .. } = &mut read.body.operations[0].kind else { unreachable!() };
    assert_eq!(source.path.pop(), Some(PlaceProjection::Referent));
    assert_eq!(returned_stored_loan(&psi, "read"), Err("the borrow must cross the stored descriptor, not select the record's own field"));
}

/// The same read of a stored `@mut Leaf` either moves the reference out of the
/// field or reborrows it while suspending that field. Neither is lowered yet,
/// so lowering refuses at the read instead of forming a shared borrow.
#[test]
fn reading_a_stored_exclusive_reference_still_needs_its_move_or_reborrow() {
    let file = "load_next.omg";
    let source = "data Leaf{v:u8;} data Node{next:@mut Leaf;} machine read(x:Node)->@mut Leaf{x.next}";
    sources::refuse(&[(file, source)], file, sources::unique_span(source, "x.next"), "a stored reference read needs its shared field place");
}

#[test]
fn a_closed_restricted_instance_does_not_replace_an_exported_template() {
    let file = "export.omg";
    let template = "machine use_it<const E:Local>(){E::put()}";
    let prefix = "public trait Log{machine put();} public trait Local=Log{machine * reaches none;} Quiet satisfies Log{machine put(){}}";
    let program = |wrapper: &str| format!("{prefix} {wrapper} machine f(){{use_it::<Quiet>()}}");
    let local = program(template);
    assert!(lower(file, &local).1.unwrap().machines.iter().any(|m| m.declaration.path == "use_it"));
    let exported = format!("public {template}");
    let public = program(&exported);
    let message = "exported restricted evidence requires an open machine interface";
    let (_, error) = refusal(file, &public, sources::unique_span(&public, &exported), message);
    assert_eq!(error.message, message);
}

/// The selected instance owes the wildcard bound, not a nominal satisfaction
/// of Local. Follow both calls through the public package after preparation.
#[test]
fn restricted_evidence_reaches_the_selected_base_member() {
    use omega_lowering::{CallTarget, MemberRealization, StaticArgument};
    let file = "refine.omg";
    let source = "trait Log{machine put() blocks;} trait Local=Log{machine * blocks false reaches none terminates;} Quiet satisfies Log{machine put(){}} Busy satisfies Log{machine put() blocks {}} machine use_it<const E:Local>(){E::put()} machine f(){use_it::<Quiet>()}";
    let wrong_selection = source.replace("::<Quiet>", "::<Busy>");
    let wrong_package = sources::resolve(&[(file, &wrong_selection)]);
    let Err(omega_preparation::PreparationFailure::Typing(error)) = omega_preparation::prepare_source_package_diagnosed(&wrong_package) else {
        panic!("the blocking Busy row must be rejected by typing at selection")
    };
    assert_eq!(error.message, "Busy::put exceeds the refined contract of Local");
    assert_eq!(error.span.slice(wrong_selection.as_bytes()), Some(&b"use_it::<Busy>()"[..]));
    let twin = wrong_selection.replace("put() blocks {}", "put(){}");
    let changed_bound = source.replace("reaches none terminates", "suspends false");
    let packages = [(source, "Quiet"), (twin.as_str(), "Busy"), (changed_bound.as_str(), "Quiet")].map(|(text, selected)| {
        let (obligations, package) = with_prepared(file, text, |prepared| {
            let typed = prepared.typed();
            let [row] = typed.refinements.as_slice() else { panic!("expected Local's restriction") };
            assert_eq!(row.schema.path, "Local");
            assert_eq!(typed.refinement_bases[&row.schema].declaration.path, "Log");
            sources::lower_prepared(prepared, &[(file, text)]).unwrap()
        });
        let refinements: Vec<_> = obligations.iter().filter_map(|o| match &o.kind {
            ObligationKind::RefineEvidence { binder, implementation } => Some((spelled(&o.site, file, text), binder.declaration.path.as_str(), implementation.path.as_str())),
            _ => None,
        }).collect();
        assert_eq!(refinements.len(), 1);
        let call = format!("use_it::<{selected}>()");
        assert_eq!(refinements[0], (call.as_str(), "Local", selected));
        (package, selected)
    });
    let mut bounds = Vec::new();
    let mut dispatch = Vec::new();
    for (package, selected) in &packages {
        let log = package.traits.iter().find(|t| t.declaration.path == "Log").unwrap();
        let local = package.traits.iter().find(|t| t.declaration.path == "Local").unwrap();
        let restriction = local.restriction.as_ref().unwrap();
        let [row] = restriction.all_members.as_slice() else { panic!("expected one wildcard") };
        let [put] = log.requirements.as_slice() else { panic!("expected one base requirement") };
        assert_eq!(put.pause.block, Some(true));
        assert!(local.requirements.is_empty());
        assert_eq!(restriction.base.declaration, log.declaration);
        assert!(restriction.base.arguments.is_empty());
        bounds.push((row.prohibits_block, row.prohibits_suspend, row.reach.as_deref(), row.requires_termination));
        let busy = package.machines.iter().find(|m| m.declaration.path == "Busy::put").unwrap();
        assert_eq!(busy.contract.realized_pause.block, *selected != "Busy");
        let f = package.machines.iter().find(|m| m.declaration.path == "f").unwrap();
        let by_id = |id| package.machines.iter().find(|m| m.id == id).unwrap();
        let target = |m: &omega_lowering::LoweredMachine| {
            let [op] = m.body.operations.as_slice() else { panic!("expected one call in {:?}", m.body) };
            let OperationKind::Call { callee, .. } = op.kind else { panic!("not a call") };
            callee
        };
        let CallTarget::Machine(id) = target(f) else { panic!("not a direct call") };
        let instance = by_id(id);
        assert_eq!(instance.declaration.path, "use_it");
        let [StaticArgument::Conformance { declaration, bound }] = instance.arguments.as_slice() else { panic!("not selected evidence") };
        let CallTarget::Member(slot) = target(instance) else { panic!("not a member call") };
        let member = &package.member_calls[slot];
        let MemberRealization::Conformance { declaration: supplied, machine } = &member.target else { panic!("not local evidence") };
        let conf = package.conformances.iter().find(|c| c.declaration.path == *selected).unwrap();
        let [impl_row] = conf.members.as_slice() else { panic!("expected one supplying row") };
        assert_eq!(impl_row.implementation.path, format!("{selected}::put"));
        assert_eq!(member.requirement, impl_row.requirement);
        assert_eq!(member.requirement.declaration, put.declaration);
        assert_eq!(member.owner, restriction.base);
        assert_eq!(conf.implemented, restriction.base);
        assert_eq!(member.restriction.as_ref(), Some(bound));
        assert_eq!((&bound.declaration, bound.arguments.len()), (&local.declaration, 0));
        assert_eq!((declaration, supplied, &by_id(*machine).declaration), (&conf.declaration, &conf.declaration, &impl_row.implementation));
        dispatch.push((bound.declaration.path.as_str(), declaration.path.as_str(), member.requirement.declaration.path.as_str(), by_id(*machine).declaration.path.as_str()));
    }
    spec_corpus::observe_handoff!(psi "psi-output/restricted-base-bound", assert_eq!(bounds, [
        (true, false, Some(&[][..]), true), (true, false, Some(&[][..]), true), (true, true, None, false),
    ]));
    spec_corpus::observe_handoff!(psi "psi-output/restricted-evidence-dispatch", assert_eq!(dispatch, [
        ("Local", "Quiet", "Log::put", "Quiet::put"), ("Local", "Busy", "Log::put", "Busy::put"), ("Local", "Quiet", "Log::put", "Quiet::put"),
    ]));
}
