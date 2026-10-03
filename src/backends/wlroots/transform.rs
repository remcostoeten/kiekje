use wayland_client::protocol::wl_output::Transform;

pub struct Upright {
    pub width: u32,
    pub height: u32,
    pub stride: usize,
    pub data: Vec<u8>,
}

/// Turns a captured buffer into an upright image in logical orientation.
///
/// `transform` is what the compositor applied to the buffer contents, so its inverse is applied
/// here. `y_invert` flips the rows first, as wlr-screencopy reports it.
///
/// # Examples
///
/// ```ignore
/// let image = upright(&data, 2560, 1440, 10240, Transform::_90, false);
/// assert_eq!((image.width, image.height), (1440, 2560));
/// ```
pub fn upright(
    data: &[u8],
    width: u32,
    height: u32,
    stride: usize,
    transform: Transform,
    y_invert: bool,
) -> Upright {
    if !y_invert && matches!(transform, Transform::Normal) {
        return Upright {
            width,
            height,
            stride,
            data: data[..stride * height as usize].to_vec(),
        };
    }

    let swaps_axes = matches!(
        transform,
        Transform::_90 | Transform::_270 | Transform::Flipped90 | Transform::Flipped270
    );
    let (out_width, out_height) = if swaps_axes {
        (height, width)
    } else {
        (width, height)
    };
    let mut out = Vec::with_capacity(out_width as usize * out_height as usize * 4);
    for y in 0..out_height {
        for x in 0..out_width {
            let (source_x, source_y) = source_position(transform, x, y, out_width, out_height);
            let row = if y_invert {
                height - 1 - source_y
            } else {
                source_y
            };
            let start = row as usize * stride + source_x as usize * 4;
            out.extend_from_slice(&data[start..start + 4]);
        }
    }
    Upright {
        width: out_width,
        height: out_height,
        stride: out_width as usize * 4,
        data: out,
    }
}

fn source_position(transform: Transform, x: u32, y: u32, width: u32, height: u32) -> (u32, u32) {
    match transform {
        Transform::_90 => (y, width - 1 - x),
        Transform::_180 => (width - 1 - x, height - 1 - y),
        Transform::_270 => (height - 1 - y, x),
        Transform::Flipped => (width - 1 - x, y),
        Transform::Flipped90 => (y, x),
        Transform::Flipped180 => (x, height - 1 - y),
        Transform::Flipped270 => (height - 1 - y, width - 1 - x),
        _ => (x, y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: [u8; 4] = [1, 1, 1, 255];
    const B: [u8; 4] = [2, 2, 2, 255];
    const C: [u8; 4] = [3, 3, 3, 255];

    fn pixels(values: &[[u8; 4]]) -> Vec<u8> {
        values.concat()
    }

    #[test]
    fn keeps_normal_buffers_untouched() {
        let image = upright(&pixels(&[A, B]), 2, 1, 8, Transform::Normal, false);
        assert_eq!((image.width, image.height), (2, 1));
        assert_eq!(image.data, pixels(&[A, B]));
    }

    #[test]
    fn undoes_counter_clockwise_rotation() {
        let image = upright(&pixels(&[A, B]), 2, 1, 8, Transform::_90, false);
        assert_eq!((image.width, image.height), (1, 2));
        assert_eq!(image.data, pixels(&[A, B]));

        let image = upright(&pixels(&[A, B]), 2, 1, 8, Transform::_270, false);
        assert_eq!(image.data, pixels(&[B, A]));
    }

    #[test]
    fn undoes_half_turn_and_mirror() {
        let image = upright(&pixels(&[A, B, C]), 3, 1, 12, Transform::_180, false);
        assert_eq!(image.data, pixels(&[C, B, A]));

        let image = upright(&pixels(&[A, B, C]), 3, 1, 12, Transform::Flipped, false);
        assert_eq!(image.data, pixels(&[C, B, A]));
    }

    #[test]
    fn flips_rows_for_y_inverted_buffers() {
        let image = upright(&pixels(&[A, B]), 1, 2, 4, Transform::Normal, true);
        assert_eq!(image.data, pixels(&[B, A]));
    }

    #[test]
    fn skips_row_padding() {
        let data = pixels(&[A, [0; 4], B, [0; 4]]);
        let image = upright(&data, 1, 2, 8, Transform::_180, false);
        assert_eq!(image.data, pixels(&[B, A]));
    }
}
