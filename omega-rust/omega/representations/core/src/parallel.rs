//! A fixed pool of threads in which a thread waiting on a result keeps
//! working, so a job may use the same pool recursively without deadlocking it.
//!
//! The design turns on that one property. Spawn `worker_count` threads over a
//! shared `VecDeque` behind a `Mutex` and a `Condvar`, write the obvious
//! `map_ordered` — push N jobs, block on a channel for N results — and it
//! deadlocks the first time one of those jobs calls `map_ordered` again: every
//! thread is parked inside `recv()` waiting for work that only a thread could
//! do. `wait_for_result` refuses to block while the queue still has jobs in it.
//! It `try_recv`s, and on `Empty` pops a job and runs it on the calling thread,
//! so a caller becomes a worker for the duration of its own wait. It falls
//! through to a blocking `recv()` only once `take_job` returns `None`, which
//! means the queue is drained and every outstanding result is already in flight
//! on a real worker.
//!
//! `nested_jobs_complete_on_same_pool` is the test that pins this. Delete the
//! `take_job` branch and that test hangs rather than failing, which is worth
//! knowing before you go looking for a stuck build.
//!
//! At `worker_count == 1` every entry point runs its closures inline on the
//! caller instead. Stealing would still make that case correct; running inline
//! skips a boxed closure, a channel, and a lock acquisition per job when there
//! is no parallelism to win.
//!
//! @Robustness: a `WorkerPoolHandle` outlives the `WorkerPool` it came from —
//! it holds an `Arc` of the shared state, and `Drop` only sets `shutdown` and
//! joins the threads. Calling `map_ordered` on such a handle queues jobs no
//! thread will run, then `take_job` returns `None` because of `shutdown`, and
//! the blocking `recv()` waits forever on senders that are sitting in the
//! queued jobs. It hangs; it does not panic. Whether the handle should carry a
//! liveness flag or the pool should refuse to drop while handles exist is open.
//!
//! @Speed: `map_ordered` collects `(index, value)` pairs off one channel and
//! sorts at the end, over results that arrive close to sorted already. Writing
//! into preallocated slots would drop the sort, and with no caller in the
//! workspace there is no measurement to justify the change.

use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

type Job = Box<dyn FnOnce() + Send + 'static>;

pub struct WorkerPool {
    handle: WorkerPoolHandle,
    workers: Vec<JoinHandle<()>>,
}

#[derive(Clone)]
pub struct WorkerPoolHandle {
    shared: Arc<SharedWorkerState>,
    worker_count: usize,
}

struct SharedWorkerState {
    state: Mutex<WorkerState>,
    ready: Condvar,
}

#[derive(Default)]
struct WorkerState {
    jobs: VecDeque<Job>,
    shutdown: bool,
}

impl WorkerPool {
    pub fn new(worker_count: usize) -> Self {
        let worker_count = worker_count.max(1);
        let shared = Arc::new(SharedWorkerState {
            state: Mutex::new(WorkerState::default()),
            ready: Condvar::new(),
        });
        let mut workers = Vec::with_capacity(worker_count);

        for _ in 0..worker_count {
            let shared = Arc::clone(&shared);
            workers.push(thread::spawn(move || run_worker(shared)));
        }

        Self {
            handle: WorkerPoolHandle {
                shared,
                worker_count,
            },
            workers,
        }
    }

    pub fn with_available_parallelism() -> Self {
        let worker_count = thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1);

        Self::new(worker_count)
    }

    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    pub fn handle(&self) -> WorkerPoolHandle {
        self.handle.clone()
    }

    pub fn execute(&self, job: impl FnOnce() + Send + 'static) {
        self.handle.execute(job);
    }

    pub fn map_ordered<T>(
        &self,
        job_count: usize,
        worker: impl Fn(usize) -> T + Send + Sync + 'static,
    ) -> Vec<T>
    where
        T: Send + 'static,
    {
        self.handle.map_ordered(job_count, worker)
    }

    pub fn join2<A, B>(
        &self,
        first: impl FnOnce() -> A + Send + 'static,
        second: impl FnOnce() -> B + Send + 'static,
    ) -> (A, B)
    where
        A: Send + 'static,
        B: Send + 'static,
    {
        self.handle.join2(first, second)
    }

    pub fn join3<A, B, C>(
        &self,
        first: impl FnOnce() -> A + Send + 'static,
        second: impl FnOnce() -> B + Send + 'static,
        third: impl FnOnce() -> C + Send + 'static,
    ) -> (A, B, C)
    where
        A: Send + 'static,
        B: Send + 'static,
        C: Send + 'static,
    {
        self.handle.join3(first, second, third)
    }
}

impl WorkerPoolHandle {
    pub fn worker_count(&self) -> usize {
        self.worker_count
    }

    pub fn execute(&self, job: impl FnOnce() + Send + 'static) {
        let mut state = self.shared.state.lock().expect("worker queue poisoned");
        state.jobs.push_back(Box::new(job));
        self.shared.ready.notify_one();
    }

    pub fn map_ordered<T>(
        &self,
        job_count: usize,
        worker: impl Fn(usize) -> T + Send + Sync + 'static,
    ) -> Vec<T>
    where
        T: Send + 'static,
    {
        if job_count == 0 {
            return Vec::new();
        }

        if self.worker_count == 1 {
            return (0..job_count).map(worker).collect();
        }

        let worker = Arc::new(worker);
        let (sender, receiver) = mpsc::channel();

        for index in 0..job_count {
            let worker = Arc::clone(&worker);
            let sender = sender.clone();
            self.execute(move || {
                sender
                    .send((index, worker(index)))
                    .expect("worker result receiver should stay alive");
            });
        }

        drop(sender);

        let mut results = Vec::with_capacity(job_count);
        for _ in 0..job_count {
            results.push(self.wait_for_result(&receiver));
        }

        results.sort_by_key(|(index, _)| *index);
        results.into_iter().map(|(_, result)| result).collect()
    }

    pub fn join2<A, B>(
        &self,
        first: impl FnOnce() -> A + Send + 'static,
        second: impl FnOnce() -> B + Send + 'static,
    ) -> (A, B)
    where
        A: Send + 'static,
        B: Send + 'static,
    {
        if self.worker_count == 1 {
            return (first(), second());
        }

        let (first_sender, first_receiver) = mpsc::channel();
        let (second_sender, second_receiver) = mpsc::channel();

        self.execute(move || {
            first_sender
                .send(first())
                .expect("join receiver should stay alive");
        });
        self.execute(move || {
            second_sender
                .send(second())
                .expect("join receiver should stay alive");
        });

        (
            self.wait_for_result(&first_receiver),
            self.wait_for_result(&second_receiver),
        )
    }

    pub fn join3<A, B, C>(
        &self,
        first: impl FnOnce() -> A + Send + 'static,
        second: impl FnOnce() -> B + Send + 'static,
        third: impl FnOnce() -> C + Send + 'static,
    ) -> (A, B, C)
    where
        A: Send + 'static,
        B: Send + 'static,
        C: Send + 'static,
    {
        if self.worker_count == 1 {
            return (first(), second(), third());
        }

        let (first_sender, first_receiver) = mpsc::channel();
        let (second_sender, second_receiver) = mpsc::channel();
        let (third_sender, third_receiver) = mpsc::channel();

        self.execute(move || {
            first_sender
                .send(first())
                .expect("join receiver should stay alive");
        });
        self.execute(move || {
            second_sender
                .send(second())
                .expect("join receiver should stay alive");
        });
        self.execute(move || {
            third_sender
                .send(third())
                .expect("join receiver should stay alive");
        });

        (
            self.wait_for_result(&first_receiver),
            self.wait_for_result(&second_receiver),
            self.wait_for_result(&third_receiver),
        )
    }

    fn wait_for_result<T>(&self, receiver: &Receiver<T>) -> T {
        loop {
            match receiver.try_recv() {
                Ok(result) => return result,
                Err(TryRecvError::Disconnected) => {
                    panic!("worker result sender dropped before producing a value")
                }
                Err(TryRecvError::Empty) => {}
            }

            // Work while we wait. A caller parked here holds one of the
            // pool's threads, so if the queue still has jobs it runs one
            // itself rather than waiting for a thread that may be parked in
            // exactly this loop.
            if let Some(job) = self.take_job() {
                job();
            } else {
                return receiver
                    .recv()
                    .expect("worker should produce a value before disconnecting");
            }
        }
    }

    fn take_job(&self) -> Option<Job> {
        let mut state = self.shared.state.lock().expect("worker queue poisoned");

        if state.shutdown {
            return None;
        }

        state.jobs.pop_front()
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        {
            let mut state = self
                .handle
                .shared
                .state
                .lock()
                .expect("worker queue poisoned");
            state.shutdown = true;
        }

        self.handle.shared.ready.notify_all();

        while let Some(worker) = self.workers.pop() {
            worker.join().expect("worker should shut down cleanly");
        }
    }
}

fn run_worker(shared: Arc<SharedWorkerState>) {
    loop {
        let job = {
            let mut state = shared.state.lock().expect("worker queue poisoned");

            loop {
                if state.shutdown {
                    return;
                }

                if let Some(job) = state.jobs.pop_front() {
                    break job;
                }

                state = shared.ready.wait(state).expect("worker queue poisoned");
            }
        };

        job();
    }
}

#[cfg(test)]
mod tests {
    use super::WorkerPool;

    #[test]
    fn map_ordered_preserves_job_order() {
        let workers = WorkerPool::new(4);

        let results = workers.map_ordered(8, |index| 7 - index);

        assert_eq!(results, vec![7, 6, 5, 4, 3, 2, 1, 0]);
    }

    #[test]
    fn nested_jobs_complete_on_same_pool() {
        let workers = WorkerPool::new(2);
        let handle = workers.handle();

        let (nested_sum, direct_value) = workers.join2(
            move || {
                handle
                    .map_ordered(4, |index| index + 1)
                    .into_iter()
                    .sum::<usize>()
            },
            || 32usize,
        );

        assert_eq!(nested_sum, 10);
        assert_eq!(direct_value, 32);
    }
}
