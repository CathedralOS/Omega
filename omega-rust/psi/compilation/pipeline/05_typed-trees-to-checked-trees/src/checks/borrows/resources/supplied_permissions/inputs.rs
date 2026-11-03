//! Keep the inputs promised by a completed output at their original captures.
//! A stored reference comes from the captured contents, so its lifetime does
//! not turn the short loan of the output slot into a long exclusive borrow.

use super::super::call_operands::ResourceDraft;
use super::super::permission_sources;
use arena::{Arena, Handle, HandleSpan};
use checked_trees::{
    BorrowLoanFact, CheckFacts, CheckedCallBorrowLoanResource, CheckedCallBorrowLoanSource,
    CheckedDirectBorrowParentLifetime, FlowCallFact, FlowOperandRetainedSupplier, FlowStateFact,
};

pub(super) struct InputResourceDraft {
    pub(super) source: Handle<FlowOperandRetainedSupplier>,
    pub(super) resource: ResourceDraft,
}

pub(super) struct InputPlan {
    pub(super) resources: Vec<InputResourceDraft>,
    pub(super) parents: Arena<Handle<BorrowLoanFact>>,
}

pub(super) fn plan(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: Handle<FlowCallFact>,
    sources: HandleSpan<FlowOperandRetainedSupplier>,
    ceilings: &[language_semantics::ReferenceAccess],
) -> Result<InputPlan, &'static str> {
    let mut resources = Vec::new();
    let mut parents = Arena::new();
    let recorded = facts
        .flow
        .control
        .operand_retained_suppliers
        .span(sources)
        .ok_or("completed output lifetime inputs are unavailable")?;
    if recorded.len() != ceilings.len() {
        return Err("completed output input access roster is incomplete");
    }
    for ((handle, supplier), ceiling) in sources.handles().zip(recorded).zip(ceilings) {
        let contract = supplier
            .contract
            .ok_or("completed output lifetime input has no original applied contract")?;
        let source_type = crate::borrow::lifetime_contract::source_type(
            program,
            &facts.borrow,
            &facts.flow.control,
            supplier,
        )
        .ok_or("completed output lifetime input does not match its captured call argument")?;
        let projection = facts
            .flow
            .control
            .operand_retained_paths
            .span(supplier.projection)
            .ok_or("completed output lifetime input lost its captured projection")?;
        let source_path = facts
            .flow
            .control
            .operand_retained_paths
            .span(contract.source_path)
            .ok_or("completed output lifetime input lost its declared source path")?;
        let leaves = if let Some(shell) = validation::reference_shell(program, source_type) {
            vec![(Vec::new(), shell.access)]
        } else {
            validation::borrow_contracts::carried_reference_paths(program, source_type)
                .ok_or("completed output input has an unresolved reference frontier")?
        };
        for (leaf, access) in leaves {
            let mut captured_path = projection.to_vec();
            captured_path.extend(crate::borrow::owner_place_segments(&leaf));
            let mut owner_path =
                crate::borrow::owner_path_from_place_segments(program, source_path);
            owner_path.extend_from_slice(&leaf);
            let access = match (*ceiling, access) {
                (
                    language_semantics::ReferenceAccess::Shared,
                    language_semantics::ReferenceAccess::WriteOnly,
                )
                | (
                    language_semantics::ReferenceAccess::WriteOnly,
                    language_semantics::ReferenceAccess::Shared,
                ) => {
                    return Err(
                        "completed input's access ceiling permits no requested leaf access",
                    );
                }
                (language_semantics::ReferenceAccess::Shared, _)
                | (_, language_semantics::ReferenceAccess::Shared) => {
                    checked_trees::BorrowAccessKind::Read
                }
                (language_semantics::ReferenceAccess::WriteOnly, _)
                | (_, language_semantics::ReferenceAccess::WriteOnly) => {
                    checked_trees::BorrowAccessKind::WriteOnly
                }
                _ => checked_trees::BorrowAccessKind::Mutable,
            };
            let (suppliers, captured_parents) = permission_sources::lending_sources(
                program,
                facts,
                state,
                call,
                supplier.operand,
                &captured_path,
                &access,
            )?;
            for mut selected in suppliers {
                if let CheckedCallBorrowLoanSource::CallAccess { parents: old, .. } =
                    &mut selected.source
                {
                    let retained = captured_parents
                        .span(*old)
                        .ok_or("completed input permission has an unavailable parent roster")?;
                    *old = parents.insert_many(retained.iter().copied());
                }
                resources.push(InputResourceDraft {
                    source: handle,
                    resource: ResourceDraft {
                        resource: CheckedCallBorrowLoanResource {
                            machine_symbol: state.machine_symbol,
                            state_symbol: state.state_symbol,
                            call,
                            owner_operand: contract.input,
                            operand: supplier.operand,
                            source: selected.source,
                            access: access.clone(),
                            parent_lifetime: CheckedDirectBorrowParentLifetime {
                                machine_symbol: state.machine_symbol,
                                state_symbol: state.state_symbol,
                                root: selected.backing.root,
                            },
                            ..Default::default()
                        },
                        path: owner_path.clone(),
                        backing: selected.backing,
                    },
                });
            }
        }
    }
    Ok(InputPlan { resources, parents })
}
