//! Selected exits partition established result custody, named or anonymous.
//! Source checking owns cleanup eligibility and the exact remaining complement.
//! This join identifies the retained producer before matching its selected exit
//! receipt or delegating a whole no-work occurrence to affine frontier closure.
//! Normal cleanup materializes selected hooks; graph closure retains no-code
//! births, transfers and deaths independently of an exit receipt.

use checked_trees::{CheckFacts, CheckedUnitEffectOperationPlan};
use language_semantics::{Multiplicity, PermissionEventSource};

pub(super) fn permits_disposal(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    operation: &CheckedUnitEffectOperationPlan,
    operations: &[CheckedUnitEffectOperationPlan],
    statement: Option<u32>,
    continuation: bool,
    no_code_completion_owner: bool,
) -> bool {
    validate(
        program,
        facts,
        state,
        operation,
        operations,
        statement,
        continuation,
        no_code_completion_owner,
    )
    .is_some()
}

fn validate(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    operation: &CheckedUnitEffectOperationPlan,
    operations: &[CheckedUnitEffectOperationPlan],
    statement: Option<u32>,
    continuation: bool,
    no_code_completion_owner: bool,
) -> Option<()> {
    let result = operation.structural_result_binding()?;
    let root = super::super::cleanup::normal_edges::producer_root(facts, state.symbol, operation)?;
    // A named home can have a later occupant. Only the result still occupying
    // it at this operation prefix can own the home's eventual disposition.
    if let Some(symbol) = root.as_symbol()
        && super::super::local_storage::occupant(program, state, operations, symbol)
            != Some(result.binding_ordinal)
    {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let statement_index = statement.map_or_else(
        || {
            if matches!(
                statements.last(),
                Some(typed_trees::statement::StatementNode::Expression(_))
            ) {
                statements.len() - 1
            } else {
                statements.len()
            }
        },
        |statement| statement as usize,
    );
    if result.statement_index as usize > statement_index {
        return None;
    }
    let reference = validation::canonical_place_type_reference(
        program,
        state.symbol,
        statement_index,
        &validation::CanonicalPlace {
            root,
            segments: Vec::new(),
        },
    )?;
    let expected_identity =
        validation::stored_structural_type_identity(program, reference, &[], &[]);
    let referent_identity = validation::reference_shell(program, reference)
        .map(|shell| validation::stored_structural_type_identity(program, shell.referee, &[], &[]));
    let multiplicity =
        validation::reference_result_custody::binding_multiplicity(program, reference);
    // A borrowed descriptor's release is not an additional owned disposal.
    if (referent_identity.is_some() && result.multiplicity != Multiplicity::Unrestricted)
        || (result.type_identity != expected_identity
            && referent_identity != Some(result.type_identity))
        || result.multiplicity != multiplicity
    {
        return None;
    }
    if multiplicity == Multiplicity::Unrestricted {
        return Some(());
    }
    // Recursive machine fusion can turn a pending terminal invocation into
    // an internal arrival. The source invocation has no normal exit receipt.
    // Its still-current, claim-free owned no-code result is instead managed by
    // Stage05's affine frontier equations, which retain births and transfers
    // across every selected edge. This is not a fabricated source cleanup or
    // a normal-return judgment; references and executable cleanup keep their
    // independently selected disposition below.
    if no_code_completion_owner
        && validation::has_no_code_cleanup_contents(program, reference)
        && validation::has_contents(
            program,
            reference,
            &[],
            validation::ContentsRequirement::BorrowFree,
        )
    {
        return Some(());
    }
    let machine = facts
        .flow
        .control
        .state_by_symbol(state.symbol)?
        .machine_symbol;
    let exit = checked_trees::FlowClaimJoinExit {
        state_symbol: state.symbol,
        statement_index,
        kind: if continuation {
            checked_trees::FlowClaimJoinExitKind::TransitionContinuation
        } else if statements
            .get(statement_index)
            .is_some_and(|statement| statement.as_transition().is_some())
        {
            checked_trees::FlowClaimJoinExitKind::TransitionTarget
        } else {
            checked_trees::FlowClaimJoinExitKind::FinalExpression
        },
    };
    facts
        .contextual_cleanup
        .dispositions
        .iter()
        .any(|(_, disposition)| {
            disposition.machine == machine
                && disposition.state == state.symbol
                && disposition.root == root
                && disposition.source == PermissionEventSource::StateExit
                && disposition.exit == exit
        })
        .then_some(())
}
