use std::cell::Cell;

type Point = (f64, f64);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Anchor {
    pointer: Point,
    point: Point,
}

/// Turns pointer positions into the selection point, slowed down while Shift is held.
///
/// Wayland clients cannot move the system pointer, so precision mode moves a separate point
/// at a fraction of the pointer speed and drops back to the pointer when Shift is released.
pub struct Precision {
    factor: f64,
    anchor: Cell<Option<Anchor>>,
    last: Cell<Option<Anchor>>,
}

impl Precision {
    pub fn new(factor: f64) -> Self {
        Self {
            factor,
            anchor: Cell::new(None),
            last: Cell::new(None),
        }
    }

    /// Maps a desktop logical pointer position to the point the overlay should use.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let point = precision.track((640.0, 320.0), state.contains(gdk::ModifierType::SHIFT_MASK));
    /// ```
    pub fn track(&self, pointer: Point, slow: bool) -> Point {
        let point = if slow {
            let anchor = self.anchor.get().unwrap_or_else(|| {
                self.last.get().unwrap_or(Anchor {
                    pointer,
                    point: pointer,
                })
            });
            self.anchor.set(Some(anchor));
            slowed(anchor, pointer, self.factor)
        } else {
            self.anchor.set(None);
            pointer
        };
        self.last.set(Some(Anchor { pointer, point }));
        point
    }

    pub fn active(&self) -> bool {
        self.anchor.get().is_some()
    }
}

fn slowed(anchor: Anchor, pointer: Point, factor: f64) -> Point {
    (
        anchor.point.0 + (pointer.0 - anchor.pointer.0) * factor,
        anchor.point.1 + (pointer.1 - anchor.pointer.1) * factor,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_the_pointer_without_shift() {
        let precision = Precision::new(0.125);
        assert_eq!(precision.track((10.0, 20.0), false), (10.0, 20.0));
        assert_eq!(precision.track((50.0, 70.0), false), (50.0, 70.0));
        assert!(!precision.active());
    }

    #[test]
    fn moves_an_eighth_of_the_pointer_distance_while_shift_is_held() {
        let precision = Precision::new(0.125);
        precision.track((100.0, 100.0), false);
        assert_eq!(precision.track((116.0, 92.0), true), (102.0, 99.0));
        assert_eq!(precision.track((132.0, 92.0), true), (104.0, 99.0));
        assert!(precision.active());
    }

    #[test]
    fn starts_slowing_from_the_last_point_without_a_jump() {
        let precision = Precision::new(0.125);
        precision.track((100.0, 100.0), true);
        precision.track((180.0, 100.0), true);
        precision.track((300.0, 300.0), false);
        assert_eq!(precision.track((308.0, 300.0), true), (301.0, 300.0));
    }

    #[test]
    fn uses_the_configured_slow_down() {
        let precision = Precision::new(0.25);
        precision.track((0.0, 0.0), false);
        assert_eq!(precision.track((8.0, -4.0), true), (2.0, -1.0));
    }

    #[test]
    fn returns_to_the_pointer_when_shift_is_released() {
        let precision = Precision::new(0.125);
        precision.track((100.0, 100.0), false);
        precision.track((180.0, 100.0), true);
        assert_eq!(precision.track((180.0, 100.0), false), (180.0, 100.0));
        assert!(!precision.active());
    }
}
