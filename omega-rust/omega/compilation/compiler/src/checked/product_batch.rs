// We partition a batch of `PreparedPackageBuild` items into equal-product-source
// cohorts (`select_build_batch`), check each cohort's source once, adopt
// shared Terminal publications across equal publication inputs, and prepare
// every target's declarations before publishing any shared code. Selection then
// consumes those prepared values sequentially and restores caller order.

use super::CompilationSession;
use super::PreparedProductTarget;
use super::checking::{check_product_source, complete_prepared_build};
use crate::execution_settlement::SelectedCompileOutcome;
use compilation_products::{CompileFailure, PreparedPackageBuild};

impl CompilationSession {
    pub(super) fn select_build_batch<Owner>(
        &mut self,
        builds: Vec<(Owner, PreparedPackageBuild)>,
        mut frontier: Option<&mut Option<super::checking::typed_frontier::TypedFrontier>>,
    ) -> Vec<(Owner, SelectedCompileOutcome)> {
        let mut outcomes = Vec::with_capacity(builds.len());
        let mut applications = Vec::with_capacity(builds.len());
        let mut canonical_sources: Vec<compilation_products::CheckedProductSource> = Vec::new();
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
            let (_, target, inputs, rollback, mut prepared) = first.into_continuation();
            let first_timings = std::mem::replace(&mut prepared.timings, completion_timings);
            let discovery = prepared
                .built
                .computed_build_config
                .independent_components();
            let checked = complete_prepared_build(prepared, Some(&inputs)).and_then(|source| {
                check_product_source(source, Some(&inputs), &rollback, frontier.as_deref_mut())
            });
            match checked {
                Ok(mut source) => {
                    match canonical_sources
                        .iter()
                        .find(|prior| prior.shares_terminal_publication(&source))
                    {
                        Some(canonical) => source.adopt_derived_code(canonical),
                        None => canonical_sources.push(source.clone()),
                    }
                    let completion_timings = std::mem::replace(&mut source.timings, first_timings);
                    source.timings.extend(&completion_timings);
                    applications.push((
                        first_position,
                        first_owner,
                        source.clone(),
                        target,
                        inputs,
                    ));
                    for (position, owner, member) in members {
                        let (_, target, inputs, _, prepared) = member.into_continuation();
                        let mut application = source.clone();
                        application.admission = prepared.built.admission;
                        let observed = match source
                            .computed_build_config
                            .with_execution_observation(&prepared.built.computed_build_config)
                        {
                            Ok(observed) => observed,
                            Err(reason) => {
                                outcomes.push((
                                    position,
                                    owner,
                                    SelectedCompileOutcome {
                                        result: Err(CompileFailure::from(vec![
                                            diagnostics::Diagnostic::error(reason),
                                        ])),
                                        independent_components: prepared
                                            .built
                                            .computed_build_config
                                            .independent_components(),
                                        tests: Vec::new(),
                                        test_notes: Vec::new(),
                                    },
                                ));
                                continue;
                            }
                        };
                        application.computed_build_config = std::sync::Arc::new(observed);
                        application.timings = prepared.timings;
                        application.timings.extend(&completion_timings);
                        applications.push((position, owner, application, target, inputs));
                    }
                }
                Err(diagnostics) => {
                    outcomes.push((
                        first_position,
                        first_owner,
                        SelectedCompileOutcome {
                            result: Err(diagnostics.clone()),
                            independent_components: discovery,
                            tests: Vec::new(),
                            test_notes: Vec::new(),
                        },
                    ));
                    for (position, owner, member) in members {
                        outcomes.push((
                            position,
                            owner,
                            SelectedCompileOutcome {
                                result: Err(diagnostics.clone()),
                                independent_components: member.build().independent_components(),
                                tests: Vec::new(),
                                test_notes: Vec::new(),
                            },
                        ));
                    }
                }
            }
        }
        // Later children must not inspect checked source after an earlier
        // child publishes their shared artifact. Keep test preparation errors
        // deferred until the application succeeds, as test execution requires.
        let prepared_applications = applications
            .into_iter()
            .map(|(position, owner, source, target, inputs)| {
                let prepared = PreparedProductTarget::capture(&source, Some(target), Some(&inputs));
                let tests = super::test_roots::select_test_roots(&source, target, &inputs).map(
                    |selection| {
                        let roots = selection
                            .roots
                            .into_iter()
                            .map(|root| {
                                let prepared = PreparedProductTarget::capture(
                                    &root.source,
                                    Some(target),
                                    Some(&inputs),
                                );
                                (root, prepared)
                            })
                            .collect::<Vec<_>>();
                        (selection.notes, roots)
                    },
                );
                (position, owner, source, prepared, tests)
            })
            .collect::<Vec<_>>();
        for (position, owner, source, prepared, tests) in prepared_applications {
            let mut outcome = crate::execution_settlement::select_product_target(&source, prepared);
            if outcome.result.is_ok() {
                match tests {
                    Ok((notes, roots)) => {
                        outcome.test_notes = notes;
                        for (root, prepared) in roots {
                            let selected = crate::execution_settlement::select_product_target(
                                &root.source,
                                prepared,
                            );
                            outcome.tests.push((root.identity, selected.result));
                        }
                    }
                    Err(diagnostics) => outcome.result = Err(CompileFailure::from(diagnostics)),
                }
            }
            outcomes.push((position, owner, outcome));
        }
        outcomes.sort_unstable_by_key(|(position, _, _)| *position);
        outcomes
            .into_iter()
            .map(|(_, owner, outcome)| (owner, outcome))
            .collect()
    }
}
