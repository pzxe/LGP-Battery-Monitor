//! Renders the tray icon (percentage digits + charge bar) into an RGBA buffer.

pub const SIZE: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Good,
    Low,
    Critical,
    Charging,
    Offline,
}

impl Tone {
    pub fn rgb(self) -> [u8; 3] {
        match self {
            Tone::Good => [0x5B, 0xE0, 0x8A],
            Tone::Low => [0xFF, 0xC1, 0x4D],
            Tone::Critical => [0xFF, 0x5C, 0x5C],
            Tone::Charging => [0x4F, 0xC3, 0xF7],
            Tone::Offline => [0x9A, 0x9A, 0xA6],
        }
    }
}

// 3x5 digit glyphs, one row per u8 (low 3 bits).
const DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b001, 0b001, 0b001],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];
const DASH: [u8; 5] = [0, 0, 0b111, 0, 0];

fn put(buf: &mut [u8], x: usize, y: usize, c: [u8; 3], a: u8) {
    if x < SIZE && y < SIZE {
        let i = (y * SIZE + x) * 4;
        buf[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], a]);
    }
}

fn rect(buf: &mut [u8], x0: usize, y0: usize, w: usize, h: usize, c: [u8; 3]) {
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            put(buf, x, y, c, 255);
        }
    }
}

/// `percent = None` draws "--".
pub fn render(percent: Option<u8>, tone: Tone) -> Vec<u8> {
    let mut buf = vec![0u8; SIZE * SIZE * 4];
    // rounded dark background
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (cx, cy) = (x.min(SIZE - 1 - x), y.min(SIZE - 1 - y));
            let corner = cx < 4 && cy < 4 && (4 - cx) * (4 - cx) + (4 - cy) * (4 - cy) > 17;
            if !corner {
                put(&mut buf, x, y, [0x1E, 0x1F, 0x24], 255);
            }
        }
    }
    let color = tone.rgb();
    let glyphs: Vec<&[u8; 5]> = match percent {
        Some(p) => p
            .min(100)
            .to_string()
            .bytes()
            .map(|b| &DIGITS[(b - b'0') as usize])
            .collect(),
        None => vec![&DASH, &DASH],
    };
    let scale = if glyphs.len() >= 3 { 2 } else { 3 };
    let gap = scale - 1;
    let total_w = glyphs.len() * 3 * scale + (glyphs.len() - 1) * gap;
    let x_start = (SIZE - total_w) / 2;
    let y_start = 4;
    for (gi, g) in glyphs.iter().enumerate() {
        let gx = x_start + gi * (3 * scale + gap);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..3 {
                if bits & (1 << (2 - col)) != 0 {
                    rect(
                        &mut buf,
                        gx + col * scale,
                        y_start + row * scale,
                        scale,
                        scale,
                        color,
                    );
                }
            }
        }
    }
    // charge bar
    let (bx, by, bw, bh) = (4, 24, 24, 4);
    rect(&mut buf, bx, by, bw, bh, [0x3A, 0x3B, 0x44]);
    if let Some(p) = percent {
        let fill = (bw * p.min(100) as usize).div_ceil(100).max(1);
        rect(&mut buf, bx, by, fill, bh, color);
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_size_and_content() {
        for p in [None, Some(0), Some(7), Some(36), Some(100)] {
            let b = render(p, Tone::Good);
            assert_eq!(b.len(), SIZE * SIZE * 4);
            // some pixel uses the tone color
            assert!(b.chunks(4).any(|px| px[..3] == Tone::Good.rgb()));
        }
    }
}
