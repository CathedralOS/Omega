//! Normal-return case promises name the completed result occurrence.
//!
//! The retained Boolean contract still supplies its proof obligation and full
//! identity. Its selected case is also a live value fact on the caller result,
//! so case consumers never interpret a callee's reserved `result` as caller
//! syntax or parse a rendered contract label. Caller writes invalidate this
//! fact through the ordinary place/context lifetime rules.

use arena::HandleSpan;
use checked_trees::{
    ContractCallFact, ContractProofFact, ContractProofFactKind, ContractProofFactOwner,
};
use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, FactRef, ProgramPoint,
    QualificationEvidence,
};

pub(super) fn append(
    program: &dyn validation::CheckingInput,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    contract: &ContractProofFact,
    point: ProgramPoint,
    evidence: QualificationEvidence,
    refs: &mut HandleSpan<FactRef>,
) {
    if contract.kind != ContractProofFactKind::Ensures {
        return;
    }
    let owns_selected_result = match contract.owner {
        ContractProofFactOwner::Machine { machine_symbol } => {
            machine_symbol == call.target_machine_symbol
        }
        ContractProofFactOwner::MachineState {
            machine_symbol,
            state_symbol,
        } => {
            machine_symbol == call.target_machine_symbol && state_symbol == call.target_state_symbol
        }
        _ => false,
    };
    if !owns_selected_result {
        return;
    }
    let typed_trees::domain::ProofFact::Expression(expression) =
        program.proof_facts.get(contract.fact)
    else {
        return;
    };
    let Some(machine) = program.machine_by_symbol(call.target_machine_symbol) else {
        return;
    };
    let Some(state) = program.state_in_machine(machine, call.target_state_symbol) else {
        return;
    };
    let mut pending = vec![*expression];
    while let Some(expression) = pending.pop() {
        let Some(binary) = program.expression_table.binary_node(expression) else {
            continue;
        };
        if binary.operator == typed_trees::expression::BinaryOperator::And
            && validation::has_builtin_decomposed_guard_meaning(
                program,
                machine,
                Some(state),
                expression,
            )
        {
            pending.extend([binary.left, binary.right]);
            continue;
        }
        if !validation::has_exact_case_membership_meaning(
            program,
            machine,
            Some(state),
            expression,
            binary,
        ) {
            continue;
        }
        let Some(subject) = validation::reserved_result_place(program, binary.left) else {
            continue;
        };
        if subject.machine_symbol != machine.symbol
            || !detached_result_projection(program, &subject)
        {
            continue;
        }
        let Some(case) = program.expression_table.name_path_node(binary.right) else {
            continue;
        };
        let Some(place) = crate::semantic::places::instantiate_call_contract_expression_place(
            program,
            facts,
            call,
            binary.left,
        ) else {
            continue;
        };
        let fact = facts.append_fact(Fact {
            place: FactPlace::Place(place),
            point,
            origin: FactOrigin::CallEnsures,
            evidence,
            payload: FactPayload::AssignedCase {
                variant: case.symbol,
            },
        });
        facts.append_ref(refs, fact);
    }
}

/// A completed result occurrence owns its by-value projections. A projection
/// through a reference still denotes mutable external storage and must retain
/// the original contract's referent dependencies rather than a detached tag.
fn detached_result_projection(
    program: &dyn validation::CheckingInput,
    subject: &validation::ReservedResultPlace,
) -> bool {
    let Some((_, root_type)) = validation::reserved_result_owner(program, subject.root) else {
        return false;
    };
    (0..=subject.segments.len()).all(|length| {
        validation::project_type_reference_from_segments(
            program,
            root_type,
            &subject.segments[..length],
        )
        .is_some_and(|reference| validation::reference_shell(program, reference).is_none())
    })
}
