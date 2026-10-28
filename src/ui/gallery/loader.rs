use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use super::{Gallery, cards};
use crate::library::thumbs;

const TEXTURE_LIMIT: usize = 240;
const WORKERS: usize = 3;

#[derive(Default)]
pub(super) struct Textures {
    loaded: HashMap<Rc<str>, (gdk::Texture, u64)>,
    order: BTreeMap<u64, Rc<str>>,
    clock: u64,
}

impl Textures {
    pub(super) fn get(&mut self, path: &str) -> Option<gdk::Texture> {
        let (texture, stamp) = self.loaded.get_mut(path)?;
        self.clock += 1;
        let key = self.order.remove(stamp)?;
        self.order.insert(self.clock, key);
        *stamp = self.clock;
        Some(texture.clone())
    }

    pub(super) fn put(&mut self, path: &str, texture: gdk::Texture) {
        self.clock += 1;
        let key = match self.loaded.get_mut(path) {
            Some((held, stamp)) => {
                *held = texture;
                let key = self.order.remove(stamp);
                *stamp = self.clock;
                key
            }
            None => {
                let key: Rc<str> = Rc::from(path);
                self.loaded.insert(key.clone(), (texture, self.clock));
                Some(key)
            }
        };
        if let Some(key) = key {
            self.order.insert(self.clock, key);
        }
        while self.loaded.len() > TEXTURE_LIMIT {
            let Some((_, oldest)) = self.order.pop_first() else {
                break;
            };
            self.loaded.remove(&oldest);
        }
    }
}

type Waiter = (glib::WeakRef<gtk::ListItem>, glib::WeakRef<gtk::Picture>);

#[derive(Default)]
pub(super) struct Thumbnails {
    waiting: Vec<String>,
    wanted: HashMap<String, Vec<Waiter>>,
    failed: HashSet<String>,
    running: usize,
}

impl Thumbnails {
    pub(super) fn forget_failures(&mut self) {
        self.failed.clear();
    }
}

fn still_bound(item: &glib::WeakRef<gtk::ListItem>, path: &str) -> bool {
    item.upgrade()
        .is_some_and(|item| cards::bound_path(&item).as_deref() == Some(path))
}

impl Gallery {
    pub(super) fn thumbnail(
        self: &Rc<Self>,
        item: &gtk::ListItem,
        picture: &gtk::Picture,
        path: String,
    ) {
        if let Some(texture) = self.textures.borrow_mut().get(&path) {
            cards::show_texture(picture, &texture);
            return;
        }
        picture.set_paintable(gdk::Paintable::NONE);
        picture.add_css_class("skeleton");
        {
            let mut queue = self.queue.borrow_mut();
            if queue.failed.contains(&path) {
                return;
            }
            let waiter = (item.downgrade(), picture.downgrade());
            match queue.wanted.get_mut(&path) {
                Some(waiters) => waiters.push(waiter),
                None => {
                    queue.wanted.insert(path.clone(), vec![waiter]);
                    queue.waiting.push(path);
                }
            }
        }
        self.pump();
    }

    fn pump(self: &Rc<Self>) {
        loop {
            let path = {
                let mut queue = self.queue.borrow_mut();
                if queue.running >= WORKERS {
                    return;
                }
                let Some(path) = queue.waiting.pop() else {
                    return;
                };
                let live = queue.wanted.get_mut(&path).is_some_and(|waiters| {
                    waiters.retain(|(item, _)| still_bound(item, &path));
                    !waiters.is_empty()
                });
                if !live {
                    queue.wanted.remove(&path);
                    continue;
                }
                queue.running += 1;
                path
            };
            self.start(path);
        }
    }

    fn start(self: &Rc<Self>, path: String) {
        let gallery = self.clone();
        glib::spawn_future_local(async move {
            let source = PathBuf::from(&path);
            let made = gio::spawn_blocking(move || thumbs::texture(&thumbs::dir(), &source)).await;
            let texture = match made {
                Ok(Ok(texture)) => Some(texture),
                Ok(Err(error)) => {
                    warn!("no thumbnail: {error}");
                    None
                }
                Err(_) => {
                    error!("the thumbnail worker stopped");
                    None
                }
            };
            let waiters = {
                let mut queue = gallery.queue.borrow_mut();
                queue.running = queue.running.saturating_sub(1);
                if texture.is_none() {
                    queue.failed.insert(path.clone());
                }
                queue.wanted.remove(&path).unwrap_or_default()
            };
            if let Some(texture) = texture {
                gallery.textures.borrow_mut().put(&path, texture.clone());
                for (item, picture) in waiters {
                    if let Some(picture) = picture.upgrade()
                        && still_bound(&item, &path)
                    {
                        cards::show_texture(&picture, &texture);
                    }
                }
            }
            gallery.pump();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel() -> gdk::Texture {
        gdk::MemoryTexture::new(
            1,
            1,
            gdk::MemoryFormat::R8g8b8a8,
            &glib::Bytes::from_static(&[0, 0, 0, 255]),
            4,
        )
        .upcast()
    }

    #[test]
    fn keeps_only_the_newest_textures() {
        let mut textures = Textures::default();
        for index in 0..=TEXTURE_LIMIT {
            textures.put(&index.to_string(), pixel());
        }
        assert!(textures.get("0").is_none());
        assert!(textures.get(&TEXTURE_LIMIT.to_string()).is_some());
        assert_eq!(textures.loaded.len(), TEXTURE_LIMIT);
        textures.put("1", pixel());
        assert_eq!(textures.order.len(), TEXTURE_LIMIT);
        assert_eq!(textures.loaded.len(), TEXTURE_LIMIT);
    }

    #[test]
    fn drops_the_least_recently_used_texture_first() {
        let mut textures = Textures::default();
        for index in 0..TEXTURE_LIMIT {
            textures.put(&index.to_string(), pixel());
        }
        assert!(textures.get("0").is_some());
        textures.put("fresh", pixel());
        assert!(textures.get("0").is_some());
        assert!(textures.get("1").is_none());
        assert!(textures.get("2").is_some());
        assert_eq!(textures.order.len(), textures.loaded.len());
    }
}
