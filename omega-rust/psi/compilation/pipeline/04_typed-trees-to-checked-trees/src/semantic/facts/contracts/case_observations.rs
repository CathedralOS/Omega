//! Case contracts publish discriminator observations with their own lifetime.
//!
//! The retained Boolean contract still supplies its proof obligation and full
//! identity. Its selected case is also a live value fact on the caller result,
//! so case consumers never interpret a callee's reserved `result` as caller
//! syntax or parse a rendered contract label. Caller writes invalidate this
//! fact through the ordinary place/context lifetime rules. Independent tag
//! contexts keep payload promises and whole-value snapshots from extending or
//! shortening the discriminator lifetime; dynamic selectors remain dependencies.

use checked_trees::{
    ContractCallFact, ContractProofFact, ContractProofFactKind, ContractProofFactOwner,
};
use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint, QualificationEvidence,
};

pub(super) fn append_result(
    program: &dyn validation::CheckingInput,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    contract: &ContractProofFact,
    point: ProgramPoint,
    evidence: QualificationEvidence,
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
    let Some(machine) = program.machine_by_symbol(call.target_machine_symbol) else {
        return;
    };
    let Some(state) = program.state_in_machine(machine, call.target_state_symbol) else {
        return;
    };
    for (subject_expression, variant) in selected_cases(program, machine, state, contract) {
        let Some(subject) = validation::reserved_result_place(program, subject_expression) else {
            continue;
        };
        if subject.machine_symbol != machine.symbol
            || !detached_result_projection(program, &subject)
        {
            continue;
        }
        let Some(place) = crate::semantic::places::instantiate_call_contract_expression_place(
            program,
            facts,
            call,
            subject_expression,
        ) else {
            continue;
        };
        let dependencies: Vec<_> = selector_occurrences(program, subject_expression)
            .into_iter()
            .map(|expression| {
                crate::semantic::places::instantiate_call_contract_expression_place(
                    program, facts, call, expression,
                )
                .map(FactPlace::Place)
                .unwrap_or(FactPlace::Unknown)
            })
            .collect();
        crate::semantic::facts::append_observation_context(
            facts,
            Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::CallEnsures,
                evidence,
                payload: FactPayload::CaseValue {
                    variant,
                    value: true,
                },
            },
            &dependencies,
        );
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

/// Requirements are live assumptions only at their exact entry. Extract the
/// same positive case atoms as result contracts; never treat an ensures
/// declaration or an inherited foreign parameter as an entry observation.
pub(super) fn append_requirement(
    program: &dyn validation::CheckingInput,
    proof: &checked_trees::ProofFacts,
    facts: &mut FactPlan,
    contract: &ContractProofFact,
    declaration: &Fact,
) {
    if contract.kind != ContractProofFactKind::Requires || contract.inherited_scope.is_some() {
        return;
    }
    let ProgramPoint::State {
        machine_symbol,
        state_symbol,
    } = declaration.point
    else {
        return;
    };
    let Some((machine, state)) = validation::find_state(program, state_symbol)
        .filter(|(machine, _)| machine.symbol == machine_symbol)
    else {
        return;
    };
    for (expression, variant) in selected_cases(program, machine, state, contract) {
        let Some(place) =
            super::places::contract_expression_place(program, proof, facts, contract, expression)
        else {
            continue;
        };
        let dependencies: Vec<_> = selector_occurrences(program, expression)
            .into_iter()
            .map(|expression| {
                super::places::contract_expression_place(
                    program, proof, facts, contract, expression,
                )
                .map(FactPlace::Place)
                .unwrap_or(FactPlace::Unknown)
            })
            .collect();
        crate::semantic::facts::append_observation_context(
            facts,
            Fact {
                place: FactPlace::Place(place),
                payload: FactPayload::CaseValue {
                    variant,
                    value: true,
                },
                ..*declaration
            },
            &dependencies,
        );
    }
}

/// Only resolved builtin conjunction and exact case membership justify
/// decomposition. Authored operators and alternatives grant no case atom.
fn selected_cases(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    contract: &ContractProofFact,
) -> Vec<(
    typed_trees::expression::ExpressionHandle,
    symbols::SymbolHandle,
)> {
    let typed_trees::domain::ProofFact::Expression(expression) =
        program.proof_facts.get(contract.fact)
    else {
        return Vec::new();
    };
    let mut cases = Vec::new();
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
        if let Some(case) = validation::applied_case_membership_variant(
            program,
            machine,
            Some(state),
            expression,
            binary,
        ) {
            cases.push((binary.left, case));
        }
    }
    cases
}

/// Index operands choose which discriminator the observation names. They keep
/// whole-value dependencies even though writes below the selected sum do not.
fn selector_occurrences(
    program: &dyn validation::CheckingInput,
    subject: typed_trees::expression::ExpressionHandle,
) -> Vec<typed_trees::expression::ExpressionHandle> {
    let mut occurrences = Vec::new();
    crate::facts::contract_occurrences::append_expression_occurrences(
        program,
        subject,
        &mut occurrences,
    );
    occurrences.retain(|occurrence| *occurrence != subject);
    occurrences
}
