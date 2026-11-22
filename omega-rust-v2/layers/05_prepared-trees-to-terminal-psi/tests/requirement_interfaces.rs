#[allow(dead_code)]
#[path = "obligation_outputs/pipeline.rs"]
mod pipeline;
#[path = "support/source.rs"]
mod sources;
use omega_lowering::{PredicateOperator, PredicateStep, ScalarType, TypeShape};

/// Receipt for the L5 readiness report, printed after the checks above it pass.
fn observed(label: &str) {
    if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
        println!("OMEGA_L5_ASSERT\t{label}");
    }
}

/// Each conformance row carries one `RefineRequirement` naming the applied
/// requirement and its implementation at that row. The Psi conformance member
/// at the same site names that requirement, its trait arguments and that
/// implementation. Two conformances of one trait swap their implementations,
/// and a member call through each conformance dispatches to its own row.
#[test]
fn refine_requirement_names_the_supplied_implementation_of_each_conformance_row() {
    use omega_lowering::{CallTarget, MemberRealization, OperationKind, TypeExpression, TypeExpressionStep};
    use omega_typed::{ObligationKind, ValueType};
    let source = "trait Render<T>{machine paint(x:T)->bool; machine fill(x:T)->bool;}
                  machine red(x:u8)->bool{true} machine blue(x:u8)->bool{false}
                  Red satisfies Render<u8>{Render::paint=red; Render::fill=blue;}
                  Blue satisfies Render<u8>{Render::paint=blue; Render::fill=red;}
                  machine use_map<T,const E:Render<T>>(x:T)->bool{E::paint(x)}
                  machine one(x:u8)->bool{use_map::<u8,Red>(x)} machine two(x:u8)->bool{use_map::<u8,Blue>(x)}";
    let lowered = pipeline::lower(source);
    let package = &lowered.package;
    let refinements = lowered.obligations("RefineRequirement");
    let rows = [
        ("Red", "Render::paint", "red"),
        ("Red", "Render::fill", "blue"),
        ("Blue", "Render::paint", "blue"),
        ("Blue", "Render::fill", "red"),
    ];
    assert_eq!(refinements.len(), rows.len(), "one refinement per conformance row: {refinements:?}");
    let byte = TypeExpression(vec![TypeExpressionStep::Scalar(ScalarType::Integer {
        signed: false,
        width: 8,
    })]);
    for (owner, requirement, implementation) in rows {
        let row = format!("{requirement}={implementation};");
        let [obligation] = refinements
            .iter()
            .filter(|o| lowered.spelling(&o.site) == row)
            .collect::<Vec<_>>()[..]
        else {
            panic!("{owner}: expected one refinement at {row}")
        };
        let ObligationKind::RefineRequirement {
            requirement: slot,
            implementation: selected,
        } = &obligation.kind
        else {
            unreachable!()
        };
        assert_eq!(
            (slot.declaration.path.as_str(), selected.path.as_str()),
            (requirement, implementation),
            "{owner}"
        );
        assert_eq!(slot.trait_arguments, [ValueType::Integer { signed: false, width: Some(8) }], "{owner}");

        let conformance = package
            .conformances
            .iter()
            .find(|c| c.declaration.path == owner)
            .unwrap_or_else(|| panic!("missing conformance {owner}"));
        assert_eq!(conformance.implemented.declaration.path, "Render");
        let [member] = conformance
            .members
            .iter()
            .filter(|m| m.site == obligation.site)
            .collect::<Vec<_>>()[..]
        else {
            panic!("{owner}: expected one Psi member at {row}")
        };
        assert_eq!(member.requirement.declaration.path, requirement, "{owner}");
        assert_eq!(member.requirement.arguments, std::slice::from_ref(&byte), "{owner}");
        assert_eq!(member.implementation.path, implementation, "{owner}");
    }

    // The consuming boundary: a member call through each conformance reaches its paint row.
    for (caller, owner, paint) in [("one", "Red", "red"), ("two", "Blue", "blue")] {
        let by_id = |id: omega_lowering::MachineId| {
            package
                .machines
                .iter()
                .find(|m| m.id == id)
                .expect("packaged machine")
        };
        let [call] = lowered.machine(caller).body.operations.as_slice() else {
            panic!("{caller}: expected one wrapper call")
        };
        let OperationKind::Call {
            callee: CallTarget::Machine(wrapper),
            ..
        } = call.kind
        else {
            panic!("{caller}: wrapper must be closed")
        };
        let [member_call] = by_id(wrapper).body.operations.as_slice() else {
            panic!("{caller}: expected one member call")
        };
        let (selection, _) = psi_handoff_consumer::member_call(package, &member_call.kind).expect("member selection");
        let MemberRealization::Conformance { declaration, machine } = &selection.target else {
            panic!("{caller}: lost conformance")
        };
        assert_eq!(
            (declaration.path.as_str(), selection.requirement.declaration.path.as_str()),
            (owner, "Render::paint")
        );
        assert_eq!(by_id(*machine).declaration.path, paint, "{caller}");
    }
    observed("obligation-output/refine-requirement");
}

#[test]
fn requirement_reach_names_the_declared_service_independently_of_its_owner() {
    let text = "boundary trait Radio<T> {} boundary trait MotorControl {
        machine set_speed(speed:u32) reaches Radio<u8> crashes none;
        machine stop() reaches MotorControl crashes none;
    }";
    let package = sources::lower(&[("motor.omg", text)]).unwrap();
    assert!(package.machines.is_empty());
    let motor = package
        .traits
        .iter()
        .find(|t| t.declaration.path == "MotorControl")
        .unwrap();
    assert!(motor.boundary);
    let speed = motor
        .requirements
        .iter()
        .find(|r| r.declaration.path.ends_with("set_speed"))
        .unwrap();
    let [radio] = speed.reach.as_deref().unwrap() else {
        panic!("expected the applied Radio service")
    };
    assert_eq!(radio.declaration.path, "Radio");
    assert_eq!(
        radio.arguments[0].0,
        [omega_lowering::TypeExpressionStep::Scalar(
            ScalarType::Integer { signed: false, width: 8 }
        )]
    );
    assert_eq!(speed.crash_ceiling.as_deref(), Some(&[][..]));
    assert_eq!(
        package.types.get(speed.parameter_types[0].carrier()),
        Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 32 }))
    );
    let stop = motor
        .requirements
        .iter()
        .find(|r| r.declaration.path.ends_with("stop"))
        .unwrap();
    assert_eq!(stop.reach.as_ref().unwrap()[0].declaration, motor.declaration);
}

#[test]
fn a_runtime_index_has_its_own_carrier_and_contract_slot() {
    let text = "trait ByteScanner { machine scan<Width:u32>(limit:u32)->u64 requires Width == 16 || Width == limit; }";
    let package = sources::lower(&[("scan.omg", text)]).unwrap();
    let scan = &package.traits[0].requirements[0];
    assert_eq!(scan.reach, None);
    assert_eq!(scan.crash_ceiling, None);
    assert_eq!(scan.parameter_types, [omega_lowering::ParameterType::Scalar(scan.value_indices[0])]);
    use PredicateStep::{Apply, Formal, Integer, ValueIndex};
    assert_eq!(
        scan.requires[0].predicate,
        [
            ValueIndex(0),
            Integer(16.into()),
            Apply(PredicateOperator::Equal),
            ValueIndex(0),
            Formal(0),
            Apply(PredicateOperator::Equal),
            Apply(PredicateOperator::Or)
        ]
    );
}

#[test]
fn shared_byte_fields_retain_their_region_slots_while_sharing_a_storage_type() {
    let text = "data Pair<'first,'second> { left:@'second [u8]; right:@'first [u8]; }";
    let package = sources::lower(&[("pair.omg", text)]).unwrap();
    let pair = &package.data[0];
    assert_eq!(pair.lifetime_count, 2);
    let [left, right] = pair.fields.as_slice() else {
        panic!("expected both fields")
    };
    assert_eq!((left.lifetime, right.lifetime), (Some(1), Some(0)));
    assert_eq!(left.carrier, right.carrier);
    assert_eq!(package.types.get(left.carrier), Some(&TypeShape::SharedByteView));
}

#[test]
fn finite_index_alternatives_keep_their_correlations_when_normalized() {
    use omega_lowering::ScalarLiteral::Integer;
    let text = "trait Scan { machine go<X:u8,Y:u8>() requires (X == 3 && Y == 9) || (Y == 4 && X == 7) || (Y == 9 && X == 3); }";
    let package = sources::lower(&[("rows.omg", text)]).unwrap();
    let rows = package.traits[0].requirements[0].family.as_ref().unwrap();
    assert_eq!(
        *rows,
        [
            vec![Integer(3.into()), Integer(9.into())],
            vec![Integer(7.into()), Integer(4.into())]
        ]
    );
}

#[test]
fn indexed_record_default_domain_names_the_stored_field_and_runtime_limit() {
    use omega_lowering::PredicateStep::{Apply, Field, SelfValue, ValueIndex};
    let text = "data Index<Limit:u32> requires value < Limit { value:u32; }";
    let package = sources::lower(&[("index.omg", text)]).unwrap();
    let schema = &package.data[0];
    let field = &schema.fields[0];
    assert_eq!(schema.value_indices, [field.carrier]);
    assert_eq!(
        schema.requires[0].predicate,
        [
            SelfValue,
            Field(field.declaration.clone()),
            ValueIndex(0),
            Apply(PredicateOperator::LessThan)
        ]
    );
}

#[test]
fn case_constraints_compare_the_declared_type_parameter_for_each_case() {
    use PredicateStep::{Apply, IsCase, SelfValue, TypesEqual};
    use omega_lowering::{TypeExpression, TypeExpressionStep as Term};
    let source = "data Value<T> requires ((self in Value::Integer) && T == i32) || ((self in Value::Boolean) && T == bool) { case Integer(value:i32); case Boolean(value:bool); }";
    let package = sources::lower(&[("value.omg", source)]).unwrap();
    let value = &package.data_templates[0];
    let parameter = TypeExpression(vec![Term::GenericParameter {
        owner: value.declaration.clone(),
        index: 0,
    }]);
    let integer = TypeExpression(vec![Term::Scalar(ScalarType::Integer { signed: true, width: 32 })]);
    let boolean = TypeExpression(vec![Term::Scalar(ScalarType::Boolean)]);
    assert_eq!(value.cases.len(), 2);
    assert_eq!(value.cases[0].fields[0].ty, integer);
    assert_eq!(value.cases[1].fields[0].ty, boolean);
    assert_eq!(
        value.requires[0].predicate,
        [
            SelfValue,
            IsCase(value.cases[0].declaration.clone()),
            TypesEqual {
                left: parameter.clone(),
                right: integer
            },
            Apply(PredicateOperator::And),
            SelfValue,
            IsCase(value.cases[1].declaration.clone()),
            TypesEqual {
                left: parameter,
                right: boolean
            },
            Apply(PredicateOperator::And),
            Apply(PredicateOperator::Or),
        ]
    );
}

#[test]
fn a_guarded_crash_is_published_as_its_route_not_an_unconditional_ceiling() {
    use PredicateStep::{Apply, Formal, Integer};
    let src = "trait Service { machine read(x:u8, y:u8) crashes Trap y == 0; }";
    let package = sources::lower(&[("route.omg", src)]).unwrap();
    let read = &package.traits[0].requirements[0];
    assert_eq!(read.crash_ceiling.as_deref(), Some(&[omega_lowering::CrashCause::Trap][..]));
    let [route] = read.crash_routes.as_slice() else {
        panic!("expected one published route")
    };
    assert_eq!(route.cause, omega_lowering::CrashCause::Trap);
    assert_eq!(route.site.span.slice(src.as_bytes()), Some(b"crashes Trap y == 0".as_slice()));
    let guard = route.guard.as_ref().expect("the guard stays on its route");
    assert_eq!(guard.site.span.slice(src.as_bytes()), Some(b"y == 0".as_slice()));
    assert_eq!(guard.predicate, [Formal(1), Integer(0.into()), Apply(PredicateOperator::Equal)]);
    // The other formal is a different entry identity.
    let other = sources::lower(&[("route.omg", &src.replace("y == 0", "x == 0"))]).unwrap();
    let guard = other.traits[0].requirements[0].crash_routes[0]
        .guard
        .as_ref()
        .unwrap();
    assert_eq!(guard.predicate, [Formal(0), Integer(0.into()), Apply(PredicateOperator::Equal)]);
    // An unconditional route is Truth, not a missing guard of the same cause.
    let plain = sources::lower(&[("route.omg", &src.replace(" y == 0", ""))]).unwrap();
    let [route] = plain.traits[0].requirements[0].crash_routes.as_slice() else {
        panic!("expected one unconditional route")
    };
    assert_eq!((route.cause, route.guard.as_ref()), (omega_lowering::CrashCause::Trap, None));
}

#[test]
fn a_trait_publishes_its_direct_applied_parent() {
    let case = include_str!("fixtures/package_cases/declared-trait-parent/case/case.omg");
    let control = include_str!("fixtures/package_cases/declared-trait-parent/control/case.omg");
    let resolved = sources::resolve(&[("case.omg", case)]);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let parents = &prepared.typed().trait_parents;
    let (child, base) = parents.iter().find(|(child, _)| child.path == "C").unwrap();
    assert_eq!((child.path.as_str(), base.declaration.path.as_str(), base.trait_arguments.len()), ("C", "P", 0));
    let pkg = sources::lower_prepared(&prepared, &[("case.omg", case)]).unwrap();
    drop(prepared);
    let plain = sources::lower(&[("case.omg", control)]).unwrap();
    let get = psi_handoff_consumer::declared_trait_parent;
    let parent = get(&pkg, "C").unwrap().unwrap();
    spec_corpus::observe_handoff!(psi "psi-output/declared-trait-parent", assert_eq!(
        ((parent.0.declaration.path.as_str(), parent.1.len()), get(&plain, "C"), get(&pkg, "P")),
        (("P", 0), Some(None), Some(None))
    ));

    let source = "trait P<T>{machine id(x:T)->T;} trait Q<T>{machine id(x:T)->T;} trait C:Q<u8>{}
                  machine same(x:u8)->u8{x} Selected satisfies C {Q::id = same;}";
    let pkg = sources::lower(&[("case.omg", source)]).unwrap();
    let (parent, arguments) = get(&pkg, "C").unwrap().unwrap();
    let [requirement] = &parent.dependent_requirements[..] else { panic!("expected parent's id") };
    let [row] = &pkg.conformances[0].members[..] else { panic!("expected inherited row") };
    assert_eq!(requirement.declaration.path, "Q::id");
    assert_eq!(row.requirement.declaration, requirement.declaration);
    assert_eq!(row.implementation.path, "same");
    let byte = omega_psi::TypeExpression(vec![omega_psi::TypeExpressionStep::Scalar(ScalarType::Integer { signed: false, width: 8 })]);
    spec_corpus::observe_handoff!("ordinary-trait-parent/consumer", {
        assert_eq!(parent.declaration.path, "Q");
        assert_eq!(arguments, std::slice::from_ref(&byte));
        assert_eq!(row.requirement.arguments.as_slice(), arguments);
    });
}

#[test]
fn bodyless_entry_ceilings_preserve_the_applied_target_and_omission() {
    use omega_psi::{SynchronousEntry, TypeExpressionStep};
    let source = "boundary trait Handler<T> { machine run() reaches none; }
        boundary trait Driver {
            machine read() reaches none invokes Handler<u8>;
            machine idle() reaches none;
            machine generic<T>() reaches none invokes Handler<T>;
        }
        machine caller() {Driver::read();}";
    let pkg = sources::lower(&[("entries.omg", source)]).unwrap();
    let driver = pkg.traits.iter().find(|row| row.declaration.path == "Driver").unwrap();
    let read = driver.requirements.iter().find(|row| row.declaration.path == "Driver::read").unwrap();
    assert!(read.reach.as_ref().unwrap().is_empty());
    let [clause] = read.invokes.as_slice() else { panic!("one synchronous entry allowed") };
    let SynchronousEntry::Declaration(slot) = &clause.entry else { panic!("expected named entry") };
    assert_eq!(slot.declaration.path, "Handler");
    assert_eq!(slot.arguments.len(), 1);
    let caller = pkg.machines.iter().find(|row| row.declaration.path == "caller").unwrap();
    assert_eq!(caller.contract.invokes.as_slice(), std::slice::from_ref(&clause.entry));
    assert!(caller.contract.realized_reach.is_empty());
    assert_eq!(caller.contract.realized_pause, Default::default());
    assert_eq!(clause.site.span.slice(source.as_bytes()), Some(&b"invokes Handler<u8>"[..]));
    assert!(driver.requirements.iter().find(|row| row.declaration.path == "Driver::idle").unwrap().invokes.is_empty());
    let [generic] = driver.dependent_requirements.as_slice() else { panic!("one generic member") };
    let [clause] = generic.invokes.as_slice() else { panic!("generic entry was lost") };
    let SynchronousEntry::Declaration(open) = &clause.entry else { panic!("expected applied entry") };
    assert_eq!(open.declaration, slot.declaration);
    assert!(matches!(open.arguments[0].0.as_slice(),
        [TypeExpressionStep::GenericParameter {owner, index: 0}] if owner == &generic.declaration));
}

#[test]
fn entry_ceilings_distinguish_same_typed_callable_parameters_and_static_binders() {
    use omega_psi::SynchronousEntry;
    let program = "data Fn = machine() reaches none crashes none;
        boundary trait Driver {
            machine first(a:Fn,b:Fn) reaches none invokes a;
            machine second(a:Fn,b:Fn) reaches none invokes b;
            machine invoke<const F:Fn>() reaches none invokes F;
        }";
    let pkg = sources::lower(&[("entries.omg", program)]).unwrap();
    let driver = &pkg.traits[0];
    for (name, index) in [("Driver::first", 0), ("Driver::second", 1)] {
        let requirement = driver.requirements.iter().find(|r| r.declaration.path == name).unwrap();
        let [clause] = requirement.invokes.as_slice() else { panic!("expected one parameter") };
        assert_eq!(clause.entry, SynchronousEntry::RuntimeParameter(index));
    }
    let [generic] = driver.dependent_requirements.as_slice() else { panic!("expected one static binder") };
    let [clause] = generic.invokes.as_slice() else { panic!("expected one static target") };
    assert_eq!(clause.entry, SynchronousEntry::StaticParameter(0));
}

#[test]
fn a_forwarded_entry_reaches_the_caller_with_its_exact_requirement() {
    use omega_psi::SynchronousEntry;
    let source = "trait Entry { machine enter() reaches none crashes none; }
        boundary trait Registrar { machine dispatch() reaches none crashes none invokes Entry::enter; }
        machine inferred() { Registrar::dispatch(); }
        machine declared() invokes Entry::enter { inferred(); }";
    let pkg = sources::lower(&[("entry.omg",source)]).unwrap();
    let registrar = pkg.traits.iter().find(|row| row.declaration.path == "Registrar").unwrap();
    let [required] = registrar.requirements.as_slice() else { panic!("one dispatch") };
    let [clause] = required.invokes.as_slice() else { panic!("one entry") };
    let SynchronousEntry::Declaration(slot) = &clause.entry else { panic!("named entry") };
    assert_eq!(slot.declaration.path, "Entry::enter");
    for name in ["inferred", "declared"] {
        let caller = pkg.machines.iter().find(|row| row.declaration.path == name).unwrap();
        assert_eq!(caller.contract.invokes.as_slice(), std::slice::from_ref(&clause.entry));
        assert!(caller.contract.realized_reach.is_empty());
        if name == "declared" {
            let [written] = caller.contract.invocation_ceiling.as_slice() else { panic!("one authored entry") };
            assert_eq!(written.entry, clause.entry);
            assert_ne!(written.site, clause.site);
        } else {
            assert!(caller.contract.invocation_ceiling.is_empty());
        }
    }
}

#[test]
fn entry_arguments_keep_their_values_without_service_parent_closure() {
    use omega_psi::{SynchronousEntry, TypeExpressionStep};
    let source = "boundary trait Parent<const N:u64> {}
        boundary trait Handler<const N:u64>:Parent<0u64> {}
        boundary trait Driver { machine dispatch() reaches none invokes Handler<17u64> invokes Handler<29u64>; }
        machine caller() { Driver::dispatch(); }";
    let pkg = sources::lower(&[("entries.omg",source)]).unwrap();
    let driver = pkg.traits.iter().find(|row| row.declaration.path == "Driver").unwrap();
    let [requirement] = driver.requirements.as_slice() else { panic!("one dispatch") };
    assert_eq!(requirement.invokes.len(), 2);
    for (row, n) in requirement.invokes.iter().zip([17, 29]) {
        let SynchronousEntry::Declaration(slot) = &row.entry else { panic!("declared entry") };
        assert_eq!(slot.declaration.path, "Handler");
        assert!(matches!(slot.arguments[0].0.as_slice(),
            [TypeExpressionStep::StaticInteger {value, ..}] if *value == n.into()));
    }
    let caller = &pkg.machines[0].contract;
    assert_eq!(caller.invokes.len(), 2);
    assert!(requirement.invokes.iter().all(|row| caller.invokes.contains(&row.entry)));
    assert!(caller.realized_reach.is_empty());
}

#[test]
fn multiple_missing_template_callees_report_the_same_owner_in_either_source_order() {
    let first = "machine a<T>(y:T){Sink::<T>::send(y);}";
    let last = "machine z<T>(x:T){Sink::<T>::send(x);}";
    for declarations in [[first, last], [last, first]] {
        let source = format!("boundary trait Sink<T>{{machine send(value:T) reaches none;}} {}", declarations.join(" "));
        let package = sources::resolve(&[("sink.omg", &source)]);
        let prepared = omega_preparation::prepare_source_package(&package).unwrap();
        for _ in 0..8 {
            let error = omega_lowering::lower_package(&prepared).unwrap_err();
            assert_eq!(error.message, "template call needs its packaged declaration");
            assert_eq!(error.site.file, "sink.omg");
            assert_eq!(error.site.span.slice(source.as_bytes()), Some(b"Sink::<T>::send(y)".as_slice()));
        }
    }
}
