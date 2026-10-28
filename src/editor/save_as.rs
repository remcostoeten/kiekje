use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};

use super::{Editor, Ending, Host, render};
use crate::notify::Notice;
use crate::{config, storage};

impl Editor {
    pub(super) fn save_as(self: &Rc<Self>) {
        let Some((picture, shapes, crop)) = self.begin() else {
            return;
        };
        let editor = self.clone();
        let mode = self.mode;
        let annotated = !shapes.is_empty();
        glib::spawn_future_local(async move {
            let prepared = gio::spawn_blocking(move || {
                let image = render::flatten(&picture, &shapes, crop)?;
                let settings = config::load().storage;
                let base = settings.filename.render(&storage::Naming {
                    mode,
                    stamp: storage::now(),
                    width: image.width,
                    height: image.height,
                });
                let name = format!("{base}.{}", settings.format.extension());
                Ok::<_, gtk::cairo::Error>((image, settings.captures_dir(), name))
            })
            .await;
            let (image, folder, name) = match prepared {
                Ok(Ok(prepared)) => prepared,
                Ok(Err(error)) => {
                    editor.busy.set(false);
                    editor.say(&format!("Not saved: {error}"), true);
                    return;
                }
                Err(_) => {
                    editor.busy.set(false);
                    editor.say("The save worker stopped. Try Save as again.", true);
                    return;
                }
            };
            if let Some(suspend) = editor.suspend.borrow().as_ref() {
                suspend(true);
            }
            let dialog = gtk::FileDialog::builder()
                .title("Save the capture")
                .modal(true)
                .initial_name(name)
                .initial_folder(&gio::File::for_path(folder))
                .build();
            let saved = match dialog.save_future(None::<&gtk::Window>).await {
                Ok(file) => match file.path() {
                    Some(path) => match gio::spawn_blocking(move || {
                        crate::app::store_at(image, mode, annotated, &path)
                    })
                    .await
                    {
                        Ok(notice @ Notice::Saved { .. }) => Some(notice),
                        Ok(Notice::SaveFailed { reason, .. }) => {
                            editor.say(&format!("Not saved: {reason}. Try Save as again."), true);
                            None
                        }
                        Ok(_) | Err(_) => {
                            editor.say("The save worker stopped. Try Save as again.", true);
                            None
                        }
                    },
                    None => {
                        editor.say("Choose a location on this computer.", true);
                        None
                    }
                },
                Err(error) if error.matches(gtk::DialogError::Dismissed) => None,
                Err(error) => {
                    editor.say(
                        &format!("The save dialog failed: {error}. Try again."),
                        true,
                    );
                    None
                }
            };
            editor.busy.set(false);
            if let Some(notice) = saved {
                editor.document.borrow_mut().mark_saved();
                if let Host::Inline(ending) = &editor.host
                    && let Some(end) = ending.take()
                {
                    end(Ending::Saved(notice));
                }
            } else {
                if let Some(suspend) = editor.suspend.borrow().as_ref() {
                    suspend(false);
                }
                editor.canvas.grab_focus();
            }
        });
    }
}
