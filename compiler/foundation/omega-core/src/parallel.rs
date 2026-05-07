use std::collections::VecDeque;
use std::sync::mpsc;
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
            results.push(
                receiver
                    .recv()
                    .expect("worker should send one result per job"),
            );
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
            first_receiver
                .recv()
                .expect("first join job should produce a value"),
            second_receiver
                .recv()
                .expect("second join job should produce a value"),
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
            first_receiver
                .recv()
                .expect("first join job should produce a value"),
            second_receiver
                .recv()
                .expect("second join job should produce a value"),
            third_receiver
                .recv()
                .expect("third join job should produce a value"),
        )
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
