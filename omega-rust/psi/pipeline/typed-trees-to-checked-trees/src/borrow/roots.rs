use checked_trees::statement::StatementNode;
use checked_trees::{BorrowRootKind, BorrowWritableRootFact};

pub(super) fn append_state_writable_roots(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    writable_roots: &mut arena::Arena<BorrowWritableRootFact>,
    writable_roots_span: &mut arena::HandleSpan<BorrowWritableRootFact>,
) {
    for field in attached_data_fields(program, machine)
        .filter(|_| validation::receiver_allows_mutation(program, program.state_parameters(state)))
    {
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

    // A consuming `self` owns its attached storage outright — the receiver
    // contract treats that storage as mutable without a `mut` marker — so
    // the whole-receiver place is writable exactly as a `mut` parameter is.
    // `&mut`, `mut`, and `&write` receivers are already counted above.
    for parameter in program
        .state_parameters(state)
        .iter()
        .filter(|parameter| is_consuming_self_parameter(program, parameter))
    {
        writable_roots.append_to_span(
            writable_roots_span,
            BorrowWritableRootFact {
                symbol: parameter.symbol,
                kind: BorrowRootKind::OwnedData,
            },
        );
    }
}

/// A `self` parameter that carries the machine's storage by value: owned,
/// mutation-capable (`receiver_allows_mutation`), and not already reported
/// as a `mut`/reference parameter.
fn is_consuming_self_parameter(
    program: &typed_trees::TypedTrees,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    parameter.is_self
        && !parameter.is_mutable
        && !parameter.is_const
        && !parameter.relevance.is_erased()
        && parameter.type_reference.is_valid()
        && !matches!(
            program
                .type_reference_table
                .type_reference(parameter.type_reference),
            typed_trees::types::TypeReferenceNode::Reference { .. }
        )
}

pub(super) fn attached_data_fields<'program>(
    program: &'program typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> impl Iterator<Item = &'program typed_trees::data::DataField> {
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
            typed_trees::data::DataMember::Field(field) => Some(field),
            typed_trees::data::DataMember::Variant(_) => None,
        })
}

pub(super) fn mutable_parameter_count(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
) -> usize {
    program
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.is_mutable)
        .count()
}

/// Consuming `self` parameters join the writable roots beside the `mut`
/// parameters, so the estimate counts them too.
fn consuming_self_parameter_count(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
) -> usize {
    program
        .state_parameters(state)
        .iter()
        .filter(|parameter| is_consuming_self_parameter(program, parameter))
        .count()
}

pub(super) fn estimated_borrow_root_capacity(program: &typed_trees::TypedTrees) -> usize {
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
                        + consuming_self_parameter_count(program, state)
                })
                .sum::<usize>()
        })
        .sum()
}
