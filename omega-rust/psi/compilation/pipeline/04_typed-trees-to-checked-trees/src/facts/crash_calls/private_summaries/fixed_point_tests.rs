//! Compare the production scheduler with the literal pre-dirtiness sweeps.
use super::*;
use checked_trees::{CheckedBooleanExpression, CrashCause, CrashPredicateExpression};

fn original_sweeps(
    equations: &[PrivateSummaryEquation<'_>],
) -> Vec<(SymbolHandle, Vec<SummaryCrashBucket>)> {
    let mut equation_index_by_machine = symbols::SymbolKeyMap::<SymbolHandle, usize>::default();
    let mut resolved = Vec::with_capacity(equations.len());
    for (index, equation) in equations.iter().enumerate() {
        equation_index_by_machine
            .entry(equation.machine)
            .or_insert(index);
        resolved.push((equation.machine, equation.direct.clone()));
    }
    for _ in 0..=equations.len() {
        let mut changed = false;
        for (equation_index, equation) in equations.iter().enumerate() {
            let mut buckets = equation.direct.clone();
            buckets.extend(equation.published_dependencies.clone());
            for dependency in &equation.private_dependencies {
                let &target_index = equation_index_by_machine
                    .get(&dependency.machine)
                    .expect("every private dependency belongs to the viable fixed point");
                let selected = &resolved[target_index].1;
                buckets.extend(selected.iter().map(|bucket| {
                    let bucket = if dependency.recursive {
                        // Substitution around a recursive cycle can create an
                        // unbounded family such as p(n), p(n - 1), ... . The
                        // finite conservative lattice widens exactly those SCC
                        // edges to their cause bucket.
                        SummaryCrashBucket::unconditional(bucket.cause)
                    } else if let Some(substitution) = &dependency.substitution {
                        bucket.substitute(substitution)
                    } else {
                        bucket.clone()
                    };
                    bucket.on_path(&dependency.path_guard)
                }));
            }
            let buckets = normalize_summary_buckets(buckets);
            let current = &mut resolved[equation_index].1;
            if *current != buckets {
                *current = buckets;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    resolved
}

fn guarded(cause: CrashCause, parameter: u32, scalar: Option<bool>) -> SummaryCrashBucket {
    SummaryCrashBucket {
        cause,
        alternative_guards: vec![SummaryCrashRouteGuard::Predicate(SummaryCrashPredicate {
            identity: CrashPredicateExpression::Parameter(parameter),
            builtin_meaning: true,
            scalar: scalar.map(CheckedBooleanExpression::Constant),
        })],
    }
}

fn equation(machine: SymbolHandle) -> PrivateSummaryEquation<'static> {
    PrivateSummaryEquation {
        machine,
        direct: Vec::new(),
        private_dependencies: Vec::new(),
        published_dependencies: Vec::new(),
    }
}

fn dependency(
    machine: SymbolHandle,
    recursive: bool,
    parameter: Option<u32>,
) -> PrivateSummaryDependency<'static> {
    PrivateSummaryDependency {
        machine,
        substitution: None,
        path_guard: parameter.map_or(SummaryCrashRouteGuard::Truth, |parameter| {
            SummaryCrashRouteGuard::Predicate(SummaryCrashPredicate {
                identity: CrashPredicateExpression::Parameter(parameter),
                builtin_meaning: true,
                scalar: None,
            })
        }),
        recursive,
    }
}

fn compare(equations: &[PrivateSummaryEquation<'_>]) {
    let expected = original_sweeps(equations);
    let actual = solve_private_summary_fixed_point(equations);
    // Summary predicate equality intentionally ignores scalar annotations.
    // Debug observes those annotations too, so the scheduler cannot silently
    // change the original first retained annotation or output row order.
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
}

#[test]
fn dirty_equations_match_original_sweeps_for_exact_dependency_graphs() {
    let machines = [
        SymbolHandle::from_parts(7, 1),
        SymbolHandle::from_parts(7, 2),
        SymbolHandle::from_parts(9, 1),
    ];
    // Every directed graph on three exact identities, in both visitation
    // orders: absent/stable edges, earlier/later targets, self edges and SCCs.
    // Every graph also retains a duplicate dependency when it has an edge.
    for edges in 0_u16..(1 << 9) {
        for reverse in [false, true] {
            let mut equations: Vec<_> = machines.into_iter().map(equation).collect();
            equations[0].direct = vec![guarded(CrashCause::Trap, 0, Some(true))];
            equations[1].published_dependencies = vec![guarded(CrashCause::Abort, 1, None)];
            for source in 0..3 {
                for (target, &machine) in machines.iter().enumerate() {
                    if edges & (1 << (source * 3 + target)) != 0 {
                        let recursive = source == target || (source + target) % 2 == 0;
                        let parameter = (source != target).then_some(source as u32 + 2);
                        equations[source]
                            .private_dependencies
                            .push(dependency(machine, recursive, parameter));
                        if equations[source].private_dependencies.len() == 1 {
                            equations[source]
                                .private_dependencies
                                .push(dependency(machine, recursive, parameter));
                        }
                    }
                }
            }
            if reverse {
                equations.reverse();
            }
            compare(&equations);
        }
    }
    // Duplicate equation machine keys select their first row, whereas a
    // same-index/different-generation row remains a separate target. Keep
    // conflicting annotation payloads so equality alone cannot hide a change.
    let mut equations: Vec<_> = [machines[0], machines[1], machines[0], machines[2]]
        .into_iter()
        .map(equation)
        .collect();
    equations[0].direct = vec![guarded(CrashCause::Trap, 0, Some(true))];
    equations[1].published_dependencies = vec![guarded(CrashCause::Abort, 0, Some(false))];
    equations[1]
        .private_dependencies
        .push(dependency(machines[0], false, None));
    equations[2].direct = vec![guarded(CrashCause::Trap, 0, Some(false))];
    equations[2]
        .private_dependencies
        .push(dependency(machines[1], false, None));
    equations[3]
        .private_dependencies
        .push(dependency(machines[0], false, None));
    compare(&equations);
    equations.reverse();
    compare(&equations);
    compare(&[]);
}
