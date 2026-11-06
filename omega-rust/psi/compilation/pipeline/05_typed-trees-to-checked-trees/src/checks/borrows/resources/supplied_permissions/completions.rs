//! A helper's completed store supplies a reference without lending the output
//! variable itself. Keep that permission and every promised input together;
//! copying the reference must not forget either when the variable changes.

use arena::{Handle, HandleSpan};
use checked_trees::{
    BorrowLoanLineage, CheckFacts, CheckedCallBorrowLoanSource,
    CheckedOutputLifetimeDependencyResource, CheckedSuppliedOutputBorrowResource,
    IncomingReferenceOrigin,
};
use diagnostics::Diagnostic;

pub(in crate::checks::borrows::resources) fn install(
    program: &dyn validation::CheckingInput,
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut plans = Vec::new();
    for (loan, row) in facts.borrow.loans.iter() {
        if row.lineage != BorrowLoanLineage::SuppliedOutput {
            continue;
        }
        let Some(call) = super::normal_call(&facts.borrow, &facts.flow, loan) else {
            continue; // A syntactic alternative does not establish a permission.
        };
        let incoming = facts.borrow.incoming_references.get(row.incoming_reference);
        let diagnostic = |reason: &'static str| {
            let invocation = facts.flow.control.calls.get(call);
            let message = format!(
                "{reason}; completed output from `{}` in machine `{}`, state `{}` ({:?}), statement {}, call {}, loan {loan:?}",
                crate::published::labels::call_target_label(program, invocation.target_symbol),
                crate::published::labels::symbol_name(program, incoming.machine_symbol),
                crate::published::labels::symbol_name(program, incoming.state_symbol),
                incoming.state_symbol,
                row.statement_index,
                invocation.call_ordinal,
            );
            let mut diagnostic = Diagnostic::error(message);
            let span = invocation
                .authored_expression
                .is_valid()
                .then(|| {
                    program
                        .expression_table
                        .source_span(invocation.authored_expression)
                })
                .filter(|span| span.span.end > span.span.start)
                .or_else(|| {
                    let machine = program.machine_by_symbol(incoming.machine_symbol)?;
                    let state = program.state_in_machine(machine, incoming.state_symbol)?;
                    crate::flow::state_statements(program, state.statement_nodes)
                        .get(invocation.statement_index)
                        .and_then(|statement| validation::statement_source_span(program, statement))
                });
            if let Some(span) = span {
                diagnostic = diagnostic.with_source_span(span);
            }
            vec![diagnostic]
        };
        let IncomingReferenceOrigin::Output { identity } = incoming.origin else {
            return refuse("completed output lost its output identity");
        };
        let owner = facts
            .borrow
            .owner_segments
            .span(incoming.owner_path)
            .ok_or_else(|| diagnostic("completed output lost its declared descriptor path"))?;
        let path = crate::borrow::owner_place_segments(owner);
        let mut receipts =
            facts
                .flow
                .control
                .output_lifetime_sources
                .iter()
                .filter(|(_, receipt)| {
                    receipt.output == identity
                        && facts
                            .flow
                            .control
                            .operand_retained_paths
                            .span(receipt.owner_path)
                            .is_some_and(|candidate| same_path(program, candidate, &path))
                });
        let (_, receipt) = receipts
            .next()
            .ok_or_else(|| diagnostic("completed output has no captured lifetime-input receipt"))?;
        if receipts.next().is_some() {
            return refuse("completed output has ambiguous lifetime-input receipts");
        }
        let sources = receipt
            .sources
            .ok_or_else(|| diagnostic("completed output lifetime inputs are unavailable"))?;
        let ceilings = crate::borrow::lifetime_contract::output_input_accesses(
            program,
            &facts.borrow,
            &facts.flow.control,
            identity,
            owner,
            sources,
        )
        .map_err(diagnostic)?;
        let state = facts
            .flow
            .control
            .unique_state(incoming.machine_symbol, incoming.state_symbol)
            .ok_or_else(|| diagnostic("completed output has no unique flow owner"))?;
        let inputs = super::inputs::plan(program, facts, state, call, sources, &ceilings)
            .map_err(diagnostic)?;
        let origin = facts
            .borrow
            .supplied_output_call(loan)
            .ok_or_else(|| diagnostic("completed output lost its establishing call"))?;
        let debts = facts
            .borrow
            .calls
            .get(origin)
            .completed_reference_loans
            .handles()
            .filter(|handle| {
                let debt = facts.borrow.loans.get(*handle);
                debt.lineage == BorrowLoanLineage::LifetimeDependency
                    && debt.storage_owner == row.storage_owner
                    && facts.borrow.loan_owner_path(debt) == facts.borrow.loan_owner_path(row)
            })
            .collect::<Vec<_>>();
        plans.push((
            CheckedSuppliedOutputBorrowResource {
                loan,
                machine_symbol: incoming.machine_symbol,
                state_symbol: incoming.state_symbol,
                call,
                identity: row.incoming_reference,
                lifetime_sources: sources,
                inputs: HandleSpan::empty(),
                debts: HandleSpan::empty(),
                holders: HandleSpan::empty(),
                holder_weakening: Handle::invalid(),
                final_weakening: Handle::invalid(),
            },
            inputs,
            debts,
        ));
    }
    facts
        .borrow
        .supplied_output_resources
        .reset_retain_capacity();
    facts.borrow.supplied_output_inputs.reset_retain_capacity();
    facts
        .borrow
        .output_lifetime_dependencies
        .reset_retain_capacity();
    facts
        .borrow
        .output_dependency_suppliers
        .reset_retain_capacity();
    for (mut output, inputs, debts) in plans {
        let mut handles = Vec::with_capacity(inputs.resources.len());
        for input in inputs.resources {
            let (permission, capture_path) = match input.permission {
                super::inputs::InputPermissionDraft::Borrowed(mut draft) => {
                    if let CheckedCallBorrowLoanSource::CallAccess { parents, .. } =
                        &mut draft.resource.source
                    {
                        let recorded = inputs.parents.span(*parents).ok_or_else(|| {
                            diagnostic("completed input lost its exact borrowing parents")
                        })?;
                        *parents = facts
                            .borrow
                            .call_loan_parents
                            .insert_many(recorded.iter().copied());
                    }
                    let resource =
                        super::super::call_operands::install_resource(program, facts, draft)
                            .ok_or_else(|| {
                                diagnostic("completed input permission has no typed capture path")
                            })?;
                    (
                        checked_trees::CheckedOutputBorrowInputPermission::Borrowed { resource },
                        facts.borrow.call_loan_resources.get(resource).capture_path,
                    )
                }
                super::inputs::InputPermissionDraft::Incoming {
                    place,
                    access,
                    owner_path,
                } => {
                    let contract = facts
                        .flow
                        .control
                        .operand_retained_suppliers
                        .get(input.source)
                        .contract
                        .ok_or_else(|| {
                            diagnostic("completed input has no original declared edge")
                        })?;
                    let actual = facts.flow.control.operands.get(contract.input);
                    let path =
                        crate::borrow::carrier_path(program, actual.type_reference, &owner_path)
                            .ok_or_else(|| {
                                diagnostic("incoming input has no typed captured leaf path")
                            })?;
                    let capture_path = facts.borrow.call_loan_paths.insert_many(path);
                    (
                        checked_trees::CheckedOutputBorrowInputPermission::Incoming {
                            identity: place.incoming_reference,
                            access,
                            root: place.root,
                            path: facts
                                .borrow
                                .call_loan_backing_paths
                                .insert_many(place.segments),
                            owner_path: facts
                                .borrow
                                .call_loan_backing_paths
                                .insert_many(place.incoming_owner_path),
                        },
                        capture_path,
                    )
                }
            };
            handles.push(checked_trees::CheckedOutputBorrowInput {
                capture_path,
                source: input.source,
                stored: input.stored,
                permission,
            });
        }
        output.inputs = facts.borrow.supplied_output_inputs.insert_many(handles);
        // The span belongs to loan_suppliers, which is never reset by resource
        // installation. Holder spans are installed later in resource_holders.
        output.debts = facts
            .borrow
            .loan_suppliers
            .insert_many(debts.iter().copied());
        let inputs = output.inputs;
        let output = facts.borrow.supplied_output_resources.insert(output);
        for debt in debts {
            facts.borrow.output_lifetime_dependencies.insert(
                CheckedOutputLifetimeDependencyResource {
                    loan: debt,
                    source: checked_trees::CheckedOutputLifetimeDependencySource::Completion {
                        output,
                    },
                    inputs,
                },
            );
        }
    }
    super::debts::install_transfers(facts)?;
    Ok(())
}

fn same_path(
    program: &dyn validation::CheckingInput,
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            validation::storage_path_relations::canonical_place_segments_equal(
                program, *left, *right,
            )
        })
}

fn diagnostic(reason: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(reason)]
}

fn refuse<T>(reason: &'static str) -> Result<T, Vec<Diagnostic>> {
    Err(diagnostic(reason))
}
