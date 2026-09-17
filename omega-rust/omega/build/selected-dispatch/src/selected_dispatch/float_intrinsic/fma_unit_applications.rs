//! Selected IEEE float fused multiply-add unit applications.

use crate::selected_dispatch::float_intrinsic::intrinsic_resolution::SelectedIntrinsicUse;
use crate::selected_dispatch::float_intrinsic::{NamedFloatRealization, StagedNamedFloatRewrite};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use numerics::literals::FloatFormat;
use typed_trees::expression::ExpressionNode;

pub(crate) fn selected_ieee_float_fma_unit_applications(
    checked: &CheckedTrees,
    rewrites: &[StagedNamedFloatRewrite],
) -> Result<Vec<typed_trees_to_checked_trees::SelectedIeeeFloatFmaUnitApplication>, Diagnostic> {
    let mut applications = Vec::new();
    for rewrite in rewrites {
        let format = match rewrite.realization {
            NamedFloatRealization::FusedMultiplyAdd(FloatFormat::F32) => {
                semantic_vocabulary::IeeeFloatFormat::Binary32
            }
            NamedFloatRealization::FusedMultiplyAdd(FloatFormat::F64) => {
                semantic_vocabulary::IeeeFloatFormat::Binary64
            }
            _ => continue,
        };
        // Both spellings retain the use as a checked fact; the FMA application
        // keys on the requirement symbol either fact carries.
        let uses = checked
            .facts
            .operators
            .named_uses()
            .map(SelectedIntrinsicUse::from)
            .chain(
                checked
                    .facts
                    .operators
                    .named_requirement_uses()
                    .map(SelectedIntrinsicUse::from),
            )
            .filter(|selected_use| selected_use.expression == rewrite.expression)
            .collect::<Vec<_>>();
        let Some(selected_use) = uses.first().copied() else {
            return Err(Diagnostic::error(format!(
                "selected nearest FMA expression {:?} retains no checked named use",
                rewrite.expression,
            )));
        };
        if uses.iter().any(|candidate| {
            candidate.requirement_symbol != selected_use.requirement_symbol
                || candidate.policy_adapter != selected_use.policy_adapter
                || candidate.provider_plan_report_fingerprint
                    != selected_use.provider_plan_report_fingerprint
                || candidate.provider_plan_commitment != selected_use.provider_plan_commitment
        }) {
            return Err(Diagnostic::error(format!(
                "selected nearest FMA expression {:?} carries contradictory checked named-use custody",
                rewrite.expression,
            )));
        }
        let mut local_initializer_origins = Vec::new();
        for candidate in &uses {
            if matches!(
                candidate.origin,
                checked_trees::CheckedValueOrigin::StateStatement {
                    role: checked_trees::CheckedValueStatementRole::LocalInitializer,
                    ..
                }
            ) && !local_initializer_origins.contains(&candidate.origin)
            {
                local_initializer_origins.push(candidate.origin);
            }
        }
        let origin = match local_initializer_origins.as_slice() {
            [] => continue,
            [origin] => *origin,
            origins => {
                return Err(Diagnostic::error(format!(
                    "selected nearest FMA expression {:?} is attributed to {} distinct local initializers",
                    rewrite.expression,
                    origins.len(),
                )));
            }
        };
        let ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(rewrite.expression)
        else {
            return Err(Diagnostic::error(format!(
                "selected nearest FMA expression {:?} lost its checked call shape",
                rewrite.expression,
            )));
        };
        applications.push(
            typed_trees_to_checked_trees::SelectedIeeeFloatFmaUnitApplication {
                expression: rewrite.expression,
                origin,
                requirement_operator: selected_use.requirement_symbol,
                provider_plan_report_fingerprint: selected_use.provider_plan_report_fingerprint,
                provider_plan_commitment: selected_use.provider_plan_commitment,
                format,
                operands: checked
                    .typed
                    .expression_table
                    .expression_handles(call.arguments)
                    .to_vec(),
            },
        );
    }
    Ok(applications)
}

pub(crate) fn validate_selected_ieee_float_fma_unit_applications(
    checked: &CheckedTrees,
    applications: &[typed_trees_to_checked_trees::SelectedIeeeFloatFmaUnitApplication],
) -> Result<(), Diagnostic> {
    for application in applications {
        let checked_trees::CheckedValueOrigin::StateStatement {
            machine_symbol,
            state_symbol,
            statement_index,
            role: checked_trees::CheckedValueStatementRole::LocalInitializer,
        } = application.origin
        else {
            continue;
        };
        let matches = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .filter(|machine| {
                machine.machine == machine_symbol && machine.state == state_symbol
            })
            .flat_map(|machine| &machine.operations)
            .filter(|operation| {
                matches!(
                    operation,
                    checked_trees::CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd {
                        coordinate,
                        requirement_operator,
                        provider_plan_report_fingerprint,
                        provider_plan_commitment,
                        format,
                        ..
                    } if usize::try_from(coordinate.statement_index).ok() == Some(statement_index)
                        && *requirement_operator == application.requirement_operator
                        && *provider_plan_report_fingerprint == application.provider_plan_report_fingerprint
                        && *provider_plan_commitment == application.provider_plan_commitment
                        && *format == application.format
                )
            })
            .count();
        if matches > 1 {
            return Err(Diagnostic::error(format!(
                "selected nearest FMA at {:?} retained {matches} exact checked Unit operations; expected at most one",
                application.origin,
            )));
        }
    }
    Ok(())
}
