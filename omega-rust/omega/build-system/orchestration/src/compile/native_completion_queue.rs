// Drain native-completion jobs on the invocation's execution budget, at most
// `MAX_NATIVE_JOBS_IN_FLIGHT` (4) at a time.
//
// Native jobs no longer start threads of their own: they run on the workers of
// the `execution_admission::ExecutionBudget` the invocation already holds, so
// parsing and native completion share one bound on active workers and their
// 256 MiB stacks. The in-flight cap stays because each native job holds its
// own lowered program and image while it runs; the budget bounds workers, not
// memory. Zero or one job runs inline on the calling thread. Otherwise the
// caller and up to `MAX_NATIVE_JOBS_IN_FLIGHT - 1` helper tasks drain one
// shared `Mutex<IntoIter>`; each lock guard is dropped before `execute()`, and
// a job that starts parallel work of its own reuses the same workers.

const MAX_NATIVE_JOBS_IN_FLIGHT: usize = 4;

pub(super) fn execute<Work, Output>(jobs: Vec<(usize, Work)>) -> Vec<(usize, Output)>
where
    Work: FnOnce() -> Output + Send,
    Output: Send,
{
    let count = jobs.len();
    let pending = std::sync::Mutex::new(jobs.into_iter());
    if count < 2 {
        return drain(&pending);
    }
    execution_admission::within_budget(|| {
        let helpers = count
            .min(MAX_NATIVE_JOBS_IN_FLIGHT)
            .min(execution_admission::current_workers())
            - 1;
        let helped = std::sync::Mutex::new(Vec::with_capacity(count));
        let mut completed = rayon::scope(|scope| {
            for _ in 0..helpers {
                scope.spawn(|_| {
                    let mut drained = drain(&pending);
                    helped
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .append(&mut drained);
                });
            }
            drain(&pending)
        });
        completed.append(
            &mut helped
                .into_inner()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        completed
    })
}

fn drain<Work: FnOnce() -> Output, Output>(
    pending: &std::sync::Mutex<std::vec::IntoIter<(usize, Work)>>,
) -> Vec<(usize, Output)> {
    let mut completed = Vec::new();
    loop {
        let job = pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .next();
        let Some((position, execute)) = job else {
            break;
        };
        completed.push((position, execute()));
    }
    completed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn every_native_ticket_keeps_its_owner_and_runs_once_after_refusal() {
        let counts = (0..9).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>();
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let jobs = counts
            .iter()
            .enumerate()
            .map(|(position, count)| {
                let active = &active;
                let peak = &peak;
                (position, move || {
                    count.fetch_add(1, Ordering::Relaxed);
                    let running = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(running, Ordering::Relaxed);
                    std::thread::yield_now();
                    active.fetch_sub(1, Ordering::SeqCst);
                    (
                        format!("owner-{position}"),
                        if position == 2 {
                            Err(position)
                        } else {
                            Ok(position)
                        },
                    )
                })
            })
            .collect();
        let budget = execution_admission::ExecutionBudget::with_workers(8).unwrap();
        let mut completed = budget.run(|| execute(jobs));
        completed.sort_unstable_by_key(|(position, _)| *position);
        assert_eq!(completed.len(), counts.len());
        for (expected, (position, (owner, result))) in completed.into_iter().enumerate() {
            assert_eq!(position, expected);
            assert_eq!(owner, format!("owner-{position}"));
            assert_eq!(
                result,
                if position == 2 {
                    Err(position)
                } else {
                    Ok(position)
                }
            );
            assert_eq!(counts[position].load(Ordering::Relaxed), 1);
        }
        assert!(peak.load(Ordering::Relaxed) <= MAX_NATIVE_JOBS_IN_FLIGHT);
        assert_eq!(active.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn native_jobs_never_exceed_the_invocation_budget() {
        let budget = execution_admission::ExecutionBudget::with_workers(2).unwrap();
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let jobs = (0..9)
            .map(|position| {
                let active = &active;
                let peak = &peak;
                (position, move || {
                    let running = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(running, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(2));
                    active.fetch_sub(1, Ordering::SeqCst);
                    std::thread::current().name().map(str::to_owned)
                })
            })
            .collect();
        let completed = budget.run(|| execute(jobs));
        assert_eq!(completed.len(), 9);
        assert!(peak.load(Ordering::SeqCst) <= 2);
        assert!(completed.iter().all(|(_, name)| {
            name.as_deref()
                .is_some_and(|name| name.starts_with("omega-worker-"))
        }));
    }

    #[test]
    fn zero_or_one_native_job_stays_on_the_coordinator() {
        assert!(execute(Vec::<(usize, fn() -> ())>::new()).is_empty());
        let coordinator = std::thread::current().id();
        assert_eq!(
            execute(vec![(7, || std::thread::current().id())]),
            vec![(7, coordinator)]
        );
    }
}
