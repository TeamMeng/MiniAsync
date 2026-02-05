use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    thread,
    time::Duration,
};

// 任务队列
type Task = Pin<Box<dyn Future<Output = ()> + Send>>;

struct Executor {
    tasks: Arc<Mutex<VecDeque<Task>>>,
}

impl Executor {
    fn new() -> Self {
        Executor {
            tasks: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    // 提交任务
    fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        let task = Box::pin(future);
        self.tasks.lock().unwrap().push_back(task);
    }

    fn create_waker(&self) -> Waker {
        fn raw_wake(_: *const ()) {}
        fn raw_wake_by_ref(_: *const ()) {}
        fn raw_drop(_: *const ()) {}

        static VTABLE: RawWakerVTable =
            RawWakerVTable::new(raw_clone, raw_wake, raw_wake_by_ref, raw_drop);

        fn raw_clone(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VTABLE)
        }

        let raw_waker = RawWaker::new(std::ptr::null(), &VTABLE);
        unsafe { Waker::from_raw(raw_waker) }
    }

    // 运行所有任务
    fn run(&self) {
        loop {
            // 取出一个任务
            let mut task = match self.tasks.lock().unwrap().pop_front() {
                Some(task) => task,
                None => break,
            };

            // 创建 Waker
            let waker = self.create_waker();
            let mut context = Context::from_waker(&waker);

            match task.as_mut().poll(&mut context) {
                Poll::Ready(()) => {}
                Poll::Pending => {
                    self.tasks.lock().unwrap().push_back(task);
                }
            }
        }
    }
}

// 共享状态，用于在线程间传递信息
struct SharedState {
    completed: bool,      // 是否完成
    waker: Option<Waker>, // 保存的 Waker
}

pub struct TimeFuture {
    shared_state: Arc<Mutex<SharedState>>,
}

impl TimeFuture {
    pub fn new(duration: Duration) -> Self {
        let shared_state = Arc::new(Mutex::new(SharedState {
            completed: false,
            waker: None,
        }));

        // 启动后台线程
        let thread_shared_state = shared_state.clone();
        thread::spawn(move || {
            thread::sleep(duration);

            // 标记完成
            let mut state = thread_shared_state.lock().unwrap();
            state.completed = true;

            // 如果有 Waker, 调用它
            if let Some(waker) = state.waker.take() {
                waker.wake();
            }
        });

        TimeFuture { shared_state }
    }
}

impl Future for TimeFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut shared_state = self.shared_state.lock().unwrap();

        if shared_state.completed {
            Poll::Ready(())
        } else {
            shared_state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

#[tokio::main]
async fn main() {
    let executor = Executor::new();

    executor.spawn(async {
        println!("task 1 start");
        TimeFuture::new(Duration::from_secs(1)).await;
        println!("task 1 finish");
    });

    executor.spawn(async {
        println!("task 2 start");
        TimeFuture::new(Duration::from_secs(2)).await;
        println!("task 2 finish");
    });

    executor.run();
}
