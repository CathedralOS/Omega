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
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{LoweredPackage, LoweringError, MachineBody, OperationKind, PsiOperation, ScalarLiteral, ValueOperand, ValueOrigin};
use omega_preparation::{PreparedTrees, prepare_source_package};
use omega_typed::{ObligationKind, Site, TypingObligation};

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
    let literal = "Options::<'r>{count:1,..}";
    let source = "data Options<'r>{count:u8;handle:@'r u8;} machine build<'r>(){let options=Options::<'r>{count:1,..};}";
    let (obligations, error) = with_prepared(file, source, |prepared| {
        sources::refuse_prepared_with_cli_message(
            prepared,
            &[(file, source)],
            file,
            sources::unique_span(source, literal),
            "omitted field `Options::handle` of type `@u8 at app\\build<parameter 0>` needs a borrowed reference, which `..` never supplies",
            "omitted field `Options::handle` of type `@u8 at root\\build<parameter 0>` needs a borrowed reference, which `..` never supplies",
        )
    });
    assert!(obligations.iter().any(|o| matches!(o.kind, ObligationKind::ZeroField {..}) && o.site == error.site));
    assert!(
        obligations
            .iter()
            .any(|o| matches!(&o.kind, ObligationKind::ZeroField {field, ty: omega_typed::ValueType::AtLifetime(reference, _)} if field.path == "Options::handle" && matches!(**reference, omega_typed::ValueType::Reference(..))))
    );
    let scalar = source.replace("handle:@'r u8", "handle:u8");
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

const ACCESS_PLAN: &str = include_str!("fixtures/domain_access/unrestricted/access_plan.omg");

/// The original policy selection and the emitted issuance rule belong to the same domain.
#[test]
fn the_selected_unrestricted_access_plan_reaches_its_domain() {
    let file = "access_plan.omg";
    let source = ACCESS_PLAN;
    let (obligations, package) = sources::lower_built_fixture("domain_access/unrestricted", file, source);
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
    let [emitted] = package.domains.as_slice() else { panic!("one policy domain") };
    assert_eq!(emitted.declaration.path, domain.path);
    spec_corpus::observe_handoff!(psi "psi-output/executed-unrestricted-domain",
        assert_eq!((&emitted.declaration.path, emitted.issuers), (&domain.path, omega_psi::DomainIssuers::Unrestricted)),
    );
}

/// A selected policy and its conformance can be checked independently of the
/// pending domain row. Changing the source selection changes the member body.
#[test]
fn selected_policy_definition_uses_the_prepared_selection() {
    let source = ACCESS_PLAN;
    let other_source = source.replace("Rule", "OtherPolicy");
    let mut first = sources::resolve(&[("access_plan.omg", source)]);
    let mut other = sources::resolve(&[("access_plan.omg", &other_source)]);
    first.bind_package_key("app", [1; 32]).unwrap();
    other.bind_package_key("app", [2; 32]).unwrap();
    let first = prepare_source_package(&first).unwrap();
    let other = prepare_source_package(&other).unwrap();
    let [selected] = first.typed().access_policies.as_slice() else { panic!("one policy") };
    let [different] = other.typed().access_policies.as_slice() else { panic!("one different policy") };
    let policy = omega_lowering::lower_selected_policy_definition(&first, selected).unwrap();
    assert_eq!(policy.domain.path, "i32::Locked");
    assert_eq!(policy.conformance.declaration.path, "Rule");
    assert_eq!(policy.plan.declaration.path, "Rule::plan");
    assert_eq!(policy.conformance.members[0].implementation, policy.plan.declaration);
    assert_eq!(policy.site, selected.site);
    let error = omega_lowering::lower_selected_policy_definition(&first, different).unwrap_err();
    assert_eq!(error.message, "access policy selection is absent from the prepared package");
    let other_policy = omega_lowering::lower_selected_policy_definition(&other, different).unwrap();
    assert_eq!(other_policy.plan.declaration.path, "OtherPolicy::plan");
}

/// The exact identity the toolchain exports for `name` under `language\core\`,
/// as Psi spells it. The test takes it from the provider, not from this source.
fn provided(name: &str) -> omega_lowering::DeclarationId {
    use omega_lowering::DeclarationKind;
    use omega_parser::NodeKind;
    omega_toolchain::core_interface()
        .id(name)
        .try_map_kind(&|kind| match kind {
            NodeKind::Machine => Some(DeclarationKind::Machine),
            NodeKind::Data => Some(DeclarationKind::Data),
            NodeKind::Case => Some(DeclarationKind::Case),
            NodeKind::CaseField => Some(DeclarationKind::CaseField),
            _ => None,
        })
        .unwrap()
}

const TOOLCHAIN: &str = omega_toolchain::TOOLCHAIN_PACKAGE;
const POLICY_PLAN: &str = "language\\core\\AccessPolicy::plan";
const NEW_PLAN: &str = "language\\core\\access\\new_plan";

/// The closed bodies of `package` in machine order, each under the package that declares it.
fn bodies(package: &LoweredPackage) -> Vec<(&str, &str)> {
    package
        .machines
        .iter()
        .map(|m| (m.declaration.package.as_str(), m.declaration.path.as_str()))
        .collect()
}

fn body_of<'a>(
    package: &'a LoweredPackage,
    path: &str,
) -> &'a omega_lowering::LoweredMachine {
    let found: Vec<_> = package
        .machines
        .iter()
        .filter(|m| m.declaration.package == "app" && m.declaration.path == path)
        .collect();
    let [machine] = found.as_slice() else {
        panic!("expected one closed body of {path}, found {}", found.len())
    };
    machine
}

/// The direct calls of the closed body `path`, in operation order: the package
/// and path of each callee, and the caller's region the call applies to each
/// region binder of that callee.
fn calls<'a>(
    package: &'a LoweredPackage,
    path: &str,
) -> Vec<((&'a str, &'a str), Vec<usize>)> {
    let caller = body_of(package, path);
    caller
        .body
        .operations
        .iter()
        .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .map(|operation| {
            let site = omega_lowering::CallSite::Operation(operation.id);
            let (callee, applied) = psi_handoff_consumer::applied_regions(package, caller, site).unwrap();
            let regions = applied.iter().map(|region| region.ordinal).collect();
            ((callee.declaration.package.as_str(), callee.declaration.path.as_str()), regions)
        })
        .collect()
}

/// From Psi alone: the issuance the closed `Rule::plan` hands the toolchain's
/// `new_plan`, after following every body of this package it calls. The
/// schema must reach the constructor as a shared loan of the caller's
/// argument, and the issuance as storage the plan formed, never a copy.
fn policy_issuance(package: &LoweredPackage) -> psi_handoff_consumer::BodyValue<'_> {
    use psi_handoff_consumer::{BodyValue, Supply};
    let plan = psi_handoff_consumer::member_machine(package, "Rule", POLICY_PLAN).unwrap();
    let value = psi_handoff_consumer::body_result(package, plan, &[BodyValue::Argument(0)]).unwrap();
    let BodyValue::Provided {
        package: TOOLCHAIN,
        path: NEW_PLAN,
        operands,
    } = value
    else {
        panic!("Rule::plan does not return the toolchain constructor: {value:?}")
    };
    let [
        (Supply::Lent(omega_lowering::ReferenceAccess::Shared), BodyValue::Argument(0)),
        (Supply::Moved, issuance),
    ] = <[_; 2]>::try_from(operands).unwrap()
    else {
        panic!("expected the lent schema, then the moved issuance")
    };
    issuance
}

/// What a consumer reads for `Issuance::Unrestricted{}` in a closed body.
fn unrestricted() -> psi_handoff_consumer::BodyValue<'static> {
    psi_handoff_consumer::BodyValue::Construct {
        declaration: "language\\core\\Issuance::Unrestricted",
        fields: vec![],
    }
}

/// `source` with `plan` and each machine in `helpers` made reusable. Each
/// gains a static integer `N` that only a caller's application supplies, so
/// it has no body of its own and lowers as an open template. The toolchain's
/// `AccessPolicy::plan` takes no such binder, so `Rule` satisfies a local
/// trait whose `plan` declares it.
fn reusable(
    source: &str,
    helpers: &[&str],
) -> String {
    let planner = "trait Planner { machine plan<'t, const N: u64>(schema:TypeSchema<'t>)->AccessPlan<'t> reaches none crashes none terminates; }\n";
    let mut text = source
        .replace("Rule satisfies AccessPolicy {", &format!("{planner}Rule satisfies Planner {{"))
        .replace("machine plan<'t>(", "machine plan<'t, const N: u64>(");
    for helper in helpers {
        let declared = format!("machine {helper}<'t>(");
        let called = format!("{helper}::<'t>(");
        assert!(
            text.contains(&declared) || text.contains(&called),
            "{helper} is neither declared nor called with only a lifetime here"
        );
        text = text
            .replace(&declared, &format!("machine {helper}<'t, const N: u64>("))
            .replace(&called, &format!("{helper}::<'t, N>("));
    }
    text
}

/// The requirement `Rule::plan` implements in a `reusable` source.
const REUSABLE_PLAN: &str = "Planner::plan";

/// From Psi alone: the value the open template implementing `requirement`
/// for `Rule` passes to the toolchain's `new_plan`, after following every
/// packaged helper. Its first operand must be the borrowed schema the caller
/// supplies.
fn template_issuance<'a>(
    package: &'a LoweredPackage,
    requirement: &str,
) -> psi_handoff_consumer::TemplateValue<'a> {
    use psi_handoff_consumer::TemplateValue;
    let plan = psi_handoff_consumer::member_template(package, "Rule", requirement).unwrap();
    let value = psi_handoff_consumer::template_result(package, plan, &[TemplateValue::Argument(0)]).unwrap();
    let TemplateValue::Provided {
        package: TOOLCHAIN,
        path: NEW_PLAN,
        operands,
    } = value
    else {
        panic!("Rule::plan does not return the toolchain constructor: {value:?}")
    };
    let [TemplateValue::Borrowed(schema), issuance] = <[_; 2]>::try_from(operands).unwrap() else {
        panic!("expected a borrowed schema")
    };
    assert_eq!(*schema, TemplateValue::Argument(0));
    issuance
}

/// What a consumer reads for `Issuance::Unrestricted{}` in an open template.
fn open_unrestricted() -> psi_handoff_consumer::TemplateValue<'static> {
    psi_handoff_consumer::TemplateValue::Construct {
        declaration: "language\\core\\Issuance::Unrestricted",
        fields: vec![],
        establishes: false,
    }
}

/// With the selection removed, the declared plan is one closed body. Its
/// lifetime sits inside `TypeSchema<'t>` and `AccessPlan<'t>`, toolchain
/// records that keep a type row, so one region fills the schema formal and
/// the result. The body establishes the issuance, then calls the toolchain's
/// `new_plan`. This package carries that body and the records the two bodies
/// store under the toolchain's package and path, and declares none of them.
///
/// A plan that also takes a static integer has no body until a caller applies
/// it. That twin is an open template with the same two steps, and it names
/// the toolchain's constructor without carrying anything of the toolchain.
#[test]
fn an_unselected_policy_plan_calls_the_toolchain_constructor() {
    use omega_lowering::{
        AggregateRegionSlot, BlockTerminator, CallArgument, CallRegions, CallSite, CallTarget, MachineRegion, MachineTemplateBinder, OperationResult,
        Parameter, ProjectedPlace, ReferenceAccess, ReturnEdge, ScalarType, TemplateAccess, TemplateBinder, TemplateCallTarget, TemplateOperand,
        TemplateResult, TemplateStep, TypeExpression, TypeExpressionStep,
    };
    use psi_handoff_consumer::{BodyCallee, TemplateCallee};
    let source = ACCESS_PLAN.replace(" access Rule;", ";");
    let constructor = "new_plan::<'t>(@schema, Issuance::Unrestricted{})";
    let (obligations, package) = lower("unused.omg", &source);
    assert!(
        !obligations
            .iter()
            .any(|o| matches!(o.kind, ObligationKind::EvaluateAccessPlan { .. }))
    );
    let package = package.unwrap_or_else(|error| panic!("the unselected policy lowers: {error:?}"));
    assert!(package.machine_templates.is_empty() && package.data_templates.is_empty());
    assert_eq!(bodies(&package), [("app", "Rule::plan"), (TOOLCHAIN, NEW_PLAN)]);
    let [plan, new_plan] = package.machines.as_slice() else { unreachable!() };
    assert_eq!(new_plan.declaration, provided("access\\new_plan"));
    let body = &plan.body;
    let [issuance, call] = body.operations.as_slice() else {
        panic!("expected the issuance establishment, then the constructor call: {:?}", body.operations)
    };
    assert_eq!(spelled(&body.diagnostics.operations[call.id.index()], "unused.omg", &source), constructor);
    assert_eq!(
        spelled(&body.diagnostics.operations[issuance.id.index()], "unused.omg", &source),
        "Issuance::Unrestricted{}"
    );
    let data = |name: &str| {
        package
            .data
            .iter()
            .find(|d| d.declaration == provided(name))
            .unwrap_or_else(|| panic!("the package carries the toolchain's {name}"))
    };
    assert_eq!(
        (&plan.parameter_types[..], plan.result_type),
        (&[data("reflect\\TypeSchema").carrier][..], Some(data("AccessPlan").carrier))
    );
    let [Parameter::Structural(schema)] = body.parameters[..] else {
        panic!("one structural formal")
    };
    let OperationResult::Structural(formed) = issuance.result else {
        panic!("the issuance has its own storage")
    };
    let OperationKind::Call {
        callee,
        arguments,
        requirements,
    } = &call.kind
    else {
        panic!("expected the constructor call, found {:?}", call.kind)
    };
    let [block] = body.blocks.as_slice() else { panic!("one block") };
    let BlockTerminator::Return {
        result: ReturnEdge::Structural { source: returned, .. },
        cleanup,
        ..
    } = &block.terminator
    else {
        panic!("a structural return")
    };
    // One region. It fills the schema the formal holds and the plan the body returns.
    let region = MachineRegion {
        aggregate_parameters: vec![AggregateRegionSlot { parameter: 0, binder: 0 }],
        aggregate_result: vec![0],
        ..Default::default()
    };
    let region_row = CallRegions {
        call: CallSite::Operation(call.id),
        arguments: vec![omega_lowering::CallRegion::Binder(0)],
    };
    spec_corpus::observe_handoff!(psi "psi-output/policy-template-toolchain-callee",
        assert_eq!(
            (&plan.regions[..], plan.arguments.len(), callee, &body.call_regions[..], OperationResult::Structural(*returned)),
            (&[region][..], 0, &CallTarget::Machine(new_plan.id), &[region_row][..], call.result)
        ),
    );
    let lent = CallArgument::Borrow {
        place: ProjectedPlace { root: schema, path: vec![] },
        access: ReferenceAccess::Shared,
    };
    let established = OperationKind::EstablishCase {
        case: provided("Issuance::Unrestricted"),
        fields: vec![],
    };
    spec_corpus::observe_handoff!(psi "psi-output/policy-template-constructor-operands",
        assert_eq!(
            (&arguments[..], (&issuance.kind, body.places[formed.index()].ty)),
            (&[lent, CallArgument::MoveWhole(formed)][..], (&established, data("Issuance").carrier))
        ),
    );
    assert!(requirements.is_empty() && cleanup.is_empty());
    // The toolchain owns new_plan, TypeSchema, AccessPlan and Issuance.
    assert!(package.data.iter().all(|d| d.declaration.package == TOOLCHAIN));
    let [conformance] = package.conformances.as_slice() else {
        panic!("expected the Rule conformance")
    };
    assert_eq!(
        conformance
            .members
            .iter()
            .map(|m| &m.implementation)
            .collect::<Vec<_>>(),
        [&plan.declaration]
    );

    // Independent consumer: from Psi alone, the member's body returns a call
    // of the body carried under the toolchain's identity. Two other spellings
    // of the same import name the same canonical declaration and the same value.
    fn plan_callee(package: &LoweredPackage) -> BodyCallee<'_> {
        let member = psi_handoff_consumer::member_machine(package, "Rule", POLICY_PLAN).unwrap();
        psi_handoff_consumer::returned_body_call(package, member)
            .unwrap()
            .callee
    }
    let toolchain = BodyCallee::Provided {
        package: TOOLCHAIN,
        path: NEW_PLAN,
    };
    assert_eq!(plan_callee(&package), toolchain);
    assert_eq!(policy_issuance(&package), unrestricted());
    let qualified = source.replace(constructor, "access\\new_plan::<'t>(@schema, Issuance::Unrestricted{})");
    let unimported = source
        .replace("use omega\\language\\core\\access;", "")
        .replace(constructor, "omega\\language\\core\\access\\new_plan::<'t>(@schema, Issuance::Unrestricted{})");
    for twin in [qualified, unimported] {
        let package = lower("unused.omg", &twin)
            .1
            .unwrap_or_else(|error| panic!("{error:?}\n{twin}"));
        assert_eq!(plan_callee(&package), toolchain, "{twin}");
        assert_eq!(policy_issuance(&package), unrestricted(), "{twin}");
    }

    let open = reusable(&source, &[]);
    let package = lower("unused.omg", &open)
        .1
        .unwrap_or_else(|error| panic!("the reusable plan lowers: {error:?}"));
    let [template] = package.machine_templates.as_slice() else {
        panic!("expected only the plan template, found {:?}", package.machine_templates)
    };
    assert_eq!(
        (template.declaration.package.as_str(), template.declaration.path.as_str()),
        ("app", "Rule::plan")
    );
    let [TemplateStep::Construct(issuance), TemplateStep::Call(call)] = template.body.as_slice() else {
        panic!("expected the issuance construction, then the constructor call: {:?}", template.body)
    };
    assert_eq!(spelled(&call.site, "unused.omg", &open), constructor);
    assert_eq!(spelled(&issuance.site, "unused.omg", &open), "Issuance::Unrestricted{}");
    let lifetime = TypeExpressionStep::GenericParameter {
        owner: template.declaration.clone(),
        index: 0,
    };
    let applied = |name| {
        TypeExpression(vec![
            lifetime.clone(),
            TypeExpressionStep::Named {
                declaration: provided(name),
                argument_count: 1,
            },
        ])
    };
    assert_eq!(
        (&template.signature.parameters[..], &template.signature.result),
        (&[applied("reflect\\TypeSchema")][..], &Some(applied("AccessPlan")))
    );
    let u64 = TypeExpression(vec![TypeExpressionStep::Scalar(ScalarType::Integer {
        signed: false,
        width: 64,
    })]);
    assert_eq!(
        (&template.parameters[..], &call.target, &call.arguments[..], &call.result_type, &template.result),
        (
            &[
                MachineTemplateBinder::Type(TemplateBinder::Lifetime),
                MachineTemplateBinder::Const(u64)
            ][..],
            &TemplateCallTarget::Declaration(provided("access\\new_plan")),
            &[TypeExpression(vec![lifetime.clone()])][..],
            &Some(applied("AccessPlan")),
            &Some(TemplateResult::Operand(TemplateOperand::Step(1)))
        )
    );
    assert_eq!(
        (&call.operands[..], (&issuance.declaration, &issuance.ty, &issuance.fields[..])),
        (
            &[
                TemplateOperand::Borrow {
                    formal: 0,
                    access: TemplateAccess::Shared
                },
                TemplateOperand::Step(0)
            ][..],
            (&provided("Issuance::Unrestricted"), &applied("Issuance"), &[][..])
        )
    );
    // An open template names the toolchain's declarations and carries none of them.
    assert!(package.machines.is_empty() && package.data.is_empty() && package.data_templates.is_empty());
    let [conformance] = package.conformances.as_slice() else {
        panic!("expected the Rule conformance")
    };
    assert_eq!(
        conformance
            .members
            .iter()
            .map(|m| &m.implementation)
            .collect::<Vec<_>>(),
        [&template.declaration]
    );
    let returned = psi_handoff_consumer::member_template_call(&package, "Rule", REUSABLE_PLAN).unwrap();
    assert_eq!(
        returned.callee,
        TemplateCallee::Provided {
            package: TOOLCHAIN,
            path: NEW_PLAN
        }
    );
    assert_eq!(template_issuance(&package, REUSABLE_PLAN), open_unrestricted());
}

/// The coordinating plan-evaluation twins, exact bytes. Each differs from the
/// original only in how the plan forms its issuance.
const TWINS: [(&str, &str); 4] = [
    ("dotdot", include_str!("fixtures/policy_templates/dotdot.omg")),
    ("helper", include_str!("fixtures/policy_templates/helper.omg")),
    ("helper_sel2", include_str!("fixtures/policy_templates/helper_sel2.omg")),
    ("local_annot", include_str!("fixtures/policy_templates/local_annot.omg")),
];

/// What a consumer reads in a closed body for `Issuance::Only` with no routes.
fn no_routes() -> psi_handoff_consumer::BodyValue<'static> {
    use psi_handoff_consumer::BodyValue;
    BodyValue::Construct {
        declaration: "language\\core\\Issuance::Only",
        fields: vec![("language\\core\\Issuance::Only::routes", BodyValue::Array(vec![]))],
    }
}

/// The closed package of one twin without its selecting domain: no template
/// of its own, and only toolchain data.
fn unselected_twin(
    name: &str,
    selected: &str,
) -> (Vec<TypingObligation>, LoweredPackage) {
    let source = selected.replace(" access Rule;", ";");
    assert_ne!(source, selected, "{name} selects Rule");
    let (obligations, package) = lower("unused.omg", &source);
    assert!(
        !obligations
            .iter()
            .any(|o| matches!(o.kind, ObligationKind::EvaluateAccessPlan { .. })),
        "{name}"
    );
    let package = package.unwrap_or_else(|error| panic!("{name}: {error:?}"));
    assert!(package.machine_templates.is_empty() && package.data_templates.is_empty(), "{name}");
    assert!(package.data.iter().all(|d| d.declaration.package == TOOLCHAIN), "{name}");
    (obligations, package)
}

/// Without a domain selecting `Rule`, each twin still packages the plan, and
/// any helper it calls, as closed bodies of this package. Psi keeps the
/// issuance the source forms: for `Only{..}` the body establishes the zero of
/// `routes` itself, an empty sequence at the omission's own site; a helper's
/// construction reaches the constructor through the helper's call; a `let`
/// local moves on once. No policy name is recognized: the consumer follows
/// the published operations.
#[test]
fn issuance_twins_package_their_plan_and_helpers_without_a_selected_domain() {
    let mut observed = Vec::new();
    for (name, selected) in [TWINS[0], TWINS[1], TWINS[3]] {
        let (obligations, package) = unselected_twin(name, selected);
        // `..` owes the zero of each omitted field, and the body forms it where the source omits it.
        let zeros: Vec<_> = obligations
            .iter()
            .filter(|o| matches!(o.kind, ObligationKind::ZeroField { .. }) && o.site.package == "app")
            .collect();
        let empty = OperationKind::EstablishStructuralArray { elements: vec![] };
        let formed = package
            .machines
            .iter()
            .flat_map(|m| m.body.operations.iter().map(move |op| (m, op)))
            .filter(|(m, op)| {
                op.kind == empty
                    && zeros
                        .iter()
                        .any(|zero| zero.site == m.body.diagnostics.operations[op.id.index()])
            })
            .count();
        assert_eq!(formed, zeros.len(), "{name}");
        let own: Vec<_> = bodies(&package)
            .into_iter()
            .filter(|(owner, _)| *owner == "app")
            .map(|(_, path)| path)
            .collect();
        let issuance = policy_issuance(&package);
        let issued = if issuance == unrestricted() {
            "Unrestricted"
        } else if issuance == no_routes() {
            "Only without routes"
        } else {
            panic!("{name}: {issuance:?}")
        };
        observed.push((name, own.join(" "), zeros.len(), issued));
    }
    spec_corpus::observe_handoff!(psi "psi-output/policy-template-issuance-twins",
        assert_eq!(observed, [
            ("dotdot", "Rule::plan".to_owned(), 1, "Only without routes"),
            ("helper", "Rule::plan open".to_owned(), 0, "Unrestricted"),
            ("local_annot", "Rule::plan".to_owned(), 0, "Unrestricted"),
        ]),
    );
}

/// The `helper_sel2` twin: the plan passes `false` to a helper whose Boolean
/// `match` forms `Issuance::Unrestricted{}` or `Issuance::Only{..}`. The
/// helper is a closed body ending in a conditional on its formal. The arm is
/// the consumer's choice from the published constant, not lowering's:
/// changing only the plan's `false` to `true` selects the other arm.
#[test]
fn a_helpers_boolean_match_leaves_the_issuance_arm_to_the_published_constant() {
    use omega_lowering::{BlockTerminator, Parameter};
    let (name, selected) = TWINS[2];
    let (_, package) = unselected_twin(name, selected);
    assert_eq!(bodies(&package), [("app", "Rule::plan"), ("app", "choose"), (TOOLCHAIN, NEW_PLAN)]);
    assert_eq!(calls(&package, "Rule::plan"), [(("app", "choose"), vec![0]), ((TOOLCHAIN, NEW_PLAN), vec![0])]);
    assert_eq!(policy_issuance(&package), no_routes());
    let choose = body_of(&package, "choose");
    let [Parameter::Scalar(open)] = choose.body.parameters[..] else {
        panic!("one scalar formal")
    };
    assert!(
        matches!(&choose.body.blocks[0].terminator, BlockTerminator::Conditional { condition, .. } if *condition == open),
        "{:?}",
        choose.body.blocks[0].terminator
    );
    let other = selected.replace("choose::<'t>(false)", "choose::<'t>(true)");
    let (_, package) = unselected_twin(name, &other);
    assert_eq!(policy_issuance(&package), unrestricted());
}

/// With a static integer binder on the plan and its helpers, every twin stays
/// a package of open templates. Psi keeps the issuance the source forms: a
/// zero-filled `routes` for `Only{..}`, a helper's construction, the `false`
/// arm of a Boolean `match`, and a `let` local. No step runs and no policy
/// name is recognized: the consumer evaluates the published steps, and
/// changing only the helper's `false` to `true` selects the other arm.
#[test]
fn every_reusable_issuance_twin_packages_its_plan_and_helpers_as_templates() {
    use psi_handoff_consumer::TemplateValue;
    let twin = |name: &str, selected: &str| {
        let helpers: &[&str] = match name {
            "helper" => &["open"],
            "helper_sel2" => &["choose"],
            _ => &[],
        };
        reusable(&selected.replace(" access Rule;", ";"), helpers)
    };
    let zero_routes = |package: &LoweredPackage| {
        let plan = package
            .machine_templates
            .iter()
            .find(|t| t.declaration.path == "Rule::plan")
            .unwrap();
        package
            .machine_templates
            .iter()
            .find(|t| t.declaration.path == "choose")
            .map_or(&plan.declaration, |t| &t.declaration)
            .clone()
    };
    let mut observed = Vec::new();
    for (name, selected) in TWINS {
        let source = twin(name, selected);
        let package = lower("unused.omg", &source)
            .1
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        assert!(
            package.machines.is_empty() && package.data.is_empty() && package.data_templates.is_empty(),
            "{name}"
        );
        let mut templates: Vec<_> = package
            .machine_templates
            .iter()
            .map(|t| t.declaration.path.as_str())
            .collect();
        templates.sort_unstable();
        let templates = templates.join(" ");
        let expected_zero = omega_lowering::TypeExpression(vec![
            omega_lowering::TypeExpressionStep::GenericParameter {
                owner: zero_routes(&package),
                index: 0,
            },
            omega_lowering::TypeExpressionStep::Named {
                declaration: provided("reflect\\CallableInfo"),
                argument_count: 1,
            },
            omega_lowering::TypeExpressionStep::Slice,
        ]);
        let issuance = template_issuance(&package, REUSABLE_PLAN);
        let zero_filled = TemplateValue::Construct {
            declaration: "language\\core\\Issuance::Only",
            fields: vec![("language\\core\\Issuance::Only::routes", TemplateValue::Zero(&expected_zero))],
            establishes: false,
        };
        let expected = if matches!(name, "dotdot" | "helper_sel2") {
            zero_filled
        } else {
            open_unrestricted()
        };
        assert_eq!(issuance, expected, "{name}");
        observed.push((name, templates, issuance == open_unrestricted()));
    }
    assert_eq!(
        observed,
        [
            ("dotdot", "Rule::plan".to_owned(), false),
            ("helper", "Rule::plan open".to_owned(), true),
            ("helper_sel2", "Rule::plan choose".to_owned(), false),
            ("local_annot", "Rule::plan".to_owned(), true),
        ]
    );
    // The arm is the consumer's choice from the published constant, not lowering's.
    let selected = twin(TWINS[2].0, TWINS[2].1);
    let other = selected.replace("choose::<'t, N>(false)", "choose::<'t, N>(true)");
    assert_ne!(other, selected);
    let package = lower("unused.omg", &other).1.unwrap();
    assert_eq!(template_issuance(&package, REUSABLE_PLAN), open_unrestricted());
    let choose = package
        .machine_templates
        .iter()
        .find(|t| t.declaration.path == "choose")
        .unwrap();
    let [omega_lowering::TemplateStep::Select(select), ..] = choose.body.as_slice() else {
        panic!("expected the Boolean dispatch first")
    };
    assert_eq!(
        (
            &select.subject,
            select
                .arms
                .iter()
                .map(|arm| (arm.when, arm.steps.clone()))
                .collect::<Vec<_>>()
        ),
        (&omega_lowering::TemplateOperand::Formal(0), vec![(Some(true), 1..2), (Some(false), 2..3)])
    );
}

/// Control: the same plan calling a local wrapper of the constructor. The
/// wrapper is its own closed body under this package, the plan's call names
/// it, and only the wrapper's call names the toolchain's body. The wrapper
/// forwards its reference formal by lending the referent again and moves its
/// issuance formal on.
///
/// With a static integer binder on both, the planner packages the local
/// callee as its own template and follows it.
#[test]
fn a_local_constructor_wrapper_is_the_plans_callee_and_only_it_calls_the_toolchain() {
    use omega_lowering::{CallArgument, Parameter, PlaceProjection, ReferenceAccess, TemplateOperand};
    use psi_handoff_consumer::{BodyCallee, TemplateCallee};
    let wrapper = "machine mint<'t>(schema:@TypeSchema<'t>, issuance:Issuance<'t>)->AccessPlan<'t> reaches none crashes none terminates { new_plan::<'t>(schema, issuance) }\n";
    let source = format!(
        "{}{wrapper}",
        ACCESS_PLAN
            .replace(" access Rule;", ";")
            .replace("new_plan::<'t>(@schema", "mint::<'t>(@schema")
    );
    let package = lower("wrapped.omg", &source)
        .1
        .unwrap_or_else(|error| panic!("{error:?}"));
    assert!(package.machine_templates.is_empty());
    assert_eq!(bodies(&package), [("app", "Rule::plan"), ("app", "mint"), (TOOLCHAIN, NEW_PLAN)]);
    assert_eq!(calls(&package, "Rule::plan"), [(("app", "mint"), vec![0])]);
    assert_eq!(calls(&package, "mint"), [((TOOLCHAIN, NEW_PLAN), vec![0])]);
    let plan = psi_handoff_consumer::member_machine(&package, "Rule", POLICY_PLAN).unwrap();
    let returned = psi_handoff_consumer::returned_body_call(&package, plan).unwrap();
    let BodyCallee::Packaged(mint) = returned.callee else {
        panic!("expected the local wrapper, found {:?}", returned.callee)
    };
    assert_eq!((mint.declaration.package.as_str(), mint.declaration.path.as_str()), ("app", "mint"));
    let lends = |call: &PsiOperation, root, path: &[PlaceProjection]| {
        let OperationKind::Call { arguments, .. } = &call.kind else {
            panic!("not a call: {:?}", call.kind)
        };
        matches!(arguments.as_slice(), [CallArgument::Borrow { place, access: ReferenceAccess::Shared }, CallArgument::MoveWhole(_)]
            if place.root == root && place.path == path)
    };
    let [Parameter::Structural(schema)] = plan.body.parameters[..] else {
        panic!("one structural formal")
    };
    assert!(lends(returned.call, schema, &[]), "{:?}", returned.call);
    let forwarded = psi_handoff_consumer::returned_body_call(&package, mint).unwrap();
    assert_eq!(
        forwarded.callee,
        BodyCallee::Provided {
            package: TOOLCHAIN,
            path: NEW_PLAN
        }
    );
    let [Parameter::Structural(reference), Parameter::Structural(_)] = mint.body.parameters[..] else {
        panic!("two structural formals")
    };
    assert!(lends(forwarded.call, reference, &[PlaceProjection::Referent]), "{:?}", forwarded.call);
    assert_eq!(policy_issuance(&package), unrestricted());

    let open = reusable(&source, &["mint"]);
    let package = lower("wrapped.omg", &open)
        .1
        .unwrap_or_else(|error| panic!("{error:?}"));
    let plan = psi_handoff_consumer::member_template_call(&package, "Rule", REUSABLE_PLAN).unwrap();
    let TemplateCallee::Packaged(mint) = plan.callee else {
        panic!("expected the local wrapper, found {:?}", plan.callee)
    };
    assert_eq!((mint.declaration.package.as_str(), mint.declaration.path.as_str()), ("app", "mint"));
    assert_eq!(
        plan.call.operands,
        [
            TemplateOperand::Borrow {
                formal: 0,
                access: omega_lowering::TemplateAccess::Shared
            },
            TemplateOperand::Step(0)
        ]
    );
    let forwarded = psi_handoff_consumer::returned_template_call(&package, mint).unwrap();
    assert_eq!(
        forwarded.callee,
        TemplateCallee::Provided {
            package: TOOLCHAIN,
            path: NEW_PLAN
        }
    );
    assert_eq!(forwarded.call.operands, [TemplateOperand::Formal(0), TemplateOperand::Formal(1)]);
    assert!(package.machines.is_empty());
    assert_eq!(package.machine_templates.len(), 2);
    assert_eq!(template_issuance(&package, REUSABLE_PLAN), open_unrestricted());
}

/// Control from ordinary package input: two modules each declare `open`. Both
/// helpers are closed bodies under their own modules, the plan calls exactly
/// the imported owner's body before the toolchain's constructor, and only the
/// import line selects which issuance the consumer reads.
///
/// With a static integer binder on the plan and both helpers, the three are
/// open templates under the same owners and the plan's steps name the same
/// two declarations.
#[test]
fn same_named_helpers_in_two_modules_keep_their_owners() {
    use omega_lowering::TemplateCallTarget;
    let head = "use omega\\language\\core;\nuse omega\\language\\core\\reflect;\n";
    let high = format!("{head}public machine open<'t>() -> Issuance<'t> reaches none crashes none terminates {{ Issuance::Unrestricted{{}} }}\n");
    let low = format!("{head}public machine open<'t>() -> Issuance<'t> reaches none crashes none terminates {{ Issuance::Only{{..}} }}\n");
    let original = ACCESS_PLAN.replace(" access Rule;", ";");
    let access = "use omega\\language\\core\\access;";
    let mut seen = Vec::new();
    let mut templates = Vec::new();
    for module in ["high", "low"] {
        let main = original
            .replace(access, &format!("{access}\nuse {module}\\open;"))
            .replace("new_plan::<'t>(@schema, Issuance::Unrestricted{})", "new_plan::<'t>(@schema, open::<'t>())");
        let package = sources::lower(&[
            ("main.omg", &main),
            ("high/open.omg", &high),
            ("low/open.omg", &low),
        ])
        .unwrap_or_else(|error| panic!("{module}: {error:?}"));
        assert!(package.machine_templates.is_empty(), "{module}");
        assert_eq!(
            bodies(&package),
            [
                ("app", "Rule::plan"),
                ("app", "high\\open"),
                ("app", "low\\open"),
                (TOOLCHAIN, NEW_PLAN)
            ]
        );
        let called: Vec<_> = calls(&package, "Rule::plan")
            .into_iter()
            .map(|((owner, path), _)| (owner.to_owned(), path.to_owned()))
            .collect();
        seen.push((called, policy_issuance(&package) == unrestricted()));

        let package = sources::lower(&[
            ("main.omg", &reusable(&main, &["open"])),
            ("high/open.omg", &reusable(&high, &["open"])),
            ("low/open.omg", &reusable(&low, &["open"])),
        ])
        .unwrap_or_else(|error| panic!("reusable {module}: {error:?}"));
        let mut owners: Vec<_> = package
            .machine_templates
            .iter()
            .map(|t| (t.declaration.package.as_str(), t.declaration.path.as_str()))
            .collect();
        owners.sort_unstable();
        assert_eq!(
            owners,
            [
                ("app", "Rule::plan"),
                ("app", "high\\open"),
                ("app", "low\\open")
            ]
        );
        let plan = package
            .machine_templates
            .iter()
            .find(|t| t.declaration.path == "Rule::plan")
            .unwrap();
        let called: Vec<_> = plan
            .body
            .iter()
            .filter_map(|step| match &step.call()?.target {
                TemplateCallTarget::Declaration(id) => Some(id.path.clone()),
                _ => None,
            })
            .collect();
        templates.push((called, template_issuance(&package, REUSABLE_PLAN) == open_unrestricted()));
    }
    let own = |path: &str| ("app".to_owned(), path.to_owned());
    let constructor = (TOOLCHAIN.to_owned(), NEW_PLAN.to_owned());
    spec_corpus::observe_handoff!(psi "psi-output/policy-template-helper-owners",
        assert_eq!(seen, [
            (vec![own("high\\open"), constructor.clone()], true),
            (vec![own("low\\open"), constructor.clone()], false),
        ]),
    );
    assert_eq!(
        templates,
        [
            (vec!["high\\open".to_owned(), NEW_PLAN.to_owned()], true),
            (vec!["low\\open".to_owned(), NEW_PLAN.to_owned()], false),
        ]
    );
}

/// Selecting `Rule` for a domain adds the access selection and its evaluation
/// request at the clause. Nothing lowering reads for the plan and its helpers
/// changes: their calls, signatures, binders and obligations are the same for
/// each twin. So the plan's body cannot block the selected domain: lowering
/// that domain is the plan evaluation's work, and the body is lowered from
/// the same inputs.
#[test]
fn selecting_the_policy_changes_no_input_of_its_plan_and_helpers() {
    fn inputs(source: &str) -> (Vec<String>, Vec<String>, usize) {
        let package = sources::resolve(&[("unused.omg", source)]);
        let prepared = prepare_source_package(&package).unwrap();
        let typed = prepared.typed();
        let app = |id: &omega_resolved::DeclarationId| id.package == "app";
        let mut calls: Vec<_> = typed
            .calls
            .iter()
            .filter(|c| c.caller.as_ref().is_some_and(app))
            .map(|c| {
                format!(
                    "{} -> {} {:?} {:?} ops={} {:?}",
                    c.caller.as_ref().unwrap().path,
                    c.machine.path,
                    c.type_arguments,
                    c.result,
                    c.operands.len(),
                    c.use_site
                )
            })
            .collect();
        calls.sort_unstable();
        let mut machines: Vec<_> = typed
            .signatures
            .iter()
            .filter(|s| app(&s.machine))
            .map(|s| {
                format!(
                    "{} {:?} {:?} {:?} {:?}",
                    s.machine.path,
                    s.parameters.iter().map(|p| &p.1).collect::<Vec<_>>(),
                    s.result,
                    typed.generic_parameters.get(&s.machine),
                    typed.machine_effects.iter().find(|e| e.machine == s.machine)
                )
            })
            .collect();
        machines.sort_unstable();
        let mut kinds: Vec<_> = typed
            .obligations
            .iter()
            .filter(|o| !matches!(o.kind, ObligationKind::EvaluateAccessPlan { .. }))
            .map(|o| format!("{:?}", std::mem::discriminant(&o.kind)))
            .collect();
        kinds.sort_unstable();
        let selections = typed
            .obligations
            .iter()
            .filter(|o| matches!(o.kind, ObligationKind::EvaluateAccessPlan { .. }))
            .count()
            + typed.access_policies.len();
        (calls, [machines, kinds].concat(), selections)
    }
    for (name, selected) in [
        ("unrestricted", ACCESS_PLAN),
        TWINS[0],
        TWINS[1],
        TWINS[2],
        TWINS[3],
    ] {
        let unselected = selected.replace(" access Rule;", ";");
        let (with_calls, with_rest, with_selection) = inputs(selected);
        let (without_calls, without_rest, without_selection) = inputs(&unselected);
        assert_eq!((with_selection, without_selection), (2, 0), "{name}");
        assert_eq!(with_calls, without_calls, "{name}");
        assert_eq!(with_rest, without_rest, "{name}");
    }
}

/// `..` never supplies a borrowed reference. `pin` is one closed body, and
/// its construction refuses the omitted `@'t u8` field by name, with the
/// field's type at the package that declares `pin`. The `u8` twin forms the
/// zero of that field in the body.
///
/// With a static integer binder `pin` is an open template. It refuses for the
/// same reason, and its `u8` twin publishes the zero of that field.
#[test]
fn an_omitted_reference_field_has_no_zero_in_a_closed_body_or_an_open_template() {
    use omega_lowering::{OperationResult, TemplateOperand, TemplateStep};
    let source = "use omega\\language\\core;\nuse omega\\language\\core\\reflect;\ndata Pinned<'t> { at: @'t u8; }\nmachine pin<'t>(s: TypeSchema<'t>) -> Pinned<'t> reaches none crashes none terminates { Pinned{..} }\n";
    let message = "omitted field `Pinned::at` of type `@u8 at app\\pin<parameter 0>` needs a borrowed reference, which `..` never supplies";
    let cli_message = "omitted field `Pinned::at` of type `@u8 at root\\pin<parameter 0>` needs a borrowed reference, which `..` never supplies";
    sources::refuse_with_cli_message(
        &[("pinned.omg", source)],
        "pinned.omg",
        sources::unique_span(source, "Pinned{..}"),
        message,
        cli_message,
    );
    let plain = source.replace("at: @'t u8;", "at: u8;");
    let package = lower("pinned.omg", &plain).1.unwrap();
    assert!(package.machine_templates.is_empty());
    let [pin] = package.machines.as_slice() else {
        panic!("expected the closed pin body")
    };
    let [zero, record] = pin.body.operations.as_slice() else {
        panic!("expected the zero, then the construction: {:?}", pin.body.operations)
    };
    for operation in [zero, record] {
        assert_eq!(
            spelled(&pin.body.diagnostics.operations[operation.id.index()], "pinned.omg", &plain),
            "Pinned{..}"
        );
    }
    let OperationResult::Scalar(supplied) = zero.result else {
        panic!("the zero defines a scalar")
    };
    let OperationKind::EstablishRecord { fields } = &record.kind else {
        panic!("expected a record establishment, found {:?}", record.kind)
    };
    assert_eq!(zero.kind, OperationKind::Constant(ScalarLiteral::Integer(0.into())));
    assert_eq!(
        fields
            .iter()
            .map(|field| (field.field.path.as_str(), &field.operand))
            .collect::<Vec<_>>(),
        [("Pinned::at", &ValueOperand::Scalar(supplied))]
    );

    let open = reusable(source, &["pin"]);
    sources::refuse_with_cli_message(
        &[("pinned.omg", &open)],
        "pinned.omg",
        sources::unique_span(&open, "Pinned{..}"),
        message,
        cli_message,
    );
    let package = lower("pinned.omg", &reusable(&plain, &["pin"])).1.unwrap();
    assert!(package.machines.is_empty());
    let [pin] = package.machine_templates.as_slice() else {
        panic!("expected the pin template")
    };
    let [TemplateStep::Construct(construct)] = pin.body.as_slice() else {
        panic!("expected one construction")
    };
    let u8 = omega_lowering::TypeExpression(vec![omega_lowering::TypeExpressionStep::Scalar(
        omega_lowering::ScalarType::Integer { signed: false, width: 8 },
    )]);
    assert_eq!(
        construct
            .fields
            .iter()
            .map(|(field, value)| (field.path.as_str(), value))
            .collect::<Vec<_>>(),
        [("Pinned::at", &TemplateOperand::Zero(u8))]
    );
}

/// `new_plan::<'t>(@schema, keep::<'t>(schema))` writes the borrow before a
/// later operand that reads the same formal. Evaluating that helper in a
/// preceding `let` changes when the loan starts.
///
/// The closed plan calls `keep` with a copy of the formal, then the
/// constructor with a shared loan of that formal and the helper's result
/// moved in. Its loan is an operand of the constructor call in both forms, so
/// the closed body says nothing about a loan that starts earlier.
///
/// With a static integer binder the plan is an open template, and its steps
/// keep that interval: the direct form starts the loan before the helper
/// step, and the hoisted form starts it at the constructor call.
#[test]
fn an_operand_step_reading_a_borrowed_formal_keeps_the_earlier_loan_live() {
    use omega_lowering::{CallArgument, Parameter, ReferenceAccess, TemplateCallTarget, TemplateOperand, TemplateStep};
    let helper = "machine keep<'t>(s: TypeSchema<'t>) -> Issuance<'t> reaches none crashes none terminates { Issuance::Unrestricted{} }\n";
    let original = ACCESS_PLAN.replace(" access Rule;", ";");
    let constructor = "new_plan::<'t>(@schema, Issuance::Unrestricted{})";
    let after = format!("{helper}{}", original.replace(constructor, "new_plan::<'t>(@schema, keep::<'t>(schema))"));
    let before = format!(
        "{helper}{}",
        original.replace(constructor, "let i: Issuance<'t> = keep::<'t>(schema); new_plan::<'t>(@schema, i)")
    );
    for source in [&after, &before] {
        let package = lower("order.omg", source).1.unwrap();
        assert!(package.machine_templates.is_empty());
        assert_eq!(calls(&package, "Rule::plan"), [(("app", "keep"), vec![0]), ((TOOLCHAIN, NEW_PLAN), vec![0])]);
        let plan = body_of(&package, "Rule::plan");
        let [Parameter::Structural(schema)] = plan.body.parameters[..] else {
            panic!("one structural formal")
        };
        let operands: Vec<_> = plan
            .body
            .operations
            .iter()
            .filter_map(|operation| match &operation.kind {
                OperationKind::Call { arguments, .. } => Some(arguments.as_slice()),
                _ => None,
            })
            .collect();
        let [
            read,
            [
                CallArgument::Borrow { place, access },
                CallArgument::MoveWhole(_),
            ],
        ] = operands.as_slice()
        else {
            panic!("expected the helper call, then the constructor call: {operands:?}")
        };
        assert_eq!(*read, [CallArgument::CopyWhole(schema)]);
        assert_eq!((place.root, place.path.as_slice(), *access), (schema, &[][..], ReferenceAccess::Shared));
        assert_eq!(policy_issuance(&package), unrestricted());
    }

    let after = reusable(&after, &["keep"]);
    let after_package = lower("order.omg", &after).1.unwrap();
    let after_plan = after_package
        .machine_templates
        .iter()
        .find(|t| t.declaration.path == "Rule::plan")
        .unwrap();
    let [TemplateStep::Call(helper_call), TemplateStep::Call(outer)] = after_plan.body.as_slice() else {
        panic!("helper then outer call")
    };
    let [
        TemplateOperand::BorrowBefore { formal, access, step, site },
        TemplateOperand::Step(0),
    ] = outer.operands.as_slice()
    else {
        panic!("ordered borrow")
    };
    assert!(matches!(&helper_call.target, TemplateCallTarget::Declaration(id) if id.path == "keep"));
    assert_eq!(helper_call.operands, [TemplateOperand::Formal(0)]);
    assert_eq!(
        (*formal, access, *step, site.span.slice(after.as_bytes())),
        (0, &omega_psi::TemplateAccess::Shared, 0, Some(b"@schema".as_slice()))
    );
    let package = lower("order.omg", &reusable(&before, &["keep"])).1.unwrap();
    let plan = package
        .machine_templates
        .iter()
        .find(|t| t.declaration.path == "Rule::plan")
        .unwrap();
    let [TemplateStep::Call(keep), TemplateStep::Call(new_plan)] = plan.body.as_slice() else {
        panic!("expected keep, then new_plan: {:?}", plan.body)
    };
    assert!(matches!(&keep.target, TemplateCallTarget::Declaration(id) if id.path == "keep"));
    assert_eq!(
        (&keep.operands[..], &new_plan.operands[..]),
        (
            &[TemplateOperand::Formal(0)][..],
            &[
                TemplateOperand::Borrow {
                    formal: 0,
                    access: omega_lowering::TemplateAccess::Shared
                },
                TemplateOperand::Step(0)
            ][..]
        )
    );
    spec_corpus::observe_handoff!(psi "psi-output/template-borrow-order", assert_eq!(
        (psi_handoff_consumer::formal_borrow_interval(after_plan,1,0), psi_handoff_consumer::formal_borrow_interval(plan,1,0)),
        (Some((0,0..=1)), Some((0,1..=1)))
    ));
    assert_eq!(template_issuance(&package, REUSABLE_PLAN), open_unrestricted());
}

/// An array literal and a reflected descriptor in an open body, on input L3
/// types without any slice destination: the plan passes `[reflect\callable(pick)]`
/// at a fixed `[CallableInfo<'t>; 1]` formal of a local helper. The descriptor
/// names the selected machine exactly, with its arguments, and is not a call.
#[test]
fn a_descriptor_array_names_its_element_type_and_selected_machine() {
    use omega_lowering::{DeclarationKind, TemplateCallTarget, TemplateOperand, TemplateStep, TypeExpression, TypeExpressionStep};
    let helpers = "machine pick() -> bool { true }\nmachine take<'t>(routes: [CallableInfo<'t>; 1]) -> Issuance<'t> reaches none crashes none terminates { Issuance::Unrestricted{} }\n";
    let source = format!(
        "{helpers}{}",
        ACCESS_PLAN.replace(" access Rule;", ";").replace(
            "new_plan::<'t>(@schema, Issuance::Unrestricted{})",
            "new_plan::<'t>(@schema, take::<'t>([reflect\\callable(pick)]))"
        )
    );
    let package = lower("routes.omg", &source)
        .1
        .unwrap_or_else(|error| panic!("{error:?}"));
    let plan = package
        .machine_templates
        .iter()
        .find(|t| t.declaration.path == "Rule::plan")
        .unwrap();
    let [
        TemplateStep::Describe(describe),
        TemplateStep::Array(array),
        TemplateStep::Call(take),
        TemplateStep::Call(new_plan),
    ] = plan.body.as_slice()
    else {
        panic!("expected descriptor, array, helper call and constructor call: {:?}", plan.body)
    };
    let element = TypeExpression(vec![
        TypeExpressionStep::GenericParameter {
            owner: plan.declaration.clone(),
            index: 0,
        },
        TypeExpressionStep::Named {
            declaration: provided("reflect\\CallableInfo"),
            argument_count: 1,
        },
    ]);
    let mut fixed = element.0.clone();
    fixed.push(TypeExpressionStep::Array(1));
    assert_eq!(spelled(&describe.site, "routes.omg", &source), "reflect\\callable(pick)");
    spec_corpus::observe_handoff!(psi "psi-output/policy-template-descriptor-array",
        assert_eq!(
            ((describe.selection.declaration.package.as_str(), describe.selection.declaration.path.as_str(), describe.selection.declaration.kind),
                describe.selection.arguments.len() + describe.selection.enclosing_arguments.len(), &describe.ty, &array.element, &array.ty, &array.elements[..]),
            (("app", "pick", DeclarationKind::Machine), 0, &element, &element, &TypeExpression(fixed), &[TemplateOperand::Step(0)][..])
        ),
    );
    assert!(matches!(&take.target, TemplateCallTarget::Declaration(id) if id.path == "take") && take.operands == [TemplateOperand::Step(1)]);
    assert_eq!(new_plan.operands[1], TemplateOperand::Step(2));
    // `pick` is the package's own concrete machine; the descriptor names it, it does not call it.
    assert_eq!(
        package
            .machines
            .iter()
            .map(|m| m.declaration.path.as_str())
            .collect::<Vec<_>>(),
        ["pick"]
    );
    assert_eq!(template_issuance(&package, POLICY_PLAN), open_unrestricted());
}

/// The explicit route lists, exact plan-evaluation bytes.
const ONLY_EMPTY: &str = include_str!("fixtures/policy_templates/only_empty.omg");
const NONEMPTY: &str = include_str!("fixtures/policy_templates/nonempty.omg");

/// The routes of the `Issuance::Only` the closed plan hands the constructor.
fn only_routes<'a>(
    name: &str,
    package: &'a LoweredPackage,
) -> Vec<psi_handoff_consumer::BodyValue<'a>> {
    use psi_handoff_consumer::BodyValue;
    let BodyValue::Construct {
        declaration: "language\\core\\Issuance::Only",
        mut fields,
    } = policy_issuance(package)
    else {
        panic!("{name}: not Only")
    };
    let Some(("language\\core\\Issuance::Only::routes", BodyValue::Array(routes))) = fields.pop().filter(|_| fields.is_empty()) else {
        panic!("{name}: expected exactly the routes field")
    };
    routes
}

/// `Only{routes: []}` without a selecting domain. The closed plan establishes
/// the written `[]` as a sequence with no elements at the literal's own site,
/// and the case copies that sequence into `routes`.
#[test]
fn an_explicit_empty_route_list_is_an_empty_sequence_in_the_closed_plan() {
    use omega_lowering::{OperationResult, TypeShape};
    let (_, package) = unselected_twin("only_empty", ONLY_EMPTY);
    let source = ONLY_EMPTY.replace(" access Rule;", ";");
    assert_eq!(bodies(&package), [("app", "Rule::plan"), (TOOLCHAIN, NEW_PLAN)]);
    let (body, list) = operation_spelled(&package, "Rule::plan", &source, "[]");
    let (_, only) = operation_spelled(&package, "Rule::plan", &source, "Issuance::Only{routes:[]}");
    let OperationResult::Structural(sequence) = list.result else {
        panic!("the list has its own storage")
    };
    let element = package
        .data
        .iter()
        .find(|d| d.declaration == provided("reflect\\CallableInfo"))
        .expect("the package carries the toolchain's CallableInfo")
        .carrier;
    let OperationKind::EstablishCase { case, fields } = &only.kind else {
        panic!("expected a case establishment, found {:?}", only.kind)
    };
    let [routes] = fields.as_slice() else {
        panic!("expected the one payload field, found {fields:?}")
    };
    assert_eq!(
        (routes.field.package.as_str(), routes.field.path.as_str()),
        (TOOLCHAIN, "language\\core\\Issuance::Only::routes")
    );
    spec_corpus::observe_handoff!(psi "psi-output/policy-template-route-lists",
        assert_eq!(
            (&list.kind, package.types.get(body.places[sequence.index()].ty), case, &routes.operand, only_routes("only_empty", &package)),
            (&OperationKind::EstablishStructuralArray { elements: vec![] }, Some(&TypeShape::ElementSequence { element }),
                &provided("Issuance::Only"), &ValueOperand::CopyWhole(sequence), vec![])
        ),
    );
}

/// `Only{routes: [reflect\callable(pick)]}` without a selecting domain. The
/// closed plan holds one route. A descriptor is a value: no body calls
/// `pick`, and `pick` stays the package's own machine.
#[test]
fn a_described_route_is_a_value_in_the_closed_plan_and_never_a_call() {
    let (_, package) = unselected_twin("nonempty", NONEMPTY);
    assert_eq!(bodies(&package), [("app", "Rule::plan"), ("app", "pick"), (TOOLCHAIN, NEW_PLAN)]);
    assert_eq!(calls(&package, "Rule::plan"), [((TOOLCHAIN, NEW_PLAN), vec![0])]);
    assert!(calls(&package, "pick").is_empty());
    assert_eq!(only_routes("nonempty", &package).len(), 1);
}

/// With a static integer binder the plan is an open template. `[]` is an
/// empty array step at the owned `[CallableInfo<'t>]`.
/// `[reflect\callable(pick)]` holds one descriptor step naming the package's
/// own `pick` with no arguments. A descriptor is a value: no step calls
/// `pick`, no template invokes it, and it is never a callable or authority.
#[test]
fn reusable_route_lists_keep_an_empty_array_step_and_a_descriptor_step() {
    use omega_lowering::{DeclarationKind, TemplateCallTarget, TemplateStep, TypeExpression, TypeExpressionStep};
    use psi_handoff_consumer::TemplateValue;
    let mut observed = Vec::new();
    for (name, selected) in [("only_empty", ONLY_EMPTY), ("nonempty", NONEMPTY)] {
        let source = reusable(&selected.replace(" access Rule;", ";"), &[]);
        let (obligations, package) = lower("unused.omg", &source);
        assert!(
            !obligations
                .iter()
                .any(|o| matches!(o.kind, ObligationKind::EvaluateAccessPlan { .. }))
        );
        let package = package.unwrap_or_else(|error| panic!("{name}: {error:?}"));
        assert!(package.data.is_empty() && package.data_templates.is_empty(), "{name}");
        assert!(
            package
                .machines
                .iter()
                .all(|m| m.declaration.package == "app" && m.declaration.path == "pick"),
            "{name}"
        );
        let plan = package
            .machine_templates
            .iter()
            .find(|t| t.declaration.path == "Rule::plan")
            .unwrap();
        let element = TypeExpression(vec![
            TypeExpressionStep::GenericParameter {
                owner: plan.declaration.clone(),
                index: 0,
            },
            TypeExpressionStep::Named {
                declaration: provided("reflect\\CallableInfo"),
                argument_count: 1,
            },
        ]);
        let mut slice = element.0.clone();
        slice.push(TypeExpressionStep::Slice);
        let arrays: Vec<_> = plan
            .body
            .iter()
            .filter_map(|step| match step {
                TemplateStep::Array(array) => Some((&array.element, &array.ty, array.elements.len())),
                _ => None,
            })
            .collect();
        assert_eq!(arrays, [(&element, &TypeExpression(slice), usize::from(name == "nonempty"))], "{name}");
        for template in &package.machine_templates {
            assert!(template.invokes.is_empty() && template.invokes_machines.is_empty(), "{name}");
            assert!(
                template
                    .body
                    .iter()
                    .filter_map(TemplateStep::call)
                    .all(|call| !matches!(&call.target, TemplateCallTarget::Declaration(id) if id.path == "pick")),
                "{name}: a descriptor is not a call"
            );
        }
        let TemplateValue::Construct {
            declaration: "language\\core\\Issuance::Only",
            fields,
            establishes: false,
        } = template_issuance(&package, REUSABLE_PLAN)
        else {
            panic!("{name}: not Only")
        };
        let [("language\\core\\Issuance::Only::routes", TemplateValue::Array(routes))] = fields.as_slice() else {
            panic!("{name}: {fields:?}")
        };
        let described: Vec<_> = routes
            .iter()
            .map(|route| match route {
                TemplateValue::Descriptor(selection) => (
                    selection.declaration.package.clone(),
                    selection.declaration.path.clone(),
                    selection.declaration.kind,
                    selection.arguments.len() + selection.enclosing_arguments.len(),
                ),
                other => panic!("{name}: a route is not a descriptor: {other:?}"),
            })
            .collect();
        observed.push((name, described));
    }
    assert_eq!(
        observed,
        [
            ("only_empty", vec![]),
            ("nonempty", vec![("app".to_owned(), "pick".to_owned(), DeclarationKind::Machine, 0)])
        ]
    );
}

/// A construction that owes its data's `requires` keeps that obligation at
/// the construction. In the closed `mk`, `Small{n: n}` establishes the record
/// whose schema carries `self.n < 10`, at the site typing owes it, and the
/// twin without the clause owes nothing. The checker, not lowering, proves
/// the clause for the constructed value.
///
/// With a static integer binder `mk` is an open template, and the same
/// obligation is `establishes` on its construction step.
#[test]
fn a_construction_keeps_the_establishment_its_data_requires() {
    use omega_lowering::{Parameter, TemplateOperand, TemplateStep};
    let maker = "machine mk<'t>(s: TypeSchema<'t>, n: u8) -> Small reaches none crashes none terminates { Small{n: n} }\n";
    let head = "use omega\\language\\core;\nuse omega\\language\\core\\reflect;\n";
    let mut seen = Vec::new();
    let mut templates = Vec::new();
    for data in [
        "data Small requires self.n < 10 { n: u8; }\n",
        "data Small { n: u8; }\n",
    ] {
        let source = format!("{head}{data}{maker}");
        let (obligations, package) = lower("small.omg", &source);
        let owed: Vec<_> = obligations
            .iter()
            .filter(|o| matches!(o.kind, ObligationKind::EstablishData(_)))
            .collect();
        let package = package.unwrap_or_else(|error| panic!("{error:?}"));
        assert!(package.machine_templates.is_empty());
        let [mk] = package.machines.as_slice() else {
            panic!("expected the closed mk body")
        };
        let [Parameter::Structural(_), Parameter::Scalar(n)] = mk.body.parameters[..] else {
            panic!("the schema, then the scalar")
        };
        let [small] = mk.body.operations.as_slice() else {
            panic!("expected one construction: {:?}", mk.body.operations)
        };
        let OperationKind::EstablishRecord { fields } = &small.kind else {
            panic!("expected a record establishment, found {:?}", small.kind)
        };
        assert_eq!(
            fields
                .iter()
                .map(|f| (f.field.path.as_str(), &f.operand))
                .collect::<Vec<_>>(),
            [("Small::n", &ValueOperand::Scalar(n))]
        );
        let schema = result_schema(&package, &mk.body, small);
        assert_eq!(schema.declaration.path, "Small");
        let site = &mk.body.diagnostics.operations[small.id.index()];
        seen.push((owed.len(), owed.iter().filter(|o| &o.site == site).count(), schema.requires.len()));

        let (obligations, package) = lower("small.omg", &reusable(&source, &["mk"]));
        let owed = obligations
            .iter()
            .filter(|o| matches!(o.kind, ObligationKind::EstablishData(_)))
            .count();
        let package = package.unwrap_or_else(|error| panic!("{error:?}"));
        assert!(package.machines.is_empty());
        let [mk] = package.machine_templates.as_slice() else { panic!("expected mk") };
        let [TemplateStep::Construct(small)] = mk.body.as_slice() else {
            panic!("expected one construction: {:?}", mk.body)
        };
        assert_eq!(
            small
                .fields
                .iter()
                .map(|(f, v)| (f.path.as_str(), v))
                .collect::<Vec<_>>(),
            [("Small::n", &TemplateOperand::Formal(1))]
        );
        templates.push((
            owed,
            small.establishes,
            package
                .data
                .iter()
                .find(|d| d.declaration.path == "Small")
                .map(|d| d.requires.len()),
        ));
    }
    spec_corpus::observe_handoff!(psi "psi-output/policy-template-construct-establishment",
        assert_eq!(seen, [(1, 1, 1), (0, 0, 0)]),
    );
    assert_eq!(templates, [(1, true, Some(1)), (0, false, Some(0))]);
}

/// A source-defined `local\new_plan` with the toolchain constructor's name
/// and signature. Called module-qualified, the plan names the package's own
/// body, which itself calls the toolchain's; the bare call in the same
/// package still names the toolchain's. No identity is taken from the
/// spelling, in the closed bodies or in the reusable templates.
#[test]
fn a_same_named_local_constructor_keeps_its_own_owner() {
    use psi_handoff_consumer::{BodyCallee, TemplateCallee};
    let local = "use omega\\language\\core;\nuse omega\\language\\core\\reflect;\nuse omega\\language\\core\\access;\npublic machine new_plan<'t>(schema: @TypeSchema<'t>, issuance: Issuance<'t>) -> AccessPlan<'t> reaches none crashes none terminates { access\\new_plan::<'t>(schema, issuance) }\n";
    let original = ACCESS_PLAN.replace(" access Rule;", ";");
    let qualified = original.replace("new_plan::<'t>(@schema", "local\\new_plan::<'t>(@schema");
    let expected = [
        ("app".to_owned(), "local\\new_plan".to_owned()),
        (TOOLCHAIN.to_owned(), NEW_PLAN.to_owned()),
    ];
    let mut seen = Vec::new();
    for main in [&qualified, &original] {
        let package = sources::lower(&[("main.omg", main), ("local/plan.omg", local)]).unwrap_or_else(|error| panic!("{error:?}"));
        assert!(package.machine_templates.is_empty());
        // The public local constructor is a body of this package whether or not the plan calls it.
        assert_eq!(
            bodies(&package),
            [
                ("app", "Rule::plan"),
                ("app", "local\\new_plan"),
                (TOOLCHAIN, NEW_PLAN)
            ]
        );
        let plan = psi_handoff_consumer::member_machine(&package, "Rule", POLICY_PLAN).unwrap();
        let owner = match psi_handoff_consumer::returned_body_call(&package, plan)
            .unwrap()
            .callee
        {
            BodyCallee::Packaged(body) => {
                let inner = psi_handoff_consumer::returned_body_call(&package, body).unwrap();
                assert_eq!(
                    inner.callee,
                    BodyCallee::Provided {
                        package: TOOLCHAIN,
                        path: NEW_PLAN
                    }
                );
                (body.declaration.package.clone(), body.declaration.path.clone())
            }
            BodyCallee::Provided { package, path } => (package.to_owned(), path.to_owned()),
        };
        assert_eq!(policy_issuance(&package), unrestricted());
        seen.push(owner);
    }
    assert_eq!(seen, expected);

    let local = local.replace("machine new_plan<'t>(", "machine new_plan<'t, const N: u64>(");
    let qualified = reusable(&qualified, &[]).replace("local\\new_plan::<'t>(", "local\\new_plan::<'t, N>(");
    let original = reusable(&original, &[]);
    let mut seen = Vec::new();
    for main in [&qualified, &original] {
        let package = sources::lower(&[("main.omg", main), ("local/plan.omg", &local)]).unwrap_or_else(|error| panic!("{error:?}"));
        let plan = psi_handoff_consumer::member_template_call(&package, "Rule", REUSABLE_PLAN).unwrap();
        let owner = match plan.callee {
            TemplateCallee::Packaged(template) => {
                let inner = psi_handoff_consumer::returned_template_call(&package, template).unwrap();
                assert_eq!(
                    inner.callee,
                    TemplateCallee::Provided {
                        package: TOOLCHAIN,
                        path: NEW_PLAN
                    }
                );
                (template.declaration.package.clone(), template.declaration.path.clone())
            }
            TemplateCallee::Provided { package, path } => (package.to_owned(), path.to_owned()),
        };
        assert!(
            package
                .machine_templates
                .iter()
                .all(|t| t.declaration.package == "app")
        );
        seen.push(owner);
    }
    assert_eq!(seen, expected);
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
    use omega_lowering::{AggregateRegionSlot, MachineRegion};
    use omega_psi::{DataArgument, DataFormation, ScalarType, TemplateBinder, TypeExpression, TypeExpressionStep as Step, TypeShape};
    let file = "recursive.omg";
    let changing = "data Link<'r,const N:u64>{next:@'r Link<'r,N+1>;value:u<N>;} machine caller<'r>(x:Link<'r,3>){}";
    let same = changing.replace("@'r Link<'r,N+1>", "@'r Link<'r,N>");
    let (obligations, psi) = lower(file, changing);
    assert!(
        obligations
            .iter()
            .any(|o| matches!(&o.kind, ObligationKind::FormRecursiveApplication { declaration, active_arguments, arguments }
                if active_arguments.iter().map(ToString::to_string).eq([r"app\caller<parameter 0>", "3"]) && declaration.path == "Link"
                    && arguments.iter().map(ToString::to_string).eq([r"app\caller<parameter 0>", "4"]))
                && spells(&o.site, file, changing, "Link<'r,3>")),
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
        // The lifetime selects no schema, so the closed argument alone is the
        // type argument. The region positions stay beside the rows: the
        // template's binder list, the schema's count and its stored
        // reference, and the machine's slot for the formal.
        assert_eq!(link.type_arguments, [DataArgument::Integer {carrier,value:3.into()}]);
        assert_eq!(template.parameters, [TemplateBinder::Lifetime, TemplateBinder::Const(TypeExpression(vec![Step::Scalar(carrier)]))]);
        assert_eq!(link.lifetime_count, 1);
        assert_eq!(psi.machines[0].regions, [MachineRegion {
            aggregate_parameters: vec![AggregateRegionSlot { parameter: 0, binder: 0 }],
            ..Default::default()
        }]);
        let [edge, width] = link.fields.as_slice() else {panic!("next, value")};
        assert_eq!((edge.declaration.path.as_str(), width.declaration.path.as_str()), ("Link::next", "Link::value"));
        assert_eq!((edge.lifetime, edge.aggregate_lifetimes.as_slice()), (Some(0), &[0][..]));
        assert_eq!((width.lifetime, width.aggregate_lifetimes.as_slice()), (None, &[][..]));
        assert_eq!(psi.types.get(width.carrier), Some(&TypeShape::PrimitiveScalar(ScalarType::Integer {signed:false,width:3})));
        let Some(TypeShape::IndexedSharedReference(id)) = psi.types.get(edge.carrier) else { panic!("shared recursion") };
        assert_eq!(psi.types.indexed_referent(*id), Some((&link.declaration, [DataArgument::Integer { carrier, value: next.into() }].as_slice())));
        // Binder 0 is the lifetime `'r` and binder 1 the index `N`.
        let region = Step::GenericParameter { owner: link.declaration.clone(), index:0 };
        let parameter = Step::GenericParameter { owner: link.declaration.clone(), index:1 };
        let integer = |n: i32| Step::StaticInteger { carrier, value: n.into() };
        // `@'r Link<'r, N+1>`: the referent applied at `'r` and the next
        // index, behind a shared reference held at `'r`.
        let mut referred = vec![region.clone(), parameter.clone()];
        if next == 4 {
            referred.extend([integer(1), Step::IndexSum { carrier, operands: 2 }]);
        }
        referred.extend([
            Step::Named { declaration: link.declaration.clone(), argument_count: 2 },
            Step::Reference(omega_psi::TemplateAccess::Shared),
            region.clone(),
            Step::AtLifetime,
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
            // Each application keeps its lifetime argument first: the data
            // binder `'r` inside the declaration, and `caller`'s own region
            // binder where its formal applies `Link`.
            let applied = TypeExpression(vec![Step::GenericParameter { owner: psi.machines[0].declaration.clone(), index: 0 }]);
            let prior = [TypeExpression(vec![region.clone()]), TypeExpression(vec![parameter.clone(), integer(1), Step::IndexSum {carrier,operands:2}])];
            let open = [TypeExpression(vec![region]), TypeExpression(vec![parameter, integer(1), integer(1), Step::IndexSum {carrier,operands:3}])];
            let closed = [applied.clone(), TypeExpression(vec![integer(4)])];
            let actual = [applied, TypeExpression(vec![integer(3)])];
            spec_corpus::observe_handoff!(psi "psi-output/recursive-index-formation", assert_eq!(recursive, [
                ("@'r Link<'r,N+1>", prior.as_slice(), open.as_slice()),
                ("Link<'r,3>", actual.as_slice(), closed.as_slice()),
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
    let source = "data Link<'r,const N:u64>{next:@'r Link<'r,N+1>;value:u<N>;} machine read<'r>(x:Link<'r,3>)->@Link<'r,4>{x.next}";
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
    let plain = "data Leaf{v:u8;} data Node<'r>{next:@'r Leaf;} machine read<'r>(x:Node<'r>)->@Leaf{x.next}";
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

/// A lifetime argument selects no storage. `first`, `second` and `keep` apply
/// `Link` at four different machine regions and the same closed arguments, so
/// their formals share one `Link<3>` schema and one `@Link<4>` referent, which
/// is also the referent the schema's own `next` field names at the data
/// binder `'r`. `Link<5>` is a second schema. What a row forgets stays beside
/// it: the schema's lifetime count, the binder its stored reference is held
/// at and the binder that fills the referent, and each machine's region slots.
#[test]
fn machine_regions_at_the_same_const_arguments_share_one_schema_and_keep_their_positions() {
    use omega_lowering::{AggregateRegionSlot, MachineRegion};
    use omega_psi::{DataArgument, ScalarType, TemplateBinder, TypeExpression, TypeExpressionStep as Step, TypeShape};
    let file = "shared_link.omg";
    let source = "data Link<'r,const N:u64>{next:@'r Link<'r,N+1>;value:u<N>;} \
        machine first<'a>(x:Link<'a,3>){} \
        machine second<'p,'q>(w:Link<'p,3>,x:Link<'q,3>){} \
        machine third<'a>(x:Link<'a,5>){} \
        machine keep<'a>(x:@Link<'a,4>)->@Link<'a,4>{x}";
    let (_, psi) = lower(file, source);
    let psi = psi.unwrap_or_else(|error| panic!("every machine has one body: {error:?}"));
    assert!(psi.machine_templates.is_empty(), "{:?}", psi.machine_templates);
    let carrier = ScalarType::Integer { signed: false, width: 64 };
    let integer = |value: i32| DataArgument::Integer { carrier, value: value.into() };

    // The template keeps the lifetime binder at its position before the index.
    let [template] = psi.data_templates.as_slice() else { panic!("one Link template") };
    assert_eq!(
        template.parameters,
        [TemplateBinder::Lifetime, TemplateBinder::Const(TypeExpression(vec![Step::Scalar(carrier)]))]
    );

    // One schema for each closed argument, whatever regions apply it.
    let [three, five] = psi.data.as_slice() else { panic!("Link<3> and Link<5>: {:?}", psi.data) };
    assert_eq!((&three.type_arguments, &five.type_arguments), (&vec![integer(3)], &vec![integer(5)]));
    assert_eq!((&three.declaration, &five.declaration), (&template.declaration, &template.declaration));
    assert_ne!((three.id, three.carrier), (five.id, five.carrier));
    for (schema, successor) in [(three, 4), (five, 6)] {
        assert_eq!(psi.types.get(schema.carrier), Some(&TypeShape::Nominal(schema.id)));
        // The data region positions: `next` is a reference held at binder 0,
        // and binder 0 fills the one lifetime binder of the `Link` it refers to.
        assert_eq!(schema.lifetime_count, 1);
        let [next, value] = schema.fields.as_slice() else { panic!("next, value") };
        assert_eq!((next.lifetime, next.aggregate_lifetimes.as_slice()), (Some(0), &[0][..]));
        assert_eq!((value.lifetime, value.aggregate_lifetimes.as_slice()), (None, &[][..]));
        let Some(TypeShape::IndexedSharedReference(id)) = psi.types.get(next.carrier) else { panic!("shared recursion") };
        assert_eq!(psi.types.indexed_referent(*id), Some((&template.declaration, [integer(successor)].as_slice())));
    }

    // The machine region positions: each binder lists the formal it fills and
    // the data binder it fills there, and for `keep` the result's as well.
    let machine = |name: &str| psi.machines.iter().find(|m| m.declaration.path == name).unwrap_or_else(|| panic!("no machine {name}"));
    let region = |parameter: usize, result: &[usize]| MachineRegion {
        aggregate_parameters: vec![AggregateRegionSlot { parameter, binder: 0 }],
        aggregate_result: result.to_vec(),
        ..Default::default()
    };
    let stored_reference = three.fields[0].carrier;
    for (name, formals, regions) in [
        ("first", vec![three.carrier], vec![region(0, &[])]),
        ("second", vec![three.carrier, three.carrier], vec![region(0, &[]), region(1, &[])]),
        ("third", vec![five.carrier], vec![region(0, &[])]),
        ("keep", vec![stored_reference], vec![region(0, &[0])]),
    ] {
        let lowered = machine(name);
        assert_eq!(lowered.parameter_types, formals, "{name}");
        assert_eq!(lowered.regions, regions, "{name}");
        // A region is not a static argument: no machine here is a specialization.
        assert!(lowered.arguments.is_empty(), "{name}");
    }
    // `keep` at `'a` returns the very row the schema's field stores at `'r`.
    assert_eq!(machine("keep").result_type, Some(stored_reference));
}

/// Returning a stored `@mutable Leaf` by value from an owned record transfers the
/// caller's loan out of the field (language/ownership.md: "A by-value
/// reference transfers or copies its loan carrier"). The result is that stored
/// loan itself, so it points into the original backing; nothing copies it,
/// downgrades it to a shared read, or discards it with the record.
#[test]
fn reading_a_stored_exclusive_reference_transfers_its_loan() {
    use omega_lowering::{BlockTerminator, CleanupAction, PlaceProjection, ProjectedPlace, ReturnEdge};
    use psi_handoff_consumer::{ExclusiveOrigin, ReturnedExclusive, returned_exclusive};
    let file = "load_next.omg";
    let source = "data Leaf{v:u8;} data Node<'r>{next:@'r mutable Leaf;} machine read<'r>(x:Node<'r>)->@mutable Leaf{x.next}";
    let (obligations, package) = lower(file, source);
    let mut package = package.unwrap();
    // L3 hands the return relation to the checker; L5 keeps it for L6.
    assert!(
        obligations
            .iter()
            .any(|o| matches!(&o.kind, omega_typed::ObligationKind::ReturnBorrow { machine } if machine.path == "read"))
    );
    let machine = &package.machines[0];
    let body = &machine.body;
    let [omega_lowering::Parameter::Structural(x)] = body.parameters[..] else {
        panic!("one incoming record")
    };
    let [op] = body.operations.as_slice() else { panic!("one transfer") };
    let OperationKind::MoveStructural { source: moved } = &op.kind else {
        panic!("a by-value exclusive read must transfer, not copy or reborrow: {:?}", op.kind)
    };
    assert_eq!(moved.root, x);
    assert!(matches!(moved.path.as_slice(), [PlaceProjection::Field(f)] if f.path == "Node::next"));
    let [block] = body.blocks.as_slice() else { panic!("one block") };
    let BlockTerminator::Return {
        result: ReturnEdge::Structural { source: returned, .. },
        cleanup,
        ..
    } = &block.terminator
    else {
        panic!("a structural return")
    };
    assert_eq!(op.result, omega_lowering::OperationResult::Structural(*returned));
    assert!(cleanup.is_empty(), "the only field left the record with the result");
    let expected = ReturnedExclusive {
        origin: ExclusiveOrigin::Transfer,
        formal: 0,
        path: vec!["Node::next".into()],
        moves_of_source: 1,
        cleanup: vec![],
    };
    spec_corpus::observe_handoff!(psi "psi-output/stored-exclusive-transfer", assert_eq!(returned_exclusive(&package, "read"), Ok(expected)));

    // Corrupted products the reader must reject or tell apart.
    let moved = moved.clone();
    let original = package.machines[0].body.operations[0].kind.clone();
    package.machines[0].body.operations[0].kind = OperationKind::CopyStructural { source: moved.clone() };
    assert_eq!(returned_exclusive(&package, "read"), Err("an exclusive reference is copied"));
    package.machines[0].body.operations[0].kind = OperationKind::BorrowPlace {
        source: ProjectedPlace {
            root: x,
            path: [moved.path.clone(), vec![PlaceProjection::Referent]].concat(),
        },
        access: omega_lowering::ReferenceAccess::Shared,
    };
    assert_eq!(returned_exclusive(&package, "read"), Err("a mutable reborrow became shared"));
    package.machines[0].body.operations[0].kind = original;
    let BlockTerminator::Return { cleanup, .. } = &mut package.machines[0].body.blocks[0].terminator else {
        unreachable!()
    };
    cleanup.push(CleanupAction::DiscardRoot(x));
    assert_eq!(returned_exclusive(&package, "read"), Err("cleanup discards the transferred loan"));
}

/// Exporting `use_it` owes external callers its open interface, so the package
/// publishes the template beside the closed instance that `f` selects. The
/// template names the binder's restricted trait `Local` and calls the base
/// requirement `Log::put` through that binder. A consumer reading Psi alone
/// follows `Local` to its base and finds the requirement and the wildcard row
/// (`reaches none`) that the checker must hold every later selection to.
#[test]
fn a_closed_restricted_instance_does_not_replace_an_exported_template() {
    use omega_lowering::{MachineTemplateBinder, TemplateCallTarget, TemplateStep};
    use psi_handoff_consumer::machine_templates::restricted_member;
    let file = "export.omg";
    let template = "machine use_it<const E:Local>(){E::put()}";
    let prefix = "public trait Log{machine put();} public trait Local=Log{machine * reaches none;} Quiet satisfies Log{machine put(){}}";
    let program = |wrapper: &str| format!("{prefix} {wrapper} machine f(){{use_it::<Quiet>()}}");
    let local = lower(file, &program(template)).1.unwrap();
    assert!(local.machines.iter().any(|m| m.declaration.path == "use_it"));
    assert!(local.machine_templates.is_empty());

    // The exact bytes of the readiness fixture `handoff/exported wrapper with a restricted evidence binder`.
    let public = program(&format!("public {template}"));
    let package = lower(file, &public).1.unwrap();
    assert!(
        package.machines.iter().any(|m| m.declaration.path == "use_it"),
        "f still calls its closed instance"
    );
    let [open] = package.machine_templates.as_slice() else {
        panic!("expected the exported use_it template")
    };
    let [MachineTemplateBinder::Implementation(slot)] = open.parameters.as_slice() else {
        panic!("expected the evidence binder, got {:?}", open.parameters)
    };
    let [TemplateStep::Call(call)] = open.body.as_slice() else {
        panic!("expected the one member call")
    };
    spec_corpus::observe_handoff!(
        psi "psi-output/exported-restricted-template",
        assert!(
            open.public && open.declaration.path == "use_it" && slot.declaration.path == "Local" && slot.arguments.is_empty()
                && matches!(&call.target, TemplateCallTarget::Member { binder: 0, requirement } if requirement.path == "Log::put")
                && open.result.is_none()
        ),
    );
    assert_eq!(spelled(&call.site, file, &public), "E::put()");
    let member = restricted_member(&package, open, 0).expect("Local resolves to its published base");
    let limits: Vec<_> = member
        .limits
        .iter()
        .map(|row| (row.prohibits_block, row.prohibits_suspend, row.reach.as_deref(), row.requires_termination))
        .collect();
    spec_corpus::observe_handoff!(
        psi "psi-output/exported-restricted-template-consumer",
        assert_eq!(
            (member.restriction.path.as_str(), member.base.path.as_str(), member.requirement.declaration.path.as_str(), limits.as_slice()),
            ("Local", "Log", "Log::put", [(false, false, Some(&[][..]), false)].as_slice())
        ),
    );

    // Without the local caller, only the template remains; the call through
    // `Local` still names the base requirement.
    let alone = format!("{} public {template}", prefix.replace(" Quiet satisfies Log{machine put(){}}", ""));
    let package = lower(file, &alone).1.unwrap();
    assert!(package.machines.iter().all(|m| m.declaration.path != "use_it"));
    let [open] = package.machine_templates.as_slice() else {
        panic!("expected the exported use_it template")
    };
    assert_eq!(
        restricted_member(&package, open, 0).map(|m| m.requirement.declaration.path.as_str()),
        Some("Log::put")
    );
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
