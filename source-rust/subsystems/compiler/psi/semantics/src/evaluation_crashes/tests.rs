use super::*;
use semantic_vocabulary::compiler_evaluation::{
    CompilerEvaluationDeclaration, CompilerEvaluationEffects, DeclarationCustody,
};
use semantic_vocabulary::{
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, ScalarApplicationId, ScalarTerm,
    ValueId,
};
use terminal_psi::{CrashCause, CrashPredicateTerm, ProofCallableHeader};

fn exclude_failure_routes(routes: &Result<Vec<CrashRouteBucket>, String>) -> Result<(), String> {
    let context = PropositionContext::from_value_types([(
        ValueId::FIRST,
        ScalarType::Integer(integer_type()),
    )])
    .unwrap();
    super::exclude_failure_routes(routes.as_ref().map_err(Clone::clone)?, &context)
}

fn integer_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}

fn integer(value: u128) -> ScalarTerm {
    ScalarTerm::integer(integer_type(), IntegerValue::Unsigned(value)).unwrap()
}

#[test]
fn exact_shift_formation_requires_a_concrete_width_and_valid_count() {
    let parameter = semantic_vocabulary::WidthParameter {
        owner: semantic_vocabulary::MachineId::new(1).unwrap(),
        value: ValueId::new(2).unwrap(),
    };
    let shift = |value_type, count| ScalarTerm::ExactIntegerShiftRight {
        value_type,
        count_type: integer_type(),
        value: Box::new(ScalarTerm::value(
            ValueId::FIRST,
            ScalarType::Integer(value_type),
        )),
        count: Box::new(integer(count)),
    };
    let symbolic = IntegerType::parameterized(IntegerSign::Unsigned, parameter);
    assert!(
        require_formed_scalar(&shift(symbolic, 0))
            .unwrap_err()
            .contains("requires a concrete integer width")
    );
    let closed = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_eq!(require_formed_scalar(&shift(closed, 7)), Ok(false));
    assert!(
        require_formed_scalar(&shift(closed, 8))
            .unwrap_err()
            .contains("undefined exact-shift count")
    );
}

fn route(cause: CrashCause, guard: Proposition) -> CrashRouteBucket {
    CrashRouteBucket {
        cause,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(guard))],
    }
}

#[test]
fn complete_summary_distinguishes_no_routes_and_false_routes() {
    assert!(exclude_failure_routes(&Ok(Vec::new())).is_ok());
    for value in [1, 7] {
        assert!(
            exclude_failure_routes(&Ok(vec![route(
                CrashCause::Trap,
                Proposition::Equal(integer(value), integer(0)),
            )]))
            .is_ok()
        );
    }
}

#[test]
fn any_possible_or_unresolved_failure_prevents_execution() {
    let unknowns = [
        ScalarTerm::value(ValueId::FIRST, ScalarType::Integer(integer_type())),
        ScalarTerm::Application {
            application: ScalarApplicationId::new(1).unwrap(),
            scalar_type: ScalarType::Integer(integer_type()),
        },
    ];
    for unknown in unknowns {
        let reason = exclude_failure_routes(&Ok(vec![route(
            CrashCause::Trap,
            Proposition::Equal(unknown, integer(0)),
        )]))
        .unwrap_err();
        assert!(reason.contains("cannot be proved impossible"));
    }
    let reason = exclude_failure_routes(&Ok(vec![
        route(CrashCause::Trap, Proposition::Falsehood),
        CrashRouteBucket {
            cause: CrashCause::Abort,
            alternatives: vec![CrashRouteGuard::Truth],
        },
    ]))
    .unwrap_err();
    assert!(reason.contains("Abort crash route can occur"));
    assert!(
        exclude_failure_routes(&Ok(vec![route(
            CrashCause::Trap,
            Proposition::Equal(integer(0), integer(0)),
        )]))
        .unwrap_err()
        .contains("Trap crash route can occur")
    );
}

#[test]
fn exact_scalar_terms_and_boolean_guards_keep_their_checked_meaning() {
    let sum = ScalarTerm::exact_integer_add(integer_type(), integer(1), integer(2)).unwrap();
    assert!(
        exclude_failure_routes(&Ok(vec![route(
            CrashCause::Trap,
            Proposition::Equal(sum, integer(0)),
        )]))
        .is_ok()
    );
    assert!(
        exclude_failure_routes(&Ok(vec![route(
            CrashCause::Trap,
            Proposition::Equal(ScalarTerm::Boolean(false), ScalarTerm::Boolean(true)),
        )]))
        .is_ok()
    );
    assert!(
        exclude_failure_routes(&Ok(vec![route(
            CrashCause::Trap,
            Proposition::IntegerMathLessThan(
                IntegerMathTerm::literal(IntegerValue::Unsigned(2)),
                IntegerMathTerm::literal(IntegerValue::Unsigned(1)),
            ),
        )]))
        .is_ok()
    );
    let open = Proposition::Equal(
        ScalarTerm::value(ValueId::FIRST, ScalarType::Integer(integer_type())),
        integer(0),
    );
    assert!(
        exclude_failure_routes(&Ok(vec![route(
            CrashCause::Trap,
            Proposition::Conjunction(vec![open.clone(), Proposition::Falsehood]),
        )]))
        .is_ok()
    );
    assert!(
        exclude_failure_routes(&Ok(vec![route(
            CrashCause::Trap,
            Proposition::Implication {
                premise: Box::new(Proposition::Truth),
                conclusion: Box::new(Proposition::Falsehood),
            },
        )]))
        .is_ok()
    );
    assert!(
        exclude_failure_routes(&Ok(vec![route(
            CrashCause::Trap,
            Proposition::Disjunction(vec![open, Proposition::Falsehood]),
        )]))
        .is_err()
    );
}

#[test]
fn float_guard_decides_literal_ieee_comparison() {
    // The same Binary32 guards the interpreter decides: 1.0 < 2.0,
    // and a NaN is not equal to itself.
    use closed_predicates::ClosedIntegerEvaluator;
    use semantic_vocabulary::{IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatValue};
    let guard = |kind, left, right| Proposition::ScalarIeeeFloatComparison {
        kind,
        format: IeeeFloatFormat::Binary32,
        left: ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(left)),
        right: ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(right)),
    };
    let mut integers = ClosedIntegerEvaluator::default();
    assert_eq!(
        super::closed_guard(
            &guard(IeeeFloatComparisonKind::LessThan, 0x3f80_0000, 0x4000_0000),
            &mut integers,
        )
        .unwrap(),
        Some(true)
    );
    assert_eq!(
        super::closed_guard(
            &guard(IeeeFloatComparisonKind::Equal, 0x7fc0_0000, 0x7fc0_0000),
            &mut integers,
        )
        .unwrap(),
        Some(false)
    );
    let mismatched = Proposition::ScalarIeeeFloatComparison {
        kind: IeeeFloatComparisonKind::LessThan,
        format: IeeeFloatFormat::Binary32,
        left: ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(0x3f80_0000)),
        right: ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(0x4000_0000_0000_0000)),
    };
    assert_eq!(
        super::closed_guard(&mismatched, &mut integers).unwrap(),
        None
    );
}

#[test]
fn malformed_or_oversized_guard_never_proves_absence() {
    assert!(
        exclude_failure_routes(&Ok(vec![CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: Vec::new(),
        }]))
        .is_err()
    );
    assert!(
        exclude_failure_routes(&Ok(vec![route(
            CrashCause::Trap,
            Proposition::Equal(ScalarTerm::Boolean(false), integer(0)),
        )]))
        .unwrap_err()
        .contains("malformed")
    );
    let mut too_deep = Proposition::Falsehood;
    for _ in 0..70 {
        too_deep = Proposition::Conjunction(vec![too_deep]);
    }
    assert!(
        exclude_failure_routes(&Ok(vec![route(CrashCause::Trap, too_deep)]))
            .unwrap_err()
            .contains("evaluation limits")
    );
}

fn header(declaration: ProofDeclarationId) -> ProofCallableHeader {
    ProofCallableHeader {
        declaration,
        compiler_evaluation: Ok(CompilerEvaluationDeclaration {
            name: format!("root{}", declaration.0),
            custody: DeclarationCustody::Toolchain,
            effects: CompilerEvaluationEffects::default(),
            local_terminates: true,
            entry_requires: false,
            linear_carrier: None,
            invocation_refusal: None,
            selections: Ok(Vec::new()),
        }),
        ..ProofCallableHeader::default()
    }
}

#[test]
fn admission_uses_only_the_complete_root_summary() {
    let root = ProofDeclarationId(1);
    let unrelated = ProofDeclarationId(2);
    let mut module = terminal_psi::TerminalModule::declarations();
    module.machines = vec![machine(1), machine(2)];
    module.machines[1].contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    let scope = &mut module.scalars.scalar_applications.proof_scope;
    for declaration in [root, unrelated] {
        scope.callables.insert(ProofCallableAssociation::Callable {
            declaration,
            callable: TargetMachineCallable::Machine(
                semantic_vocabulary::MachineId::new(u64::from(declaration.0)).unwrap(),
            ),
        });
    }
    scope.callable_headers.insert(header(root));
    scope.callable_headers.insert(header(unrelated));
    assert!(require_crash_free_evaluation(&module, root).is_ok());
    assert!(
        require_crash_free_evaluation(&module, unrelated)
            .unwrap_err()
            .contains("constant evaluation requires proof")
    );
    assert!(require_crash_free_evaluation(&module, ProofDeclarationId(3)).is_err());
    module
        .scalars.scalar_applications
        .proof_scope
        .callable_headers
        .insert(header(root));
    assert!(
        require_crash_free_evaluation(&module, root)
            .unwrap_err()
            .contains("competing")
    );
}

fn machine(id: u64) -> terminal_psi::TerminalMachine {
    use semantic_vocabulary::{BlockId, ContractId, EdgeId, MachineId};
    use terminal_psi::{Block, TerminalMachine, TerminalMachineResult, Terminator};
    let block = BlockId::new(1).unwrap();
    TerminalMachine {
        assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
        id: MachineId::new(id).unwrap(),
        attachment: None,
        ranked_scc: None,
        structural_places: Vec::new(),
        call_formations: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block,
        blocks: vec![Block::plain(
            block,
            Vec::new(),
            Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            },
        )],
        contract: terminal_psi::MachineContract {
            selected_application: None,
            declaration: None,
            parameters: vec![terminal_psi::ValueDeclaration {
                id: ValueId::FIRST,
                scalar_type: ScalarType::Integer(integer_type()),
                qualifications: Default::default(),
            }],
            parameter_order: vec![terminal_psi::BoundaryParameterKind::Scalar],
            structural_parameters: Vec::new(),
            result: TerminalMachineResult::Unit,
            entry_claims: Vec::new(),
            declared_service_reach: None,
            closed_reach_application: None,
            published_synchronous_entries: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            ..terminal_psi::MachineContract::empty(ContractId::new(1).unwrap())
        },
    }
}

fn suppressed(predicate: Proposition) -> Proposition {
    Proposition::Conjunction(vec![Proposition::Falsehood, predicate])
}

#[test]
fn false_guards_cannot_hide_undefined_scalar_operands() {
    let open = ScalarTerm::value(ValueId::FIRST, ScalarType::Integer(integer_type()));
    let undefined = [
        ScalarTerm::ExactIntegerDivide {
            scalar_type: integer_type(),
            left: Box::new(integer(1)),
            right: Box::new(integer(0)),
        },
        ScalarTerm::ExactIntegerRemainder {
            scalar_type: integer_type(),
            left: Box::new(integer(1)),
            right: Box::new(integer(0)),
        },
        ScalarTerm::ExactIntegerMultiply {
            scalar_type: integer_type(),
            left: Box::new(integer(u64::MAX.into())),
            right: Box::new(integer(2)),
        },
        ScalarTerm::ExactIntegerDivide {
            scalar_type: integer_type(),
            left: Box::new(open.clone()),
            right: Box::new(integer(0)),
        },
        ScalarTerm::ExactIntegerShiftLeft {
            value_type: integer_type(),
            count_type: integer_type(),
            value: Box::new(open.clone()),
            count: Box::new(integer(64)),
        },
        ScalarTerm::ExactIntegerShiftRight {
            value_type: integer_type(),
            count_type: integer_type(),
            value: Box::new(open),
            count: Box::new(integer(64)),
        },
    ];
    for term in undefined {
        let equality = Proposition::Equal(term, integer(0));
        for guard in [
            suppressed(equality.clone()),
            Proposition::Implication {
                premise: Box::new(Proposition::Falsehood),
                conclusion: Box::new(equality.clone()),
            },
        ] {
            assert!(
                exclude_failure_routes(&Ok(vec![route(CrashCause::Trap, guard)]))
                    .unwrap_err()
                    .contains("undefined")
            );
        }
    }
    let open = ScalarTerm::value(ValueId::FIRST, ScalarType::Integer(integer_type()));
    for term in [
        ScalarTerm::ExactIntegerDivide {
            scalar_type: integer_type(),
            left: Box::new(open.clone()),
            right: Box::new(integer(1)),
        },
        ScalarTerm::ExactIntegerShiftRight {
            value_type: integer_type(),
            count_type: integer_type(),
            value: Box::new(open),
            count: Box::new(integer(63)),
        },
    ] {
        assert!(
            exclude_failure_routes(&Ok(vec![route(
                CrashCause::Trap,
                suppressed(Proposition::Equal(term, integer(0)))
            )]))
            .is_ok()
        );
    }
}

#[test]
fn mathematical_formation_checks_open_siblings_and_embedded_partial_operations() {
    let open = IntegerMathTerm::MathValue {
        source_type: integer_type(),
        value: ValueId::FIRST,
    };
    for value in [
        IntegerMathTerm::literal(IntegerValue::Unsigned(1)),
        open.clone(),
    ] {
        let shifted = IntegerMathTerm::ShiftLeft {
            value: Box::new(value),
            count: Box::new(IntegerMathTerm::literal(IntegerValue::Signed(-1))),
        };
        let term = IntegerMathTerm::Add(Box::new(open.clone()), Box::new(shifted));
        let guard = suppressed(Proposition::IntegerMathLessThan(
            term,
            IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
        ));
        assert!(
            exclude_failure_routes(&Ok(vec![route(CrashCause::Trap, guard)]))
                .unwrap_err()
                .contains("undefined negative mathematical shift")
        );
    }
    let embedded = IntegerMathTerm::Embed(Box::new(ScalarTerm::ExactIntegerDivide {
        scalar_type: integer_type(),
        left: Box::new(integer(1)),
        right: Box::new(integer(0)),
    }));
    let guard = suppressed(Proposition::IntegerMathLessThan(
        embedded,
        IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
    ));
    assert!(
        exclude_failure_routes(&Ok(vec![route(CrashCause::Trap, guard)]))
            .unwrap_err()
            .contains("undefined zero-divisor")
    );
    let safe = IntegerMathTerm::ShiftLeft {
        value: Box::new(open),
        count: Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(1))),
    };
    let guard = suppressed(Proposition::IntegerMathLessThan(
        safe,
        IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
    ));
    assert!(exclude_failure_routes(&Ok(vec![route(CrashCause::Trap, guard)])).is_ok());
}

#[test]
fn admission_validates_the_whole_guard_in_the_emitted_entry_context() {
    let root = ProofDeclarationId(1);
    let mut module = terminal_psi::TerminalModule::declarations();
    module.machines.push(machine(1));
    module
        .scalars.scalar_applications
        .proof_scope
        .callables
        .insert(ProofCallableAssociation::Callable {
            declaration: root,
            callable: TargetMachineCallable::Machine(
                semantic_vocabulary::MachineId::new(1).unwrap(),
            ),
        });
    let valid = suppressed(Proposition::Equal(
        ScalarTerm::value(ValueId::FIRST, ScalarType::Integer(integer_type())),
        integer(0),
    ));
    let undeclared = suppressed(Proposition::Equal(
        ScalarTerm::value(
            ValueId::new(99).unwrap(),
            ScalarType::Integer(integer_type()),
        ),
        integer(0),
    ));
    let unbound_place = suppressed(Proposition::Equal(
        ScalarTerm::ViewExtent {
            root: semantic_vocabulary::PlaceId::new(1).unwrap(),
            path: Vec::new(),
            scalar_type: integer_type(),
        },
        integer(0),
    ));
    module
        .scalars.scalar_applications
        .proof_scope
        .callable_headers
        .insert(header(root));
    for (guard, passes) in [(valid, true), (undeclared, false), (unbound_place, false)] {
        module.machines[0].contract.crash_routes = vec![route(CrashCause::Trap, guard)];
        let result = require_crash_free_evaluation(&module, root);
        if passes {
            assert!(result.is_ok(), "{result:?}");
        } else {
            assert!(result.unwrap_err().contains("malformed"));
        }
    }
    module.machines.clear();
    assert!(
        require_crash_free_evaluation(&module, root)
            .unwrap_err()
            .contains("no emitted root body")
    );
}
