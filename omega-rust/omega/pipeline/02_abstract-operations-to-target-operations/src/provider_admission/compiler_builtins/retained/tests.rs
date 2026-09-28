use super::{
    TerminalCompilerBuiltinProposal, prepare_retained_compiler_builtins,
    validate_retained_compiler_builtins,
};
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use target_operations::CompilerBuiltinExecution;

fn intrinsic_plan(requirements: &[&str]) -> ProviderPlan {
    ProviderPlan {
        rows: requirements
            .iter()
            .map(|requirement| ProviderPlanRow {
                method: "operation".to_owned(),
                requirement_identity: (*requirement).to_owned(),
                requirement_lifetime_partition: Vec::new(),
                binding: ProviderBinding::CompilerIntrinsic {
                    machine: "realization".to_owned(),
                },
            })
            .collect(),
        ..ProviderPlan::default()
    }
}

fn builtin(
    requirement: &str,
    plan_index: usize,
    execution: CompilerBuiltinExecution,
) -> TerminalCompilerBuiltinProposal {
    TerminalCompilerBuiltinProposal::new(requirement.to_owned(), plan_index, execution)
        .expect("nonempty retained requirement")
}

#[test]
fn retained_builtins_preserve_order_exact_plan_borrows_and_execution() {
    let plans = [
        intrinsic_plan(&["read"]),
        intrinsic_plan(&["write", "exit"]),
    ];
    let builtins = [
        builtin("exit", 1, CompilerBuiltinExecution::HostedExitProcessI32),
        builtin("read", 0, CompilerBuiltinExecution::HostedReadByte),
        builtin("write", 1, CompilerBuiltinExecution::HostedWriteByteI32),
    ];
    validate_retained_compiler_builtins(&builtins, &plans).expect("exact unique rows");
    let prepared =
        prepare_retained_compiler_builtins(&builtins, &plans).expect("exact retained plan indices");
    assert_eq!(prepared.len(), builtins.len());
    for (settlement, proposal) in prepared.iter().zip(&builtins) {
        assert_eq!(
            settlement.requirement_identity,
            proposal.requirement_identity()
        );
        assert_eq!(settlement.execution, proposal.execution());
        assert!(std::ptr::eq(
            settlement.provider_plan,
            &plans[proposal.provider_plan_index()]
        ));
    }
    validate_retained_compiler_builtins(&[], &plans).expect("no builtin demand");
    assert!(
        prepare_retained_compiler_builtins(&[], &plans)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn retained_builtins_reject_missing_duplicate_and_nonintrinsic_rows() {
    let proposal = [builtin("read", 0, CompilerBuiltinExecution::HostedReadByte)];
    let mut nonintrinsic = intrinsic_plan(&["read"]);
    nonintrinsic.rows[0].binding = ProviderBinding::Syscall { number: 0 };
    for plan in [
        intrinsic_plan(&[]),
        intrinsic_plan(&["other"]),
        intrinsic_plan(&["read", "read"]),
        nonintrinsic,
    ] {
        assert_eq!(
            validate_retained_compiler_builtins(&proposal, &[plan]),
            Err("Terminal native proposal compiler builtin does not rejoin one exact selected row")
        );
    }
    assert_eq!(
        validate_retained_compiler_builtins(&proposal, &[]),
        Err("Terminal native proposal names an absent selected provider plan")
    );
    let missing = prepare_retained_compiler_builtins(&proposal, &[])
        .expect_err("missing index cannot construct a settlement");
    assert_eq!(missing.len(), 1);
    assert_eq!(
        missing[0].message,
        "Terminal native proposal: compiler builtin `read` names an absent selected provider plan"
    );
}

#[test]
fn retained_builtins_keep_roster_rejection_precedence() {
    let plans = [intrinsic_plan(&["read"]), intrinsic_plan(&["read"])];
    let first = builtin("read", 0, CompilerBuiltinExecution::HostedReadByte);
    for second_index in [1, usize::MAX] {
        let duplicate = builtin(
            "read",
            second_index,
            CompilerBuiltinExecution::HostedReadByte,
        );
        assert_eq!(
            validate_retained_compiler_builtins(&[first.clone(), duplicate], &plans),
            Err("Terminal native proposal repeats a compiler-builtin requirement")
        );
    }
    let missing = builtin(
        "missing",
        usize::MAX,
        CompilerBuiltinExecution::HostedReadByte,
    );
    assert_eq!(
        validate_retained_compiler_builtins(&[missing, first], &plans),
        Err("Terminal native proposal names an absent selected provider plan")
    );
    assert_eq!(
        TerminalCompilerBuiltinProposal::new(
            String::new(),
            0,
            CompilerBuiltinExecution::HostedReadByte
        ),
        Err("Terminal compiler-builtin proposal has an empty requirement identity")
    );
}

#[test]
fn retained_builtins_do_not_replace_native_execution_admission() {
    // Retention joins the declared intrinsic row, not the receiving target
    // catalog. In particular, it must not infer execution from method spelling
    // or silently repair a supplied execution. Native admission owns that check.
    let plans = [intrinsic_plan(&["read"])];
    let proposal = [builtin(
        "read",
        0,
        CompilerBuiltinExecution::HostedExitProcessI32,
    )];
    validate_retained_compiler_builtins(&proposal, &plans).expect("structural row custody only");
    let prepared = prepare_retained_compiler_builtins(&proposal, &plans).unwrap();
    assert_eq!(
        prepared[0].execution,
        CompilerBuiltinExecution::HostedExitProcessI32
    );
}
