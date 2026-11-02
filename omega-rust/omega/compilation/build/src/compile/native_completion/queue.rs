//! Drain admitted native products with a bounded number of compiler-stack workers.

const MAX_NATIVE_EXECUTORS: usize = 4;

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
    std::thread::scope(|scope| {
        let mut workers = Vec::with_capacity(count.min(MAX_NATIVE_EXECUTORS) - 1);
        for _ in 1..count.min(MAX_NATIVE_EXECUTORS) {
            match compiler::checked::spawn_compile_worker(scope, || drain(&pending)) {
                Ok(worker) => workers.push(worker),
                // An optional worker failure leaves every job in the queue.
                Err(_) => phase_profile::count("native.scheduling.worker_spawn_fallback", 1),
            }
        }
        phase_profile::count("native.scheduling.executors", (workers.len() + 1) as u64);
        phase_profile::count("native.scheduling.jobs", count as u64);
        let mut completed = drain(&pending);
        for worker in workers {
            completed.extend(
                worker
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
            );
        }
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
        // The guard is dropped before native work or its failure runs.
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
        let mut completed = execute(jobs);
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
        assert!(peak.load(Ordering::Relaxed) <= MAX_NATIVE_EXECUTORS);
        assert_eq!(active.load(Ordering::Relaxed), 0);
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
