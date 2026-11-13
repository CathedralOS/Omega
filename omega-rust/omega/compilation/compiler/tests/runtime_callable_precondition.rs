//! An unknown callable must establish its public precondition for the actual
//! supplied to it. This preserved rejection calls it with 200 against an upper
//! bound of 100; a missing hosted entry or an unsupported construct is not the
//! rejection this test retains.

use lowered_discharge::RouteVerdict;
use platform_custody::temporary_directory::TemporaryDirectory;
use semantic_vocabulary::{IntegerValue, Proposition, ScalarTerm, ScalarType};
use structure_elaboration::{Compilation, elaborate};
use terminal_check_tables::edges::{Actual, Callee};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::values::ValueOrigin;
use terminal_psi::{BoundaryParameterKind, OperationKind};

// Preserved source SHA-256: 3e3e00c91368d7805f96ef210086bad767251abafc46ce5d0ad3c39cbc4a4154.
const INVALID: &str = r#"// A machine value call must prove the public precondition for its actuals.
// The value argument is 200; it cannot satisfy `value <= 100`.

trait Shrink<T> {
    machine shrink(scale: &T, value: u64) -> u64
    requires value >= 3 && value <= 100
    ensures result <= value;
}

data Scale {
    unit: u8;
}

TrimOne satisfies Shrink<Scale> {
    machine shrink(scale: &Scale, value: u64) -> u64
    requires value >= 3 && value <= 100
    ensures result <= value
    {
        transition {
            _ -> (value - 1)
        }
    }
}

machine probe(one: machine(scale: &Scale, value: u64) -> u64 reaches none crashes none
    requires value >= 3 && value <= 100
    ensures result <= value, scale: &Scale) -> u64 {
    let first: u64 = one(scale, 200);
    transition {
        _ -> first
    }
}

machine entry(scale: &Scale) -> u64 {
    let one: machine(scale: &Scale, value: u64) -> u64 reaches none crashes none
    requires value >= 3 && value <= 100
    ensures result <= value = TrimOne::shrink;
    transition {
        _ -> probe(one, scale)
    }
}
"#;

fn contains_upper_bound(clause: &Proposition, upper: &Proposition) -> bool {
    clause == upper
        || matches!(clause, Proposition::Conjunction(parts)
        if parts.iter().any(|part| contains_upper_bound(part, upper)))
}

fn check_preserved_rejection() {
    let directory = TemporaryDirectory::create("runtime-callable-precondition", false)
        .expect("private source directory");
    let main = directory.path().join("main.omg");
    std::fs::write(&main, INVALID).expect("unchanged invalid source");
    let main = main.canonicalize().expect("exact source path");
    let frontier = compiler::CompilationSession::default()
        .typed_frontier_without_build(
            compiler::CheckedCompileRequest::new(&main, Some("macos_arm64")),
            None,
        )
        .unwrap_or_else(|failure| {
            panic!(
                "source preparation must complete: {:?}",
                failure.into_diagnostics()
            )
        });
    assert!(
        frontier.entries.is_empty(),
        "the source declares no main entry"
    );
    assert!(
        frontier.bindings.is_empty(),
        "no hosted ProgramEntry is fabricated"
    );
    let typed = &frontier.typed;
    let declaration = |name: &str| {
        let declarations = typed
            .machines()
            .iter()
            .filter(|machine| {
                machine.name.as_str() == name
                    && typed
                        .symbols
                        .symbol_source_span(machine.symbol)
                        .is_some_and(|span| {
                            typed.symbols.source_file(span).is_some_and(|file| {
                                file.path.canonicalize().is_ok_and(|path| path == main)
                            })
                        })
            })
            .collect::<Vec<_>>();
        let [only] = declarations.as_slice() else {
            panic!("the unchanged source must declare exactly one {name}");
        };
        only.symbol
    };
    let entry = declaration("entry");
    let probe = declaration("probe");
    // This is a checking root with its authored parameters, not a hosted
    // ProgramEntry. The ordinary reader and judge own its complete closure.
    let elaboration = elaborate(
        typed,
        Compilation {
            entries: &[entry],
            subjects: &[frontier.subject],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
        },
    )
    .expect("the unchanged source must elaborate instead of reporting unsupported");
    assert!(elaboration.generic_templates.is_empty());
    let probe = elaboration
        .machines
        .iter()
        .find_map(|(id, source)| (*source == probe).then_some(id))
        .expect("the judged closure retains the actual probe declaration");
    let tables = elaboration
        .tables
        .machines
        .get(probe)
        .expect("probe tables");
    let selected = tables
        .invocations
        .iter()
        .filter(|(_, invocation)| matches!(invocation.callee, Callee::Selected { .. }))
        .collect::<Vec<_>>();
    let [(invocation_id, invocation)] = selected.as_slice() else {
        panic!("probe must contain its one genuine selected call");
    };
    let Callee::Selected { callable } = invocation.callee else {
        unreachable!()
    };
    let callee = tables.values.get(callable).expect("evaluated callee");
    assert!(
        matches!(callee.origin, ValueOrigin::MachineParameter { .. }),
        "the public contract belongs to unknown input code, not a chosen body"
    );
    let ScalarType::Callable(contract) = callee.scalar_type else {
        panic!("the selected callee must carry its exact callable contract");
    };
    let contract = elaboration
        .module
        .callables
        .contract(contract)
        .expect("public contract");
    let [amount] = contract.scalar_parameters.as_slice() else {
        panic!("the public signature must retain its one scalar amount formal");
    };
    let ScalarType::Integer(carrier) = amount.scalar_type else {
        panic!("amount is u64")
    };
    let upper = Proposition::LessOrEqual(
        ScalarTerm::Value {
            id: amount.id,
            scalar_type: amount.scalar_type,
        },
        ScalarTerm::Integer {
            scalar_type: carrier,
            value: IntegerValue::Unsigned(100),
        },
    );
    let clause = contract
        .proof
        .requires
        .iter()
        .position(|clause| contains_upper_bound(clause, &upper))
        .expect("the public requires must retain value <= 100");
    let actual_position = contract
        .parameter_order
        .iter()
        .position(|kind| *kind == BoundaryParameterKind::Scalar)
        .expect("the scalar's authored runtime position");
    let Actual::Value(actual) = invocation.actuals[actual_position] else {
        panic!("the amount actual must be a captured scalar");
    };
    let ValueOrigin::Operation(constant) = tables.values.get(actual).expect("actual value").origin
    else {
        panic!("the amount must retain its actual constant construction");
    };
    let machine = elaboration.module.machine(probe).expect("probe body");
    assert!(
        machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| {
                operation.id == constant
                    && operation.kind
                        == OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(200),
                        }
            }),
        "the call must pass the preserved actual 200"
    );
    let obligation = invocation.requirements[clause];
    let kind = ObligationKind::CallRequirement {
        invocation: *invocation_id,
        clause: u32::try_from(clause).expect("clause position"),
    };
    assert_eq!(
        tables
            .obligations
            .get(obligation)
            .expect("call requirement")
            .kind,
        kind
    );
    let site = *elaboration
        .sources
        .get(probe)
        .expect("probe source")
        .obligations
        .get(obligation)
        .expect("exact requirement source");
    let span = site
        .span()
        .expect("the call has an authored source occurrence");
    assert_eq!(
        &INVALID[span.span.start as usize..span.span.end as usize],
        "one(scale, 200)"
    );

    let verdict = lowered_discharge::judge(Ok(elaboration)).expect("one ordinary source judgment");
    let RouteVerdict::Rejected {
        formations,
        unproved,
        unestablished,
        conflicts,
        exceeded,
    } = verdict
    else {
        panic!(
            "the invalid upper-bound call must be rejected, not accepted, incomplete or unsupported: {verdict:?}"
        );
    };
    assert!(
        formations.is_empty()
            && unestablished.is_empty()
            && conflicts.is_empty()
            && exceeded.is_empty(),
        "an unrelated formation, custody or effect failure cannot satisfy this rejection"
    );
    assert!(
        unproved.iter().any(|failure| {
            failure.machine == probe
                && failure.obligation == obligation
                && failure.kind == kind
                && failure.site == site
        }),
        "the selected call's exact public value <= 100 requirement must remain unproved: {unproved:?}"
    );
}

#[test]
fn unknown_callable_rejects_actual_200_above_its_public_upper_100() {
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, check_preserved_rejection)
            .expect("compiler worker")
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    });
}
