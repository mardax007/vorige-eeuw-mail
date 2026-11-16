//! The unread count on the taskbar icon. Windows has no badge number for desktop apps, only an
//! overlay icon, so the shell draws a small red disc with the number itself.

/// Size of the overlay image. Windows scales it to the small-icon size of the display.
pub const SIZE: u32 = 32;

const RED: [u8; 4] = [0xC4, 0x2B, 0x1C, 0xFF];
const WHITE: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];

/// 3 × 5 pixel glyphs, one row per entry, the high bit of three on the left.
fn glyph(c: char) -> [u8; 5] {
    match c {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b001, 0b001],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        _ => [0b000, 0b010, 0b111, 0b010, 0b000], // '+'
    }
}

/// The text on the disc: the count, or "99+" above 99.
pub fn label(count: u32) -> String {
    if count > 99 { "99+".to_string() } else { count.to_string() }
}

/// RGBA pixels (`SIZE` × `SIZE`) of the overlay for `count` unread messages, or None for 0,
/// which removes the overlay.
pub fn pixels(count: u32) -> Option<Vec<u8>> {
    if count == 0 {
        return None;
    }
    let text = label(count);
    let mut px = vec![0u8; (SIZE * SIZE * 4) as usize];
    let r = SIZE as f32 / 2.0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (dx, dy) = (x as f32 + 0.5 - r, y as f32 + 0.5 - r);
            if dx * dx + dy * dy <= r * r {
                put(&mut px, x, y, RED);
            }
        }
    }
    let n = text.chars().count() as u32;
    let scale: u32 = match n {
        1 => 4,
        2 => 3,
        _ => 2,
    };
    let gap = scale;
    let width = n * 3 * scale + (n - 1) * gap;
    let height = 5 * scale;
    let (left, top) = ((SIZE - width) / 2, (SIZE - height) / 2);
    for (i, c) in text.chars().enumerate() {
        let x0 = left + i as u32 * (3 * scale + gap);
        for (row, bits) in glyph(c).iter().enumerate() {
            for col in 0..3u32 {
                if bits & (0b100 >> col) == 0 {
                    continue;
                }
                for sy in 0..scale {
                    for sx in 0..scale {
                        put(&mut px, x0 + col * scale + sx, top + row as u32 * scale + sy, WHITE);
                    }
                }
            }
        }
    }
    Some(px)
}

fn put(px: &mut [u8], x: u32, y: u32, color: [u8; 4]) {
    let i = ((y * SIZE + x) * 4) as usize;
    px[i..i + 4].copy_from_slice(&color);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(px: &[u8], x: u32, y: u32) -> [u8; 4] {
        let i = ((y * SIZE + x) * 4) as usize;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    }

    #[test]
    fn no_overlay_without_unread_mail() {
        assert!(pixels(0).is_none());
    }

    #[test]
    fn disc_is_red_with_transparent_corners() {
        let px = pixels(3).unwrap();
        assert_eq!(px.len(), (SIZE * SIZE * 4) as usize);
        assert_eq!(at(&px, 0, 0)[3], 0);
        assert_eq!(at(&px, 2, SIZE / 2), RED);
    }

    #[test]
    fn number_is_drawn_in_white_and_fits() {
        for n in [1, 7, 42, 99, 100, 12_345] {
            let px = pixels(n).unwrap();
            let white = px.chunks(4).filter(|p| *p == WHITE).count();
            assert!(white > 0, "{n} has no white pixels");
        }
        assert_eq!(label(100), "99+");
        assert_eq!(label(42), "42");
    }
}
