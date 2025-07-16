use std::{
    cmp::{max, min},
    sync::{
        atomic::AtomicBool,
        mpsc::{Receiver, RecvError, RecvTimeoutError, Sender, TryRecvError},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use osm_tag_compression::compressed_data::UncompressedOsmData;
use tree::bbox::{BoundingBox, EARTH_BBOX};
use winit::dpi::PhysicalSize;

pub(super) enum Message {
    Zoom(f64, Option<(f64, f64)>),
    Pan(f64, f64),
    Resize(u32, u32),
}

pub struct GeometryLoader {
    objects: Arc<Mutex<Vec<(BoundingBox<i32>, UncompressedOsmData)>>>,
    render_sender: Sender<Message>,
    has_updates: Arc<AtomicBool>,
}

impl GeometryLoader {
    pub fn new(
        geography: tree::StoredTree<2, 8000, BoundingBox<i32>, UncompressedOsmData>,
    ) -> Self {
        let bbox = geography.root_bbox().to_owned().into();
        let has_updates = Arc::new(false.into());

        let objects = Default::default();

        let render_sender = start_object_loading(
            bbox,
            PhysicalSize {
                width: 1,
                height: 1,
            },
            geography,
            Arc::clone(&objects),
            Arc::clone(&has_updates),
        );

        Self {
            objects,
            render_sender,
            has_updates,
        }
    }
    pub fn objects(&self) -> Arc<Mutex<Vec<(BoundingBox<i32>, UncompressedOsmData)>>> {
        Arc::clone(&self.objects)
    }

    pub fn resize_window(&self, w: u32, h: u32) {
        self.render_sender.send(Message::Resize(w, h)).unwrap();
    }

    pub fn pan_relative(&self, x: f64, y: f64) {
        self.render_sender.send(Message::Pan(x, y)).unwrap();
    }
    pub fn zoom_relative(&self, zoom_factor: f64, center: Option<(f64, f64)>) {
        self.render_sender
            .send(Message::Zoom(zoom_factor, center))
            .unwrap();
    }

    pub fn is_updated(&self) -> bool {
        self.has_updates
            .fetch_and(false, std::sync::atomic::Ordering::Relaxed)
    }
}

fn debounce<T: Send + 'static>(delay: Duration) -> (Sender<T>, Receiver<T>) {
    let (itx, irx) = std::sync::mpsc::channel();
    let (otx, orx) = std::sync::mpsc::channel();

    let mut last_send = Instant::now() - delay;
    let mut end_time = Instant::now() + delay;
    let mut send: Option<T> = None;

    std::thread::spawn(move || loop {
        let end_time_elapsed = Instant::now() >= end_time;
        let time_since_last_send_elapsed = (Instant::now() - delay) < last_send;

        if end_time_elapsed || time_since_last_send_elapsed {
            if let Some(send) = send.take() {
                dbg!(Instant::now());
                last_send = Instant::now();
                otx.send(send).unwrap();
            }
        }
        match irx.try_recv() {
            Ok(o) => {
                end_time = Instant::now() + delay;
                send = Some(o);
            }
            Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => continue,
        }
    });

    (itx, orx)
}

fn compute_deepest_level_for_visible_objects(
    bbox: &BoundingBox<f64>,
    window_size: &PhysicalSize<u32>,
) -> usize {
    let smallest_size_pixels = 5.0;
    let window_size_max = std::cmp::max(window_size.width, window_size.height) as f64;
    let bbox_size_min = f64::min(bbox.height(), bbox.width());

    //the use of the maximum dimension for the window size and the minimum dimension for the
    //geographical bbox gives the smallest size possible.
    let smallest_size_carto = ((smallest_size_pixels / window_size_max) * bbox_size_min) as u32;

    let smallest_globe = std::cmp::min(EARTH_BBOX.width(), EARTH_BBOX.height());

    if smallest_size_carto == 0 {
        return usize::MAX;
    }

    if smallest_size_carto > smallest_globe {
        return 3;
    }

    //since the data is 2-dimensional, only _every other_ level splits on a given dimension: as such,
    //we multiply the 1D level by 2
    return ((smallest_globe.ilog2() - smallest_size_carto.ilog2()) * 2) as usize;
}

pub fn start_object_loading(
    mut bbox: BoundingBox<f64>,
    mut window_size: PhysicalSize<u32>,
    geography: tree::StoredTree<2, 8000, BoundingBox<i32>, UncompressedOsmData>,
    shared_buf: Arc<Mutex<Vec<(BoundingBox<i32>, UncompressedOsmData)>>>,
    has_updates: Arc<AtomicBool>,
) -> Sender<Message> {
    let (otx, channel) = std::sync::mpsc::channel::<Message>();

    let (debounce_tx, debounce_rx) = debounce(Duration::from_millis(100));

    let mut level = compute_deepest_level_for_visible_objects(&bbox, &window_size);

    std::thread::spawn(move || {
        let mut bbox_i32 = bbox.as_i32();
        let mut loader_iterator =
            reload_objects(&bbox_i32, level, &shared_buf, &geography, &has_updates);
        loader_iterator.all(|_| true);

        loop {
            if debounce_rx.try_recv().is_ok() {
                drop(loader_iterator);
                bbox_i32 = bbox.as_i32();
                loader_iterator =
                    reload_objects(&bbox_i32, level, &shared_buf, &geography, &has_updates);
            }

            loader_iterator.nth(10);

            match channel.try_recv() {
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    break;
                }
                Ok(message) => {
                    match message {
                        Message::Resize(width, height) => {
                            window_size.height = height;
                            window_size.width = width;
                        }
                        Message::Zoom(zoom, center) => {
                            bbox.zoom(zoom, center);
                            debounce_tx.send(()).unwrap();
                        }
                        Message::Pan(x, y) => {
                            bbox.shift_over(x, y);
                            debounce_tx.send(()).unwrap();
                        }
                    }
                    level = compute_deepest_level_for_visible_objects(&bbox, &window_size);
                }
            }
        }
    });

    otx
}

fn reload_objects<'a>(
    bbox: &'a BoundingBox<i32>,
    maximum_level: usize,
    store: &'a Arc<Mutex<Vec<(BoundingBox<i32>, UncompressedOsmData)>>>,
    geography: &'a tree::StoredTree<2, 8000, BoundingBox<i32>, UncompressedOsmData>,
    has_updates: &'a AtomicBool,
) -> impl Iterator<Item = ()> + 'a {
    dbg!(&bbox);
    dbg!(&maximum_level);
    let mut entries = geography.find_entries_touching_box(bbox, maximum_level);
    let mut v = Vec::new();
    let mut done = false;

    std::iter::from_fn(move || match entries.next() {
        Some(e) => {
            v.push(e);
            Some(())
        }
        None if !done => {
            dbg!(v.len());
            *store.lock().unwrap() = std::mem::take(&mut v);
            has_updates.store(true, std::sync::atomic::Ordering::Relaxed);
            done = true;
            None
        }
        None => None,
    })
}
