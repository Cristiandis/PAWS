use std::{
    error::Error,
    fmt, io,
    num::NonZeroUsize,
    panic::{self, AssertUnwindSafe},
    sync::{Arc, Mutex, mpsc},
    thread,
};

type Job = Box<dyn FnOnce() + Send + 'static>;

/// Returned by `execute` when the pool can no longer accept jobs.
#[derive(Debug)]
pub struct ExecuteError;

impl fmt::Display for ExecuteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "thread pool is shut down or has no live workers")
    }
}

impl Error for ExecuteError {}

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

impl ThreadPool {
    /// Creates a pool with `size` worker threads.
    ///
    /// Fails if the OS refuses to spawn a thread.
    pub fn new(size: NonZeroUsize) -> io::Result<ThreadPool> {
        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size.get());
        for id in 0..size.get() {
            workers.push(Worker::new(id, Arc::clone(&receiver))?);
        }

        Ok(ThreadPool {
            workers,
            sender: Some(sender),
        })
    }

    pub fn execute<F>(&self, f: F) -> Result<(), ExecuteError>
    where
        F: FnOnce() + Send + 'static,
    {
        let sender = self.sender.as_ref().ok_or(ExecuteError)?;
        sender.send(Box::new(f)).map_err(|_| ExecuteError)
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());

        for worker in self.workers.drain(..) {
            if worker.thread.join().is_err() {
                eprintln!("worker {} terminated with a panic", worker.id);
            }
        }
    }
}

struct Worker {
    id: usize,
    thread: thread::JoinHandle<()>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> io::Result<Worker> {
        let thread = thread::Builder::new()
            .name(format!("worker-{id}"))
            .spawn(move || {
                loop {
                    let message = receiver
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .recv();

                    match message {
                        Ok(job) => {
                            if panic::catch_unwind(AssertUnwindSafe(job)).is_err() {
                                eprintln!("worker {id}: job panicked, worker keeps running");
                            }
                        }
                        Err(_) => break,
                    }
                }
            })?;

        Ok(Worker { id, thread })
    }
}
