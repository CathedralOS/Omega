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
            static_application: None,
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
    let scope = &mut module.scalar_applications.proof_scope;
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
    assert!(require_root_failure_admission(&module, root).is_ok());
    assert!(
        require_root_failure_admission(&module, unrelated)
            .unwrap_err()
            .contains("constant evaluation requires proof")
    );
    assert!(require_root_failure_admission(&module, ProofDeclarationId(3)).is_err());
    module
        .scalar_applications
        .proof_scope
        .callable_headers
        .insert(header(root));
    assert!(
        require_root_failure_admission(&module, root)
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
        declaration: None,
        attachment: None,
        nominal_cleanup_subject: None,
        parameters: vec![terminal_psi::ValueDeclaration {
            id: ValueId::FIRST,
            scalar_type: ScalarType::Integer(integer_type()),
            qualifications: Default::default(),
        }],
        parameter_order: vec![terminal_psi::BoundaryParameterKind::Scalar],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
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
        contract: terminal_psi::MachineContract::empty(ContractId::new(1).unwrap()),
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
        .scalar_applications
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
        .scalar_applications
        .proof_scope
        .callable_headers
        .insert(header(root));
    for (guard, passes) in [(valid, true), (undeclared, false), (unbound_place, false)] {
        module.machines[0].contract.crash_routes = vec![route(CrashCause::Trap, guard)];
        let result = require_root_failure_admission(&module, root);
        if passes {
            assert!(result.is_ok(), "{result:?}");
        } else {
            assert!(result.unwrap_err().contains("malformed"));
        }
    }
    module.machines.clear();
    assert!(
        require_root_failure_admission(&module, root)
            .unwrap_err()
            .contains("no emitted root body")
    );
}
