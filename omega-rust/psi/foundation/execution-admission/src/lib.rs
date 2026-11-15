/*
    One worker budget per compiler invocation.

    Parsing and native completion used to choose their own workers: parsing
    kept a process-global Rayon pool sized to every logical core, and native
    completion spawned up to four more threads of its own. Neither knew about
    the other, so an invocation's active workers and 256 MiB stack
    reservations were bounded only per stage. `ExecutionBudget` is the single
    owner of those workers. The CLI creates one and runs the invocation inside
    it; every stage that wants parallelism asks the budget it is already
    running on, so nested work (a native job that parses) reuses the same
    workers through Rayon work stealing instead of starting another pool.

    This crate lives in Psi's foundation because `psi/` cannot import `omega/`
    crates; it knows nothing about targets or stages.
*/

#![forbid(unsafe_code)]

/// Stack size of every budget worker. Recursive syntax, type and proof walks
/// still need deep stacks until they move onto flat tables
/// (`PIPELINE-ARCHITECTURE-ENFORCEMENT`).
pub const WORKER_STACK_SIZE: usize = 256 * 1024 * 1024;

thread_local! {
    static ON_BUDGET_WORKER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The invocation's fixed set of large-stack workers.
pub struct ExecutionBudget {
    pool: rayon::ThreadPool,
}

impl ExecutionBudget {
    /// The host's default budget: `RAYON_NUM_THREADS`, otherwise the logical
    /// CPUs this process may run on.
    pub fn for_host() -> Result<Self, rayon::ThreadPoolBuildError> {
        Self::build(rayon::ThreadPoolBuilder::new())
    }

    /// A budget of exactly `workers` threads (at least one).
    pub fn with_workers(workers: usize) -> Result<Self, rayon::ThreadPoolBuildError> {
        Self::build(rayon::ThreadPoolBuilder::new().num_threads(workers.max(1)))
    }

    fn build(builder: rayon::ThreadPoolBuilder) -> Result<Self, rayon::ThreadPoolBuildError> {
        builder
            .thread_name(|index| format!("omega-worker-{index}"))
            .stack_size(WORKER_STACK_SIZE)
            .start_handler(|_| ON_BUDGET_WORKER.set(true))
            .build()
            .map(|pool| Self { pool })
    }

    pub fn workers(&self) -> usize {
        self.pool.current_num_threads()
    }

    /// Run `operation` on one of this budget's workers; parallel work it
    /// starts shares the same workers.
    pub fn run<Output: Send>(&self, operation: impl FnOnce() -> Output + Send) -> Output {
        self.pool.install(operation)
    }
}

/// Whether the current thread is a worker of some invocation budget.
pub fn on_budget_worker() -> bool {
    ON_BUDGET_WORKER.get()
}

/// Workers in the budget the current thread runs on; one outside a budget.
pub fn current_workers() -> usize {
    if on_budget_worker() {
        rayon::current_num_threads()
    } else {
        1
    }
}

/// Run `operation` where Rayon parallelism lands on an invocation budget.
///
/// On a budget worker this runs inline. Elsewhere it runs on a temporary
/// single-worker budget, so parallel iterators inside never reach Rayon's
/// global pool, whose threads have small stacks and ignore the invocation.
pub fn within_budget<Output: Send>(operation: impl FnOnce() -> Output + Send) -> Output {
    if on_budget_worker() {
        return operation();
    }
    match ExecutionBudget::with_workers(1) {
        Ok(budget) => budget.run(operation),
        Err(error) => panic!("execution budget worker must spawn: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn parallel_work_inside_a_budget_never_exceeds_its_workers() {
        let budget = ExecutionBudget::with_workers(3).unwrap();
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let threads = budget.run(|| {
            assert!(on_budget_worker());
            assert_eq!(current_workers(), 3);
            (0..64)
                .into_par_iter()
                .map(|_| {
                    // Nested parallelism reuses the same workers.
                    within_budget(|| {
                        let running = active.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(running, Ordering::SeqCst);
                        std::thread::yield_now();
                        active.fetch_sub(1, Ordering::SeqCst);
                        std::thread::current().name().map(str::to_owned)
                    })
                })
                .collect::<Vec<_>>()
        });
        assert!(peak.load(Ordering::SeqCst) <= 3);
        assert!(threads.iter().all(|name| {
            name.as_deref()
                .is_some_and(|name| name.starts_with("omega-worker-"))
        }));
    }

    #[test]
    fn work_outside_a_budget_runs_on_one_large_stack_worker() {
        assert!(!on_budget_worker());
        assert_eq!(current_workers(), 1);
        let (on_worker, workers) = within_budget(|| (on_budget_worker(), current_workers()));
        assert!(on_worker);
        assert_eq!(workers, 1);
    }
}
