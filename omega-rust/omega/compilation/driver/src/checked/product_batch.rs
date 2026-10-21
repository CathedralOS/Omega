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
use diagnostics::Diagnostic;

impl CompilationSession {
    pub(super) fn select_build_batch(
        &mut self,
        builds: Vec<PreparedPackageBuild>,
    ) -> Vec<SelectedCompileOutcome> {
        let mut outcomes: Vec<_> = (0..builds.len()).map(|_| None).collect();
        let mut groups: Vec<Vec<(usize, PreparedPackageBuild)>> = Vec::new();
        for (position, build) in builds.into_iter().enumerate() {
            artifacts::compile_timings::counters::record_checked_compilation();
            if let Some(group) = groups.iter_mut().find(|group| {
                group
                    .first()
                    .is_some_and(|(_, prior)| prior.same_product_source(&build))
            }) {
                group.push((position, build));
            } else {
                groups.push(vec![(position, build)]);
            }
        }
        for group in groups {
            let completion_timings = if group.iter().any(|(_, build)| build.timings().is_enabled())
            {
                artifacts::compile_timings::CompileTimings::enabled()
            } else {
                Default::default()
            };
            let mut members = group.into_iter();
            let Some((first_position, first)) = members.next() else {
                continue;
            };
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
                    outcomes[first_position] =
                        Some(crate::execution_settlement::select_product_target(
                            &source,
                            Some(target),
                            Some(&inputs),
                            false,
                        ));
                    for (position, member) in members {
                        let (_, target, inputs, _, prepared) = member.into_continuation();
                        let mut application = source.clone();
                        // The program and source custody are common; admission,
                        // accounting and Build receipts belong to this member.
                        application.admission = prepared.built.admission;
                        application.computed_build_config = prepared.built.computed_build_config;
                        application.retained_checks.build = prepared.built.retained_build_check;
                        application.timings = prepared.timings;
                        application.timings.extend(&completion_timings);
                        outcomes[position] =
                            Some(crate::execution_settlement::select_product_target(
                                &application,
                                Some(target),
                                Some(&inputs),
                                false,
                            ));
                    }
                }
                Err(diagnostics) => {
                    outcomes[first_position] = Some(SelectedCompileOutcome {
                        result: Err(CompileFailure::from(diagnostics.clone())),
                        independent_components: discovery,
                    });
                    for (position, member) in members {
                        outcomes[position] = Some(SelectedCompileOutcome {
                            result: Err(CompileFailure::from(diagnostics.clone())),
                            independent_components: member.build().independent_components(),
                        });
                    }
                }
            }
        }
        outcomes
            .into_iter()
            .map(|outcome| {
                let mut outcome = outcome.unwrap_or_else(|| SelectedCompileOutcome {
                    result: Err(
                        vec![Diagnostic::error("source batch lost a requested target")].into(),
                    ),
                    independent_components: Default::default(),
                });
                self.attach_component_products(&mut outcome);
                outcome
            })
            .collect()
    }
}
