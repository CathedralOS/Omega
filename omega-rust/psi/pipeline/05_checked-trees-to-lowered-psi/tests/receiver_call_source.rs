//! Source-produced whole borrowed receiver calls through canonical Terminal admission.

use checked_trees::{
    CheckedCallScalarArgument, CheckedScalarExpression, CheckedStructuralAccess,
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralValue,
};
use terminal_psi::{
    OperationKind, OperationResult, StructuralAccess, StructuralFieldType, StructuralMultiplicity,
    StructuralTypeShape, TerminalMachineResult, Terminator,
};

#[path = "receiver_call_source/forwarding.rs"]
mod forwarding;
#[path = "receiver_call_source/projections.rs"]
mod projections;

#[path = "receiver_call_source/indexed.rs"]
mod indexed;

#[path = "receiver_call_source/aliases.rs"]
mod aliases;

#[path = "receiver_call_source/cyclic.rs"]
mod cyclic;

fn unit_plan<'a>(
    checked: &'a checked_trees::CheckedTrees,
    name: &str,
) -> &'a CheckedUnitEffectMachinePlan {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("authored machine")
        .symbol;
    checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == machine)
        .expect("authored machine retains its checked Unit plan")
}
