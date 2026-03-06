use crate::editor::tools::{Tool, ToolKind};
use anyhow::{Context, Result};
use gdk_pixbuf::PixbufLoader;
use gtk4 as gtk;
use gtk::cairo;
use gtk::gdk;
use gtk::prelude::*;
use image::{ImageBuffer, Rgba, RgbaImage};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone)]
pub struct EditorCanvas {
    state: Rc<RefCell<CanvasState>>,
    root: gtk::Overlay,
}

#[derive(Clone)]
struct CanvasState {
    base_png: Vec<u8>,
    tool: Tool,
    annotations: Vec<Annotation>,
    current: Option<Annotation>,
}

#[derive(Clone)]
enum Annotation {
    Rect { x: f64, y: f64, w: f64, h: f64, rgba: (f64, f64, f64, f64) },
    Arrow { x1: f64, y1: f64, x2: f64, y2: f64, rgba: (f64, f64, f64, f64) },
    Freehand { points: Vec<(f64, f64)>, rgba: (f64, f64, f64, f64), width: f64 },
    Text { x: f64, y: f64, text: String, rgba: (f64, f64, f64, f64) },
    Highlight { x: f64, y: f64, w: f64, h: f64 },
}

impl EditorCanvas {
    pub fn new(png_data: &[u8]) -> Result<Self> {
        let texture = png_to_texture(png_data)?;
        let picture = gtk::Picture::for_paintable(&texture);
        picture.set_can_shrink(false);

        let draw_area = gtk::DrawingArea::new();
        draw_area.set_content_width(texture.width());
        draw_area.set_content_height(texture.height());
        draw_area.set_hexpand(true);
        draw_area.set_vexpand(true);

        let root = gtk::Overlay::new();
        root.set_hexpand(true);
        root.set_vexpand(true);
        root.set_child(Some(&picture));
        root.add_overlay(&draw_area);

        let state = Rc::new(RefCell::new(CanvasState {
            base_png: png_data.to_vec(),
            tool: Tool::default(),
            annotations: Vec::new(),
            current: None,
        }));

        {
            let state = Rc::clone(&state);
            draw_area.set_draw_func(move |_, cr, _, _| {
                let guard = state.borrow();
                for a in &guard.annotations {
                    draw_annotation(cr, a);
                }
                if let Some(current) = &guard.current {
                    draw_annotation(cr, current);
                }
            });
        }

        let drag = gtk::GestureDrag::new();
        draw_area.add_controller(drag.clone());

        {
            let state = Rc::clone(&state);
            let draw_area = draw_area.clone();
            drag.connect_drag_begin(move |_, x, y| {
                let mut s = state.borrow_mut();
                s.current = Some(match s.tool.kind {
                    ToolKind::Rectangle => Annotation::Rect {
                        x,
                        y,
                        w: 0.0,
                        h: 0.0,
                        rgba: (1.0, 0.2, 0.2, 1.0),
                    },
                    ToolKind::Arrow => Annotation::Arrow {
                        x1: x,
                        y1: y,
                        x2: x,
                        y2: y,
                        rgba: (0.2, 0.9, 0.2, 1.0),
                    },
                    ToolKind::Freehand => Annotation::Freehand {
                        points: vec![(x, y)],
                        rgba: (0.2, 0.7, 1.0, 1.0),
                        width: 3.0,
                    },
                    ToolKind::Text => Annotation::Text {
                        x,
                        y,
                        text: "Text".to_string(),
                        rgba: (1.0, 1.0, 1.0, 1.0),
                    },
                    ToolKind::Highlight => Annotation::Highlight {
                        x,
                        y,
                        w: 0.0,
                        h: 0.0,
                    },
                });
                draw_area.queue_draw();
            });
        }

        {
            let state = Rc::clone(&state);
            let draw_area = draw_area.clone();
            drag.connect_drag_update(move |_, dx, dy| {
                let mut s = state.borrow_mut();
                if let Some(current) = s.current.as_mut() {
                    match current {
                        Annotation::Rect { w, h, .. } | Annotation::Highlight { w, h, .. } => {
                            *w = dx;
                            *h = dy;
                        }
                        Annotation::Arrow { x2, y2, x1, y1, .. } => {
                            *x2 = *x1 + dx;
                            *y2 = *y1 + dy;
                        }
                        Annotation::Freehand { points, .. } => {
                            if let Some((x0, y0)) = points.first().copied() {
                                points.push((x0 + dx, y0 + dy));
                            }
                        }
                        Annotation::Text { .. } => {}
                    }
                }
                draw_area.queue_draw();
            });
        }

        {
            let state = Rc::clone(&state);
            let draw_area = draw_area.clone();
            drag.connect_drag_end(move |_, _, _| {
                let mut s = state.borrow_mut();
                if let Some(annotation) = s.current.take() {
                    s.annotations.push(annotation);
                }
                draw_area.queue_draw();
            });
        }

        Ok(Self { state, root })
    }

    pub fn widget(&self) -> gtk::Overlay {
        self.root.clone()
    }

    pub fn set_tool(&self, tool: Tool) {
        self.state.borrow_mut().tool = tool;
    }

    pub fn undo(&self) {
        let mut state = self.state.borrow_mut();
        state.annotations.pop();
        self.root.queue_draw();
    }

    pub fn clear(&self) {
        let mut state = self.state.borrow_mut();
        state.annotations.clear();
        self.root.queue_draw();
    }

    pub fn render_png(&self) -> Result<Vec<u8>> {
        let state = self.state.borrow();
        let mut img: RgbaImage = image::load_from_memory(&state.base_png)
            .context("failed to decode base image")?
            .to_rgba8();

        for annotation in &state.annotations {
            draw_annotation_on_image(&mut img, annotation);
        }

        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .context("failed to encode annotated image")?;
        Ok(out)
    }
}

fn png_to_texture(png_data: &[u8]) -> Result<gdk::Texture> {
    let loader = PixbufLoader::new();
    loader
        .write(png_data)
        .context("failed to load PNG bytes into pixbuf loader")?;
    loader.close().context("failed to close pixbuf loader")?;
    let pixbuf = loader.pixbuf().context("pixbuf not available after load")?;
    Ok(gdk::Texture::for_pixbuf(&pixbuf))
}

fn draw_annotation(cr: &cairo::Context, annotation: &Annotation) {
    match annotation {
        Annotation::Rect { x, y, w, h, rgba } => {
            cr.set_source_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
            cr.set_line_width(2.0);
            cr.rectangle(*x, *y, *w, *h);
            let _ = cr.stroke();
        }
        Annotation::Arrow { x1, y1, x2, y2, rgba } => {
            cr.set_source_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
            cr.set_line_width(3.0);
            cr.move_to(*x1, *y1);
            cr.line_to(*x2, *y2);
            let _ = cr.stroke();

            let angle = (y2 - y1).atan2(x2 - x1);
            let head = 12.0;
            let left = angle + 2.6;
            let right = angle - 2.6;
            cr.move_to(*x2, *y2);
            cr.line_to(*x2 + head * left.cos(), *y2 + head * left.sin());
            cr.move_to(*x2, *y2);
            cr.line_to(*x2 + head * right.cos(), *y2 + head * right.sin());
            let _ = cr.stroke();
        }
        Annotation::Freehand { points, rgba, width } => {
            if points.len() < 2 {
                return;
            }
            cr.set_source_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
            cr.set_line_width(*width);
            let (x0, y0) = points[0];
            cr.move_to(x0, y0);
            for (x, y) in points.iter().skip(1) {
                cr.line_to(*x, *y);
            }
            let _ = cr.stroke();
        }
        Annotation::Text { x, y, text, rgba } => {
            cr.set_source_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
            cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            cr.set_font_size(24.0);
            cr.move_to(*x, *y);
            let _ = cr.show_text(text);
        }
        Annotation::Highlight { x, y, w, h } => {
            cr.set_source_rgba(1.0, 1.0, 0.0, 0.25);
            cr.rectangle(*x, *y, *w, *h);
            let _ = cr.fill();
        }
    }
}

fn draw_annotation_on_image(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, annotation: &Annotation) {
    match annotation {
        Annotation::Rect { x, y, w, h, rgba } => draw_rect(img, *x, *y, *w, *h, rgba_to_u8(*rgba), false),
        Annotation::Highlight { x, y, w, h } => draw_rect(img, *x, *y, *w, *h, [255, 255, 0, 80], true),
        Annotation::Arrow { x1, y1, x2, y2, rgba } => draw_line(img, *x1, *y1, *x2, *y2, rgba_to_u8(*rgba), 3),
        Annotation::Freehand { points, rgba, width } => {
            for pair in points.windows(2) {
                let (x1, y1) = pair[0];
                let (x2, y2) = pair[1];
                draw_line(img, x1, y1, x2, y2, rgba_to_u8(*rgba), *width as i32);
            }
        }
        Annotation::Text { x, y, .. } => {
            // Minimal placeholder text rendering in export.
            draw_rect(img, *x, *y - 18.0, 120.0, 24.0, [255, 255, 255, 180], true);
        }
    }
}

fn rgba_to_u8((r, g, b, a): (f64, f64, f64, f64)) -> [u8; 4] {
    [
        (r.clamp(0.0, 1.0) * 255.0) as u8,
        (g.clamp(0.0, 1.0) * 255.0) as u8,
        (b.clamp(0.0, 1.0) * 255.0) as u8,
        (a.clamp(0.0, 1.0) * 255.0) as u8,
    ]
}

fn draw_rect(img: &mut RgbaImage, x: f64, y: f64, w: f64, h: f64, rgba: [u8; 4], fill: bool) {
    let (x0, y0, x1, y1) = normalized_box(x, y, w, h);
    if fill {
        for yy in y0..=y1 {
            for xx in x0..=x1 {
                blend_pixel(img, xx, yy, rgba);
            }
        }
    } else {
        for xx in x0..=x1 {
            blend_pixel(img, xx, y0, rgba);
            blend_pixel(img, xx, y1, rgba);
        }
        for yy in y0..=y1 {
            blend_pixel(img, x0, yy, rgba);
            blend_pixel(img, x1, yy, rgba);
        }
    }
}

fn draw_line(img: &mut RgbaImage, x1: f64, y1: f64, x2: f64, y2: f64, rgba: [u8; 4], width: i32) {
    let steps = ((x2 - x1).abs().max((y2 - y1).abs()) as i32).max(1);
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let x = x1 + (x2 - x1) * t;
        let y = y1 + (y2 - y1) * t;
        for wx in -width..=width {
            for wy in -width..=width {
                blend_pixel(img, x as i32 + wx, y as i32 + wy, rgba);
            }
        }
    }
}

fn normalized_box(x: f64, y: f64, w: f64, h: f64) -> (i32, i32, i32, i32) {
    let x0 = x.min(x + w).floor() as i32;
    let y0 = y.min(y + h).floor() as i32;
    let x1 = x.max(x + w).ceil() as i32;
    let y1 = y.max(y + h).ceil() as i32;
    (x0, y0, x1, y1)
}

fn blend_pixel(img: &mut RgbaImage, x: i32, y: i32, src: [u8; 4]) {
    if x < 0 || y < 0 {
        return;
    }
    let (x, y) = (x as u32, y as u32);
    if x >= img.width() || y >= img.height() {
        return;
    }

    let dst = img.get_pixel_mut(x, y);
    let alpha = src[3] as f32 / 255.0;
    let inv = 1.0 - alpha;

    dst[0] = (src[0] as f32 * alpha + dst[0] as f32 * inv) as u8;
    dst[1] = (src[1] as f32 * alpha + dst[1] as f32 * inv) as u8;
    dst[2] = (src[2] as f32 * alpha + dst[2] as f32 * inv) as u8;
    dst[3] = 255;
}
