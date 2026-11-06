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
            // A containing loan owns the descriptors inside its referent
            // transitively, and an incoming entry leaf already owns its exact
            // slot through the incoming constraints. This inventory ends at a
            // reference; a deeper load needs its own row.
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
            // A supplier owns a resolved slot when its own owner path ends
            // at or above that slot's stored path. Minted rows keep strict
            // rebase-parent identity; an incoming leaf owns its place at
            // entry and never joins the minted-loan arena, so there the
            // covering slot is the supplier's identity.
            let cover = |source: &super::RebasedBorrowPlace| {
                current.iter().find(|supplier| {
                    supplier.storage_owner == source.source_place.root
                        && super::owner_path_matches(
                            program,
                            &supplier.owner_path,
                            &source.source_place.segments,
                        )
                        && (supplier.handle == source.parent_loan || !supplier.handle.is_valid())
                })
            };
            if !rebased.is_empty() && rebased.iter().all(|source| cover(source).is_some()) {
                // An exact-path cover already owns the requested slot: a
                // minted rebase parent is itself a retained occurrence, and an
                // incoming leaf resolves the slot through the incoming
                // constraints. Only a load that lands inside a leaf's referent
                // — deeper than every covering path — needs a prepared row;
                // the leaf cannot stand in for the deeper pointer's slot.
                if rebased.iter().all(|source| {
                    cover(source).is_some_and(|loan| {
                        loan.owner_path.len() == source.source_place.segments.len()
                    })
                }) {
                    return;
                }
                if rebased.len() == 1 {
                    let source = &rebased[0];
                    // The ingress leaf grants this descriptor already, but an
                    // entry tracker is invisible to the retained-occurrence
                    // resolver. Carry the resolved slot on the prepared row:
                    // same-slot loads then share one carrier generation, and
                    // the referent family names the real incoming leaf. The
                    // expression still owns the storage marker — descriptor
                    // binding and the per-evaluation lifecycle key on it —
                    // while the resolved leaf path is the slot identity.
                    let owner_path = super::owner_path_from_place_segments(
                        program,
                        &source.source_place.segments,
                    );
                    pending.push(StatementBorrowLoan {
                        incoming_origin: Some(PendingIncomingReference {
                            identity: IncomingReferenceIdentity {
                                machine_symbol: machine.symbol,
                                state_symbol: state.symbol,
                                origin: IncomingReferenceOrigin::DescriptorLoad { expression },
                                carrier_type,
                                owner_path: Default::default(),
                            },
                            owner_path: owner_path.clone(),
                        }),
                        formation_expression: expression,
                        formation_owner_path: Some(Vec::new()),
                        owner_symbol: state.symbol,
                        storage_owner: facts::PlaceRoot::Expression(expression),
                        owner_name: state.name.clone(),
                        owner_path,
                        place: CapturedPlace {
                            root: source.source_place.root,
                            ..Default::default()
                        },
                        representation_address: None,
                        retained_operand: None,
                        source_owner_symbol: source.source_owner_symbol,
                        supplying_loans: Vec::new(),
                        lineage: BorrowLoanLineage::DirectRoot,
                        kind,
                        formation: super::BorrowFormation::Ordinary,
                    });
                    return;
                }
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
