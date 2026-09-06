//! An implicit mutable or write-only receiver is an exclusive call operand.

use checked_trees::{BorrowAccessKind, BorrowCallFact, CapturedPlace, CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;
use language_semantics::ReferenceAccess;
use typed_trees::TypedTrees;
use typed_trees::types::TypeReferenceNode;

use super::super::overlap::captured_place_compatibility;

pub(super) fn check_exclusive_receiver_conflicts(
    program: &TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call: &BorrowCallFact,
    entry_constraints: arena::HandleSpan<checked_trees::FlowConstraintRef>,
    target_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !call.has_receiver {
        return;
    }
    let Some((receiver_access, receiver_name)) =
        crate::call_target_parameters(program, call.target_symbol).and_then(|parameters| {
            parameters
                .iter()
                .filter(|parameter| parameter.is_self)
                .find_map(|parameter| {
                    match program
                        .type_reference_table
                        .type_reference(parameter.type_reference)
                    {
                        TypeReferenceNode::Reference {
                            access: ReferenceAccess::Mutable,
                            ..
                        } => Some((BorrowAccessKind::Mutable, "mutable")),
                        TypeReferenceNode::Reference {
                            access: ReferenceAccess::WriteOnly,
                            ..
                        } => Some((BorrowAccessKind::WriteOnly, "write-only")),
                        _ => None,
                    }
                })
        })
    else {
        return;
    };
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == state_flow.machine_symbol)
    else {
        return;
    };
    let receiver = crate::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )
    .and_then(|site| {
        crate::flow::canonical_receiver_place_for_call_site(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
            &site,
        )
    });
    let Some(crate::flow::CanonicalPlace {
        root: facts::PlaceRoot::Symbol(root_symbol),
        segments,
    }) = receiver
    else {
        diagnostics.push(Diagnostic::error(format!(
            "state `{target_name}` requires an exact place for its {receiver_name} receiver"
        )));
        return;
    };
    let receiver = attached_place(
        program,
        machine,
        CapturedPlace {
            root_symbol,
            segments,
        },
    );
    if !receiver_is_writable(program, facts, state_flow, entry_constraints, &receiver) {
        diagnostics.push(Diagnostic::error(format!(
            "state `{target_name}` requires a {receiver_name} receiver, but its source is not writable in this state"
        )));
    }
    // Whole mutable-receiver calls already use complete mutation summaries
    // for field-precise interference (including known-pure helpers). Keep that
    // existing path after checking source authority; a projected receiver
    // introduces the exclusive child loan admitted here. Whole write-only
    // receivers remain strictly exclusive.
    if receiver_access == BorrowAccessKind::Mutable && receiver.segments.is_empty() {
        return;
    }
    let overlaps = |place: CapturedPlace, access: &BorrowAccessKind| {
        !captured_place_compatibility(
            program,
            &receiver,
            &receiver_access,
            &attached_place(program, machine, place),
            access,
        )
        .non_interfering
    };
    for argument in facts.borrow.argument_accesses.span_or_empty(call.accesses) {
        if overlaps(
            CapturedPlace {
                root_symbol: argument.root_symbol,
                segments: facts
                    .borrow
                    .access_segments
                    .span_or_empty(argument.segments)
                    .to_vec(),
            },
            &argument.kind,
        ) {
            diagnostics.push(Diagnostic::error(format!(
                "state `{target_name}` receives {receiver_name} receiver overlapping another argument in the same call"
            )));
        }
    }
    for loan in facts.flow.borrow_loan_constraints(entry_constraints) {
        let loan = facts.borrow.loans.get(loan);
        if overlaps(
            CapturedPlace {
                root_symbol: loan.root_symbol,
                segments: facts.borrow.loan_segments(loan).to_vec(),
            },
            &loan.kind,
        ) {
            diagnostics.push(Diagnostic::error(format!(
                "state `{target_name}` receives {receiver_name} receiver while local borrow `{}` is still active",
                program.symbols.name(loan.owner_symbol),
            )));
        }
    }
}

fn receiver_is_writable(
    program: &TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    entry_constraints: arena::HandleSpan<checked_trees::FlowConstraintRef>,
    receiver: &CapturedPlace,
) -> bool {
    let Some(state) = crate::find_state(program, state_flow.state_symbol) else {
        return false;
    };
    if receiver.root_symbol == state_flow.machine_symbol {
        return validation::receiver_allows_mutation(program, program.state_parameters(state));
    }
    // Writable-root facts retain local bindings; the declared reference access
    // still decides whether the referent can be exclusively borrowed.
    for statement in program.statement_table.statements(state.statement_nodes) {
        if let typed_trees::statement::StatementNode::LocalData(local) = statement
            && local.symbol == receiver.root_symbol
        {
            return match program
                .type_reference_table
                .type_reference(local.type_reference)
            {
                TypeReferenceNode::Reference { access, .. } => access.is_exclusive(),
                _ => local.is_mutable,
            };
        }
    }
    facts
        .flow
        .borrow_writable_root_constraints(entry_constraints)
        .any(|root| facts.borrow.writable_roots.get(root).symbol == receiver.root_symbol)
}

/// Older access rows represent a direct self field as a root. Rejoin it to
/// this caller's exact attached storage before comparing it with whole self;
/// a same-named field under another owner must not become an alias.
fn attached_place(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    mut place: CapturedPlace,
) -> CapturedPlace {
    if program.machine_states(machine).iter().any(|state| {
        program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.is_self && parameter.symbol == place.root_symbol)
    }) {
        place.root_symbol = machine.symbol;
    } else if let Some(field) = validation::exact_attached_field(
        program,
        machine,
        place.root_symbol,
        program.symbols.name(place.root_symbol),
    ) {
        place.root_symbol = machine.symbol;
        place.segments.insert(
            0,
            facts::PlaceSegment::Field {
                symbol: field.symbol,
            },
        );
    }
    place
}
