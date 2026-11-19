//! Making the picture of the drawing on a thread of its own, so that a pan or
//! zoom never waits for it: the old picture is shown moved (see
//! `layer_cache/moved.rs`) until the new one comes back.
//!
//! The thread has a `View` of its own, which keeps its hatch and block
//! reference caches warm between pictures, and paints on an egui context of its
//! own, because a context's shapes are taken by the frame that ends next. Its
//! triangles are made as the screen's context would make them (`Tess`), the
//! pictures of small text are loaded into the screen's context, and so is the
//! image, which is all that comes back. A picture that would need egui's fonts
//! (a hatch's "not drawn" note, whose glyphs are in the other context's atlas)
//! cannot be made there, and comes back empty so that it is made on the screen.
//!
//! Only the newest picture asked for each slot is made: an older one not yet
//! started is dropped. One being made is finished, not stopped, so that a pan
//! that never stands still still gets new pictures as fast as they are made.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{mpsc, Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use egui::{Context, Painter, RawInput, Rect};
use opendraft_core::{Document, EntityId, LayerId, LayoutId, PointStyle};

use super::layer_paint::Frame;
use super::{hatch_paint, text_raster, CanvasStyle, View};
use crate::camera::Camera;
use crate::layer_cache::{chunk_count, make_image, Bitmap, Geometry, LayerKey, Slot, Tess};

/// A picture to make, with everything painting reads.
pub(super) struct Job {
    pub slot: Slot,
    pub key: LayerKey,
    pub document: Arc<Document>,
    pub token: (u64, u64),
    pub camera: Camera,
    pub hidden: Option<EntityId>,
    pub point_style: PointStyle,
    pub style: CanvasStyle,
    pub rect: Rect,
    pub canvas: Rect,
    pub clip: Rect,
    pub space: Option<LayoutId>,
    pub frozen: Vec<LayerId>,
    pub geometry: Geometry,
    pub name: String,
    pub tess: Tess,
}

/// A picture made, or `None` when it could not be made on the thread.
pub(super) struct Built {
    pub slot: Slot,
    pub key: LayerKey,
    pub image: Option<Bitmap>,
    pub took: Duration,
}

#[derive(Default)]
struct Queue {
    /// The newest job of each slot, oldest slot first.
    jobs: Vec<Job>,
    closed: bool,
}

type Shared = Arc<(Mutex<Queue>, Condvar)>;

/// The thread, and what has been asked of it and has come back.
pub(super) struct Builder {
    queue: Shared,
    built: mpsc::Receiver<Built>,
    /// Whether the thread was started.
    alive: bool,
    /// What each slot asked for that has not come back.
    asked: Vec<(Slot, LayerKey)>,
    /// What came back, waiting for its slot to be painted.
    ready: Vec<Built>,
}

impl Builder {
    /// Start the thread, loading what it makes into `screen`.
    pub fn new(screen: Context) -> Self {
        let queue: Shared = Arc::default();
        let (send, built) = mpsc::channel();
        let theirs = Arc::clone(&queue);
        let alive = std::thread::Builder::new()
            .name("opendraft-picture".into())
            .spawn(move || work(&theirs, &send, &screen))
            .is_ok();
        Self {
            queue,
            built,
            alive,
            asked: Vec::new(),
            ready: Vec::new(),
        }
    }

    /// Whether pictures can be made on the thread.
    pub fn alive(&self) -> bool {
        self.alive
    }

    /// Whether the picture `key` of `slot` is asked for and not back.
    pub fn asked_for(&self, slot: Slot, key: &LayerKey) -> bool {
        self.asked.iter().any(|(s, k)| *s == slot && k == key)
    }

    /// Ask for a picture, in place of any of its slot not yet started.
    pub fn ask(&mut self, job: Job) {
        self.asked.retain(|(s, _)| *s != job.slot);
        self.asked.push((job.slot, job.key.clone()));
        let (lock, wake) = &*self.queue;
        let mut queue = lock.lock().unwrap_or_else(PoisonError::into_inner);
        match queue.jobs.iter_mut().find(|j| j.slot == job.slot) {
            Some(waiting) => *waiting = job,
            None => queue.jobs.push(job),
        }
        wake.notify_one();
    }

    /// The newest picture made for `slot` since this was last asked.
    pub fn take(&mut self, slot: Slot) -> Option<Built> {
        while let Ok(built) = self.built.try_recv() {
            self.asked
                .retain(|(s, k)| !(*s == built.slot && *k == built.key));
            self.ready.retain(|r| r.slot != built.slot);
            self.ready.push(built);
        }
        let at = self.ready.iter().position(|r| r.slot == slot)?;
        Some(self.ready.swap_remove(at))
    }
}

impl Drop for Builder {
    fn drop(&mut self) {
        let (lock, wake) = &*self.queue;
        lock.lock().unwrap_or_else(PoisonError::into_inner).closed = true;
        wake.notify_all();
    }
}

/// The thread: make each job, newest per slot, until the builder is dropped.
fn work(queue: &Shared, send: &mpsc::Sender<Built>, screen: &Context) {
    text_raster::load_into(screen.clone());
    let scratch = Context::default();
    // A context has no fonts until it has run once.
    let _ = scratch.run(RawInput::default(), |_| {});
    let mut view = View::default();
    while let Some(job) = next(queue) {
        let started = Instant::now();
        let (slot, key) = (job.slot, job.key.clone());
        // A picture that cannot be made here is made on the screen instead.
        let made = catch_unwind(AssertUnwindSafe(|| view.make_image(&scratch, screen, job)));
        let built = Built {
            slot,
            key,
            image: made.ok().flatten(),
            took: started.elapsed(),
        };
        if send.send(built).is_err() {
            return;
        }
        screen.request_repaint();
    }
}

/// The next job, waiting for one; `None` once the builder is gone.
fn next(queue: &Shared) -> Option<Job> {
    let (lock, wake) = &**queue;
    let mut queue = lock.lock().unwrap_or_else(PoisonError::into_inner);
    loop {
        if queue.closed {
            return None;
        }
        if !queue.jobs.is_empty() {
            return Some(queue.jobs.remove(0));
        }
        queue = wake.wait(queue).unwrap_or_else(PoisonError::into_inner);
    }
}

impl View {
    /// The picture `job` asks for, painted on `scratch` and loaded into
    /// `screen`; `None` if it needed egui's fonts.
    fn make_image(&mut self, scratch: &Context, screen: &Context, job: Job) -> Option<Bitmap> {
        self.camera = job.camera;
        self.hidden = job.hidden;
        self.point_style = job.point_style;
        self.document_token = Some(job.token);
        let layer = egui::LayerId::background();
        let painter = Painter::new(scratch.clone(), layer, job.clip);
        let frame = Frame {
            painter: &painter,
            rect: job.rect,
            canvas: job.canvas,
            clip: job.clip,
            style: &job.style,
            document: &job.document,
            slot: job.slot,
            space: job.space,
            frozen: &job.frozen,
            visible: job.camera.visible_world(job.rect),
            pass: 0,
            tess: &job.tess,
        };
        hatch_paint::took_font();
        let total = chunk_count(job.document.objects().len());
        let pieces: Vec<_> = (0..total)
            .map(|n| self.make_chunk(&frame, n, &[]))
            .collect();
        if hatch_paint::took_font() {
            return None;
        }
        Some(make_image(screen, &job.name, &pieces, job.geometry))
    }
}
