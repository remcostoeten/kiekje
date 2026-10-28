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
    let out_stride = out_width as usize * 4;
    let mut out = vec![0u8; out_stride * out_height as usize];
    if swaps_axes {
        for (y, target_row) in out.chunks_exact_mut(out_stride).enumerate() {
            for (x, target) in target_row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let (source_x, source_y) =
                    source_position(transform, x as u32, y as u32, out_width, out_height);
                let row = source_row(source_y, height, y_invert);
                let start = row * stride + source_x as usize * 4;
                target.copy_from_slice(&data[start..start + 4]);
            }
        }
    } else {
        let mirrored = matches!(transform, Transform::_180 | Transform::Flipped);
        let upside_down = matches!(transform, Transform::_180 | Transform::Flipped180);
        for (y, target_row) in out.chunks_exact_mut(out_stride).enumerate() {
            let source_y = if upside_down {
                height - 1 - y as u32
            } else {
                y as u32
            };
            let start = source_row(source_y, height, y_invert) * stride;
            let source = &data[start..start + out_stride];
            if mirrored {
                for (target, pixel) in target_row
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(source.as_chunks::<4>().0.iter().rev())
                {
                    target.copy_from_slice(pixel);
                }
            } else {
                target_row.copy_from_slice(source);
            }
        }
    }
    Upright {
        width: out_width,
        height: out_height,
        stride: out_stride,
        data: out,
    }
}

fn source_row(source_y: u32, height: u32, y_invert: bool) -> usize {
    if y_invert {
        (height - 1 - source_y) as usize
    } else {
        source_y as usize
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

    fn reference(
        data: &[u8],
        width: u32,
        height: u32,
        stride: usize,
        transform: Transform,
        y_invert: bool,
    ) -> Vec<u8> {
        let swaps = matches!(
            transform,
            Transform::_90 | Transform::_270 | Transform::Flipped90 | Transform::Flipped270
        );
        let (out_width, out_height) = if swaps {
            (height, width)
        } else {
            (width, height)
        };
        let mut out = Vec::new();
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
        out
    }

    #[test]
    fn matches_the_pixel_by_pixel_result_for_every_transform() {
        let transforms = [
            Transform::Normal,
            Transform::_90,
            Transform::_180,
            Transform::_270,
            Transform::Flipped,
            Transform::Flipped90,
            Transform::Flipped180,
            Transform::Flipped270,
        ];
        for (width, height, padding) in [(5u32, 3u32, 8usize), (4, 7, 0), (1, 1, 4), (6, 6, 12)] {
            let stride = width as usize * 4 + padding;
            let data: Vec<u8> = (0..stride * height as usize)
                .map(|index| (index * 31 % 251) as u8)
                .collect();
            for transform in transforms {
                for y_invert in [false, true] {
                    if !y_invert && matches!(transform, Transform::Normal) {
                        continue;
                    }
                    let image = upright(&data, width, height, stride, transform, y_invert);
                    assert_eq!(
                        image.data,
                        reference(&data, width, height, stride, transform, y_invert),
                        "{transform:?} y_invert={y_invert} {width}x{height}"
                    );
                    assert_eq!(image.stride, image.width as usize * 4);
                }
            }
        }
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
