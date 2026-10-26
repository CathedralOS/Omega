//! Normal-return guarantees shared by ordinary and composed callable bodies.
//! Unit has no result value, but its post-state can establish caller facts.
//! Checking has established every authored guarantee. Publication carries the
//! closed scalar consequences the checked producer could represent, not a
//! second source-contract judgment. Selected calls, memberships and witness
//! terms retain their own checked owners rather than becoming builtin scalars.
//! Crashes and incomplete calls establish neither a result nor these facts.

use crate::lowering_error::LoweringError;
use crate::scalar_lowering::contracts::{self, ContractStructuralResult, ContractViewNamespace};
use crate::unit::ContractClause;
use checked_trees::CheckedTrees;
use symbols::SymbolHandle;
use terminal_psi::{TerminalMachineResult, ValueDeclaration};

pub(crate) fn lower(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    state: SymbolHandle,
    parameters: &[ValueDeclaration],
    result: &TerminalMachineResult,
    erased: &[ValueDeclaration],
    views: &ContractViewNamespace<'_>,
    next_obligation: &mut u64,
) -> Result<Vec<ContractClause>, LoweringError> {
    let (source, source_state) = crate::declarations::state_declaration(checked, state)?;
    let contract = checked
        .facts
        .contract_plans
        .for_machine(source.symbol)
        .ok_or(LoweringError::Unsupported(
            "normal return has no checked contract",
        ))?;
    // Scalar results additionally publish their result refinement. The same
    // checked consequence roster serves every result shape; an absent scalar
    // reading is not an absent source proof. In particular a selected operation
    // cannot be reinterpreted as builtin arithmetic to fill that absence.
    // General receiving predicates were completed while mathematical
    // preparation was available. Their subject is formal zero; bind that
    // namespace directly to this exact returned value, not to entry formals.
    let result_refinements = if result.scalar().is_some()
        && matches!(
            checked
                .type_reference_table
                .type_reference(source_state.return_type),
            checked_trees::types::TypeReferenceNode::Constrained { .. }
        ) {
        checked
            .facts
            .values
            .scalar_type_refinement_predicates(source_state.return_type)
            .ok_or(LoweringError::Unsupported(
                "scalar result has no complete checked value refinement",
            ))?
    } else {
        &[]
    };
    // Source-checked clauses without a scalar projection erase here. They are
    // neither asserted true nor exported as opaque atoms; upstream checking
    // established the complete normal-return contract.
    // Returning execution restores the declared domain of every exclusive
    // loan: the declaration predicate roster's restored rows follow the
    // authored guarantees as the same kind of obligation, rederived from the
    // entry parameters' declared storage rather than read from a package.
    let guarantees = contracts::projected_guarantees(contract.closed_scalar_values.ensures())
        .cloned()
        .collect::<Vec<_>>();
    let restored_guarantees = validation::machine_declaration_predicates(
        &checked.typed,
        &checked.facts.operators,
        source,
    )
    .into_iter()
    .filter(|row| row.restored_on_return)
    .map(|row| {
        Some(checked_trees::ClosedScalarContractValue::Predicate(
            row.predicate,
        ))
    })
    .collect::<Vec<_>>();
    let mut namespace = parameters.to_vec();
    namespace.extend(result.scalar());
    // The result observation has a distinct structural identity; returning an
    // owned payload does not turn it into an entry formal or a scalar value.
    let views = ContractViewNamespace {
        result: match result {
            TerminalMachineResult::Structural(declaration) => {
                let entry = checked.typed.machine_states(source).first().ok_or(
                    LoweringError::Unsupported("normal guarantee owner has no entry state"),
                )?;
                let position =
                    u32::try_from(checked.typed.state_parameters(entry).len()).map_err(|_| {
                        LoweringError::Unsupported(
                            "normal result coordinate exceeds the contract namespace",
                        )
                    })?;
                ContractStructuralResult::Declared {
                    position,
                    declaration,
                }
            }
            _ => ContractStructuralResult::Absent,
        },
        ..*views
    };
    let mut propositions = contracts::clauses(identities, &guarantees, &namespace, erased, &views)?;
    for predicate in result_refinements {
        let result_predicate = contracts::proposition(
            identities,
            predicate,
            result.scalar().as_slice(),
            &[],
            &ContractViewNamespace {
                checked: Some(checked),
                structural_types: views.structural_types,
                ..ContractViewNamespace::EMPTY
            },
        )?;
        propositions = Some(match propositions {
            Some(previous) => {
                crate::proofs::contract_predicates::connective(previous, result_predicate, true)?
            }
            None => result_predicate,
        });
    }
    if let Some(restored) =
        contracts::clauses(identities, &restored_guarantees, &namespace, erased, &views)?
    {
        propositions = Some(match propositions {
            Some(previous) => {
                crate::proofs::contract_predicates::connective(previous, restored, true)?
            }
            None => restored,
        });
    }
    propositions
        .into_iter()
        .map(|proposition| {
            Ok(ContractClause {
                obligation: crate::terminal_identities::obligation_id(
                    crate::terminal_identities::allocate_dense(next_obligation)?,
                ),
                proposition,
            })
        })
        .collect()
}
