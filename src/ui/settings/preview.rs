use std::cell::RefCell;
use std::f64::consts::{SQRT_2, TAU};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{cairo, glib, pango};

use crate::config::AfterSelection;

const HEIGHT: i32 = 300;
const ART_HEIGHT: i32 = 96;
const RADIO_SIZE: i32 = 16;
const FADE_US: f64 = 200_000.0;
const DIMMED: f64 = 0.2;
const SHRUNK: f64 = 0.96;
const LAYERS: usize = 5;
const SANS: &str = "Geist";
const MONO: &str = "Geist Mono";
const LEAD_GAP: f64 = 6.0;
const ICON_GRID: f64 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Part {
    After,
    Snap,
    Hints,
    Loupe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Settings {
    pub(super) after: AfterSelection,
    pub(super) snap: bool,
    pub(super) hints: bool,
    pub(super) magnifier: bool,
}

pub(super) struct Preview {
    area: gtk::DrawingArea,
    state: Rc<RefCell<State>>,
}

impl Preview {
    /// Builds the overlay preview: a frozen desktop mock whose parts follow the capture settings.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let preview = Preview::new(Settings { after, snap: true, hints: true, magnifier: true });
    /// card.append(preview.widget());
    /// ```
    pub(super) fn new(settings: Settings) -> Self {
        let state = Rc::new(RefCell::new(State::new(settings)));
        let area = gtk::DrawingArea::builder()
            .content_height(HEIGHT)
            .hexpand(true)
            .css_classes(["preview"])
            .build();
        let drawn = state.clone();
        area.set_draw_func(move |area, cr, width, height| {
            let frame = drawn.borrow().frame(glib::monotonic_time());
            if let Err(error) = paint(area, cr, f64::from(width), f64::from(height), &frame) {
                warn!("the capture preview was not drawn: {error}");
            }
        });
        Self { area, state }
    }

    pub(super) fn widget(&self) -> &gtk::DrawingArea {
        &self.area
    }

    pub(super) fn set_settings(&self, settings: Settings) {
        self.state.borrow_mut().settings = settings;
        self.retarget();
    }

    pub(super) fn spotlight(&self, part: Option<Part>) {
        self.state.borrow_mut().spot = part;
        self.retarget();
    }

    pub(super) fn preview_after(&self, after: Option<AfterSelection>) {
        self.state.borrow_mut().hover = after;
        self.retarget();
    }

    fn retarget(&self) {
        let now = glib::monotonic_time();
        let animate = self.area.settings().is_gtk_enable_animations() && self.area.is_mapped();
        let mut state = self.state.borrow_mut();
        state.retarget(now, animate);
        let start = !state.ticking && !state.settled(now);
        if start {
            state.ticking = true;
        }
        drop(state);
        if start {
            let state = self.state.clone();
            self.area.add_tick_callback(move |area, _| {
                area.queue_draw();
                let mut state = state.borrow_mut();
                if state.settled(glib::monotonic_time()) {
                    state.ticking = false;
                    return glib::ControlFlow::Break;
                }
                glib::ControlFlow::Continue
            });
        }
        self.area.queue_draw();
    }
}

pub(super) fn choice_art(after: AfterSelection) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .content_height(ART_HEIGHT)
        .hexpand(true)
        .css_classes(["choice-art"])
        .build();
    area.set_draw_func(move |_, cr, width, height| {
        if let Err(error) = paint_art(cr, f64::from(width), f64::from(height), after) {
            warn!("the capture choice art was not drawn: {error}");
        }
    });
    area
}

pub(super) fn radio(button: &gtk::ToggleButton) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .content_width(RADIO_SIZE)
        .content_height(RADIO_SIZE)
        .valign(gtk::Align::Center)
        .css_classes(["radio"])
        .build();
    let weak = button.downgrade();
    area.set_draw_func(move |_, cr, width, height| {
        let checked = weak.upgrade().is_some_and(|button| button.is_active());
        let centre = (f64::from(width) / 2.0, f64::from(height) / 2.0);
        if let Err(error) = paint_radio(cr, centre, checked) {
            warn!("the choice radio was not drawn: {error}");
        }
    });
    let redraw = area.clone();
    button.connect_toggled(move |_| redraw.queue_draw());
    area
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layer {
    Snap,
    Edit,
    Finish,
    Loupe,
    Hints,
}

impl Layer {
    const ALL: [Self; LAYERS] = [
        Self::Snap,
        Self::Edit,
        Self::Finish,
        Self::Loupe,
        Self::Hints,
    ];

    fn index(self) -> usize {
        self as usize
    }

    fn part(self) -> Part {
        match self {
            Self::Snap => Part::Snap,
            Self::Edit | Self::Finish => Part::After,
            Self::Loupe => Part::Loupe,
            Self::Hints => Part::Hints,
        }
    }

    fn shown(self, settings: Settings, after: AfterSelection) -> bool {
        match self {
            Self::Snap => settings.snap,
            Self::Edit => after == AfterSelection::Edit,
            Self::Finish => after == AfterSelection::Finish,
            Self::Loupe => settings.magnifier,
            Self::Hints => settings.hints,
        }
    }
}

#[derive(Clone, Copy)]
struct Fade {
    from: f64,
    to: f64,
    started: i64,
}

impl Fade {
    fn still(value: f64) -> Self {
        Self {
            from: value,
            to: value,
            started: 0,
        }
    }

    fn value(self, now: i64) -> f64 {
        let progress = ((now - self.started) as f64 / FADE_US).clamp(0.0, 1.0);
        self.from + (self.to - self.from) * ease_out(progress)
    }

    fn settled(self, now: i64) -> bool {
        same(self.from, self.to) || (now - self.started) as f64 >= FADE_US
    }

    fn go(&mut self, to: f64, now: i64, animate: bool) {
        if same(self.to, to) {
            return;
        }
        let from = if animate { self.value(now) } else { to };
        *self = Self {
            from,
            to,
            started: now,
        };
    }
}

fn ease_out(progress: f64) -> f64 {
    1.0 - (1.0 - progress).powi(3)
}

fn same(left: f64, right: f64) -> bool {
    (left - right).abs() < 1e-9
}

struct State {
    settings: Settings,
    hover: Option<AfterSelection>,
    spot: Option<Part>,
    opacity: [Fade; LAYERS],
    presence: [Fade; LAYERS],
    edit: Fade,
    ticking: bool,
}

struct Frame {
    opacity: [f64; LAYERS],
    presence: [f64; LAYERS],
    edit: f64,
}

impl State {
    fn new(settings: Settings) -> Self {
        let mut state = Self {
            settings,
            hover: None,
            spot: None,
            opacity: [Fade::still(0.0); LAYERS],
            presence: [Fade::still(0.0); LAYERS],
            edit: Fade::still(0.0),
            ticking: false,
        };
        state.retarget(0, false);
        state
    }

    fn retarget(&mut self, now: i64, animate: bool) {
        let after = self.hover.unwrap_or(self.settings.after);
        for layer in Layer::ALL {
            let shown = layer.shown(self.settings, after);
            let lit = self.spot.is_none_or(|spot| spot == layer.part());
            let opacity = match (shown, lit) {
                (false, _) => 0.0,
                (true, true) => 1.0,
                (true, false) => DIMMED,
            };
            self.opacity[layer.index()].go(opacity, now, animate);
            self.presence[layer.index()].go(if shown { 1.0 } else { 0.0 }, now, animate);
        }
        let edit = if after == AfterSelection::Edit {
            1.0
        } else {
            0.0
        };
        self.edit.go(edit, now, animate);
    }

    fn settled(&self, now: i64) -> bool {
        self.opacity
            .iter()
            .chain(&self.presence)
            .chain(std::iter::once(&self.edit))
            .all(|fade| fade.settled(now))
    }

    fn frame(&self, now: i64) -> Frame {
        Frame {
            opacity: self.opacity.map(|fade| fade.value(now)),
            presence: self.presence.map(|fade| fade.value(now)),
            edit: self.edit.value(now),
        }
    }
}

#[derive(Clone, Copy)]
struct Colour {
    red: f64,
    green: f64,
    blue: f64,
    alpha: f64,
}

const fn rgb(hex: u32) -> Colour {
    rgba(hex, 1.0)
}

const fn rgba(hex: u32, alpha: f64) -> Colour {
    Colour {
        red: ((hex >> 16) & 0xff) as f64 / 255.0,
        green: ((hex >> 8) & 0xff) as f64 / 255.0,
        blue: (hex & 0xff) as f64 / 255.0,
        alpha,
    }
}

const fn clear(colour: Colour) -> Colour {
    Colour {
        alpha: 0.0,
        ..colour
    }
}

const GROUND: Colour = rgb(0x040404);
const LIGHT: Colour = rgb(0xebebeb);
const RAISED: Colour = rgb(0x0f0f0f);
const RAISED_HI: Colour = rgb(0x161616);
const STROKE: Colour = rgb(0x0d0d0d);
const STROKE_HI: Colour = rgb(0x0f0f0f);
const STROKE_TOP: Colour = rgb(0x161616);
const FG: Colour = rgb(0xd3d3d3);
const INK: Colour = rgb(0xc6c6c6);
const MUTED: Colour = rgba(0xc6c6c6, 0.8);
const ACCENT: Colour = rgb(0xfe5101);
const ACCENT_WASH: Colour = rgba(0xfe5101, 0.14);
const OK: Colour = rgb(0x5cc48e);
const OK_INK: Colour = rgb(0x0b2416);
const WHITE: Colour = rgb(0xffffff);
const WALL_TOP: Colour = rgb(0x040404);
const WALL_BOTTOM: Colour = rgb(0x020202);
const GLOW: Colour = rgba(0xfe5101, 0.24);
const SHEEN: Colour = rgba(0xd3d3d3, 0.06);
const TITLE_BAR: Colour = rgb(0x0d0d0d);
const TITLE_LINE: Colour = rgb(0x141414);
const TITLE_DOT: Colour = rgb(0x292929);
const SIDE_LINE: Colour = rgb(0x1b1b1b);
const SIDE_ACCENT: Colour = rgb(0xb33a02);
const HEADLINE: Colour = rgb(0x3b3b3b);
const BLOCK: Colour = rgb(0x0f0f0f);
const CHIP: Colour = rgba(0x040404, 0.88);
const PANEL: Colour = rgba(0x040404, 0.96);
const HINTS_BAR: Colour = rgba(0x040404, 0.92);
const SHADE: Colour = rgba(0x000000, 0.6);
const ART_SHADE: Colour = rgba(0x040404, 0.64);
const SNAP_LINE: Colour = rgba(0xd3d3d3, 0.75);
const SNAP_FILL: Colour = rgba(0xd3d3d3, 0.07);
const GRID: Colour = rgba(0x000000, 0.28);
const CURSOR_HALO: Colour = rgba(0x000000, 0.5);
const SQUARE_HALO: Colour = rgba(0x000000, 0.45);
const SUN_CORE: Colour = rgb(0xfff1e2);
const SUN_HALO: Colour = rgb(0xffcf9f);
const HILLS_FAR: Colour = rgb(0x3a1606);
const HILLS_NEAR: Colour = rgb(0x120a06);
const TOOL_DOT: Colour = rgb(0x3b3b3b);
const NOTE_LINE: Colour = rgb(0x494949);
const NOTE_LINE_DIM: Colour = rgb(0x2b2b2b);

const SUNSET: [(f64, u32); 5] = [
    (0.0, 0x1d130e),
    (0.36, 0x5a2208),
    (0.62, 0xc94306),
    (0.80, 0xfe7a2e),
    (1.0, 0xffc9a0),
];
const ART_SUNSET: [(f64, u32); 5] = [
    (0.0, 0x1d130e),
    (0.40, 0x5a2208),
    (0.66, 0xc94306),
    (0.84, 0xfe7a2e),
    (1.0, 0xffc9a0),
];
const LENS_BANDS: [(f64, f64, u32); 4] = [
    (0.0, 0.30, 0xff9a5c),
    (0.30, 0.58, 0xfe5101),
    (0.58, 0.70, 0xa83600),
    (0.70, 1.0, 0x140c08),
];
const FAR_RIDGE: [(f64, f64); 7] = [
    (0.0, 0.46),
    (0.14, 0.30),
    (0.30, 0.52),
    (0.48, 0.24),
    (0.66, 0.48),
    (0.82, 0.34),
    (1.0, 0.50),
];
const NEAR_RIDGE: [(f64, f64); 7] = [
    (0.0, 0.40),
    (0.18, 0.18),
    (0.34, 0.46),
    (0.52, 0.12),
    (0.70, 0.50),
    (0.84, 0.30),
    (1.0, 0.52),
];
const CODE_LINES: [(f64, f64, f64, Colour); 8] = [
    (0.0, 7.0, 0.46, HEADLINE),
    (20.0, 4.0, 0.88, RAISED_HI),
    (30.0, 4.0, 0.76, RAISED_HI),
    (40.0, 4.0, 0.82, RAISED_HI),
    (50.0, 4.0, 0.54, RAISED_HI),
    (66.0, 58.0, 1.0, BLOCK),
    (136.0, 4.0, 0.80, RAISED_HI),
    (146.0, 4.0, 0.64, RAISED_HI),
];
const HINTS: [(&str, &str); 4] = [
    ("F", "Screen"),
    ("W", "Window"),
    ("D", "Draw"),
    ("?", "Hide"),
];
const ACTIONS: [(&str, bool); 4] = [
    ("Accept", true),
    ("Copy", false),
    ("Save", false),
    ("Cancel", false),
];

#[derive(Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Rect {
    fn right(self) -> f64 {
        self.x + self.width
    }

    fn bottom(self) -> f64 {
        self.y + self.height
    }

    fn centre(self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    fn inset(self, by: f64) -> Self {
        Self {
            x: self.x + by,
            y: self.y + by,
            width: self.width - by * 2.0,
            height: self.height - by * 2.0,
        }
    }
}

struct Scene {
    code: Rect,
    photo: Rect,
    selection: Rect,
    loupe: (f64, f64),
}

impl Scene {
    fn new(width: f64) -> Self {
        Self {
            code: Rect {
                x: (width * 0.04).round(),
                y: 26.0,
                width: (width * 0.36).round(),
                height: 212.0,
            },
            photo: Rect {
                x: (width * 0.44).round(),
                y: 46.0,
                width: (width * 0.52).round(),
                height: 226.0,
            },
            selection: Rect {
                x: (width * 0.56).round(),
                y: 96.0,
                width: (width * 0.28).round(),
                height: 116.0,
            },
            loupe: ((width * 0.84).round() + 20.0, 108.0),
        }
    }
}

#[derive(Clone, Copy)]
struct Font {
    family: &'static str,
    size: f64,
}

const CHIP_FONT: Font = Font {
    family: SANS,
    size: 11.5,
};
const KEY_FONT: Font = Font {
    family: SANS,
    size: 10.5,
};
const MONO_FONT: Font = Font {
    family: MONO,
    size: 10.5,
};

#[derive(Clone, Copy)]
enum Lead {
    Dot,
    Square,
    Window,
}

impl Lead {
    fn width(self) -> f64 {
        match self {
            Self::Dot => 6.0,
            Self::Square => 8.0,
            Self::Window => 13.0,
        }
    }
}

#[derive(Clone, Copy)]
struct Chip<'a> {
    text: &'a str,
    font: Font,
    height: f64,
    padding: f64,
    radius: f64,
    fill: Colour,
    border: Option<Colour>,
    ink: Colour,
    lead: Option<Lead>,
}

impl<'a> Chip<'a> {
    fn plain(text: &'a str) -> Self {
        Self {
            text,
            font: CHIP_FONT,
            height: 22.0,
            padding: 8.0,
            radius: 4.0,
            fill: CHIP,
            border: Some(RAISED_HI),
            ink: FG,
            lead: None,
        }
    }

    fn accent(text: &'a str) -> Self {
        Self {
            fill: ACCENT,
            border: None,
            ink: LIGHT,
            ..Self::plain(text)
        }
    }

    fn action(text: &'a str, primary: bool) -> Self {
        let base = Self {
            height: 24.0,
            padding: 9.0,
            fill: PANEL,
            ink: INK,
            ..Self::plain(text)
        };
        if primary {
            Self {
                fill: ACCENT,
                border: None,
                ink: LIGHT,
                ..base
            }
        } else {
            base
        }
    }
}

#[derive(Clone, Copy)]
enum Icon {
    Box,
    Arrow,
    Pen,
    Text,
    Step,
    Pixelate,
    Window,
    Check,
}

const TOOLS: [Icon; 6] = [
    Icon::Box,
    Icon::Arrow,
    Icon::Pen,
    Icon::Text,
    Icon::Step,
    Icon::Pixelate,
];

fn paint(
    area: &gtk::DrawingArea,
    cr: &cairo::Context,
    width: f64,
    height: f64,
    frame: &Frame,
) -> Result<(), cairo::Error> {
    let scene = Scene::new(width);
    let all = Rect {
        x: 0.0,
        y: 0.0,
        width,
        height,
    };
    let sel = scene.selection;
    wall(cr, all)?;
    code_window(cr, scene.code)?;
    photo_window(cr, scene.photo)?;
    shade(cr, all, sel, SHADE)?;
    fade(cr, frame, Layer::Snap, scene.code.centre(), |cr| {
        snap(cr, area, scene.code)
    })?;
    selection(cr, sel)?;
    dims(cr, area, sel, frame.edit)?;
    fade(
        cr,
        frame,
        Layer::Edit,
        (sel.x + sel.width / 2.0, sel.y - 23.0),
        |cr| toolbar(cr, sel),
    )?;
    fade(
        cr,
        frame,
        Layer::Edit,
        (sel.right() - 90.0, sel.bottom() + 20.0),
        |cr| actions(cr, area, sel),
    )?;
    fade(
        cr,
        frame,
        Layer::Finish,
        (sel.right() - 70.0, sel.bottom() + 19.0),
        |cr| release(cr, area, sel),
    )?;
    cursor(cr, (sel.right(), sel.bottom()))?;
    fade(
        cr,
        frame,
        Layer::Loupe,
        (scene.loupe.0 + 35.0, scene.loupe.1 + 42.0),
        |cr| loupe(cr, area, scene.loupe),
    )?;
    fade(
        cr,
        frame,
        Layer::Hints,
        (width / 2.0, height - 27.0),
        |cr| hints(cr, area, all),
    )?;
    frozen(cr, area, width)
}

fn fade(
    cr: &cairo::Context,
    frame: &Frame,
    layer: Layer,
    centre: (f64, f64),
    draw: impl FnOnce(&cairo::Context) -> Result<(), cairo::Error>,
) -> Result<(), cairo::Error> {
    let opacity = frame.opacity[layer.index()];
    if opacity <= 0.0 {
        return Ok(());
    }
    let scale = SHRUNK + (1.0 - SHRUNK) * frame.presence[layer.index()];
    cr.save()?;
    cr.translate(centre.0, centre.1);
    cr.scale(scale, scale);
    cr.translate(-centre.0, -centre.1);
    if opacity >= 1.0 {
        draw(cr)?;
    } else {
        cr.push_group();
        draw(cr)?;
        cr.pop_group_to_source()?;
        cr.paint_with_alpha(opacity)?;
    }
    cr.restore()
}

fn wall(cr: &cairo::Context, all: Rect) -> Result<(), cairo::Error> {
    let base = cairo::LinearGradient::new(0.0, 0.0, 0.0, all.height);
    add_stop(&base, 0.0, WALL_TOP);
    add_stop(&base, 1.0, WALL_BOTTOM);
    cr.rectangle(all.x, all.y, all.width, all.height);
    cr.set_source(&base)?;
    cr.fill()?;
    radial(
        cr,
        all,
        (0.0, 0.0),
        (all.width * 0.6, all.height * 0.7),
        SHEEN,
    )?;
    radial(
        cr,
        all,
        (all.width * 0.96, all.height * 1.1),
        (all.width * 0.7, all.height * 0.9),
        GLOW,
    )
}

fn radial(
    cr: &cairo::Context,
    area: Rect,
    centre: (f64, f64),
    radii: (f64, f64),
    colour: Colour,
) -> Result<(), cairo::Error> {
    cr.save()?;
    cr.rectangle(area.x, area.y, area.width, area.height);
    cr.clip();
    cr.translate(centre.0, centre.1);
    cr.scale(radii.0, radii.1);
    let gradient = cairo::RadialGradient::new(0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
    add_stop(&gradient, 0.0, colour);
    add_stop(&gradient, 0.7, clear(colour));
    cr.set_source(&gradient)?;
    cr.paint()?;
    cr.restore()
}

fn window_frame(cr: &cairo::Context, win: Rect) -> Result<Rect, cairo::Error> {
    shadow(cr, win, 7.0, 18.0, 40.0, 0.45)?;
    fill_rounded(cr, win, 7.0, GROUND)?;
    rounded(cr, win.inset(0.5), 6.5);
    stroke(cr, RAISED, 1.0)?;
    let inner = win.inset(1.0);
    cr.save()?;
    rounded(cr, inner, 6.0);
    cr.clip();
    fill_rect(
        cr,
        Rect {
            height: 19.0,
            ..inner
        },
        TITLE_BAR,
    )?;
    fill_rect(
        cr,
        Rect {
            y: inner.y + 19.0,
            height: 1.0,
            ..inner
        },
        TITLE_LINE,
    )?;
    for dot in 0..3 {
        cr.arc(
            inner.x + 11.0 + 10.0 * f64::from(dot),
            inner.y + 10.0,
            3.0,
            0.0,
            TAU,
        );
        fill(cr, TITLE_DOT)?;
    }
    fill_rounded(
        cr,
        Rect {
            x: inner.x + 46.0,
            y: inner.y + 7.5,
            width: 54.0,
            height: 5.0,
        },
        2.5,
        RAISED_HI,
    )?;
    cr.restore()?;
    Ok(inner)
}

fn code_window(cr: &cairo::Context, win: Rect) -> Result<(), cairo::Error> {
    let inner = window_frame(cr, win)?;
    cr.save()?;
    rounded(cr, inner, 6.0);
    cr.clip();
    let side = Rect {
        x: inner.x,
        y: inner.y + 20.0,
        width: (inner.width * 0.28).round(),
        height: inner.height - 20.0,
    };
    fill_rect(cr, side, GROUND)?;
    fill_rect(
        cr,
        Rect {
            x: side.right() - 1.0,
            width: 1.0,
            ..side
        },
        TITLE_LINE,
    )?;
    for line in 0..5 {
        fill_rect(
            cr,
            Rect {
                x: side.x + 8.0,
                y: side.y + 12.0 + 14.0 * f64::from(line),
                width: side.width - 18.0,
                height: 5.0,
            },
            SIDE_LINE,
        )?;
    }
    fill_rounded(
        cr,
        Rect {
            x: side.x + 8.0,
            y: side.y + 26.0,
            width: (side.width * 0.58).round(),
            height: 5.0,
        },
        2.0,
        SIDE_ACCENT,
    )?;
    let lines = Rect {
        x: side.right() + 12.0,
        y: inner.y + 34.0,
        width: inner.right() - 14.0 - (side.right() + 12.0),
        height: inner.height - 48.0,
    };
    for &(top, tall, span, colour) in &CODE_LINES {
        fill_rect(
            cr,
            Rect {
                x: lines.x,
                y: lines.y + top,
                width: (lines.width * span).round(),
                height: tall,
            },
            colour,
        )?;
    }
    cr.restore()
}

fn photo_window(cr: &cairo::Context, win: Rect) -> Result<(), cairo::Error> {
    let inner = window_frame(cr, win)?;
    let pic = Rect {
        x: inner.x + 8.0,
        y: inner.y + 28.0,
        width: inner.width - 16.0,
        height: inner.height - 36.0,
    };
    sunset(cr, pic, 4.0, &SUNSET)?;
    cr.save()?;
    rounded(cr, pic, 4.0);
    cr.clip();
    sun(
        cr,
        (
            (pic.x + pic.width * 0.46 + 23.0).round(),
            (pic.y + pic.height * 0.30 + 23.0).round(),
        ),
    )?;
    hills(cr, pic, 0.58, &FAR_RIDGE, HILLS_FAR)?;
    hills(cr, pic, 0.44, &NEAR_RIDGE, HILLS_NEAR)?;
    cr.restore()
}

fn sunset(
    cr: &cairo::Context,
    rect: Rect,
    radius: f64,
    stops: &[(f64, u32)],
) -> Result<(), cairo::Error> {
    let gradient = cairo::LinearGradient::new(rect.x, rect.y, rect.x, rect.bottom());
    for &(offset, hex) in stops {
        add_stop(&gradient, offset, rgb(hex));
    }
    rounded(cr, rect, radius);
    cr.set_source(&gradient)?;
    cr.fill()
}

fn sun(cr: &cairo::Context, centre: (f64, f64)) -> Result<(), cairo::Error> {
    let (cx, cy) = centre;
    let reach = 23.0 * SQRT_2;
    let gradient = cairo::RadialGradient::new(cx, cy, 0.0, cx, cy, reach);
    add_stop(&gradient, 0.0, SUN_CORE);
    add_stop(&gradient, 0.38, SUN_CORE);
    add_stop(&gradient, 0.58, SUN_HALO);
    add_stop(&gradient, 0.72, clear(SUN_HALO));
    cr.arc(cx, cy, 23.0, 0.0, TAU);
    cr.set_source(&gradient)?;
    cr.fill()
}

fn hills(
    cr: &cairo::Context,
    pic: Rect,
    share: f64,
    ridge: &[(f64, f64)],
    colour: Colour,
) -> Result<(), cairo::Error> {
    let tall = pic.height * share;
    let top = pic.bottom() - tall;
    cr.new_sub_path();
    for (index, &(across, down)) in ridge.iter().enumerate() {
        let point = (pic.x + pic.width * across, top + tall * down);
        if index == 0 {
            cr.move_to(point.0, point.1);
        } else {
            cr.line_to(point.0, point.1);
        }
    }
    cr.line_to(pic.right(), pic.bottom());
    cr.line_to(pic.x, pic.bottom());
    cr.close_path();
    fill(cr, colour)
}

fn shade(cr: &cairo::Context, area: Rect, hole: Rect, colour: Colour) -> Result<(), cairo::Error> {
    cr.save()?;
    cr.set_fill_rule(cairo::FillRule::EvenOdd);
    cr.rectangle(area.x, area.y, area.width, area.height);
    cr.rectangle(hole.x, hole.y, hole.width, hole.height);
    fill(cr, colour)?;
    cr.restore()
}

fn snap(cr: &cairo::Context, area: &gtk::DrawingArea, win: Rect) -> Result<(), cairo::Error> {
    fill_rounded(cr, win, 7.0, SNAP_FILL)?;
    rounded(cr, win.inset(0.75), 6.25);
    stroke(cr, SNAP_LINE, 1.5)?;
    let chip = Chip {
        lead: Some(Lead::Window),
        ..Chip::plain("Click to take this window")
    };
    draw_chip(cr, area, chip, win.x + 8.0, win.bottom() - 30.0).map(drop)
}

fn selection(cr: &cairo::Context, sel: Rect) -> Result<(), cairo::Error> {
    let edge = sel.inset(0.5);
    cr.rectangle(edge.x, edge.y, edge.width, edge.height);
    stroke(cr, ACCENT, 1.0)?;
    for (x, y) in [
        (sel.x, sel.y),
        (sel.right(), sel.y),
        (sel.x, sel.bottom()),
        (sel.right(), sel.bottom()),
    ] {
        let handle = Rect {
            x: x - 3.0,
            y: y - 3.0,
            width: 7.0,
            height: 7.0,
        };
        fill_rounded(cr, handle, 2.0, LIGHT)?;
        rounded(cr, handle.inset(0.5), 1.5);
        stroke(cr, ACCENT, 1.0)?;
    }
    Ok(())
}

fn dims(
    cr: &cairo::Context,
    area: &gtk::DrawingArea,
    sel: Rect,
    edit: f64,
) -> Result<(), cairo::Error> {
    let finish_at = (sel.x, sel.y - 28.0);
    let edit_at = (sel.x + 7.0, sel.y + 7.0);
    let x = lerp(finish_at.0, edit_at.0, edit).round();
    let y = lerp(finish_at.1, edit_at.1, edit).round();
    let chip = if edit > 0.5 {
        Chip::plain("412 × 268")
    } else {
        Chip::accent("412 × 268")
    };
    draw_chip(cr, area, chip, x, y).map(drop)
}

fn lerp(from: f64, to: f64, by: f64) -> f64 {
    from + (to - from) * by
}

fn toolbar(cr: &cairo::Context, sel: Rect) -> Result<(), cairo::Error> {
    let width = 150.0;
    let bar = Rect {
        x: (sel.x + sel.width / 2.0 - width / 2.0).round(),
        y: sel.y - 38.0,
        width,
        height: 30.0,
    };
    shadow(cr, bar, 6.0, 8.0, 24.0, 0.5)?;
    fill_rounded(cr, bar, 6.0, PANEL)?;
    rounded(cr, bar.inset(0.5), 5.5);
    stroke(cr, RAISED_HI, 1.0)?;
    for (index, tool) in TOOLS.iter().enumerate() {
        let cell = Rect {
            x: bar.x + 4.0 + 24.0 * index as f64,
            y: bar.y + 4.0,
            width: 22.0,
            height: 22.0,
        };
        let colour = if index == 0 {
            fill_rounded(cr, cell, 4.0, ACCENT_WASH)?;
            ACCENT
        } else {
            INK
        };
        icon(cr, *tool, (cell.x + 4.5, cell.y + 4.5), 13.0, colour)?;
    }
    Ok(())
}

fn actions(cr: &cairo::Context, area: &gtk::DrawingArea, sel: Rect) -> Result<(), cairo::Error> {
    let widths: Vec<f64> = ACTIONS
        .iter()
        .map(|&(text, primary)| chip_width(area, Chip::action(text, primary)))
        .collect();
    let total = widths.iter().sum::<f64>() + 4.0 * (ACTIONS.len() - 1) as f64;
    let mut x = sel.right() - total;
    for (&(text, primary), width) in ACTIONS.iter().zip(widths) {
        draw_chip(
            cr,
            area,
            Chip::action(text, primary),
            x.round(),
            sel.bottom() + 8.0,
        )?;
        x += width + 4.0;
    }
    Ok(())
}

fn release(cr: &cairo::Context, area: &gtk::DrawingArea, sel: Rect) -> Result<(), cairo::Error> {
    let chip = Chip::plain("Let go to copy and save");
    let x = sel.right() - chip_width(area, chip);
    draw_chip(cr, area, chip, x, sel.bottom() + 8.0).map(drop)
}

fn cursor(cr: &cairo::Context, centre: (f64, f64)) -> Result<(), cairo::Error> {
    let (cx, cy) = centre;
    let arms = |cr: &cairo::Context| {
        cr.move_to(cx, cy - 8.0);
        cr.line_to(cx, cy - 3.0);
        cr.move_to(cx, cy + 3.0);
        cr.line_to(cx, cy + 8.0);
        cr.move_to(cx - 8.0, cy);
        cr.line_to(cx - 3.0, cy);
        cr.move_to(cx + 3.0, cy);
        cr.line_to(cx + 8.0, cy);
    };
    cr.set_line_cap(cairo::LineCap::Round);
    arms(cr);
    stroke(cr, CURSOR_HALO, 3.5)?;
    arms(cr);
    stroke(cr, WHITE, 1.5)
}

fn loupe(cr: &cairo::Context, area: &gtk::DrawingArea, at: (f64, f64)) -> Result<(), cairo::Error> {
    let chip = Chip {
        font: MONO_FONT,
        height: 20.0,
        padding: 6.0,
        lead: Some(Lead::Square),
        ..Chip::plain("#FE5101")
    };
    let chip_width = chip_width(area, chip);
    let column = chip_width.max(60.0);
    let cx = (at.0 + column / 2.0).round();
    lens(cr, (cx, at.1 + 30.0))?;
    draw_chip(cr, area, chip, (cx - chip_width / 2.0).round(), at.1 + 65.0).map(drop)
}

fn lens(cr: &cairo::Context, centre: (f64, f64)) -> Result<(), cairo::Error> {
    let (cx, cy) = centre;
    for step in 1..=4 {
        cr.arc(cx, cy + 8.0, 30.0 + 2.5 * f64::from(step), 0.0, TAU);
        fill(cr, rgba(0x000000, 0.55 / 4.0))?;
    }
    cr.save()?;
    cr.arc(cx, cy, 28.0, 0.0, TAU);
    cr.clip();
    let gradient = cairo::LinearGradient::new(cx - 28.0, cy - 28.0, cx + 28.0, cy + 28.0);
    for &(from, to, hex) in &LENS_BANDS {
        add_stop(&gradient, from, rgb(hex));
        add_stop(&gradient, to, rgb(hex));
    }
    cr.set_source(&gradient)?;
    cr.paint()?;
    let mut offset = 0.0;
    while offset < 56.0 {
        cr.rectangle(cx - 28.0 + offset, cy - 28.0, 1.0, 56.0);
        cr.rectangle(cx - 28.0, cy - 28.0 + offset, 56.0, 1.0);
        offset += 7.5;
    }
    fill(cr, GRID)?;
    cr.restore()?;
    cr.arc(cx, cy, 29.0, 0.0, TAU);
    stroke(cr, LIGHT, 2.0)?;
    cr.rectangle(cx - 4.5, cy - 4.5, 9.0, 9.0);
    stroke(cr, SQUARE_HALO, 1.0)?;
    cr.rectangle(cx - 3.25, cy - 3.25, 6.5, 6.5);
    stroke(cr, LIGHT, 1.5)
}

fn hints(cr: &cairo::Context, area: &gtk::DrawingArea, all: Rect) -> Result<(), cairo::Error> {
    let items: Vec<(pango::Layout, f64, pango::Layout, f64)> = HINTS
        .iter()
        .map(|&(key, label)| {
            let key_layout = layout(area, key, KEY_FONT);
            let key_width = (text_size(&key_layout).0 + 12.0).max(18.0).ceil();
            let label_layout = layout(area, label, CHIP_FONT);
            let label_width = text_size(&label_layout).0.ceil();
            (key_layout, key_width, label_layout, label_width)
        })
        .collect();
    let content: f64 = items
        .iter()
        .map(|(_, key_width, _, label_width)| key_width + LEAD_GAP + label_width)
        .sum();
    let total = 2.0 + 6.0 + 12.0 + content + 12.0 * (items.len() - 1) as f64;
    let bar = Rect {
        x: ((all.width - total) / 2.0).round(),
        y: all.height - 42.0,
        width: total,
        height: 30.0,
    };
    fill_rounded(cr, bar, 8.0, HINTS_BAR)?;
    rounded(cr, bar.inset(0.5), 7.5);
    stroke(cr, RAISED_HI, 1.0)?;
    let mut x = bar.x + 7.0;
    for (key_layout, key_width, label_layout, label_width) in &items {
        keycap(
            cr,
            key_layout,
            Rect {
                x,
                y: bar.y + 6.0,
                width: *key_width,
                height: 18.0,
            },
        )?;
        x += key_width + LEAD_GAP;
        let (_, label_height) = text_size(label_layout);
        show_text(
            cr,
            label_layout,
            x,
            (bar.y + (bar.height - label_height) / 2.0).round(),
            MUTED,
        );
        x += label_width + 12.0;
    }
    Ok(())
}

fn keycap(cr: &cairo::Context, layout: &pango::Layout, cap: Rect) -> Result<(), cairo::Error> {
    fill_rounded(cr, cap, 4.0, RAISED)?;
    fill_rect(
        cr,
        Rect {
            x: cap.x + 1.0,
            y: cap.bottom() - 2.0,
            width: cap.width - 2.0,
            height: 1.0,
        },
        STROKE,
    )?;
    rounded(cr, cap.inset(0.5), 3.5);
    stroke(cr, STROKE_HI, 1.0)?;
    let (text_width, text_height) = text_size(layout);
    show_text(
        cr,
        layout,
        (cap.x + (cap.width - text_width) / 2.0).round(),
        (cap.y + (cap.height - text_height) / 2.0).round(),
        INK,
    );
    Ok(())
}

fn frozen(cr: &cairo::Context, area: &gtk::DrawingArea, width: f64) -> Result<(), cairo::Error> {
    let chip = Chip {
        ink: MUTED,
        lead: Some(Lead::Dot),
        ..Chip::plain("Frozen frame")
    };
    let x = width - 10.0 - chip_width(area, chip);
    draw_chip(cr, area, chip, x, 10.0).map(drop)
}

fn paint_art(
    cr: &cairo::Context,
    width: f64,
    height: f64,
    after: AfterSelection,
) -> Result<(), cairo::Error> {
    let all = Rect {
        x: 0.0,
        y: 0.0,
        width,
        height,
    };
    cr.save()?;
    rounded(cr, all, 6.0);
    cr.clip();
    sunset(cr, all, 0.0, &ART_SUNSET)?;
    let sel = Rect {
        x: (width * 0.33).round(),
        y: (height * 0.30).round(),
        width: (width * 0.34).round(),
        height: (height * 0.40).round(),
    };
    shade(cr, all, sel, ART_SHADE)?;
    let edge = sel.inset(0.5);
    cr.rectangle(edge.x, edge.y, edge.width, edge.height);
    stroke(cr, ACCENT, 1.0)?;
    match after {
        AfterSelection::Edit => art_tools(cr, sel, width)?,
        AfterSelection::Finish => art_note(cr, sel, width)?,
    }
    cr.restore()
}

fn art_tools(cr: &cairo::Context, sel: Rect, width: f64) -> Result<(), cairo::Error> {
    let tools = Rect {
        x: (width / 2.0 - 26.5).round(),
        y: sel.y - 18.0,
        width: 53.0,
        height: 13.0,
    };
    fill_rounded(cr, tools, 4.0, BLOCK)?;
    for dot in 0..5 {
        let colour = if dot == 0 { ACCENT } else { TOOL_DOT };
        fill_rounded(
            cr,
            Rect {
                x: tools.x + 3.0 + 10.0 * f64::from(dot),
                y: tools.y + 3.0,
                width: 7.0,
                height: 7.0,
            },
            2.0,
            colour,
        )?;
    }
    let mut x = sel.right() - 58.0;
    for (index, bar) in [22.0, 15.0, 15.0].into_iter().enumerate() {
        let colour = if index == 0 { ACCENT } else { RAISED_HI };
        fill_rounded(
            cr,
            Rect {
                x,
                y: sel.bottom() + 5.0,
                width: bar,
                height: 9.0,
            },
            2.0,
            colour,
        )?;
        x += bar + 3.0;
    }
    Ok(())
}

fn art_note(cr: &cairo::Context, sel: Rect, width: f64) -> Result<(), cairo::Error> {
    let (cx, cy) = (sel.right(), sel.bottom());
    fill_rect(
        cr,
        Rect {
            x: cx - 0.75,
            y: cy - 6.5,
            width: 1.5,
            height: 13.0,
        },
        LIGHT,
    )?;
    fill_rect(
        cr,
        Rect {
            x: cx - 6.5,
            y: cy - 0.75,
            width: 13.0,
            height: 1.5,
        },
        LIGHT,
    )?;
    let note = Rect {
        x: width - 82.0,
        y: 8.0,
        width: 74.0,
        height: 26.0,
    };
    shadow(cr, note, 5.0, 6.0, 16.0, 0.4)?;
    fill_rounded(cr, note, 5.0, BLOCK)?;
    rounded(cr, note.inset(0.5), 4.5);
    stroke(cr, RAISED_HI, 1.0)?;
    let (ox, oy) = (note.x + 13.5, note.y + 13.0);
    cr.arc(ox, oy, 6.5, 0.0, TAU);
    fill(cr, OK)?;
    icon(cr, Icon::Check, (ox - 4.5, oy - 4.5), 9.0, OK_INK)?;
    let lines_x = ox + 12.5;
    fill_rounded(
        cr,
        Rect {
            x: lines_x,
            y: oy - 4.5,
            width: 40.0,
            height: 3.0,
        },
        1.5,
        NOTE_LINE,
    )?;
    fill_rounded(
        cr,
        Rect {
            x: lines_x,
            y: oy + 1.5,
            width: 26.0,
            height: 3.0,
        },
        1.5,
        NOTE_LINE_DIM,
    )
}

fn paint_radio(cr: &cairo::Context, centre: (f64, f64), checked: bool) -> Result<(), cairo::Error> {
    let (cx, cy) = centre;
    if checked {
        cr.arc(cx, cy, 8.0, 0.0, TAU);
        fill(cr, ACCENT)?;
        cr.arc(cx, cy, 3.0, 0.0, TAU);
        fill(cr, LIGHT)
    } else {
        cr.arc(cx, cy, 7.0, 0.0, TAU);
        stroke(cr, STROKE_TOP, 2.0)
    }
}

fn chip_width(area: &gtk::DrawingArea, chip: Chip) -> f64 {
    let (text_width, _) = text_size(&layout(area, chip.text, chip.font));
    let lead = chip.lead.map_or(0.0, |lead| lead.width() + LEAD_GAP);
    (chip.padding * 2.0 + 2.0 + lead + text_width).ceil()
}

fn draw_chip(
    cr: &cairo::Context,
    area: &gtk::DrawingArea,
    chip: Chip,
    x: f64,
    y: f64,
) -> Result<f64, cairo::Error> {
    let layout = layout(area, chip.text, chip.font);
    let (text_width, text_height) = text_size(&layout);
    let lead = chip.lead.map_or(0.0, |lead| lead.width() + LEAD_GAP);
    let width = (chip.padding * 2.0 + 2.0 + lead + text_width).ceil();
    let rect = Rect {
        x,
        y,
        width,
        height: chip.height,
    };
    fill_rounded(cr, rect, chip.radius, chip.fill)?;
    if let Some(border) = chip.border {
        rounded(cr, rect.inset(0.5), chip.radius - 0.5);
        stroke(cr, border, 1.0)?;
    }
    let mut cursor = x + 1.0 + chip.padding;
    let middle = y + chip.height / 2.0;
    if let Some(lead) = chip.lead {
        match lead {
            Lead::Dot => {
                cr.arc(cursor + 3.0, middle, 3.0, 0.0, TAU);
                fill(cr, ACCENT)?;
            }
            Lead::Square => {
                fill_rounded(
                    cr,
                    Rect {
                        x: cursor,
                        y: middle - 4.0,
                        width: 8.0,
                        height: 8.0,
                    },
                    2.0,
                    ACCENT,
                )?;
            }
            Lead::Window => icon(cr, Icon::Window, (cursor, middle - 6.5), 13.0, MUTED)?,
        }
        cursor += lead.width() + LEAD_GAP;
    }
    show_text(
        cr,
        &layout,
        cursor,
        (middle - text_height / 2.0).round(),
        chip.ink,
    );
    Ok(width)
}

fn icon(
    cr: &cairo::Context,
    icon: Icon,
    at: (f64, f64),
    size: f64,
    colour: Colour,
) -> Result<(), cairo::Error> {
    cr.save()?;
    cr.translate(at.0, at.1);
    cr.scale(size / ICON_GRID, size / ICON_GRID);
    paint_with(cr, colour);
    cr.set_line_width(1.6);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
    match icon {
        Icon::Box => {
            rounded(
                cr,
                Rect {
                    x: 4.5,
                    y: 6.0,
                    width: 15.0,
                    height: 12.0,
                },
                1.0,
            );
            cr.stroke()?;
        }
        Icon::Arrow => {
            cr.move_to(5.0, 19.0);
            cr.line_to(19.0, 5.0);
            cr.move_to(10.0, 5.0);
            cr.line_to(19.0, 5.0);
            cr.line_to(19.0, 14.0);
            cr.stroke()?;
        }
        Icon::Pen => {
            cr.move_to(4.0, 18.0);
            cr.curve_to(7.0, 12.0, 9.0, 9.0, 11.0, 9.0);
            cr.curve_to(13.0, 9.0, 11.0, 15.0, 13.0, 15.0);
            cr.curve_to(15.0, 15.0, 16.0, 11.0, 20.0, 7.0);
            cr.stroke()?;
        }
        Icon::Text => {
            cr.move_to(6.0, 6.5);
            cr.line_to(6.0, 5.0);
            cr.line_to(18.0, 5.0);
            cr.line_to(18.0, 6.5);
            cr.move_to(12.0, 5.0);
            cr.line_to(12.0, 19.0);
            cr.move_to(9.5, 19.0);
            cr.line_to(14.5, 19.0);
            cr.stroke()?;
        }
        Icon::Step => {
            cr.new_sub_path();
            cr.arc(12.0, 12.0, 8.0, 0.0, TAU);
            cr.move_to(11.0, 9.0);
            cr.line_to(12.5, 8.0);
            cr.line_to(12.5, 16.0);
            cr.stroke()?;
        }
        Icon::Pixelate => {
            for (x, y) in [(4.0, 4.0), (15.0, 4.0), (15.0, 15.0)] {
                cr.rectangle(x, y, 5.0, 5.0);
            }
            cr.stroke()?;
            for (x, y) in [(9.0, 9.0), (4.0, 15.0)] {
                cr.rectangle(x, y, 5.0, 5.0);
            }
            cr.fill()?;
        }
        Icon::Window => {
            rounded(
                cr,
                Rect {
                    x: 3.0,
                    y: 4.5,
                    width: 18.0,
                    height: 15.0,
                },
                2.0,
            );
            cr.move_to(3.0, 9.0);
            cr.line_to(21.0, 9.0);
            cr.move_to(6.5, 6.8);
            cr.line_to(6.51, 6.8);
            cr.move_to(9.0, 6.8);
            cr.line_to(9.01, 6.8);
            cr.stroke()?;
        }
        Icon::Check => {
            cr.set_line_width(3.0);
            cr.move_to(5.0, 12.5);
            cr.line_to(9.5, 17.0);
            cr.line_to(19.0, 7.5);
            cr.stroke()?;
        }
    }
    cr.restore()
}

fn shadow(
    cr: &cairo::Context,
    rect: Rect,
    radius: f64,
    offset: f64,
    blur: f64,
    alpha: f64,
) -> Result<(), cairo::Error> {
    const STEPS: u32 = 4;
    for step in 1..=STEPS {
        let spread = blur / 2.0 * f64::from(step) / f64::from(STEPS);
        let halo = Rect {
            x: rect.x - spread,
            y: rect.y + offset - spread,
            width: rect.width + spread * 2.0,
            height: rect.height + spread * 2.0,
        };
        fill_rounded(
            cr,
            halo,
            radius + spread,
            rgba(0x000000, alpha / f64::from(STEPS)),
        )?;
    }
    Ok(())
}

fn layout(area: &gtk::DrawingArea, text: &str, font: Font) -> pango::Layout {
    let layout = area.create_pango_layout(Some(text));
    let mut description = pango::FontDescription::new();
    description.set_family(font.family);
    description.set_weight(pango::Weight::Medium);
    description.set_absolute_size(font.size * f64::from(pango::SCALE));
    layout.set_font_description(Some(&description));
    layout
}

fn text_size(layout: &pango::Layout) -> (f64, f64) {
    let (_, logical) = layout.pixel_extents();
    (f64::from(logical.width()), f64::from(logical.height()))
}

fn show_text(cr: &cairo::Context, layout: &pango::Layout, x: f64, y: f64, colour: Colour) {
    paint_with(cr, colour);
    cr.move_to(x, y);
    pangocairo::functions::show_layout(cr, layout);
}

fn add_stop(gradient: &cairo::Gradient, offset: f64, colour: Colour) {
    gradient.add_color_stop_rgba(offset, colour.red, colour.green, colour.blue, colour.alpha);
}

fn rounded(cr: &cairo::Context, rect: Rect, radius: f64) {
    let radius = radius.min(rect.width / 2.0).min(rect.height / 2.0).max(0.0);
    let quarter = TAU / 4.0;
    cr.new_sub_path();
    cr.arc(
        rect.right() - radius,
        rect.y + radius,
        radius,
        -quarter,
        0.0,
    );
    cr.arc(
        rect.right() - radius,
        rect.bottom() - radius,
        radius,
        0.0,
        quarter,
    );
    cr.arc(
        rect.x + radius,
        rect.bottom() - radius,
        radius,
        quarter,
        2.0 * quarter,
    );
    cr.arc(
        rect.x + radius,
        rect.y + radius,
        radius,
        2.0 * quarter,
        3.0 * quarter,
    );
    cr.close_path();
}

fn paint_with(cr: &cairo::Context, colour: Colour) {
    cr.set_source_rgba(colour.red, colour.green, colour.blue, colour.alpha);
}

fn fill(cr: &cairo::Context, colour: Colour) -> Result<(), cairo::Error> {
    paint_with(cr, colour);
    cr.fill()
}

fn stroke(cr: &cairo::Context, colour: Colour, width: f64) -> Result<(), cairo::Error> {
    paint_with(cr, colour);
    cr.set_line_width(width);
    cr.stroke()
}

fn fill_rect(cr: &cairo::Context, rect: Rect, colour: Colour) -> Result<(), cairo::Error> {
    cr.rectangle(rect.x, rect.y, rect.width, rect.height);
    fill(cr, colour)
}

fn fill_rounded(
    cr: &cairo::Context,
    rect: Rect,
    radius: f64,
    colour: Colour,
) -> Result<(), cairo::Error> {
    rounded(cr, rect, radius);
    fill(cr, colour)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eases_out_from_zero_to_one() {
        assert!(same(ease_out(0.0), 0.0));
        assert!(same(ease_out(1.0), 1.0));
        assert!(ease_out(0.5) > 0.5);
    }

    #[test]
    fn jumps_without_animation_and_glides_with_it() {
        let mut fade = Fade::still(0.0);
        fade.go(1.0, 1_000, false);
        assert!(same(fade.value(1_000), 1.0));
        assert!(fade.settled(1_000));
        fade.go(0.0, 2_000, true);
        assert!(same(fade.value(2_000), 1.0));
        assert!(!fade.settled(2_000));
        assert!(same(fade.value(2_000 + FADE_US as i64), 0.0));
        assert!(fade.settled(2_000 + FADE_US as i64));
    }

    #[test]
    fn dims_every_part_but_the_spotlit_one() {
        let settings = Settings {
            after: AfterSelection::Edit,
            snap: true,
            hints: true,
            magnifier: false,
        };
        let mut state = State::new(settings);
        state.spot = Some(Part::Hints);
        state.retarget(0, false);
        let frame = state.frame(0);
        assert!(same(frame.opacity[Layer::Hints.index()], 1.0));
        assert!(same(frame.opacity[Layer::Snap.index()], DIMMED));
        assert!(same(frame.opacity[Layer::Edit.index()], DIMMED));
        assert!(same(frame.opacity[Layer::Finish.index()], 0.0));
        assert!(same(frame.opacity[Layer::Loupe.index()], 0.0));
    }

    #[test]
    fn previews_a_hovered_mode_without_changing_the_setting() {
        let settings = Settings {
            after: AfterSelection::Edit,
            snap: true,
            hints: true,
            magnifier: true,
        };
        let mut state = State::new(settings);
        state.hover = Some(AfterSelection::Finish);
        state.retarget(0, false);
        let frame = state.frame(0);
        assert!(same(frame.opacity[Layer::Edit.index()], 0.0));
        assert!(same(frame.opacity[Layer::Finish.index()], 1.0));
        assert!(same(frame.edit, 0.0));
        assert_eq!(state.settings.after, AfterSelection::Edit);
    }
}
