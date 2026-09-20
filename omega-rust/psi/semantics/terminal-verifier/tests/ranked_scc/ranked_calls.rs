use super::{
    AdmissionProfile, Block, BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue,
    MachineId, ModuleError, Operation, OperationId, OperationKind, OperationResult, Proposition,
    ScalarTerm, ScalarType, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
    ValueId, id, ranked_countdown, unranked_scalar_cycle, validate_module,
    verify_module_for_interpretation,
};
use terminal_psi::{CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard};

/// The ranked countdown's decrement member invokes a crash-publishing scalar
/// helper on the machine's entry argument. The continuation roster is the
/// callee's substituted ceiling; the caller publishes the covering route.
fn ranked_scalar_call_cycle() -> TerminalModule {
    let mut module = ranked_countdown();
    let routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    let scalar = module.machines[0].parameters[0].scalar_type;
    let mut callee = module.machines[0].clone();
    callee.id = id(2, MachineId::new);
    callee.contract.id = id(2, ContractId::new);
    callee.ranked_scc = None;
    callee.parameters = vec![ValueDeclaration {
        qualifications: Default::default(),
        id: id(20, ValueId::new),
        scalar_type: scalar,
    }];
    callee.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: id(21, ValueId::new),
        scalar_type: ScalarType::Boolean,
    });
    callee.entry = id(100, BlockId::new);
    callee.blocks = vec![Block {
        erased_scalar_formals: Vec::new(),
        structural_parameters: Vec::new(),
        id: id(100, BlockId::new),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::Crash {
            edge: id(20, EdgeId::new),
            cause: CrashCause::Trap,
            site_guard: Vec::new(),
            frontier_lower_bound: Vec::new(),
        },
    }];
    callee.contract.crash_routes = routes.clone();
    module.machines[0].contract.crash_routes = routes.clone();
    module.machines[0].blocks[2].operations.push(Operation {
        static_reach_binding: None,
        id: id(30, OperationId::new),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(30, ValueId::new),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::Call {
            erased_arguments: Vec::new(),
            callee: callee.id,
            arguments: vec![id(1, ValueId::new)],
            requirement_obligations: Vec::new(),
            crash_continuations: routes,
        },
    });
    module.machines.push(callee);
    module
}

#[test]
fn ranked_cycle_member_call_carries_exact_crash_continuations() {
    let module = ranked_scalar_call_cycle();
    verify_module_for_interpretation(
        &module,
        &super::ranked_countdown_proof(&module),
        &AdmissionProfile::default(),
    )
    .expect("ranked cycle member call carries its exact substituted crash continuation");
    validate_module(&module).expect("execution validation admits the same cyclic call");
}

fn call_continuations(module: &mut TerminalModule) -> &mut Vec<CrashRouteBucket> {
    let OperationKind::Call {
        crash_continuations,
        ..
    } = &mut module.machines[0].blocks[2].operations[2].kind
    else {
        unreachable!()
    };
    crash_continuations
}

#[test]
fn ranked_cycle_call_rejects_dropped_altered_or_uncovered_continuations() {
    let module = ranked_scalar_call_cycle();
    let scalar = module.machines[0].parameters[0].scalar_type;
    let call = call_continuations;
    let mut omitted = module.clone();
    call(&mut omitted).clear();
    assert!(matches!(
        validate_module(&omitted),
        Err(ModuleError::CallCrashContinuationsMismatch { .. })
    ));
    let mut cause = module.clone();
    call(&mut cause)[0].cause = CrashCause::Abort;
    assert!(matches!(
        validate_module(&cause),
        Err(ModuleError::CallCrashContinuationsMismatch { .. })
    ));
    let mut alternatives = module.clone();
    call(&mut alternatives)[0].alternatives = vec![CrashRouteGuard::Predicate(
        CrashPredicateTerm::new(Proposition::LessOrEqual(
            ScalarTerm::integer(
                IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                IntegerValue::Unsigned(0),
            )
            .unwrap(),
            ScalarTerm::value(id(1, ValueId::new), scalar),
        )),
    )];
    assert!(matches!(
        validate_module(&alternatives),
        Err(ModuleError::CallCrashContinuationsMismatch { .. })
    ));
    let mut uncovered = module.clone();
    uncovered.machines[0].contract.crash_routes.clear();
    assert!(matches!(
        validate_module(&uncovered),
        Err(ModuleError::CallCrashContinuationUncovered { .. })
    ));
}

/// A cyclic machine may itself end on an unguarded crash exit: the route
/// publishes the cause and the site carries no path predicate.
fn crashing_exit(module: &mut TerminalModule, cause: CrashCause) {
    module.machines[0].contract.crash_routes = vec![CrashRouteBucket {
        cause,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    let exit = module.machines[0]
        .blocks
        .iter()
        .position(|block| matches!(block.terminator, Terminator::ReturnUnit { .. }))
        .expect("countdown fixture exits on ReturnUnit");
    let edge = match &module.machines[0].blocks[exit].terminator {
        Terminator::ReturnUnit { edge, .. } => *edge,
        _ => unreachable!(),
    };
    module.machines[0].blocks[exit].terminator = Terminator::Crash {
        edge,
        cause,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
}

#[test]
fn cyclic_machines_terminate_at_their_own_unguarded_crash_exit() {
    let mut ranked = ranked_countdown();
    crashing_exit(&mut ranked, CrashCause::Abort);
    verify_module_for_interpretation(
        &ranked,
        &super::ranked_countdown_proof(&ranked),
        &AdmissionProfile::default(),
    )
    .expect("ranked machine's own unguarded crash exit verifies");
    let mut unranked = unranked_scalar_cycle();
    crashing_exit(&mut unranked, CrashCause::Abort);
    verify_module_for_interpretation(
        &unranked,
        &super::ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("unranked cyclic machine's own unguarded crash exit verifies");
}

/// The countdown's rank is a header block parameter, so `0 <= initial` over the
/// machine entry parameter is the independently checked form a ranked crash
/// guard may use.
fn entry_bound() -> Proposition {
    Proposition::LessOrEqual(
        ScalarTerm::integer(
            IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
            IntegerValue::Unsigned(0),
        )
        .unwrap(),
        ScalarTerm::value(
            id(1, ValueId::new),
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap()),
        ),
    )
}

#[test]
fn cyclic_machine_crash_guards_stay_on_independently_checked_facts() {
    // Ranked reconstruction omits backedges, so a crash guard may cite only
    // the machine's own entry requirements — no within-iteration path facts.
    let mut entry_provable = ranked_countdown();
    crashing_exit(&mut entry_provable, CrashCause::Abort);
    entry_provable.machines[0].contract.requires = vec![entry_bound()];
    let Terminator::Crash { site_guard, .. } = &mut entry_provable.machines[0].blocks[3].terminator
    else {
        unreachable!()
    };
    site_guard.push(CrashPredicateTerm::new(entry_bound()));
    validate_module(&entry_provable)
        .expect("entry-requirement crash guard verifies on a ranked machine");

    let mut path_guard = ranked_countdown();
    crashing_exit(&mut path_guard, CrashCause::Abort);
    let Terminator::Crash { site_guard, .. } = &mut path_guard.machines[0].blocks[3].terminator
    else {
        unreachable!()
    };
    site_guard.push(CrashPredicateTerm::new(Proposition::LessOrEqual(
        ScalarTerm::integer(
            IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
            IntegerValue::Unsigned(5),
        )
        .unwrap(),
        ScalarTerm::value(
            id(1, ValueId::new),
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap()),
        ),
    )));
    assert!(matches!(
        validate_module(&path_guard),
        Err(ModuleError::CrashSiteGuardUnproved { .. })
    ));

    // Unranked cyclic machines keep the exhaustive fail-closed walk: a guarded
    // crash site cannot enumerate unbounded paths and rejects outright.
    let mut unranked = unranked_scalar_cycle();
    unranked.machines[0].contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Abort,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            Proposition::Equal(
                ScalarTerm::value(id(10, ValueId::new), ScalarType::Boolean),
                ScalarTerm::boolean(false),
            ),
        ))],
    }];
    unranked.machines[0].blocks[1].terminator = Terminator::Crash {
        edge: id(12, EdgeId::new),
        cause: CrashCause::Abort,
        site_guard: vec![CrashPredicateTerm::new(Proposition::Equal(
            ScalarTerm::value(id(10, ValueId::new), ScalarType::Boolean),
            ScalarTerm::boolean(false),
        ))],
        frontier_lower_bound: Vec::new(),
    };
    assert!(matches!(
        validate_module(&unranked),
        Err(ModuleError::CrashSiteReconstructionLimitExceeded { .. })
    ));
}
