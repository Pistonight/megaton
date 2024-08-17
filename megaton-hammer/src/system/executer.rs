use threadpool::ThreadPool;

pub struct Executer {
    pool: ThreadPool,
}

impl Executer {
    pub fn new() -> Self {
        Self {
            pool: ThreadPool::new(num_cpus::get()),
        }
    }

    pub fn execute<TFn, TOut>(&self, f: TFn) -> Task<TOut>
    where
        TFn: FnOnce() -> TOut + Send + 'static,
        TOut: Send + Sync + 'static,
    {
        let (send, recv) = oneshot::channel();
        self.pool.execute(move || {
            send.send(f()).unwrap();
        });
        Task { recv }
    }
}

impl Drop for Executer {
    fn drop(&mut self) {
        self.pool.join();
    }
}

pub struct Task<T> {
    recv: oneshot::Receiver<T>,
}

impl<T> Task<T> {
    pub fn wait(self) -> T {
        self.recv.recv().unwrap()
    }
}
