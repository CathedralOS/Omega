//! Inference has settled before this batch starts. Each job borrows the same
//! immutable module and exclusively owns one machine's checking tables; proof
//! contexts are constructed only by running jobs, on the invocation's workers.

use rayon::prelude::*;
use semantic_vocabulary::{IdTable, MachineId};
use std::sync::atomic::{AtomicU64, Ordering};
use terminal_check_tables::{Checked, Construct, MachineTables, RefusalReason};

pub(super) fn check<T: Send>(
    tables: &mut IdTable<MachineId, MachineTables>,
    activity: &mut crate::ProofSearchActivity,
    check: impl Fn(MachineId, &mut MachineTables, &mut crate::ProofSearchActivity) -> Checked<T> + Sync,
) -> Checked<IdTable<MachineId, T>> {
    let mut result = IdTable::default();
    if execution_admission::current_workers() == 1 || tables.len() < 2 {
        for (id, tables) in tables.iter_mut() {
            result.push(check(id, tables, activity)?);
        }
        return Ok(result);
    }

    let first_failure = AtomicU64::new(u64::MAX);
    // Queue references, not prepared contexts or copies of body payloads.
    // Individual jobs remain stealable even when a few bodies dominate cost.
    let jobs: Vec<_> = tables.iter_mut().collect();
    let mut checked: Vec<_> = jobs
        .into_par_iter()
        .with_max_len(1)
        .map(|(id, tables)| {
            let mut activity = crate::ProofSearchActivity::default();
            if id.get() > first_failure.load(Ordering::Relaxed) {
                return (id, Ok(None), activity);
            }
            let verdict = check(id, tables, &mut activity);
            if verdict.is_err() {
                first_failure.fetch_min(id.get(), Ordering::Relaxed);
            }
            (id, verdict.map(Some), activity)
        })
        .collect();
    // Count every completed job before selecting the first refusal, including
    // searches in later jobs that finished before cancellation reached them.
    for (_, _, searched) in &mut checked {
        activity.merge(std::mem::take(searched));
    }
    // Indexed collection retains identity order. Do not collect a parallel
    // Result: its first completed error could hide a lower failing identity.
    for (id, checked, _) in checked {
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
                let mut activity = crate::ProofSearchActivity::default();
                let context = semantic_vocabulary::PropositionContext::default();
                let completed = AtomicUsize::new(0);
                let result = check(&mut tables, &mut activity, |id, tables, activity| {
                    assert!(execution_admission::on_budget_worker());
                    let _ = crate::prove::Prover::new(&context, &[], &[], &tables.values, activity);
                    completed.fetch_add(1, Ordering::SeqCst);
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
                assert_eq!(activity.started(), completed.load(Ordering::SeqCst));
            });
        });
        assert_eq!(active.load(Ordering::SeqCst), 0);
        assert!(peak.load(Ordering::SeqCst) <= budget.workers());
    }
}
