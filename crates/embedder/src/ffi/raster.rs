//! The engine's raster thread, run by the embedder (`render_task_runner`).
//!
//! With its own raster thread the engine presents frames that the embedder cannot place in
//! time. Here every raster task goes through [`FrameClock`]: it is numbered when the engine
//! posts it, and the thread knows which task it is running when a frame is presented. The
//! platform tasks carry the number of raster tasks posted before them: a message the Dart code
//! sends after a frame (the glass regions measured on it) names the raster task that presents
//! that frame, so the regions are committed together with it instead of a frame early or late.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread::{JoinHandle, ThreadId};
use std::time::Duration;

use super::bindings::{FlutterEngine, FlutterEngineResult, FlutterTask};
use super::{RunTaskFn, TimeFn};

type Hook = Box<dyn Fn(u64) + Send + Sync>;
/// Called with the view and the raster task of a frame about to be presented.
type PresentHook = Box<dyn Fn(i64, u64) + Send + Sync>;

/// The longest a frame waits, before its present, for the platform tasks already posted (the
/// glass regions Dart sent after drawing it, still to be taken in by the main thread).
const PLATFORM_WAIT: Duration = Duration::from_millis(2);

/// Numbers the raster tasks and tells the embedder about frames.
pub struct FrameClock {
    /// Raster tasks posted so far.
    posted: AtomicU64,
    /// The number (from 1) of the raster task running, or of the last one run.
    running: AtomicU64,
    /// Called on the raster thread right before a frame of task `n` is presented (with GL and
    /// Vulkan the driver commits the surface during the present).
    before_present: PresentHook,
    /// Called on the raster thread after task `n` has run.
    after_task: Hook,
    /// Platform tasks due at once (not delayed) posted and run so far.
    platform: Mutex<(u64, u64)>,
    platform_ran: Condvar,
    /// The engine's clock, once the engine is loaded.
    time: OnceLock<TimeFn>,
}

impl FrameClock {
    pub fn new(
        before_present: impl Fn(i64, u64) + Send + Sync + 'static,
        after_task: impl Fn(u64) + Send + Sync + 'static,
    ) -> Self {
        Self {
            posted: AtomicU64::new(0),
            running: AtomicU64::new(0),
            before_present: Box::new(before_present),
            after_task: Box::new(after_task),
            platform: Mutex::new((0, 0)),
            platform_ran: Condvar::new(),
            time: OnceLock::new(),
        }
    }

    pub(super) fn set_time(&self, time: TimeFn) {
        let _ = self.time.set(time);
    }

    /// A platform task was posted (from any thread) for `target_nanos`. True if it is due at
    /// once: only those are waited for before a present (a delayed one runs later anyway).
    pub(super) fn platform_posted(&self, target_nanos: u64) -> bool {
        let Some(time) = self.time.get() else {
            return false;
        };
        // SAFETY: function without arguments or preconditions.
        let now = unsafe { time() };
        if target_nanos > now {
            return false;
        }
        if let Ok(mut platform) = self.platform.lock() {
            platform.0 += 1;
        }
        true
    }

    /// A platform task has run (on the main thread).
    pub(super) fn platform_done(&self) {
        if let Ok(mut platform) = self.platform.lock() {
            platform.1 += 1;
        }
        self.platform_ran.notify_all();
    }

    /// Raster tasks posted so far: the stamp of a platform task posted now.
    pub fn posted(&self) -> u64 {
        self.posted.load(Ordering::Acquire)
    }

    /// The raster task running (or the last one run).
    pub fn running(&self) -> u64 {
        self.running.load(Ordering::Acquire)
    }

    /// Right before the present of the running task's frame. The glass regions Dart sent
    /// after drawing this frame are usually posted already, but the main thread may not have
    /// taken them in yet: the present waits (at most [`PLATFORM_WAIT`]) until the platform
    /// tasks posted so far have run, so the frame's commit carries them.
    pub(super) fn before_present(&self, view_id: i64) {
        if let Ok(platform) = self.platform.lock() {
            let posted = platform.0;
            let _ = self
                .platform_ran
                .wait_timeout_while(platform, PLATFORM_WAIT, |p| p.1 < posted);
        }
        (self.before_present)(view_id, self.running());
    }
}

/// A raster task waiting for its time.
struct Queued {
    target_nanos: u64,
    /// Its number, from 1, in posting order.
    seq: u64,
    task: FlutterTask,
}

impl PartialEq for Queued {
    fn eq(&self, other: &Self) -> bool {
        (self.target_nanos, self.seq) == (other.target_nanos, other.seq)
    }
}

impl Eq for Queued {}

impl PartialOrd for Queued {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Queued {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.target_nanos, self.seq).cmp(&(other.target_nanos, other.seq))
    }
}

/// The engine handle, once `FlutterEngineInitialize` has returned it.
#[derive(Clone, Copy)]
struct Handle(FlutterEngine);

// SAFETY: the engine handle is an opaque pointer the embedder API lets any thread pass to
// FlutterEngineRunTask for the tasks of the runner that thread serves.
unsafe impl Send for Handle {}

struct Queue {
    tasks: BinaryHeap<Reverse<Queued>>,
    engine: Option<Handle>,
    /// The engine is shutting down from this time: the delayed tasks posted before are dropped
    /// (the engine is destroyed when the shutdown returns, they must never run after it).
    closing: Option<(u64, u64)>,
    stop: bool,
}

// SAFETY: FlutterTask is an opaque handle (runner pointer + id) that the engine posts from any
// thread so that it runs on the runner's thread.
unsafe impl Send for Queue {}

/// What the engine's callbacks and the raster thread share.
pub(super) struct Shared {
    queue: Mutex<Queue>,
    wake: Condvar,
    thread: OnceLock<ThreadId>,
    pub(super) clock: Arc<FrameClock>,
    run_task: RunTaskFn,
    time: TimeFn,
}

/// The raster thread. Stopped (and joined) when dropped, after the engine is shut down.
pub(super) struct RasterRunner {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl RasterRunner {
    pub(super) fn start(
        clock: Arc<FrameClock>,
        run_task: RunTaskFn,
        time: TimeFn,
    ) -> Result<Self, super::Error> {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue {
                tasks: BinaryHeap::new(),
                engine: None,
                closing: None,
                stop: false,
            }),
            wake: Condvar::new(),
            thread: OnceLock::new(),
            clock,
            run_task,
            time,
        });
        let thread_shared = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("raster".to_owned())
            .spawn(move || run(&thread_shared))
            .map_err(|err| super::Error::Load(format!("raster thread: {err}")))?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    /// The `user_data` of the render task runner description.
    pub(super) fn user_data(&self) -> *mut c_void {
        Arc::as_ptr(&self.shared).cast_mut().cast::<c_void>()
    }

    /// The tasks can run from now on.
    pub(super) fn set_engine(&self, engine: FlutterEngine) {
        if let Ok(mut queue) = self.shared.queue.lock() {
            queue.engine = Some(Handle(engine));
        }
        self.shared.wake.notify_all();
    }

    /// The engine is about to shut down (see [`Queue::closing`]).
    pub(super) fn closing(&self) {
        // SAFETY: function without arguments or preconditions.
        let now = unsafe { (self.shared.time)() };
        if let Ok(mut queue) = self.shared.queue.lock() {
            let posted = self.shared.clock.posted();
            queue.closing = Some((now, posted));
        }
    }
}

impl Drop for RasterRunner {
    fn drop(&mut self) {
        if let Ok(mut queue) = self.shared.queue.lock() {
            queue.stop = true;
            queue.engine = None;
        }
        self.shared.wake.notify_all();
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::warn!("the raster thread ended with a panic");
        }
    }
}

/// The next task to run, waiting for it; `None` when the runner stops.
fn next(shared: &Shared) -> Option<(Handle, Queued)> {
    let mut queue = shared.queue.lock().ok()?;
    loop {
        if queue.stop {
            return None;
        }
        let Some(engine) = queue.engine else {
            queue = shared.wake.wait(queue).ok()?;
            continue;
        };
        let Some(Reverse(first)) = queue.tasks.peek() else {
            queue = shared.wake.wait(queue).ok()?;
            continue;
        };
        // SAFETY: function without arguments or preconditions.
        let now = unsafe { (shared.time)() };
        if first.target_nanos > now {
            let wait = Duration::from_nanos(first.target_nanos - now);
            queue = shared.wake.wait_timeout(queue, wait).ok()?.0;
            continue;
        }
        let Reverse(task) = queue.tasks.pop()?;
        if let Some((since, posted)) = queue.closing
            && task.seq <= posted
            && task.target_nanos > since
        {
            continue;
        }
        return Some((engine, task));
    }
}

fn run(shared: &Shared) {
    let _ = shared.thread.set(std::thread::current().id());
    while let Some((engine, queued)) = next(shared) {
        shared.clock.running.store(queued.seq, Ordering::Release);
        // SAFETY: the engine is alive (the runner stops before the engine is destroyed, see
        // `closing`); the task comes from this engine's post_task and runs only once, at or
        // after its target time, on the thread of its runner.
        let result = unsafe { (shared.run_task)(engine.0, &queued.task) };
        if result != FlutterEngineResult::kSuccess {
            tracing::warn!("raster task {} failed: {}", queued.seq, result.0);
        }
        (shared.clock.after_task)(queued.seq);
    }
}

/// The `Shared` of a render task runner description.
///
/// # Safety
/// `user_data` must be [`RasterRunner::user_data`] of a runner still alive.
unsafe fn shared<'a>(user_data: *mut c_void) -> &'a Shared {
    // SAFETY: guaranteed by the caller; Engine keeps the runner (and its Arc) until after
    // FlutterEngineShutdown, after which the engine no longer calls the runner.
    unsafe { &*user_data.cast_const().cast::<Shared>() }
}

pub(super) unsafe extern "C" fn post_task(
    task: FlutterTask,
    target_nanos: u64,
    user_data: *mut c_void,
) {
    // SAFETY: user_data is the one of the render runner description (see `shared`).
    let shared = unsafe { shared(user_data) };
    let seq = shared.clock.posted.fetch_add(1, Ordering::AcqRel) + 1;
    if let Ok(mut queue) = shared.queue.lock() {
        queue.tasks.push(Reverse(Queued {
            target_nanos,
            seq,
            task,
        }));
    }
    shared.wake.notify_all();
}

pub(super) unsafe extern "C" fn runs_on_current_thread(user_data: *mut c_void) -> bool {
    // SAFETY: user_data is the one of the render runner description (see `shared`).
    let shared = unsafe { shared(user_data) };
    shared.thread.get() == Some(&std::thread::current().id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tasks_run_by_time_then_by_posting_order() {
        let task = FlutterTask {
            runner: std::ptr::null_mut(),
            task: 0,
        };
        let mut heap = BinaryHeap::new();
        for (target_nanos, seq) in [(20, 1), (10, 2), (10, 3)] {
            heap.push(Reverse(Queued {
                target_nanos,
                seq,
                task,
            }));
        }
        let order: Vec<u64> = std::iter::from_fn(|| heap.pop().map(|Reverse(q)| q.seq)).collect();
        assert_eq!(order, [2, 3, 1]);
    }
}
