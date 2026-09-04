use super::*;

pub(super) fn append_state_writable_roots(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    writable_roots: &mut psi_arena::Arena<BorrowWritableRootFact>,
    writable_roots_span: &mut psi_arena::HandleSpan<BorrowWritableRootFact>,
) {
    for field in attached_data_fields(program, machine) {
        writable_roots.append_to_span(
            writable_roots_span,
            BorrowWritableRootFact {
                symbol: field.symbol,
                kind: BorrowRootKind::OwnedData,
            },
        );
    }

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
        let StatementNode::LocalData(local_data) = statement else {
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

pub(super) fn attached_data_fields<'program>(
    program: &'program psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> impl Iterator<Item = &'program psi_typed_trees::data::DataField> {
    machine
        .attached_data
        .as_ref()
        .and_then(|attached_data| {
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.name == *attached_data)
        })
        .into_iter()
        .flat_map(|definition| program.data_members(definition).iter())
        .filter_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) => Some(field),
            psi_typed_trees::data::DataMember::Variant(_) => None,
        })
}

pub(super) fn mutable_parameter_count(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
) -> usize {
    program
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.is_mutable)
        .count()
}

pub(super) fn estimated_borrow_root_capacity(program: &psi_typed_trees::TypedTrees) -> usize {
    program
        .machines()
        .iter()
        .map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .map(|state| {
                    let local_data_count = program
                        .statement_table
                        .statements(state.statement_nodes)
                        .iter()
                        .filter(|statement| matches!(statement, StatementNode::LocalData(_)))
                        .count();

                    program.machine_owned_data(machine).len()
                        + attached_data_fields(program, machine).count()
                        + local_data_count
                        + mutable_parameter_count(program, state)
                })
                .sum::<usize>()
        })
        .sum()
}
