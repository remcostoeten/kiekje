mod canvas;
mod crop;
mod document;
mod emoji;
mod emoji_index;
mod inline;
mod keys;
mod menu;
mod pixels;
mod render;
mod select;
mod shapes;
mod stamp;
mod text;
mod toolbar;
mod wheel;

use std::cell::{Cell, RefCell};
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{cairo, gdk, gio, glib};

use crate::capture::CaptureMode;
use crate::config;
use crate::library::{self, Library, NewCapture};
use crate::output::{self, Image};
use crate::storage::Naming;
use crate::{storage, ui};
use canvas::View;
use document::{
    Area, DEFAULT_EMOJI, Document, Edit, MAX_SIZE_PX, MIN_OPACITY, MIN_SIZE_PX, Point, Shape,
    Stroke, Tool,
};
use pixels::{LoadError, Picture};
use render::Layers;

pub(super) const TURN_STEP_DEGREES: f64 = 15.0;

pub use document::Color;
pub use inline::{Ending, Finish, Inline};
pub use keys::{DrawAction, DrawKey, DrawingKeys, KeyConflict};

/// What an editor window works on: a saved capture, or a fresh one that is not on disk yet.
pub enum Source {
    File { path: PathBuf, mode: CaptureMode },
    Capture { image: Image, mode: CaptureMode },
}

impl Source {
    fn mode(&self) -> CaptureMode {
        match self {
            Self::File { mode, .. } | Self::Capture { mode, .. } => *mode,
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum SaveError {
    #[error("could not draw the annotations: {0}")]
    Draw(#[from] cairo::Error),
    #[error("could not encode the image: {0}")]
    Encode(io::Error),
    #[error("could not write to {folder}: {source}")]
    Write { folder: PathBuf, source: io::Error },
}

struct Prepared {
    picture: Picture,
    base: Picture,
    hidden: Picture,
}

struct Loaded {
    picture: Picture,
    layers: Layers,
    drawing: config::Drawing,
}

type EndCallback = Box<dyn FnOnce(Ending)>;

enum Host {
    Window(glib::WeakRef<gtk::Window>),
    Inline(RefCell<Option<EndCallback>>),
}

struct Editor {
    host: Host,
    mode: CaptureMode,
    original: RefCell<Option<PathBuf>>,
    picture: Arc<Picture>,
    layers: Layers,
    document: RefCell<Document>,
    tool: Cell<Tool>,
    color: Cell<Color>,
    emoji: RefCell<String>,
    emoji_button: gtk::Button,
    recent_emoji: RefCell<Vec<String>>,
    stamp_at: Cell<Option<Point>>,
    size: gtk::Adjustment,
    opacity: gtk::Adjustment,
    keys: DrawingKeys,
    menu: RefCell<Option<menu::Menu>>,
    remembered: Cell<config::Drawing>,
    remembering: RefCell<Option<glib::SourceId>>,
    draft: RefCell<Vec<Point>>,
    view: Cell<View>,
    canvas: gtk::DrawingArea,
    status: gtk::Label,
    undo: gtk::Button,
    redo: gtk::Button,
    clear: gtk::Button,
    tools: RefCell<Vec<(Tool, gtk::ToggleButton)>>,
    swatches: RefCell<Vec<(Color, gtk::ToggleButton)>>,
    adopting: Cell<bool>,
    pointer: Cell<Option<(f64, f64)>>,
    hud: RefCell<Option<wheel::Hud>>,
    selected: Cell<Option<usize>>,
    also_selected: RefCell<Vec<usize>>,
    turn_hint: Cell<Option<Point>>,
    moving: RefCell<Option<select::Moving>>,
    writer: gtk::Popover,
    entry: gtk::Entry,
    layer: RefCell<Option<(gtk::Fixed, Point)>>,
    bars: RefCell<Option<inline::Bars>>,
    reshaping: RefCell<Option<crop::Reshaping>>,
    typing: RefCell<Option<text::Typing>>,
    busy: Cell<bool>,
    discard: Cell<bool>,
}

/// Opens an editor window for a saved capture or a fresh one.
///
/// Pixels are prepared off the main thread. Annotations never touch a saved original: saving
/// writes a new file next to it. A fresh capture is saved like any capture, through the filename
/// template. Either way the new file joins the library.
///
/// # Examples
///
/// ```ignore
/// editor::open(&app, Source::File { path, mode: CaptureMode::Region });
/// ```
pub fn open(app: &gtk::Application, source: Source) {
    ui::load_stylesheet();
    let mode = source.mode();
    let (title, original, input) = match source {
        Source::File { path, .. } => (
            format!("Edit {}", file_name(&path)),
            Some(path.clone()),
            Input::File(path),
        ),
        Source::Capture { image, .. } => ("Annotate capture".to_owned(), None, Input::Image(image)),
    };
    let window = gtk::Window::builder()
        .application(app)
        .title(title)
        .default_width(1100)
        .default_height(760)
        .css_classes(["settings-window"])
        .build();
    let loading = gtk::Label::builder()
        .label("Loading the capture")
        .vexpand(true)
        .css_classes(["note"])
        .build();
    window.set_child(Some(&panel(&loading)));
    window.present();

    glib::spawn_future_local(async move {
        let prepared = gio::spawn_blocking(move || {
            prepare(input).map(|ready| (ready, config::load().drawing))
        })
        .await;
        let failure = match prepared {
            Ok(Ok((prepared, drawing))) => match Layers::new(prepared.base, prepared.hidden) {
                Ok(layers) => {
                    let loaded = Loaded {
                        picture: prepared.picture,
                        layers,
                        drawing,
                    };
                    Editor::build(&window, mode, original, loaded);
                    return;
                }
                Err(error) => format!("The capture could not be shown: {error}"),
            },
            Ok(Err(error)) => format!("The capture could not be opened: {error}"),
            Err(_) => "The loading worker stopped".to_owned(),
        };
        eprintln!("kiekje: {failure}");
        loading.set_label(&failure);
        loading.add_css_class("problem");
    });
}

enum Input {
    File(PathBuf),
    Image(Image),
}

fn prepare(input: Input) -> Result<Prepared, LoadError> {
    let picture = match input {
        Input::File(path) => Picture::from_image(&pixels::decode(&path)?),
        Input::Image(image) => Picture::from_image(&image),
    };
    let hidden = picture.mosaic(pixels::block_size(picture.width, picture.height));
    Ok(Prepared {
        base: picture.clone(),
        picture,
        hidden,
    })
}

fn panel(child: &impl IsA<gtk::Widget>) -> gtk::Box {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .css_classes(["panel", "editor"])
        .build();
    content.append(child);
    content
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl Editor {
    fn build(window: &gtk::Window, mode: CaptureMode, original: Option<PathBuf>, loaded: Loaded) {
        let editor = Self::new(Host::Window(window.downgrade()), mode, original, loaded);
        let content = panel(&toolbar::build(&editor));
        content.append(&editor.canvas);
        content.append(&toolbar::footer(
            &editor,
            editor.picture.width,
            editor.picture.height,
        ));
        window.set_child(Some(&content));
        editor.sync();
        editor.canvas.grab_focus();

        let keys = gtk::EventControllerKey::new();
        let pressed = Rc::downgrade(&editor);
        keys.connect_key_pressed(move |_, key, _, state| match pressed.upgrade() {
            Some(editor) => editor.key(key, state),
            None => glib::Propagation::Proceed,
        });
        window.add_controller(keys);
        let owner = editor.clone();
        window.connect_close_request(move |_| owner.close_request());
    }

    fn new(host: Host, mode: CaptureMode, original: Option<PathBuf>, loaded: Loaded) -> Rc<Self> {
        let Loaded {
            picture,
            layers,
            drawing,
        } = loaded;
        let document = if original.is_some() {
            Document::default()
        } else {
            Document::unsaved()
        };
        let canvas = gtk::DrawingArea::builder()
            .hexpand(true)
            .vexpand(true)
            .focusable(true)
            .css_classes(["canvas"])
            .build();
        canvas.update_property(&[gtk::accessible::Property::Label("Capture to annotate")]);
        let status = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .label("Drag on the image to annotate. Letters pick a tool, digits a colour, Ctrl+scroll sizes.")
            .css_classes(["note"])
            .build();
        let editor = Rc::new(Self {
            host,
            mode,
            original: RefCell::new(original),
            picture: Arc::new(picture),
            layers,
            document: RefCell::new(document),
            tool: Cell::new(Tool::default()),
            color: Cell::new(drawing.color),
            emoji: RefCell::new(DEFAULT_EMOJI.to_owned()),
            emoji_button: toolbar::emoji_action(DEFAULT_EMOJI),
            recent_emoji: RefCell::new(Vec::new()),
            stamp_at: Cell::new(None),
            size: gtk::Adjustment::new(
                f64::from(drawing.size),
                f64::from(MIN_SIZE_PX),
                f64::from(MAX_SIZE_PX),
                1.0,
                4.0,
                0.0,
            ),
            opacity: gtk::Adjustment::new(
                f64::from(drawing.opacity),
                f64::from(MIN_OPACITY),
                100.0,
                5.0,
                10.0,
                0.0,
            ),
            keys: drawing.keys,
            menu: RefCell::new(None),
            remembered: Cell::new(drawing),
            remembering: RefCell::new(None),
            draft: RefCell::new(Vec::new()),
            view: Cell::new(View::IDENTITY),
            canvas,
            status,
            undo: toolbar::icon_action("kiekje-undo-symbolic", "Undo", "Undo (Ctrl+Z)"),
            redo: toolbar::icon_action("kiekje-redo-symbolic", "Redo", "Redo (Ctrl+Shift+Z)"),
            clear: toolbar::icon_action(
                "kiekje-clear-symbolic",
                "Clear",
                "Remove every annotation",
            ),
            tools: RefCell::new(Vec::new()),
            swatches: RefCell::new(Vec::new()),
            adopting: Cell::new(false),
            pointer: Cell::new(None),
            hud: RefCell::new(None),
            selected: Cell::new(None),
            also_selected: RefCell::new(Vec::new()),
            turn_hint: Cell::new(None),
            moving: RefCell::new(None),
            writer: gtk::Popover::new(),
            entry: gtk::Entry::builder()
                .placeholder_text("Type, then press Enter")
                .width_chars(24)
                .build(),
            layer: RefCell::new(None),
            bars: RefCell::new(None),
            reshaping: RefCell::new(None),
            typing: RefCell::new(None),
            busy: Cell::new(false),
            discard: Cell::new(false),
        });
        canvas::attach(&editor);
        editor.load_recent_emoji();
        text::attach(&editor);
        menu::attach(&editor);
        wheel::attach(&editor);
        for adjustment in [&editor.size, &editor.opacity] {
            let target = Rc::downgrade(&editor);
            adjustment.connect_value_changed(move |_| {
                if let Some(editor) = target.upgrade() {
                    editor.restyle();
                    editor.remember();
                }
            });
        }
        editor
    }

    fn drawing(&self) -> config::Drawing {
        config::Drawing {
            color: self.color.get(),
            size: self.size.value().round() as u32,
            opacity: self.opacity.value().round() as u32,
            keys: self.keys,
        }
    }

    fn remember(self: &Rc<Self>) {
        if self.adopting.get() {
            return;
        }
        let drawing = self.drawing();
        if drawing == self.remembered.replace(drawing) {
            return;
        }
        if let Some(pending) = self.remembering.take() {
            pending.remove();
        }
        let target = Rc::downgrade(self);
        let source = glib::timeout_add_local_once(Duration::from_millis(400), move || {
            if let Some(editor) = target.upgrade() {
                editor.remembering.take();
            }
            gio::spawn_blocking(move || {
                if let Err(error) = config::update(&config::path(), |config| {
                    config.drawing.color = drawing.color;
                    config.drawing.size = drawing.size;
                    config.drawing.opacity = drawing.opacity;
                }) {
                    eprintln!("kiekje: could not remember the drawing style: {error}");
                }
            });
        });
        self.remembering.replace(Some(source));
    }

    fn key_action(&self, key: gdk::Key, shift: bool) -> Option<DrawAction> {
        let typed = key.to_lower().to_unicode()?;
        if shift && typed.is_ascii_alphabetic() {
            return None;
        }
        self.keys.action(typed)
    }

    fn run(&self, action: DrawAction) {
        match action {
            DrawAction::Tool(tool) => self.pick(tool),
            DrawAction::Smaller => self.size.set_value(self.size.value() - 1.0),
            DrawAction::Larger => self.size.set_value(self.size.value() + 1.0),
            DrawAction::TurnLeft => self.turn_selected(-TURN_STEP_DEGREES),
            DrawAction::TurnRight => self.turn_selected(TURN_STEP_DEGREES),
        }
    }

    fn set_color(&self, color: Color) {
        let button = self
            .swatches
            .borrow()
            .iter()
            .find(|(each, _)| *each == color)
            .map(|(_, button)| button.clone());
        match button {
            Some(button) => button.set_active(true),
            None => self.color.set(color),
        }
    }

    fn size(&self) -> (f64, f64) {
        (
            f64::from(self.picture.width),
            f64::from(self.picture.height),
        )
    }

    fn stroke(&self) -> Stroke {
        let drawing = self.drawing();
        Stroke::scaled(
            drawing.color,
            drawing.size,
            drawing.opacity,
            (self.picture.width, self.picture.height),
        )
    }

    fn draft_shape(&self) -> Option<Shape> {
        let draft = self.draft.borrow();
        if draft.is_empty() {
            return None;
        }
        let step = self.document.borrow().next_step();
        self.tool
            .get()
            .shape(&draft, self.stroke(), step, &self.emoji.borrow())
    }

    fn commit(&self) {
        let shape = self.draft_shape();
        self.draft.borrow_mut().clear();
        if let Some(shape) = shape {
            let stamped = self.is_stamp_tool();
            self.document.borrow_mut().apply(Edit::Add(shape));
            self.edited();
            if stamped {
                self.settle_stamp();
            }
        }
        self.canvas.queue_draw();
    }

    fn turn_selected(&self, degrees: f64) {
        let turned = self.replace_selected(|shape| {
            if shape.turns() {
                shape.turned(shape.angle() + degrees)
            } else {
                shape.clone()
            }
        });
        if turned {
            self.canvas.queue_draw();
        } else {
            self.say("Select a text, check, cross or emoji to turn it.", false);
        }
    }

    fn settle_stamp(&self) {
        let last = self.document.borrow().shapes().len().checked_sub(1);
        self.pick(Tool::Select);
        self.selected.set(last);
        self.also_selected.borrow_mut().clear();
        self.say(
            "Placed. Scroll sizes it, arrows move it, comma and period turn it, drag the top handle to turn it freely.",
            false,
        );
    }

    fn cancel_draft(&self) -> bool {
        let had = !self.draft.borrow().is_empty();
        self.draft.borrow_mut().clear();
        self.canvas.queue_draw();
        had
    }

    fn undo(&self) {
        self.finish_text(true);
        self.deselect();
        if self.document.borrow_mut().undo() {
            self.edited();
        }
    }

    fn redo(&self) {
        self.finish_text(true);
        self.deselect();
        if self.document.borrow_mut().redo() {
            self.edited();
        }
    }

    fn clear(&self) {
        self.finish_text(true);
        let shapes = self.document.borrow().shapes().to_vec();
        if shapes.is_empty() {
            return;
        }
        self.deselect();
        self.document.borrow_mut().apply(Edit::Clear(shapes));
        self.edited();
        self.say("Removed every annotation. Ctrl+Z brings them back.", false);
    }

    fn edited(&self) {
        self.discard.set(false);
        self.sync();
        self.place_bars();
        self.canvas.queue_draw();
    }

    fn sync(&self) {
        let document = self.document.borrow();
        self.undo.set_sensitive(document.can_undo());
        self.redo.set_sensitive(document.can_redo());
        self.clear.set_sensitive(!document.shapes().is_empty());
    }

    fn pick(&self, tool: Tool) {
        if let Some((_, button)) = self.tools.borrow().iter().find(|(each, _)| *each == tool) {
            button.set_active(true);
        }
    }

    fn key(self: &Rc<Self>, key: gdk::Key, state: gdk::ModifierType) -> glib::Propagation {
        let control = state.contains(gdk::ModifierType::CONTROL_MASK);
        let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
        let alt = state.contains(gdk::ModifierType::ALT_MASK);
        let letter = key.to_lower().name().unwrap_or_default();
        let step = if shift { 10.0 } else { 1.0 };
        let handled = match (control, letter.as_str()) {
            (false, "Delete" | "BackSpace") => self.delete_selected(),
            (false, "Left") => {
                self.nudge(-step, 0.0)
                    || self.move_stamp_cursor(-step, 0.0)
                    || self.step_region(-step, 0.0, false)
            }
            (false, "Right") => {
                self.nudge(step, 0.0)
                    || self.move_stamp_cursor(step, 0.0)
                    || self.step_region(step, 0.0, false)
            }
            (false, "Up") => {
                self.nudge(0.0, -step)
                    || self.move_stamp_cursor(0.0, -step)
                    || self.step_region(0.0, -step, false)
            }
            (false, "Down") => {
                self.nudge(0.0, step)
                    || self.move_stamp_cursor(0.0, step)
                    || self.step_region(0.0, step, false)
            }
            (true, "Left") => self.step_region(-step, 0.0, true),
            (true, "Right") => self.step_region(step, 0.0, true),
            (true, "Up") => self.step_region(0.0, -step, true),
            (true, "Down") => self.step_region(0.0, step, true),
            _ => false,
        };
        if handled {
            return glib::Propagation::Stop;
        }
        match (control, letter.as_str()) {
            (true, "z") if shift => self.redo(),
            (true, "a") => self.select_all(),
            (true, "z") => self.undo(),
            (true, "y") => self.redo(),
            (true, "c") => self.copy(),
            (true, "s") if shift && self.is_inline() => self.conclude(Finish::SaveAs),
            (true, "s") => self.save(),
            (true, "p") if self.is_inline() => self.conclude(Finish::Pin),
            (false, "Return" | "KP_Enter") if self.is_inline() => self.conclude(Finish::Accept),
            (false, "Escape") => {
                if !(self.close_menu()
                    || self.cancel_reshape()
                    || self.cancel_grab()
                    || self.cancel_draft()
                    || self.deselect())
                {
                    self.dismiss();
                }
            }
            (false, "s") if self.is_inline() => self.pick(Tool::Crop),
            (false, "space") => {
                if !self.place_stamp() {
                    return glib::Propagation::Proceed;
                }
            }
            (false, "colon") => self.open_emoji_picker_with(":"),
            (false, "Menu") => self.open_menu_by_key(),
            (false, "F10") if shift => self.open_menu_by_key(),
            (false, _) if !alt => {
                if let Some(action) = self.key_action(key, shift) {
                    self.run(action);
                } else if let Some(color) = self.color_key(key, shift) {
                    self.set_color(color);
                    self.show_hud(wheel::Dial::Color);
                } else {
                    return glib::Propagation::Proceed;
                }
            }
            _ => return glib::Propagation::Proceed,
        }
        glib::Propagation::Stop
    }

    fn close_request(&self) -> glib::Propagation {
        if !self.document.borrow().is_modified() || self.discard.get() {
            return glib::Propagation::Proceed;
        }
        self.discard.set(true);
        self.say(
            "The annotations are not saved or copied. Close again to discard them.",
            true,
        );
        glib::Propagation::Stop
    }

    fn is_inline(&self) -> bool {
        matches!(self.host, Host::Inline(_))
    }

    fn dismiss(&self) {
        match &self.host {
            Host::Window(window) => {
                if let Some(window) = window.upgrade() {
                    window.close();
                }
            }
            Host::Inline(ending) => {
                self.finish_text(false);
                if let Some(end) = ending.take() {
                    end(Ending::Canceled);
                }
            }
        }
    }

    fn copy(self: &Rc<Self>) {
        if self.is_inline() {
            self.conclude(Finish::Copy);
            return;
        }
        let Some((picture, shapes, crop)) = self.begin() else {
            return;
        };
        let editor = self.clone();
        glib::spawn_future_local(async move {
            let flattened =
                gio::spawn_blocking(move || render::flatten(&picture, &shapes, crop)).await;
            editor.busy.set(false);
            match flattened {
                Ok(Ok(image)) => {
                    let (width, height) = (image.width, image.height);
                    editor.canvas.clipboard().set_texture(&image.into_texture());
                    editor.document.borrow_mut().mark_saved();
                    editor.say(&format!("Copied {width}x{height} to the clipboard"), false);
                }
                Ok(Err(error)) => {
                    editor.say(&format!("Not copied: {}", SaveError::from(error)), true);
                }
                Err(_) => editor.say("The copy worker stopped", true),
            }
        });
    }

    fn save(self: &Rc<Self>) {
        if self.is_inline() {
            self.conclude(Finish::Save);
            return;
        }
        let Some((picture, shapes, crop)) = self.begin() else {
            return;
        };
        let original = self.original.borrow().clone();
        let mode = self.mode;
        let editor = self.clone();
        glib::spawn_future_local(async move {
            let saved = gio::spawn_blocking(move || {
                write(&picture, &shapes, crop, original.as_deref(), mode)
            })
            .await;
            editor.busy.set(false);
            match saved {
                Ok(Ok(path)) => {
                    editor.document.borrow_mut().mark_saved();
                    if editor.original.borrow().is_none() {
                        editor.original.replace(Some(path.clone()));
                    }
                    editor.discard.set(false);
                    editor.say(&format!("Saved {}", storage::tilde(&path)), false);
                }
                Ok(Err(error)) => editor.say(&format!("Not saved: {error}"), true),
                Err(_) => editor.say("The save worker stopped", true),
            }
        });
    }

    fn begin(&self) -> Option<(Arc<Picture>, Vec<Shape>, Option<Area>)> {
        if self.busy.replace(true) {
            return None;
        }
        self.finish_text(true);
        self.cancel_draft();
        let document = self.document.borrow();
        Some((
            self.picture.clone(),
            document.shapes().to_vec(),
            document.crop(),
        ))
    }

    fn say(&self, message: &str, problem: bool) {
        if self.is_inline() {
            self.status.set_visible(problem);
        }
        self.status.set_label(message);
        self.status.set_tooltip_text(Some(message));
        if problem {
            self.status.add_css_class("problem");
        } else {
            self.status.remove_css_class("problem");
        }
    }
}

fn write(
    picture: &Picture,
    shapes: &[Shape],
    crop: Option<Area>,
    original: Option<&Path>,
    mode: CaptureMode,
) -> Result<PathBuf, SaveError> {
    let image = render::flatten(picture, shapes, crop)?;
    let (width, height) = (image.width, image.height);
    let storage = config::load().storage;
    let bytes =
        output::encode(image, storage.format, storage.quality).map_err(SaveError::Encode)?;
    let unsaved = storage::unsaved_dir();
    let (folder, base) = match original {
        Some(original) => {
            let folder = original
                .parent()
                .filter(|parent| *parent != unsaved)
                .map_or_else(|| storage.captures_dir(), Path::to_path_buf);
            let stem = original
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| "kiekje".to_owned());
            (folder, edited_base(&stem))
        }
        None => {
            let base = storage.filename.render(&Naming {
                mode,
                stamp: storage::now(),
                width,
                height,
            });
            (storage.captures_dir(), base)
        }
    };
    let path = storage::save(&bytes, &folder, &base, storage.format).map_err(|source| {
        SaveError::Write {
            folder: folder.clone(),
            source,
        }
    })?;
    let recorded = Library::open(&library::path()).and_then(|library| {
        let source_annotated = match original {
            Some(original) => library.is_annotated(original)?,
            None => false,
        };
        let capture = NewCapture {
            path: path.clone(),
            mode,
            width,
            height,
            bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            created_us: glib::real_time(),
            unsaved: false,
            annotated: annotated_after(source_annotated, shapes.len()),
        };
        library.insert(&capture)
    });
    if let Err(error) = recorded {
        eprintln!("kiekje: the edited capture is saved but not in the library: {error}");
    }
    Ok(path)
}

fn annotated_after(source_annotated: bool, shapes: usize) -> bool {
    source_annotated || shapes > 0
}

fn edited_base(stem: &str) -> String {
    let unnumbered = stem
        .rsplit_once('-')
        .filter(|(_, number)| {
            !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
        })
        .map_or(stem, |(head, _)| head);
    if unnumbered.ends_with("-edited") {
        unnumbered.to_owned()
    } else {
        format!("{stem}-edited")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_an_annotated_source_annotated_after_an_edit() {
        assert!(annotated_after(true, 0));
        assert!(annotated_after(true, 2));
        assert!(annotated_after(false, 1));
        assert!(!annotated_after(false, 0));
    }

    #[test]
    fn names_an_edit_after_its_original() {
        assert_eq!(
            edited_base("kiekje-region-2026-10-04-14-47-59"),
            "kiekje-region-2026-10-04-14-47-59-edited"
        );
    }

    #[test]
    fn reuses_the_name_when_editing_an_edit() {
        assert_eq!(edited_base("shot-edited"), "shot-edited");
        assert_eq!(edited_base("shot-edited-3"), "shot-edited");
    }
}
