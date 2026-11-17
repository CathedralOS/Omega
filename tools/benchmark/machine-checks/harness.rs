// Compare the same lowered program at different invocation worker counts.
// Setup, input cloning and result fingerprints stay outside the discharge timer.

use execution_admission::ExecutionBudget;
use semantic_vocabulary::{IdTable, IntegerSign, IntegerType, IntegerValue, MachineId, ScalarType};
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};
use typed_trees_to_terminal_psi::{
    Body, Ceiling, CrashCeiling, Expression, ExpressionKind, Signature, SourceOrigin, Statement,
    Strict, Target, Termination, Unpositioned, ValueType,
};

fn expression(kind: ExpressionKind, scalar: ScalarType) -> Expression {
    Expression {
        call_context: None,
        kind,
        result: Some(ValueType::plain(scalar)),
        source: SourceOrigin::Unpositioned(Unpositioned::SpanAbsent),
    }
}

fn generated(
    shape: &str,
    count: usize,
    steps: usize,
) -> terminal_check_tables::program::Elaboration {
    assert!(count > 0 && steps > 0);
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
    let mut bodies = IdTable::<MachineId, Body>::default();
    let mut signatures = IdTable::default();
    for ordinal in 0..count {
        // A few larger helpers and a long tail of small helpers. The uniform
        // case is a scaling control, not a claimed application distribution.
        let length = match shape {
            "mixed" if ordinal % 16 != 0 => 1 + ordinal % 4,
            "mixed" | "uniform" => steps,
            "tiny" => 1,
            _ => panic!("shape must be mixed, uniform or tiny"),
        };
        let mut statements = vec![Statement::Bind {
            slot: 0,
            value: expression(ExpressionKind::Integer(IntegerValue::Unsigned(0)), scalar),
        }];
        for _ in 0..length {
            statements.push(Statement::Bind {
                slot: 0,
                value: expression(
                    ExpressionKind::Strict {
                        operation: Strict::Add,
                        policy: None,
                        left: Box::new(expression(ExpressionKind::Slot(0), scalar)),
                        right: Box::new(expression(
                            ExpressionKind::Integer(IntegerValue::Unsigned(1)),
                            scalar,
                        )),
                    },
                    scalar,
                ),
            });
        }
        statements.push(Statement::Arm {
            guard: None,
            target: Target::Value(expression(ExpressionKind::Slot(0), scalar)),
            source: SourceOrigin::Unpositioned(Unpositioned::SpanAbsent),
        });
        bodies.push(Body {
            slots: vec![ValueType::plain(scalar)],
            statements,
        });
        signatures.push(Signature {
            operational: Default::default(),
            parameters: vec![],
            erased: vec![],
            borrowed: vec![],
            result: Some(ValueType::plain(scalar)),
            requires: vec![],
            ensures: vec![],
            reaches: Ceiling::Inferred,
            invokes: Ceiling::Inferred,
            crashes: CrashCeiling::Authored(vec![]),
            termination: Termination::Silent,
            ranking: None,
        });
    }
    typed_trees_to_terminal_psi::lower_bodies(
        MachineId::new(1).unwrap(),
        &bodies,
        &signatures,
        &IdTable::default(),
    )
    .unwrap()
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    assert_eq!(
        arguments.len(),
        5,
        "machine_checks <workers> <repeats> <mixed|uniform|tiny> <machines> <steps>"
    );
    let workers: usize = arguments[0].parse().unwrap();
    let repeats: usize = arguments[1].parse().unwrap();
    assert!(workers > 0 && repeats > 0);
    let budget = ExecutionBudget::with_workers(workers).unwrap();
    budget.run(|| {
        let input = generated(&arguments[2], arguments[3].parse().unwrap(), arguments[4].parse().unwrap());
        let mut elapsed = Duration::ZERO;
        let mut fingerprint = None;
        let mut obligations = 0;
        for _ in 0..repeats {
            let mut elaboration = input.clone();
            let mut activity = terminal_psi_to_terminal_psi::ProofSearchActivity::default();
            let start = Instant::now();
            let result =
                terminal_psi_to_terminal_psi::discharge(&mut elaboration, &mut activity).unwrap();
            elapsed += start.elapsed();
            obligations = 0;
            for (_, machine) in result.machines.iter() {
                for (_, verdict) in machine.verdicts.iter() {
                    verdict.as_ref().unwrap();
                    obligations += 1;
                }
                assert!(machine.unestablished().next().is_none());
            }
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            format!("{elaboration:?}{result:?}").hash(&mut hash);
            let found = hash.finish();
            if let Some(expected) = fingerprint {
                assert_eq!(found, expected, "repeated checks changed their output");
            }
            fingerprint = Some(found);
        }
        println!("{{\"schema\":\"omega-timed-work/1\",\"elapsed_ns\":{},\"work_units\":{},\"checksum\":\"{:016x}\"}}", elapsed.as_nanos(), obligations * repeats, fingerprint.unwrap());
    });
}
