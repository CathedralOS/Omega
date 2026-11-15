//! Inference has settled before this batch starts. Each job borrows the same
//! immutable module and exclusively owns one machine's checking tables; proof
//! contexts are constructed only by running jobs, on the invocation's workers.

use rayon::prelude::*;
use semantic_vocabulary::{IdTable, MachineId};
use std::sync::atomic::{AtomicU64, Ordering};
use terminal_check_tables::{Checked, Construct, MachineTables, RefusalReason};

pub(super) fn check<T: Send>(
    tables: &mut IdTable<MachineId, MachineTables>,
    check: impl Fn(MachineId, &mut MachineTables) -> Checked<T> + Sync,
) -> Checked<IdTable<MachineId, T>> {
    let mut result = IdTable::default();
    if execution_admission::current_workers() == 1 || tables.len() < 2 {
        for (id, tables) in tables.iter_mut() {
            result.push(check(id, tables)?);
        }
        return Ok(result);
    }

    let first_failure = AtomicU64::new(u64::MAX);
    // Queue references, not prepared contexts or copies of body payloads.
    // Individual jobs remain stealable even when a few bodies dominate cost.
    let jobs: Vec<_> = tables.iter_mut().collect();
    let checked: Vec<_> = jobs
        .into_par_iter()
        .with_max_len(1)
        .map(|(id, tables)| {
            if id.get() > first_failure.load(Ordering::Relaxed) {
                return (id, Ok(None));
            }
            let verdict = check(id, tables);
            if verdict.is_err() {
                first_failure.fetch_min(id.get(), Ordering::Relaxed);
            }
            (id, verdict.map(Some))
        })
        .collect();
    // Indexed collection retains identity order. Do not collect a parallel
    // Result: its first completed error could hide a lower failing identity.
    for (id, checked) in checked {
        let checked = checked?.ok_or_else(|| {
            // A skipped row must follow the error that canceled it. Never
            // expose an incomplete batch as a successful checking product.
            crate::refusal(
                RefusalReason::Unsupported,
                id,
                Construct::Machine,
                "machine checking was canceled without an earlier refusal",
            )
        })?;
        result.push(checked);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use execution_admission::ExecutionBudget;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn nested_batches_share_workers_and_keep_the_lowest_refusal() {
        let budget = ExecutionBudget::with_workers(4).unwrap();
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        budget.run(|| {
            // Nested compilation uses this same budget, without a new pool.
            (0..4).into_par_iter().for_each(|_| {
                let mut tables = IdTable::default();
                for _ in 0..64 {
                    tables.push(MachineTables::default());
                }
                let result = check(&mut tables, |id, _| {
                    assert!(execution_admission::on_budget_worker());
                    let running = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(running, Ordering::SeqCst);
                    // Deliberately let later identities make progress first.
                    for _ in 0..65 - id.get() {
                        std::thread::yield_now();
                    }
                    active.fetch_sub(1, Ordering::SeqCst);
                    if matches!(id.get(), 3 | 8 | 40) {
                        Err(crate::refusal(
                            RefusalReason::Unsupported,
                            id,
                            Construct::Machine,
                            "refused",
                        ))
                    } else {
                        Ok(id)
                    }
                });
                assert_eq!(result.unwrap_err().machine.get(), 3);
            });
        });
        assert_eq!(active.load(Ordering::SeqCst), 0);
        assert!(peak.load(Ordering::SeqCst) <= budget.workers());
    }
}
