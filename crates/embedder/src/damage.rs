//! The changed area of a software frame (silica-desktop#205): the engine's software renderer
//! gives whole frames, so the embedder compares each one with the one before and tells the
//! compositor (`wl_surface.damage_buffer`) only the rectangle that differs.

/// A rectangle in buffer pixels: x, y, width, height.
pub type Rect = (i32, i32, i32, i32);

/// The bounding box of the pixels that differ between `previous` (tightly packed rows of
/// `width * 4` bytes) and `frame` (rows `stride` bytes apart). `None` when nothing changed;
/// the whole frame when the previous one has another size or a row is missing.
pub fn changed_rect(
    previous: &[u8],
    frame: &[u8],
    stride: usize,
    width: usize,
    height: usize,
) -> Option<Rect> {
    let row_bytes = width.saturating_mul(4);
    let whole = || {
        Some((
            0,
            0,
            i32::try_from(width).unwrap_or(i32::MAX),
            i32::try_from(height).unwrap_or(i32::MAX),
        ))
    };
    if row_bytes == 0 || height == 0 {
        return None;
    }
    if previous.len() != row_bytes.saturating_mul(height) || stride < row_bytes {
        return whole();
    }
    let mut top: Option<usize> = None;
    let mut bottom = 0;
    let (mut left, mut right) = (width, 0);
    for (y, (old, new)) in previous
        .chunks_exact(row_bytes)
        .zip(frame.chunks(stride))
        .enumerate()
    {
        let Some(new) = new.get(..row_bytes) else {
            return whole();
        };
        if old == new {
            continue;
        }
        top.get_or_insert(y);
        bottom = y;
        let (old_px, new_px) = (old.as_chunks::<4>().0, new.as_chunks::<4>().0);
        let first = old_px
            .iter()
            .zip(new_px)
            .position(|(a, b)| a != b)
            .unwrap_or(0);
        let last = old_px
            .iter()
            .zip(new_px)
            .rposition(|(a, b)| a != b)
            .unwrap_or(first);
        left = left.min(first);
        right = right.max(last);
    }
    let top = top?;
    let to_i32 = |v: usize| i32::try_from(v).unwrap_or(i32::MAX);
    Some((
        to_i32(left),
        to_i32(top),
        to_i32(right - left + 1),
        to_i32(bottom - top + 1),
    ))
}

/// Copies `frame` (rows `stride` bytes apart) into `into` as tightly packed rows.
pub fn remember(into: &mut Vec<u8>, frame: &[u8], stride: usize, width: usize, height: usize) {
    let row_bytes = width.saturating_mul(4);
    into.clear();
    for row in frame.chunks(stride.max(1)).take(height) {
        into.extend_from_slice(row.get(..row_bytes).unwrap_or(row));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(width: usize, height: usize) -> Vec<u8> {
        vec![0; width * height * 4]
    }

    #[test]
    fn identical_frames_have_no_damage() {
        let a = frame(4, 3);
        assert_eq!(changed_rect(&a, &a, 16, 4, 3), None);
    }

    #[test]
    fn one_pixel_is_a_one_pixel_rect() {
        let old = frame(4, 3);
        let mut new = old.clone();
        new[(4 + 2) * 4] = 9;
        assert_eq!(changed_rect(&old, &new, 16, 4, 3), Some((2, 1, 1, 1)));
    }

    #[test]
    fn two_distant_pixels_give_their_bounding_box() {
        let old = frame(8, 8);
        let mut new = old.clone();
        new[(8 + 1) * 4 + 1] = 1;
        new[(6 * 8 + 5) * 4 + 3] = 1;
        assert_eq!(changed_rect(&old, &new, 32, 8, 8), Some((1, 1, 5, 6)));
    }

    #[test]
    fn a_stride_wider_than_the_row_is_skipped() {
        let old = frame(2, 2);
        let mut new = vec![0; 12 * 2];
        new[10] = 7; // padding of row 0: not a change
        new[12 + 4] = 7;
        assert_eq!(changed_rect(&old, &new, 12, 2, 2), Some((1, 1, 1, 1)));
    }

    #[test]
    fn another_size_damages_everything() {
        let old = frame(2, 2);
        let new = frame(3, 2);
        assert_eq!(changed_rect(&old, &new, 12, 3, 2), Some((0, 0, 3, 2)));
        assert_eq!(changed_rect(&[], &new, 12, 3, 2), Some((0, 0, 3, 2)));
    }

    #[test]
    fn remember_packs_the_rows() {
        let mut kept = Vec::new();
        let new: Vec<u8> = (0..24).collect();
        remember(&mut kept, &new, 12, 2, 2);
        assert_eq!(kept.len(), 16);
        assert_eq!(&kept[8..], &new[12..20]);
    }
}
