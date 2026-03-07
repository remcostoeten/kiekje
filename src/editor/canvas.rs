use crate::editor::tools::{Tool, ToolKind};
use anyhow::{Context, Result};
use gdk_pixbuf::PixbufLoader;
use gtk::cairo;
use gtk::gdk;
use gtk::prelude::*;
use gtk4 as gtk;
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
    image_width: i32,
    image_height: i32,
    tool: Tool,
    annotations: Vec<Annotation>,
    redo_stack: Vec<Annotation>,
    current: Option<Annotation>,
    color: (f64, f64, f64, f64),
    stroke_width: f64,
    text_value: String,
    selected: Option<usize>,
    interaction: Option<InteractionState>,
    dirty: bool,
}

#[derive(Clone)]
struct InteractionState {
    index: usize,
    mode: InteractionMode,
    start_x: f64,
    start_y: f64,
    original: Annotation,
}

#[derive(Clone, Copy)]
enum InteractionMode {
    Move,
    Resize,
}

#[derive(Clone)]
enum Annotation {
    Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        rgba: (f64, f64, f64, f64),
        line_width: f64,
    },
    Arrow {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        rgba: (f64, f64, f64, f64),
        line_width: f64,
    },
    Freehand {
        points: Vec<(f64, f64)>,
        rgba: (f64, f64, f64, f64),
        width: f64,
    },
    Text {
        x: f64,
        y: f64,
        text: String,
        rgba: (f64, f64, f64, f64),
        size: f64,
    },
    Highlight {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        rgba: (f64, f64, f64, f64),
    },
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
            image_width: texture.width(),
            image_height: texture.height(),
            tool: Tool::default(),
            annotations: Vec::new(),
            redo_stack: Vec::new(),
            current: None,
            color: (1.0, 0.2, 0.2, 1.0),
            stroke_width: 3.0,
            text_value: "Text".to_string(),
            selected: None,
            interaction: None,
            dirty: false,
        }));

        {
            let state = Rc::clone(&state);
            draw_area.set_draw_func(move |_, cr, _, _| {
                let guard = state.borrow();
                for (index, annotation) in guard.annotations.iter().enumerate() {
                    draw_annotation(cr, annotation);
                    if guard.selected == Some(index) {
                        draw_selection_outline(cr, annotation);
                    }
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
                let mut canvas = state.borrow_mut();
                if let Some((index, mode)) = hit_test_annotations(&canvas.annotations, x, y) {
                    canvas.selected = Some(index);
                    canvas.interaction = Some(InteractionState {
                        index,
                        mode,
                        start_x: x,
                        start_y: y,
                        original: canvas.annotations[index].clone(),
                    });
                    draw_area.queue_draw();
                    return;
                }

                canvas.selected = None;
                canvas.interaction = None;

                let rgba = active_color_for_tool(&canvas);
                let width = canvas.stroke_width;
                canvas.current = match canvas.tool.kind {
                    ToolKind::Rectangle => Some(Annotation::Rect {
                        x,
                        y,
                        w: 0.0,
                        h: 0.0,
                        rgba,
                        line_width: width.max(1.0),
                    }),
                    ToolKind::Arrow => Some(Annotation::Arrow {
                        x1: x,
                        y1: y,
                        x2: x,
                        y2: y,
                        rgba,
                        line_width: width.max(2.0),
                    }),
                    ToolKind::Freehand => Some(Annotation::Freehand {
                        points: vec![(x, y)],
                        rgba,
                        width: width.max(1.0),
                    }),
                    ToolKind::Highlight => Some(Annotation::Highlight {
                        x,
                        y,
                        w: 0.0,
                        h: 0.0,
                        rgba,
                    }),
                    ToolKind::Text => None,
                };
                draw_area.queue_draw();
            });
        }

        {
            let state = Rc::clone(&state);
            let draw_area = draw_area.clone();
            drag.connect_drag_update(move |_, dx, dy| {
                let mut canvas = state.borrow_mut();
                if let Some(interaction) = canvas.interaction.clone() {
                    if let Some(annotation) = canvas.annotations.get_mut(interaction.index) {
                        let x = interaction.start_x + dx;
                        let y = interaction.start_y + dy;
                        apply_interaction(
                            annotation,
                            &interaction.original,
                            interaction.mode,
                            x,
                            y,
                        );
                    }
                    draw_area.queue_draw();
                    return;
                }

                if let Some(current) = canvas.current.as_mut() {
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
                let mut canvas = state.borrow_mut();
                if canvas.interaction.take().is_some() {
                    canvas.redo_stack.clear();
                    canvas.dirty = true;
                    draw_area.queue_draw();
                    return;
                }
                if let Some(annotation) = canvas.current.take() {
                    canvas.annotations.push(annotation);
                    canvas.redo_stack.clear();
                    canvas.selected = Some(canvas.annotations.len() - 1);
                    canvas.dirty = true;
                }
                draw_area.queue_draw();
            });
        }

        let click = gtk::GestureClick::new();
        click.set_button(1);
        draw_area.add_controller(click.clone());
        {
            let state = Rc::clone(&state);
            let draw_area = draw_area.clone();
            click.connect_pressed(move |_, _, x, y| {
                let mut canvas = state.borrow_mut();
                if let Some((index, _)) = hit_test_annotations(&canvas.annotations, x, y) {
                    canvas.selected = Some(index);
                    draw_area.queue_draw();
                    return;
                }
                if canvas.tool.kind != ToolKind::Text {
                    canvas.selected = None;
                    draw_area.queue_draw();
                    return;
                }
                let text = canvas.text_value.trim();
                if text.is_empty() {
                    return;
                }
                let annotation = Annotation::Text {
                    x,
                    y,
                    text: text.to_string(),
                    rgba: canvas.color,
                    size: text_size_for_width(canvas.stroke_width),
                };
                canvas.annotations.push(annotation);
                canvas.redo_stack.clear();
                canvas.selected = Some(canvas.annotations.len() - 1);
                canvas.dirty = true;
                draw_area.queue_draw();
            });
        }

        Ok(Self { state, root })
    }

    pub fn widget(&self) -> gtk::Overlay {
        self.root.clone()
    }

    pub fn image_size(&self) -> (i32, i32) {
        let state = self.state.borrow();
        (state.image_width, state.image_height)
    }

    pub fn set_tool(&self, tool: Tool) {
        self.state.borrow_mut().tool = tool;
    }

    pub fn set_color(&self, rgba: gdk::RGBA) {
        let mut state = self.state.borrow_mut();
        let color = rgba_to_tuple(&rgba);
        state.color = color;
        if let Some(index) = state.selected {
            if let Some(annotation) = state.annotations.get_mut(index) {
                set_annotation_color(annotation, color);
                state.dirty = true;
            }
        }
        self.root.queue_draw();
    }

    pub fn color(&self) -> gdk::RGBA {
        let state = self.state.borrow();
        if let Some(index) = state.selected {
            if let Some(annotation) = state.annotations.get(index) {
                return tuple_to_rgba(annotation_color(annotation));
            }
        }
        tuple_to_rgba(state.color)
    }

    pub fn stroke_width(&self) -> f64 {
        let state = self.state.borrow();
        if let Some(index) = state.selected {
            if let Some(annotation) = state.annotations.get(index) {
                return annotation_size(annotation);
            }
        }
        state.stroke_width
    }

    pub fn adjust_stroke_width(&self, delta: f64) -> f64 {
        let mut state = self.state.borrow_mut();
        if let Some(index) = state.selected {
            if let Some(annotation) = state.annotations.get_mut(index) {
                let size = adjust_annotation_size(annotation, delta);
                state.redo_stack.clear();
                state.dirty = true;
                self.root.queue_draw();
                return size;
            }
        }
        state.stroke_width = (state.stroke_width + delta).clamp(1.0, 32.0);
        state.stroke_width
    }

    pub fn set_stroke_width(&self, width: f64) -> f64 {
        let mut state = self.state.borrow_mut();
        let width = width.clamp(1.0, 32.0);
        if let Some(index) = state.selected {
            if let Some(annotation) = state.annotations.get_mut(index) {
                let current = annotation_size(annotation);
                let delta = width - current;
                let size = adjust_annotation_size(annotation, delta);
                state.redo_stack.clear();
                state.dirty = true;
                self.root.queue_draw();
                return size;
            }
        }
        state.stroke_width = width;
        state.stroke_width
    }

    pub fn set_text_value(&self, value: String) {
        self.state.borrow_mut().text_value = value;
    }

    pub fn undo(&self) {
        let mut state = self.state.borrow_mut();
        if let Some(annotation) = state.annotations.pop() {
            state.redo_stack.push(annotation);
            state.selected = state.annotations.len().checked_sub(1);
            state.dirty = true;
            self.root.queue_draw();
        }
    }

    pub fn redo(&self) {
        let mut state = self.state.borrow_mut();
        if let Some(annotation) = state.redo_stack.pop() {
            state.annotations.push(annotation);
            state.selected = Some(state.annotations.len() - 1);
            state.dirty = true;
            self.root.queue_draw();
        }
    }

    pub fn clear(&self) {
        let mut state = self.state.borrow_mut();
        if !state.annotations.is_empty() {
            state.redo_stack = state.annotations.iter().rev().cloned().collect();
        }
        state.annotations.clear();
        state.selected = None;
        state.dirty = true;
        self.root.queue_draw();
    }

    pub fn delete_selected(&self) -> bool {
        let mut state = self.state.borrow_mut();
        let Some(index) = state.selected else {
            return false;
        };
        if index >= state.annotations.len() {
            state.selected = None;
            return false;
        }
        let removed = state.annotations.remove(index);
        state.redo_stack.clear();
        state.redo_stack.push(removed);
        state.selected = if state.annotations.is_empty() {
            None
        } else if index == 0 {
            Some(0)
        } else {
            Some(index - 1)
        };
        state.dirty = true;
        self.root.queue_draw();
        true
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.state.borrow().dirty
    }

    pub fn mark_saved(&self) {
        self.state.borrow_mut().dirty = false;
    }

    pub fn render_png(&self) -> Result<Vec<u8>> {
        let state = self.state.borrow();
        let mut surface = cairo_surface_from_png(&state.base_png)?;
        {
            let cr = cairo::Context::new(&surface).context("failed to prepare cairo context")?;
            for annotation in &state.annotations {
                draw_annotation(&cr, annotation);
            }
        }
        surface.flush();

        let png = cairo_surface_to_rgba_image(&mut surface)?;

        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(png)
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

fn cairo_surface_from_png(png_data: &[u8]) -> Result<cairo::ImageSurface> {
    let image = image::load_from_memory(png_data)
        .context("failed to decode base image for export")?
        .to_rgba8();
    let (width, height) = image.dimensions();
    let stride = cairo::Format::ARgb32
        .stride_for_width(width)
        .context("failed to compute cairo stride")?;
    let mut data = vec![0_u8; stride as usize * height as usize];

    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel(x, y).0;
            let alpha = pixel[3] as u16;
            let offset = y as usize * stride as usize + x as usize * 4;
            data[offset] = ((pixel[2] as u16 * alpha) / 255) as u8;
            data[offset + 1] = ((pixel[1] as u16 * alpha) / 255) as u8;
            data[offset + 2] = ((pixel[0] as u16 * alpha) / 255) as u8;
            data[offset + 3] = alpha as u8;
        }
    }

    cairo::ImageSurface::create_for_data(
        data,
        cairo::Format::ARgb32,
        width as i32,
        height as i32,
        stride,
    )
    .context("failed to prepare cairo image surface")
}

fn cairo_surface_to_rgba_image(surface: &mut cairo::ImageSurface) -> Result<RgbaImage> {
    let width = surface.width() as u32;
    let height = surface.height() as u32;
    let stride = surface.stride() as usize;
    let data = surface
        .data()
        .context("failed to access cairo surface data for export")?;
    let mut image = ImageBuffer::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let offset = y as usize * stride + x as usize * 4;
            let b = data[offset] as u16;
            let g = data[offset + 1] as u16;
            let r = data[offset + 2] as u16;
            let a = data[offset + 3] as u16;
            let pixel = if a == 0 {
                [0, 0, 0, 0]
            } else {
                [
                    ((r * 255) / a).min(255) as u8,
                    ((g * 255) / a).min(255) as u8,
                    ((b * 255) / a).min(255) as u8,
                    a as u8,
                ]
            };
            image.put_pixel(x, y, Rgba(pixel));
        }
    }

    Ok(image)
}

fn active_color_for_tool(state: &CanvasState) -> (f64, f64, f64, f64) {
    match state.tool.kind {
        ToolKind::Highlight => (state.color.0, state.color.1, state.color.2, 0.25),
        _ => state.color,
    }
}

fn text_size_for_width(width: f64) -> f64 {
    (width * 8.0).clamp(16.0, 64.0)
}

fn draw_annotation(cr: &cairo::Context, annotation: &Annotation) {
    match annotation {
        Annotation::Rect {
            x,
            y,
            w,
            h,
            rgba,
            line_width,
        } => {
            cr.set_source_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
            cr.set_line_width(*line_width);
            cr.rectangle(*x, *y, *w, *h);
            let _ = cr.stroke();
        }
        Annotation::Arrow {
            x1,
            y1,
            x2,
            y2,
            rgba,
            line_width,
        } => {
            cr.set_source_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
            cr.set_line_width(*line_width);
            cr.move_to(*x1, *y1);
            cr.line_to(*x2, *y2);
            let _ = cr.stroke();

            let angle = (y2 - y1).atan2(x2 - x1);
            let head = (line_width * 4.0).max(12.0);
            let left = angle + 2.6;
            let right = angle - 2.6;
            cr.move_to(*x2, *y2);
            cr.line_to(*x2 + head * left.cos(), *y2 + head * left.sin());
            cr.move_to(*x2, *y2);
            cr.line_to(*x2 + head * right.cos(), *y2 + head * right.sin());
            let _ = cr.stroke();
        }
        Annotation::Freehand {
            points,
            rgba,
            width,
        } => {
            if points.len() < 2 {
                return;
            }
            cr.set_source_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
            cr.set_line_width(*width);
            cr.set_line_cap(cairo::LineCap::Round);
            cr.set_line_join(cairo::LineJoin::Round);
            let (x0, y0) = points[0];
            cr.move_to(x0, y0);
            for (x, y) in points.iter().skip(1) {
                cr.line_to(*x, *y);
            }
            let _ = cr.stroke();
        }
        Annotation::Text {
            x,
            y,
            text,
            rgba,
            size,
        } => {
            cr.set_source_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
            cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            cr.set_font_size(*size);
            cr.move_to(*x, *y);
            let _ = cr.show_text(text);
        }
        Annotation::Highlight { x, y, w, h, rgba } => {
            cr.set_source_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
            cr.rectangle(*x, *y, *w, *h);
            let _ = cr.fill();
        }
    }
}

fn draw_selection_outline(cr: &cairo::Context, annotation: &Annotation) {
    let (x0, y0, x1, y1) = annotation_bounds(annotation);
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    cr.set_line_width(1.0);
    cr.set_dash(&[6.0, 4.0], 0.0);
    cr.rectangle(x0, y0, x1 - x0, y1 - y0);
    let _ = cr.stroke();
    cr.set_dash(&[], 0.0);
    let handle = resize_handle_rect(annotation);
    cr.set_source_rgba(0.1, 0.6, 1.0, 0.95);
    cr.rectangle(handle.0, handle.1, handle.2, handle.3);
    let _ = cr.fill();
}

fn hit_test_annotations(
    annotations: &[Annotation],
    x: f64,
    y: f64,
) -> Option<(usize, InteractionMode)> {
    for (index, annotation) in annotations.iter().enumerate().rev() {
        let handle = resize_handle_rect(annotation);
        if point_in_rect(x, y, handle) {
            return Some((index, InteractionMode::Resize));
        }
        let (x0, y0, x1, y1) = annotation_bounds(annotation);
        if point_in_rect(x, y, (x0 - 6.0, y0 - 6.0, x1 - x0 + 12.0, y1 - y0 + 12.0)) {
            return Some((index, InteractionMode::Move));
        }
    }
    None
}

fn apply_interaction(
    annotation: &mut Annotation,
    original: &Annotation,
    mode: InteractionMode,
    x: f64,
    y: f64,
) {
    match mode {
        InteractionMode::Move => move_annotation(annotation, original, x, y),
        InteractionMode::Resize => resize_annotation(annotation, original, x, y),
    }
}

fn move_annotation(annotation: &mut Annotation, original: &Annotation, x: f64, y: f64) {
    match (annotation, original) {
        (Annotation::Rect { x: ax, y: ay, .. }, Annotation::Rect { x: ox, y: oy, .. })
        | (
            Annotation::Highlight { x: ax, y: ay, .. },
            Annotation::Highlight { x: ox, y: oy, .. },
        )
        | (Annotation::Text { x: ax, y: ay, .. }, Annotation::Text { x: ox, y: oy, .. }) => {
            *ax = *ox + (x - *ox);
            *ay = *oy + (y - *oy);
        }
        (
            Annotation::Arrow {
                x1: ax1,
                y1: ay1,
                x2: ax2,
                y2: ay2,
                ..
            },
            Annotation::Arrow {
                x1: ox1,
                y1: oy1,
                x2: ox2,
                y2: oy2,
                ..
            },
        ) => {
            let dx = x - *ox1;
            let dy = y - *oy1;
            *ax1 = *ox1 + dx;
            *ay1 = *oy1 + dy;
            *ax2 = *ox2 + dx;
            *ay2 = *oy2 + dy;
        }
        (Annotation::Freehand { points: ap, .. }, Annotation::Freehand { points: op, .. }) => {
            if let Some((ox, oy)) = op.first().copied() {
                let dx = x - ox;
                let dy = y - oy;
                *ap = op.iter().map(|(px, py)| (px + dx, py + dy)).collect();
            }
        }
        _ => {}
    }
}

fn resize_annotation(annotation: &mut Annotation, original: &Annotation, x: f64, y: f64) {
    match (annotation, original) {
        (
            Annotation::Rect {
                w: aw,
                h: ah,
                x: ax,
                y: ay,
                ..
            },
            Annotation::Rect { x: ox, y: oy, .. },
        )
        | (
            Annotation::Highlight {
                w: aw,
                h: ah,
                x: ax,
                y: ay,
                ..
            },
            Annotation::Highlight { x: ox, y: oy, .. },
        ) => {
            *ax = *ox;
            *ay = *oy;
            *aw = x - *ox;
            *ah = y - *oy;
        }
        (
            Annotation::Arrow {
                x2: ax2, y2: ay2, ..
            },
            Annotation::Arrow { .. },
        ) => {
            *ax2 = x;
            *ay2 = y;
        }
        (
            Annotation::Text { size: asize, .. },
            Annotation::Text {
                x: ox,
                y: oy,
                size: osize,
                ..
            },
        ) => {
            let delta = ((x - *ox) + (y - *oy)) / 8.0;
            *asize = (*osize + delta).clamp(12.0, 96.0);
        }
        (Annotation::Freehand { width: aw, .. }, Annotation::Freehand { width: ow, .. }) => {
            let delta = (x - annotation_bounds(original).0) / 12.0;
            *aw = (*ow + delta).clamp(1.0, 32.0);
        }
        _ => {}
    }
}

fn annotation_bounds(annotation: &Annotation) -> (f64, f64, f64, f64) {
    match annotation {
        Annotation::Rect { x, y, w, h, .. } | Annotation::Highlight { x, y, w, h, .. } => {
            let x0 = x.min(x + w);
            let y0 = y.min(y + h);
            let x1 = x.max(x + w);
            let y1 = y.max(y + h);
            (x0, y0, x1, y1)
        }
        Annotation::Arrow { x1, y1, x2, y2, .. } => {
            (x1.min(*x2), y1.min(*y2), x1.max(*x2), y1.max(*y2))
        }
        Annotation::Freehand { points, .. } => {
            let mut x0 = f64::INFINITY;
            let mut y0 = f64::INFINITY;
            let mut x1 = f64::NEG_INFINITY;
            let mut y1 = f64::NEG_INFINITY;
            for (x, y) in points {
                x0 = x0.min(*x);
                y0 = y0.min(*y);
                x1 = x1.max(*x);
                y1 = y1.max(*y);
            }
            (x0, y0, x1, y1)
        }
        Annotation::Text {
            x, y, text, size, ..
        } => {
            let width = (text.len() as f64 * size * 0.6).max(*size);
            (*x, *y - size, *x + width, *y)
        }
    }
}

fn resize_handle_rect(annotation: &Annotation) -> (f64, f64, f64, f64) {
    let (_, _, x1, y1) = annotation_bounds(annotation);
    let size = 10.0;
    (x1 - size / 2.0, y1 - size / 2.0, size, size)
}

fn point_in_rect(x: f64, y: f64, rect: (f64, f64, f64, f64)) -> bool {
    x >= rect.0 && y >= rect.1 && x <= rect.0 + rect.2 && y <= rect.1 + rect.3
}

fn annotation_color(annotation: &Annotation) -> (f64, f64, f64, f64) {
    match annotation {
        Annotation::Rect { rgba, .. }
        | Annotation::Arrow { rgba, .. }
        | Annotation::Freehand { rgba, .. }
        | Annotation::Text { rgba, .. }
        | Annotation::Highlight { rgba, .. } => *rgba,
    }
}

fn set_annotation_color(annotation: &mut Annotation, rgba: (f64, f64, f64, f64)) {
    match annotation {
        Annotation::Rect { rgba: target, .. }
        | Annotation::Arrow { rgba: target, .. }
        | Annotation::Freehand { rgba: target, .. }
        | Annotation::Text { rgba: target, .. } => *target = rgba,
        Annotation::Highlight { rgba: target, .. } => *target = (rgba.0, rgba.1, rgba.2, 0.25),
    }
}

fn annotation_size(annotation: &Annotation) -> f64 {
    match annotation {
        Annotation::Rect { line_width, .. } | Annotation::Arrow { line_width, .. } => *line_width,
        Annotation::Freehand { width, .. } => *width,
        Annotation::Text { size, .. } => (*size / 8.0).clamp(1.0, 32.0),
        Annotation::Highlight { .. } => 1.0,
    }
}

fn adjust_annotation_size(annotation: &mut Annotation, delta: f64) -> f64 {
    match annotation {
        Annotation::Rect { line_width, .. } | Annotation::Arrow { line_width, .. } => {
            *line_width = (*line_width + delta).clamp(1.0, 32.0);
            *line_width
        }
        Annotation::Freehand { width, .. } => {
            *width = (*width + delta).clamp(1.0, 32.0);
            *width
        }
        Annotation::Text { size, .. } => {
            *size = (*size + delta * 8.0).clamp(12.0, 96.0);
            (*size / 8.0).clamp(1.0, 32.0)
        }
        Annotation::Highlight { .. } => 1.0,
    }
}

fn rgba_to_tuple(rgba: &gdk::RGBA) -> (f64, f64, f64, f64) {
    (
        rgba.red() as f64,
        rgba.green() as f64,
        rgba.blue() as f64,
        rgba.alpha() as f64,
    )
}

fn tuple_to_rgba((r, g, b, a): (f64, f64, f64, f64)) -> gdk::RGBA {
    gdk::RGBA::new(r as f32, g as f32, b as f32, a as f32)
}
