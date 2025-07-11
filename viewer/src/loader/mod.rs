use std::{
    sync::{
        mpsc::{Receiver, RecvError, RecvTimeoutError, Sender, TryRecvError},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use osm_tag_compression::compressed_data::UncompressedOsmData;
use tree::bbox::{BoundingBox, EARTH_BBOX};

pub(super) enum Message {
    Zoom(f64, Option<(f64, f64)>),
    Pan(f64, f64),
}

pub struct GeometryLoader {
    objects: Arc<Mutex<Vec<(BoundingBox<i32>, UncompressedOsmData)>>>,
    render_sender: Sender<Message>,
}

impl GeometryLoader {
    pub fn new(
        geography: tree::StoredTree<2, 8000, BoundingBox<i32>, UncompressedOsmData>,
    ) -> Self {
        let bbox = geography.root_bbox().to_owned().into();

        let objects = Default::default();

        let render_sender = start_object_loading(bbox, geography, Arc::clone(&objects));

        Self {
            objects,
            render_sender,
        }
    }
    pub fn objects(&self) -> Arc<Mutex<Vec<(BoundingBox<i32>, UncompressedOsmData)>>> {
        Arc::clone(&self.objects)
    }

    pub fn pan_relative(&self, x: f64, y: f64) {
        self.render_sender.send(Message::Pan(x, y)).unwrap();
    }
    pub fn zoom_relative(&self, zoom_factor: f64, center: Option<(f64, f64)>) {
        self.render_sender
            .send(Message::Zoom(zoom_factor, center))
            .unwrap();
    }
}

fn debounce<T: Send + 'static>(delay: Duration) -> (Sender<T>, Receiver<T>) {
    let (itx, irx) = std::sync::mpsc::channel();
    let (otx, orx) = std::sync::mpsc::channel();

    let mut end_time = Instant::now() + delay;
    let mut send = None;

    std::thread::spawn(move || loop {
        if Instant::now() >= end_time {
            if let Some(send) = send.take() {
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

pub fn start_object_loading(
    mut bbox: BoundingBox<f64>,
    geography: tree::StoredTree<2, 8000, BoundingBox<i32>, UncompressedOsmData>,
    shared_buf: Arc<Mutex<Vec<(BoundingBox<i32>, UncompressedOsmData)>>>,
) -> Sender<Message> {
    let (otx, channel) = std::sync::mpsc::channel::<Message>();

    let (debounce_tx, debounce_rx) = debounce(Duration::from_millis(100));

    std::thread::spawn(move || {
        reload_objects(&bbox, &shared_buf, &geography);

        loop {
            if debounce_rx.try_recv().is_ok() {
                reload_objects(&bbox, &shared_buf, &geography);
            }

            match channel.try_recv() {
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    break;
                }
                Ok(message) => match message {
                    Message::Zoom(zoom, center) => {
                        bbox.zoom(zoom, center);
                        debounce_tx.send(()).unwrap();
                    }
                    Message::Pan(x, y) => {
                        bbox.shift_over(x, y);
                        debounce_tx.send(()).unwrap();
                    }
                },
            }
        }
    });

    otx
}

fn reload_objects(
    bbox: &BoundingBox<f64>,
    store: &Arc<Mutex<Vec<(BoundingBox<i32>, UncompressedOsmData)>>>,
    geography: &tree::StoredTree<2, 8000, BoundingBox<i32>, UncompressedOsmData>,
) {
    let depth = 1000;

    let i32_bbox = bbox.as_i32();

    let mut v = Vec::new();

    let start = Instant::now();

    dbg!(&i32_bbox);

    for obj in geography.find_entries_touching_box(&i32_bbox, depth as usize) {
        let size = std::cmp::max(obj.0.width(), obj.0.height());

        v.push(obj);

        if Instant::now().duration_since(start) > Duration::from_millis(100000000) {
            break;
        }
    }

    dbg!(v.len());

    *store.lock().unwrap() = v;
}
