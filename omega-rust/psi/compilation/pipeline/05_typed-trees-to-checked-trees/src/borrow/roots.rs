use checked_trees::statement::StatementNode;
use checked_trees::{BorrowRootKind, BorrowWritableRootFact};

pub(super) fn append_state_writable_roots(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    writable_roots: &mut arena::Arena<BorrowWritableRootFact>,
    writable_roots_span: &mut arena::HandleSpan<BorrowWritableRootFact>,
) {
    for owned in program.machine_owned_data(machine) {
        writable_roots.append_to_span(
            writable_roots_span,
            BorrowWritableRootFact {
                symbol: owned.symbol,
                kind: BorrowRootKind::OwnedData,
            },
        );
    }

    for statement in program.statement_table.statements(state.statement_nodes) {
        let Some(local_data) = statement.as_local_data() else {
            continue;
        };
        writable_roots.append_to_span(
            writable_roots_span,
            BorrowWritableRootFact {
                symbol: local_data.symbol,
                kind: BorrowRootKind::LocalData,
            },
        );
    }

    for parameter in program
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.is_mutable)
    {
        writable_roots.append_to_span(
            writable_roots_span,
            BorrowWritableRootFact {
                symbol: parameter.symbol,
                kind: BorrowRootKind::MutableParameter,
            },
        );
    }
}

pub(super) fn mutable_parameter_count(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
) -> usize {
    program
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.is_mutable)
        .count()
}

pub(super) fn estimated_borrow_root_capacity(program: &dyn ::validation::CheckingInput) -> usize {
    program
        .machines()
        .iter()
        .map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .map(|state| {
                    let local_data_count =
                        crate::flow::state_statements(program, state.statement_nodes)
                            .iter()
                            .filter(|statement| matches!(statement, StatementNode::LocalData(_)))
                            .count();

                    program.machine_owned_data(machine).len()
                        + local_data_count
                        + mutable_parameter_count(program, state)
                })
                .sum::<usize>()
        })
        .sum()
}
