//! Prepare resources for pointer values outside the owned descriptor inventory.
//!
//! Preparation grants nothing. Flow must bind the row to an actual completed
//! descriptor read, retaining its source access and lifetime profile before
//! activating it. A stored pointer is independent of the receiver's address:
//! its namespace cannot be obtained by appending a field to that address.

use super::super::tracker::StateLoanTracker;
use super::{PendingIncomingReference, StatementBorrowLoan};
use checked_trees::{
    BorrowLoanLineage, CapturedPlace, IncomingReferenceIdentity, IncomingReferenceOrigin,
};
use facts::PlaceRoot;
use typed_trees::{
    expression::ExpressionNode, machine::Machine, state::State, statement::StatementNode,
};

pub(in crate::borrow) fn prepare(
    program: &dyn validation::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    statement: &StatementNode,
    current: &[StateLoanTracker],
    pending: &mut Vec<StatementBorrowLoan>,
) {
    validation::visit_statement_value_occurrences(
        program,
        machine.symbol,
        state.symbol,
        statement_index,
        statement,
        &mut |expression, _, _| {
            if !matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
            ) || pending
                .iter()
                .any(|loan| loan.storage_owner == PlaceRoot::Expression(expression))
            {
                return;
            }
            let Some(carrier_type) = validation::expression_type_reference_in_state(
                program,
                state.symbol,
                statement_index,
                expression,
            ) else {
                return;
            };
            let Some(kind) = super::types::reference_borrow_access_kind(program, carrier_type)
            else {
                return;
            };
            let Some(source) = crate::published::places::place_at(
                program,
                state.symbol,
                statement_index,
                expression,
            ) else {
                return;
            };
            // Owned carried leaves already have a permission producer. This
            // inventory ends at a reference; a deeper load needs its own row.
            if current.iter().any(|loan| {
                loan.storage_owner == source.root
                    && loan.owner_path.len() == source.segments.len()
                    && super::owner_path_matches(program, &loan.owner_path, &source.segments)
            }) {
                return;
            }
            // A current supplier reached through owned carriers already owns
            // the selected descriptor. Only an unconsumed pointer boundary
            // needs preparation; do not replace a known stored producer.
            let rebased = super::call_views::rebase_borrow_places_through_local_loans(
                program,
                CapturedPlace {
                    root: source.root,
                    segments: source.segments.clone(),
                    ..Default::default()
                },
                current,
            );
            if !rebased.is_empty()
                && rebased.iter().all(|source| {
                    current.iter().any(|supplier| {
                        supplier.handle.is_valid()
                            && supplier.handle == source.parent_loan
                            && supplier.storage_owner == source.source_place.root
                            && supplier.owner_path.len() == source.source_place.segments.len()
                            && super::owner_path_matches(
                                program,
                                &supplier.owner_path,
                                &source.source_place.segments,
                            )
                    })
                })
            {
                return;
            }
            pending.push(StatementBorrowLoan {
                incoming_origin: Some(PendingIncomingReference {
                    identity: IncomingReferenceIdentity {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                        origin: IncomingReferenceOrigin::DescriptorLoad { expression },
                        carrier_type,
                        owner_path: Default::default(),
                    },
                    owner_path: Vec::new(),
                }),
                formation_expression: expression,
                formation_owner_path: Some(Vec::new()),
                owner_symbol: state.symbol,
                storage_owner: PlaceRoot::Expression(expression),
                owner_name: state.name.clone(),
                owner_path: Vec::new(),
                place: CapturedPlace {
                    root: PlaceRoot::Expression(expression),
                    ..Default::default()
                },
                representation_address: None,
                retained_operand: None,
                source_owner_symbol: Default::default(),
                supplying_loans: Vec::new(),
                lineage: BorrowLoanLineage::DirectRoot,
                kind,
                formation: super::BorrowFormation::Ordinary,
            });
        },
    );
}
