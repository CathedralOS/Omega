//! Why a machine reached by a Unit closure has no checked Unit plan.
//!
//! The checked stage records one omission row per checked-body machine it
//! left without a plan. Closure rows name the direct dependency that was
//! unavailable, so following them from the machine the lowering asked for
//! reaches the machine whose own body failed local construction. The
//! rendered explanation is diagnostic text for the lowering error; it never
//! selects a fallback body.

use checked_trees::{CheckedTrees, CheckedUnitPlanOmissionStage};

/// Render the omission chain that starts at `machine`, or `None` when the
/// checked record does not name it (a machine outside the checked-body
/// roster, or one whose plan exists and was rejected for another reason).
pub(crate) fn unit_plan_omission_explanation(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Option<String> {
    let plans = &checked.facts.flow.terminal_unit_effects;
    let name = |symbol: symbols::SymbolHandle| checked.symbols.display_path(symbol, "::");
    let mut current = machine;
    let mut visited = Vec::new();
    let mut steps = Vec::new();
    loop {
        let row = plans.omission_for_machine(current)?;
        visited.push(current);
        let step = match row.stage {
            CheckedUnitPlanOmissionStage::LocalConstruction {
                phase,
                state_index,
                statement_index,
            } => {
                let mut position = String::new();
                if let Some(index) = state_index {
                    position.push_str(&format!(", state {index}"));
                }
                if let Some(index) = statement_index {
                    position.push_str(&format!(", statement {index}"));
                }
                format!(
                    "`{}` has no admitted body (local construction stopped at {phase}{position})",
                    name(current)
                )
            }
            CheckedUnitPlanOmissionStage::ReceiverReconciliation => format!(
                "`{}` was dropped while reconciling retained receivers",
                name(current)
            ),
            CheckedUnitPlanOmissionStage::CompetingCandidates => format!(
                "`{}` had competing candidate bodies and neither survived",
                name(current)
            ),
            CheckedUnitPlanOmissionStage::ComposedVocabulary => format!(
                "`{}` uses an operation outside the composed control vocabulary",
                name(current)
            ),
            CheckedUnitPlanOmissionStage::UnavailableCallee { target } => {
                let step = format!(
                    "`{}` calls `{}`, which has no plan",
                    name(current),
                    name(target)
                );
                steps.push(step);
                if visited.contains(&target) {
                    break;
                }
                current = target;
                continue;
            }
            CheckedUnitPlanOmissionStage::MissingBoundaryTarget { target } => format!(
                "`{}` calls boundary `{}`, which has no boundary plan",
                name(current),
                name(target)
            ),
            CheckedUnitPlanOmissionStage::UnavailableScalarTarget { target } => format!(
                "`{}` calls scalar `{}`, which has neither a registered target nor an ordinary body",
                name(current),
                name(target)
            ),
        };
        steps.push(step);
        break;
    }
    Some(steps.join("; "))
}
