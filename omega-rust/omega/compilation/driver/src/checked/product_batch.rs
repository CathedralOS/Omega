//! Complete source cohorts once, then settle each admitted target separately.
//!
//! Grouping is explicit work partitioning for this batch, not a lookup into a
//! historical cache. Every member has already supplied its own Build consent.
//! Equal source input includes pending evaluation and declaration authority;
//! equality of configuration or a common source path alone is insufficient.

use super::CompilationSession;
use super::checking::{check_product_source, complete_prepared_build};
use crate::execution_settlement::SelectedCompileOutcome;
use compilation_products::{CompileFailure, PreparedPackageBuild};

impl CompilationSession {
    pub(super) fn select_build_batch<Owner>(
        &mut self,
        builds: Vec<(Owner, PreparedPackageBuild)>,
    ) -> Vec<(Owner, SelectedCompileOutcome)> {
        let mut outcomes = Vec::with_capacity(builds.len());
        let mut groups: Vec<Vec<(usize, Owner, PreparedPackageBuild)>> = Vec::new();
        for (position, (owner, build)) in builds.into_iter().enumerate() {
            artifacts::compile_timings::counters::record_checked_compilation();
            if let Some(group) = groups.iter_mut().find(|group| {
                group
                    .first()
                    .is_some_and(|(_, _, prior)| prior.same_product_source(&build))
            }) {
                group.push((position, owner, build));
            } else {
                groups.push(vec![(position, owner, build)]);
            }
        }
        for group in groups {
            let completion_timings = if group
                .iter()
                .any(|(_, _, build)| build.timings().is_enabled())
            {
                artifacts::compile_timings::CompileTimings::enabled()
            } else {
                Default::default()
            };
            let mut members = group.into_iter();
            let Some((first_position, first_owner, first)) = members.next() else {
                continue;
            };
            let permits_unsettled = first.permits_unsettled_fused_service_fields();
            let (_, target, inputs, rollback, mut prepared) = first.into_continuation();
            let first_timings = std::mem::replace(&mut prepared.timings, completion_timings);
            let discovery = prepared
                .built
                .computed_build_config
                .independent_components();
            let checked = complete_prepared_build(prepared, Some(&inputs)).and_then(|source| {
                check_product_source(
                    source,
                    Some(&inputs),
                    &rollback,
                    self.reuse(),
                    &mut self.selections,
                )
            });
            match checked {
                Ok(mut source) => {
                    let completion_timings = std::mem::replace(&mut source.timings, first_timings);
                    source.timings.extend(&completion_timings);
                    self.retain(&source.retained_checks);
                    outcomes.push((
                        first_position,
                        first_owner,
                        crate::execution_settlement::select_product_target(
                            &source,
                            Some(target),
                            Some(&inputs),
                            permits_unsettled,
                        ),
                    ));
                    for (position, owner, member) in members {
                        let permits_unsettled = member.permits_unsettled_fused_service_fields();
                        let (_, target, inputs, _, prepared) = member.into_continuation();
                        let mut application = source.clone();
                        // The program and source custody are common; admission,
                        // accounting and Build receipts belong to this member.
                        application.admission = prepared.built.admission;
                        application.computed_build_config = prepared.built.computed_build_config;
                        application.retained_checks.build = prepared.built.retained_build_check;
                        application.timings = prepared.timings;
                        application.timings.extend(&completion_timings);
                        outcomes.push((
                            position,
                            owner,
                            crate::execution_settlement::select_product_target(
                                &application,
                                Some(target),
                                Some(&inputs),
                                permits_unsettled,
                            ),
                        ));
                    }
                }
                Err(diagnostics) => {
                    outcomes.push((
                        first_position,
                        first_owner,
                        SelectedCompileOutcome {
                            result: Err(CompileFailure::from(diagnostics.clone())),
                            independent_components: discovery,
                        },
                    ));
                    for (position, owner, member) in members {
                        outcomes.push((
                            position,
                            owner,
                            SelectedCompileOutcome {
                                result: Err(CompileFailure::from(diagnostics.clone())),
                                independent_components: member.build().independent_components(),
                            },
                        ));
                    }
                }
            }
        }
        // Cohorts finish together; restore the authored request order before
        // attaching component products. The owner never separates from its result.
        outcomes.sort_unstable_by_key(|(position, _, _)| *position);
        let mut selected = Vec::with_capacity(outcomes.len());
        for (_, owner, mut outcome) in outcomes {
            self.attach_component_products(&mut outcome);
            selected.push((owner, outcome));
        }
        selected
    }
}
