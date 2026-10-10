use std::f64::consts::{PI, TAU};

use gtk::cairo;

use super::document::{Area, Color, Point, Shape, Stroke};
use super::pixels::{self, Picture};
use super::shapes;
use crate::output::Image;

const MARKER_ALPHA: f64 = 0.85;

/// Image surfaces for one picture: the original and its pixelated copy.
pub struct Layers {
    pub base: cairo::ImageSurface,
    pub hidden: cairo::ImageSurface,
}

impl Layers {
    /// Wraps a picture and its mosaic in cairo surfaces.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let layers = Layers::new(picture.clone(), picture.mosaic(block))?;
    /// ```
    pub fn new(base: Picture, hidden: Picture) -> Result<Self, cairo::Error> {
        Ok(Self {
            base: surface(base)?,
            hidden: surface(hidden)?,
        })
    }
}

fn surface(picture: Picture) -> Result<cairo::ImageSurface, cairo::Error> {
    let stride = picture.stride() as i32;
    cairo::ImageSurface::create_for_data(
        picture.data,
        cairo::Format::ARgb32,
        picture.width as i32,
        picture.height as i32,
        stride,
    )
}

/// Paints the picture and then every shape, in image pixel coordinates.
///
/// # Examples
///
/// ```ignore
/// render::draw(&cr, &layers, document.shapes().iter())?;
/// ```
pub fn draw<'a>(
    cr: &cairo::Context,
    layers: &Layers,
    shapes: impl IntoIterator<Item = &'a Shape>,
) -> Result<(), cairo::Error> {
    cr.set_source_surface(&layers.base, 0.0, 0.0)?;
    cr.paint()?;
    draw_shapes(cr, layers, shapes)
}

/// Draws the shapes without the picture under them, for a canvas that already shows it.
///
/// Markers multiply with what is under them, so a canvas with markers paints the picture first.
///
/// # Examples
///
/// ```ignore
/// render::draw_shapes(&cr, &layers, document.shapes().iter())?;
/// ```
pub fn draw_shapes<'a>(
    cr: &cairo::Context,
    layers: &Layers,
    shapes: impl IntoIterator<Item = &'a Shape>,
) -> Result<(), cairo::Error> {
    for shape in shapes {
        cr.save()?;
        let drawn = draw_shape(cr, layers, shape);
        cr.restore()?;
        drawn?;
    }
    Ok(())
}

/// Draws the shapes onto a copy of the picture and returns it as straight RGBA.
///
/// Runs without a display, so it can run on a worker thread.
///
/// # Examples
///
/// ```ignore
/// let image = render::flatten(&picture, &shapes)?;
/// ```
pub fn flatten(
    picture: &Picture,
    shapes: &[Shape],
    crop: Option<Area>,
) -> Result<Image, cairo::Error> {
    let hidden = if shapes
        .iter()
        .any(|shape| matches!(shape, Shape::Pixelate { .. }))
    {
        picture.mosaic(pixels::block_size(picture.width, picture.height))
    } else {
        picture.clone()
    };
    let layers = Layers::new(picture.clone(), hidden)?;
    let target = cairo::ImageSurface::create(
        cairo::Format::ARgb32,
        picture.width as i32,
        picture.height as i32,
    )?;
    {
        let cr = cairo::Context::new(&target)?;
        draw(&cr, &layers, shapes)?;
    }
    target.flush();
    let stride = usize::try_from(target.stride()).map_err(|_| cairo::Error::InvalidStride)?;
    let row = picture.width as usize * 4;
    let data = target
        .take_data()
        .map_err(|_| cairo::Error::SurfaceFinished)?;
    let packed: Vec<u8> = data
        .chunks(stride.max(row))
        .take(picture.height as usize)
        .flat_map(|line| &line[..row.min(line.len())])
        .copied()
        .collect();
    let drawn = Picture {
        width: picture.width,
        height: picture.height,
        data: packed,
    };
    Ok(
        match crop.and_then(|area| pixels::region(area, picture.width, picture.height)) {
            Some(region) => drawn.cropped(region).to_image(),
            None => drawn.to_image(),
        },
    )
}

fn draw_shape(cr: &cairo::Context, layers: &Layers, shape: &Shape) -> Result<(), cairo::Error> {
    match shape {
        Shape::Rectangle { area, stroke } => {
            use_stroke(cr, *stroke);
            cr.set_line_join(cairo::LineJoin::Miter);
            cr.rectangle(area.x, area.y, area.width, area.height);
            cr.stroke()
        }
        Shape::Ellipse { area, stroke } => {
            use_stroke(cr, *stroke);
            ellipse(cr, *area)?;
            cr.stroke()
        }
        Shape::Line { from, to, stroke } => {
            use_stroke(cr, *stroke);
            cr.move_to(from.x, from.y);
            cr.line_to(to.x, to.y);
            cr.stroke()
        }
        Shape::Arrow { from, to, stroke } => arrow(cr, *from, *to, *stroke),
        Shape::Pen { points, stroke } => {
            use_stroke(cr, *stroke);
            let mut points = points.iter();
            if let Some(first) = points.next() {
                cr.move_to(first.x, first.y);
            }
            for point in points {
                cr.line_to(point.x, point.y);
            }
            cr.stroke()
        }
        Shape::Highlight { area, color } => {
            let (red, green, blue) = color.rgb();
            cr.set_operator(cairo::Operator::Multiply);
            cr.set_source_rgba(red, green, blue, MARKER_ALPHA);
            cr.rectangle(area.x, area.y, area.width, area.height);
            cr.fill()
        }
        Shape::Step { at, number, stroke } => step(cr, *at, *number, *stroke),
        Shape::Text { at, text, stroke } => write(cr, *at, text, *stroke),
        Shape::Pixelate { area } => {
            cr.rectangle(area.x, area.y, area.width, area.height);
            cr.clip();
            cr.set_source_surface(&layers.hidden, 0.0, 0.0)?;
            let pattern = cr.source();
            pattern.set_filter(cairo::Filter::Nearest);
            cr.paint()
        }
    }
}

const FONT: &str = "Geist";

/// Measures one line of annotation text in image pixels: its advance and its line height.
///
/// Falls back to a rough estimate if cairo cannot measure it.
///
/// # Examples
///
/// ```ignore
/// let (width, height) = render::measure("Bug here", 28.0);
/// ```
pub fn measure(text: &str, size: f64) -> (f64, f64) {
    let measured = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1)
        .and_then(|surface| cairo::Context::new(&surface))
        .and_then(|cr| {
            cr.select_font_face(FONT, cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            cr.set_font_size(size);
            let line = cr.font_extents()?;
            let extents = cr.text_extents(text)?;
            Ok((extents.x_advance(), line.ascent() + line.descent()))
        });
    measured.unwrap_or((text.chars().count() as f64 * size * 0.6, size * 1.2))
}

fn write(cr: &cairo::Context, at: Point, text: &str, stroke: Stroke) -> Result<(), cairo::Error> {
    let size = shapes::font_size(stroke.width);
    cr.select_font_face(FONT, cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    cr.set_font_size(size);
    let line = cr.font_extents()?;
    cr.move_to(at.x, at.y + line.ascent());
    cr.text_path(text);
    let halo = if stroke.color == Color::Black {
        Color::White
    } else {
        Color::Black
    };
    let (red, green, blue) = halo.rgb();
    cr.set_source_rgba(red, green, blue, 0.7);
    cr.set_line_width(size / 6.0);
    cr.set_line_join(cairo::LineJoin::Round);
    cr.stroke_preserve()?;
    let (red, green, blue) = stroke.color.rgb();
    cr.set_source_rgb(red, green, blue);
    cr.fill()
}

fn use_stroke(cr: &cairo::Context, stroke: Stroke) {
    let (red, green, blue) = stroke.color.rgb();
    cr.set_source_rgb(red, green, blue);
    cr.set_line_width(stroke.width);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
}

fn ellipse(cr: &cairo::Context, area: Area) -> Result<(), cairo::Error> {
    cr.save()?;
    cr.translate(area.x + area.width / 2.0, area.y + area.height / 2.0);
    cr.scale(area.width / 2.0, area.height / 2.0);
    cr.arc(0.0, 0.0, 1.0, 0.0, TAU);
    cr.restore()
}

fn arrow(cr: &cairo::Context, from: Point, to: Point, stroke: Stroke) -> Result<(), cairo::Error> {
    let length = from.distance(to);
    let head = (stroke.width * 4.0).max(12.0).min(length * 0.6);
    let angle = (to.y - from.y).atan2(to.x - from.x);
    let spread = PI / 7.0;
    let shaft_end = Point {
        x: to.x - angle.cos() * head * 0.8,
        y: to.y - angle.sin() * head * 0.8,
    };
    use_stroke(cr, stroke);
    cr.move_to(from.x, from.y);
    cr.line_to(shaft_end.x, shaft_end.y);
    cr.stroke()?;
    cr.move_to(to.x, to.y);
    cr.line_to(
        to.x - head * (angle - spread).cos(),
        to.y - head * (angle - spread).sin(),
    );
    cr.line_to(
        to.x - head * (angle + spread).cos(),
        to.y - head * (angle + spread).sin(),
    );
    cr.close_path();
    cr.fill_preserve()?;
    cr.set_line_width(stroke.width.min(head / 4.0));
    cr.stroke()
}

fn step(cr: &cairo::Context, at: Point, number: u32, stroke: Stroke) -> Result<(), cairo::Error> {
    let radius = shapes::step_radius(stroke);
    let (red, green, blue) = stroke.color.rgb();
    cr.set_source_rgb(red, green, blue);
    cr.arc(at.x, at.y, radius, 0.0, TAU);
    cr.fill()?;
    let ink = if stroke.color.is_light() {
        Color::Black
    } else {
        Color::White
    };
    let (red, green, blue) = ink.rgb();
    cr.set_source_rgb(red, green, blue);
    cr.select_font_face(FONT, cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    let label = number.to_string();
    cr.set_font_size(radius * if label.len() > 2 { 0.8 } else { 1.1 });
    let extents = cr.text_extents(&label)?;
    cr.move_to(
        at.x - extents.width() / 2.0 - extents.x_bearing(),
        at.y - extents.height() / 2.0 - extents.y_bearing(),
    );
    cr.show_text(&label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::PixelFormat;
    use crate::editor::document::Area;

    fn white(width: u32, height: u32) -> Picture {
        Picture::from_image(&Image {
            width,
            height,
            stride: width as usize * 4,
            format: PixelFormat::Rgba,
            data: vec![255; (width * height * 4) as usize],
        })
    }

    fn pixel(image: &Image, x: u32, y: u32) -> [u8; 4] {
        let start = y as usize * image.stride + x as usize * 4;
        image.data[start..start + 4].try_into().unwrap()
    }

    #[test]
    fn flattens_without_shapes_to_the_same_pixels() {
        let picture = white(4, 3);
        let image = flatten(&picture, &[], None).unwrap();
        assert_eq!(image.data, picture.to_image().data);
    }

    #[test]
    fn draws_a_rectangle_outline_and_leaves_the_inside() {
        let picture = white(40, 40);
        let shape = Shape::Rectangle {
            area: Area {
                x: 10.0,
                y: 10.0,
                width: 20.0,
                height: 20.0,
            },
            stroke: Stroke {
                color: Color::Black,
                width: 2.0,
            },
        };
        let image = flatten(&picture, &[shape], None).unwrap();
        assert_eq!(pixel(&image, 10, 20), [0x11, 0x11, 0x11, 255]);
        assert_eq!(pixel(&image, 20, 20), [255, 255, 255, 255]);
        assert_eq!(pixel(&image, 2, 2), [255, 255, 255, 255]);
    }

    #[test]
    fn pixelates_only_inside_the_area() {
        let mut source = white(32, 32).to_image();
        for y in 0..32 {
            for x in 0..32 {
                if (x + y) % 2 == 0 {
                    let start = y * source.stride + x * 4;
                    source.data[start..start + 3].copy_from_slice(&[0, 0, 0]);
                }
            }
        }
        let picture = Picture::from_image(&source);
        let shape = Shape::Pixelate {
            area: Area {
                x: 0.0,
                y: 0.0,
                width: 16.0,
                height: 16.0,
            },
        };
        let image = flatten(&picture, &[shape], None).unwrap();
        assert_eq!(pixel(&image, 0, 0), pixel(&image, 1, 0));
        assert_ne!(pixel(&image, 0, 0), [0, 0, 0, 255]);
        assert_eq!(pixel(&image, 20, 20), [0, 0, 0, 255]);
        assert_eq!(pixel(&image, 21, 20), [255, 255, 255, 255]);
    }

    #[test]
    fn crops_after_drawing() {
        let picture = white(40, 30);
        let shape = Shape::Rectangle {
            area: Area {
                x: 10.0,
                y: 10.0,
                width: 20.0,
                height: 10.0,
            },
            stroke: Stroke {
                color: Color::Black,
                width: 2.0,
            },
        };
        let crop = Area {
            x: 10.0,
            y: 5.0,
            width: 15.0,
            height: 12.0,
        };
        let image = flatten(&picture, &[shape], Some(crop)).unwrap();
        assert_eq!((image.width, image.height), (15, 12));
        assert_eq!(pixel(&image, 0, 8), [0x11, 0x11, 0x11, 255]);
        assert_eq!(pixel(&image, 5, 0), [255, 255, 255, 255]);
    }

    #[test]
    fn marks_without_hiding_dark_pixels() {
        let mut source = white(8, 8).to_image();
        source.data[..4].copy_from_slice(&[0, 0, 0, 255]);
        let picture = Picture::from_image(&source);
        let shape = Shape::Highlight {
            area: Area {
                x: 0.0,
                y: 0.0,
                width: 8.0,
                height: 8.0,
            },
            color: Color::Yellow,
        };
        let image = flatten(&picture, &[shape], None).unwrap();
        assert_eq!(pixel(&image, 0, 0), [0, 0, 0, 255]);
        let marked = pixel(&image, 4, 4);
        assert_eq!(marked[0], 255);
        assert!(marked[2] < 128);
    }
}
