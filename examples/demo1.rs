use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
    thread,
    time::{Duration, Instant},
};

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
    println!("state waiting...");

    let start = Instant::now();
    TimeFuture::new(Duration::from_secs(2)).await;

    // duration: 2
    println!("duration: {}", start.elapsed().as_secs());
}
