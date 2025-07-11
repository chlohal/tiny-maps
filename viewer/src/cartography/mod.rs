use std::sync::{mpsc::Sender, Arc, Mutex};

use osm_tag_compression::compressed_data::UncompressedOsmData;
use tree::{
    bbox::{BoundingBox, EARTH_BBOX},
    open_tree_dense,
};
use vello::{
    kurbo::{Affine, Line, Rect, Stroke, Vec2},
    peniko::{Color, Fill, Mix},
};
use winit::dpi::{PhysicalPosition, PhysicalSize};

use crate::{loader::GeometryLoader, window::WindowState, DATA_SATURATION};

const TARGET_FPS: f64 = 5.;

pub struct State {
    objects: Arc<Mutex<Vec<(BoundingBox<i32>, UncompressedOsmData)>>>,
    view_bbox: BoundingBox<f64>,

    geo_objects: GeometryLoader,

    cursor_down: bool,
    prev_cursor_position: Option<(f64, f64)>,
    cursor_position: Option<(f64, f64)>,
}
impl State {
    fn convert_pixels_to_view(&self, window_size: &PhysicalSize<u32>, pixels: f64) -> f64 {
        self.convert_pixel_pos_to_view(window_size, PhysicalPosition { x: 0., y: pixels })
            .1
    }
    fn convert_pixel_pos_to_view(
        &self,
        window_size: &PhysicalSize<u32>,
        pixel_pos: PhysicalPosition<f64>,
    ) -> (f64, f64) {
        let view_bbox = self.view_bbox;
        let view_x = *view_bbox.x() as f64;
        let view_y = *view_bbox.y() as f64;
        let (view_width, view_height) = view_bbox.size();

        let width_pc = pixel_pos.x / window_size.width as f64;
        let height_pc = pixel_pos.y / window_size.height as f64;

        (
            width_pc * (view_width as f64) + view_x,
            height_pc * (view_height as f64) + view_y,
        )
    }
}

impl WindowState for State {
    fn init() -> Self {
        let geography: tree::StoredTree<2, 8000, BoundingBox<i32>, UncompressedOsmData> =
            open_tree_dense::<2, DATA_SATURATION, BoundingBox<i32>, UncompressedOsmData>(
                std::env::current_dir().unwrap().join(".map/geography"),
                EARTH_BBOX,
            );

        let geo_objects = GeometryLoader::new(geography);
        let objects = geo_objects.objects();

        Self {
            objects,
            view_bbox: EARTH_BBOX.into(),
            geo_objects,

            cursor_down: false,
            cursor_position: None,
            prev_cursor_position: None,
        }
    }

    fn update(&mut self, rerender: &mut bool, event: &winit::event::WindowEvent, physical_size: &PhysicalSize<u32>) {
        match event {
            winit::event::WindowEvent::Resized(size) => {
            }
            winit::event::WindowEvent::CursorMoved {
                device_id,
                position,
            } => {
                let new_position = self.convert_pixel_pos_to_view(physical_size, *position);
                self.prev_cursor_position =
                    std::mem::replace(&mut self.cursor_position, Some(new_position));

                if self.cursor_down {
                    if let (Some(prev), Some(cur)) =
                        (self.prev_cursor_position, self.cursor_position)
                    {
                        let dx = cur.0 - prev.0;
                        let dy = cur.1 - prev.1;

                        self.view_bbox.shift_over(-dx, -dy);

                        self.geo_objects.pan_relative(-dx, -dy);
                        *rerender = true;
                    }
                }
            }
            winit::event::WindowEvent::CursorLeft { device_id } => {
                self.cursor_down = false;
            }
            winit::event::WindowEvent::MouseWheel {
                device_id,
                delta,
                phase,
            } => match delta {
                winit::event::MouseScrollDelta::LineDelta(_, _) => todo!(),
                winit::event::MouseScrollDelta::PixelDelta(phys) => {
                    self.view_bbox.zoom(phys.y / 150., self.cursor_position);

                    self.geo_objects
                        .zoom_relative(phys.y / 150., self.cursor_position);
                    *rerender = true;
                }
            },
            winit::event::WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => match state {
                winit::event::ElementState::Pressed => {
                    self.cursor_down = true;
                }
                winit::event::ElementState::Released => {
                    self.cursor_down = false;
                    self.prev_cursor_position = None;
                    self.cursor_position = None;
                }
            },
            winit::event::WindowEvent::TouchpadMagnify {
                device_id,
                delta,
                phase,
            } => {
                self.view_bbox.zoom(
                    self.convert_pixels_to_view(&physical_size, *delta),
                    self.cursor_position,
                );
                self.geo_objects.zoom_relative(
                    self.convert_pixels_to_view(&physical_size, *delta),
                    self.cursor_position,
                );
                *rerender = true;
            }
            winit::event::WindowEvent::TouchpadRotate {
                device_id,
                delta,
                phase,
            } => todo!(),
            winit::event::WindowEvent::Touch(_) => todo!(),
            _ => {}
        };
    }

    fn render(&self, scene: &mut vello::Scene, screen_size: &PhysicalSize<u32>) {
        let mut state = self.objects.lock().unwrap();

        let view_bbox = self.view_bbox;
        let geo_objects: Vec<_> = std::mem::take(&mut state);

        drop(state);

        let stroke = Stroke::new((view_bbox.width() as f64) / (screen_size.width as f64) * 1.);
        let rect_stroke_color = Color::from_rgb8(0x00, 0x00, 0x00);

        let bbox_transform = Affine::IDENTITY
            .then_translate(Vec2::new(
                -1. * (*view_bbox.x() as f64),
                -1. * (*view_bbox.y() as f64),
            ))
            .then_scale_non_uniform(
                (screen_size.width as f64) / (view_bbox.width() as f64),
                (screen_size.height as f64) / (view_bbox.height() as f64),
            );

        let brush_transform = Affine::IDENTITY.then_scale_non_uniform(
            (view_bbox.width() as f64) / (screen_size.width as f64),
            (view_bbox.height() as f64) / (screen_size.height as f64),
        );

        draw_lonlat_grid(scene, &stroke, &bbox_transform);

        for (bbox, itm) in geo_objects.iter() {
            let mut path = vello::kurbo::BezPath::new();

            let Some(points) = itm.decompress_way_points(bbox).transpose().unwrap() else {
                continue;
            };
            if points.len() < 2 {
                continue;
            }
            let mut points = points.into_iter().map(|(x, y)| (x as f64, y as f64));

            path.move_to(points.next().unwrap());
            for point in points {
                path.line_to(point);
            }

            scene.stroke(
                &stroke,
                bbox_transform,
                rect_stroke_color,
                Some(brush_transform),
                &path,
            );
        }

        let mut state = self.objects.lock().unwrap();

        if state.is_empty() {
            *state = geo_objects;
        }
    }
}

fn draw_lonlat_grid(scene: &mut vello::Scene, stroke: &Stroke, screen_transform: &Affine) {
    static EARTH_BBOX_FLOAT: std::sync::LazyLock<BoundingBox<f64>> = std::sync::LazyLock::new(|| EARTH_BBOX.into());

    let (left, bottom) = (*EARTH_BBOX_FLOAT.x(), *EARTH_BBOX_FLOAT.y());
    let (right, top) = (*EARTH_BBOX_FLOAT.x_end(), *EARTH_BBOX_FLOAT.y_end());

    let mut y = bottom;
    while y <= top {
        scene.stroke(
            stroke,
            *screen_transform,
            Color::from_rgb8(0x00, 0x00, 0xff),
            None,
            &Line::new((left, y), (right, y)),
        );

        y += (top - bottom) / 10.;
    }

    let mut x = left;
    while x <= right {
        scene.stroke(
            stroke,
            *screen_transform,
            Color::from_rgb8(0x00, 0x00, 0xff),
            None,
            &Line::new((x, bottom), (x, top)),
        );

        x += (right - left) / 10.;
    }
}
