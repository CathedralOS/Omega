#[path = "support/source.rs"]
mod sources;

use MathematicalStep::{Apply, FormProposition};
use TypeExpressionStep::{Function, GenericParameter, Universe};
use omega_psi::{MathematicalStep, PredicateOperator, PredicateStep, TypeExpressionStep};

#[test]
fn a_dependent_record_keeps_its_predicate_binder_and_exact_field() {
    let prefix = "data Witness<u:core\\Level,v:core\\Level,A:core\\Type<u>,P:A -> core\\Strict<v>,Q:A -> core\\Strict<v>>";
    for (predicate, binder, field) in [("P", 3, "left"), ("Q", 4, "right")] {
        let text = format!("{prefix} requires {predicate}({field}) {{left:A; right:A;}}");
        let package = sources::lower(&[("data.omg", &text)]).unwrap();
        assert!(package.data.is_empty() && package.machines.is_empty());
        let [witness] = package.data_templates.as_slice() else {
            panic!("expected a template")
        };
        let slot = |index| GenericParameter {
            owner: witness.declaration.clone(),
            index,
        };
        assert_eq!(
            witness.parameters[3],
            omega_psi::TemplateBinder::Kinded(omega_psi::TypeExpression(vec![slot(2), slot(1), Universe { strict: true }, Function]))
        );
        let [left, right] = witness.fields.as_slice() else {
            panic!("expected two stored fields")
        };
        assert_eq!(left.ty.0, [slot(2)]);
        assert_eq!(left.ty, right.ty);
        let target = if field == "left" { left } else { right };
        assert_eq!(
            witness.requires[0].predicate,
            [PredicateStep::MathematicalTerm(vec![
                MathematicalStep::GenericParameter {
                    owner: witness.declaration.clone(),
                    index: binder
                },
                MathematicalStep::SelfField(target.declaration.clone()),
                Apply {
                    generic_arguments: vec![],
                    argument_count: 1,
                    assumptions: vec![]
                },
                FormProposition(omega_psi::TypeExpression(vec![slot(1), Universe { strict: true }])),
            ])]
        );
    }
}

#[test]
fn nested_data_passes_the_private_witness_as_an_argument_and_captures_only_the_public_lifetime() {
    use TypeExpressionStep::{AtLifetime, Named, Reference, Witness};
    let text = "data Packet<'a, some S> { payload: Payload<S>; data Payload<U> { value: @'a U; } }";
    let package = sources::lower(&[("nested.omg", text)]).unwrap();
    let [packet, payload] = package.data_templates.as_slice() else {
        panic!("expected outer and inner templates")
    };
    assert_eq!(packet.parameters, [omega_psi::TemplateBinder::Lifetime]);
    assert_eq!(packet.witness_count, 1);
    assert!(packet.enclosing_parameters.is_empty());
    assert_eq!(payload.enclosing_parameters.as_slice(), std::slice::from_ref(&packet.declaration));
    assert_eq!(payload.witness_count, 0);
    assert_eq!(payload.parameters, [omega_psi::TemplateBinder::Type { multiplicity: None }]);
    let lifetime = GenericParameter {
        owner: packet.declaration.clone(),
        index: 0,
    };
    spec_corpus::observe_handoff!(
        psi "psi-output/field-witness",
        assert_eq!(
            packet.fields[0].ty.0,
            [
                lifetime.clone(),
                Witness {
                    owner: packet.declaration.clone(),
                    index: 0
                },
                Named {
                    declaration: payload.declaration.clone(),
                    argument_count: 2
                }
            ]
        ),
    );
    assert_eq!(
        payload.fields[0].ty.0,
        [
            GenericParameter {
                owner: payload.declaration.clone(),
                index: 0
            },
            Reference(omega_psi::TemplateAccess::Shared),
            lifetime,
            AtLifetime
        ]
    );
}

#[test]
fn an_existential_callable_shares_the_state_witness_and_keeps_its_postcondition() {
    use TypeExpressionStep::{AtLifetime, Callable, Reference, Scalar, Slice, Witness};
    for (token, comparison) in [
        ("<=", PredicateOperator::LessOrEqual),
        ("<", PredicateOperator::LessThan),
    ] {
        let text = format!(
            "data ReadInto<S> = machine(state: @mutable S, output: @mutable [u8]) -> u64 ensures result {token} output.length reaches none crashes none; data Reader<'a, some S> {{ state: @'a mutable S; read: ReadInto<S>; }}"
        );
        let package = sources::lower(&[("reader.omg", &text)]).unwrap();
        let reader = &package.data_templates[0];
        let witness = Witness {
            owner: reader.declaration.clone(),
            index: 0,
        };
        let mutable = Reference(omega_psi::TemplateAccess::Mut);
        let lifetime = GenericParameter {
            owner: reader.declaration.clone(),
            index: 0,
        };
        assert_eq!(reader.fields[0].ty.0, [witness.clone(), mutable.clone(), lifetime, AtLifetime]);
        let [Callable(callable)] = reader.fields[1].ty.0.as_slice() else {
            panic!("expected a callable field")
        };
        let [state, output] = callable.parameters.as_slice() else {
            panic!("ReadInto takes exactly state and output")
        };
        assert_eq!(state.0, [witness, mutable.clone()]);
        assert_eq!(
            output.0,
            [
                Scalar(omega_psi::ScalarType::Integer { signed: false, width: 8 }),
                Slice,
                mutable
            ]
        );
        assert_eq!(
            callable.result.as_ref().unwrap().0,
            [Scalar(omega_psi::ScalarType::Integer {
                signed: false,
                width: 64
            })]
        );
        assert!(callable.requires.is_empty() && callable.reach.as_deref() == Some(&[]) && callable.crash_ceiling.as_deref() == Some(&[]));
        assert_eq!(
            callable.ensures[0].predicate,
            [
                PredicateStep::Result,
                PredicateStep::Formal(1),
                PredicateStep::ByteSequenceLength,
                PredicateStep::Apply(comparison)
            ]
        );
    }
}

#[test]
fn a_type_equation_and_array_extent_refer_to_the_same_constant_binder() {
    use TypeExpressionStep::{IndexedArray, Qualified, Scalar};
    for (op, expected) in [
        ("<=", PredicateOperator::LessOrEqual),
        ("<", PredicateOperator::LessThan),
    ] {
        let text = format!(
            "domain<const Capacity:u64> u64::AtMost<Capacity> requires self {op} Capacity; data TinyBytes<Length,const Capacity:u64> requires Length == u64::AtMost::<Capacity> {{storage:[u8;Capacity];length:Length;}}"
        );
        let package = sources::lower(&[("tiny.omg", &text)]).unwrap();
        let domain = &package.domains[0];
        assert_eq!(
            domain.requires[0].predicate,
            [
                PredicateStep::SelfValue,
                PredicateStep::ValueIndex(0),
                PredicateStep::Apply(expected)
            ]
        );
        let data = &package.data_templates[0];
        let arg = |index| GenericParameter {
            owner: data.declaration.clone(),
            index,
        };
        let integer = |width| Scalar(omega_psi::ScalarType::Integer { signed: false, width });
        assert_eq!(
            data.equations,
            [(
                0,
                omega_psi::TypeExpression(vec![
                    integer(64),
                    arg(1),
                    Qualified(vec![(domain.declaration.clone(), 1)])
                ])
            )]
        );
        let capacity = omega_psi::TemplateBinder::Const(omega_psi::TypeExpression(vec![integer(64)]));
        assert_eq!(data.parameters[1], capacity);
        assert_eq!(domain.parameters, [capacity]);
        assert_eq!(data.fields[0].ty.0, [integer(8), arg(1), IndexedArray]);
        assert_eq!(data.fields[1].ty.0, [arg(0)]);
        assert_eq!(
            data.requires[0].predicate,
            [PredicateStep::TypesEqual {
                left: data.fields[1].ty.clone(),
                right: data.equations[0].1.clone()
            }]
        );
    }
}

#[test]
fn a_stored_callable_never_substitutes_an_unconditional_crash_for_a_guarded_route() {
    let prefix = "data ReadByte = machine(x:u8) reaches none crashes Trap";
    let suffix = "; data Reader<'a,some S> { state:@'a S; read:ReadByte; }";
    let plain = format!("{prefix}{suffix}");
    let product = sources::lower(&[("read.omg", &plain)]).unwrap();
    let [TypeExpressionStep::Callable(interface)] = product.data_templates[0].fields[1].ty.0.as_slice() else {
        panic!("expected a callable")
    };
    assert_eq!(interface.crash_ceiling.as_deref(), Some(&[omega_typed::CrashCause::Trap][..]));
    let [unconditional] = interface.crash_routes.as_slice() else {
        panic!("expected one unconditional route")
    };
    assert_eq!((unconditional.cause, unconditional.guard.as_ref()), (omega_typed::CrashCause::Trap, None));
    let guarded = format!("{prefix} x == 0{suffix}");
    let product = sources::lower(&[("read.omg", &guarded)]).unwrap();
    let [TypeExpressionStep::Callable(interface)] = product.data_templates[0].fields[1].ty.0.as_slice() else {
        panic!("expected a callable")
    };
    assert_eq!(interface.crash_ceiling.as_deref(), Some(&[omega_typed::CrashCause::Trap][..]));
    let [route] = interface.crash_routes.as_slice() else {
        panic!("expected the guarded route")
    };
    assert_eq!(route.cause, omega_typed::CrashCause::Trap);
    assert_eq!(route.site.span.slice(guarded.as_bytes()), Some(&b"crashes Trap x == 0"[..]));
    let guard = route.guard.as_ref().expect("the guard stays on the stored interface");
    assert_eq!(guard.site.span.slice(guarded.as_bytes()), Some(&b"x == 0"[..]));
    assert_eq!(
        guard.predicate,
        [PredicateStep::Formal(0), PredicateStep::Integer(0.into()), PredicateStep::Apply(PredicateOperator::Equal)]
    );
}
