//! Generates the Nook app icon (1024×1024 PNG) procedurally: a soft
//! rounded-square background in Nook blue with a white chat-bubble mark
//! that has a rounded "nook" cut-out.

use image::{Rgba, RgbaImage};

const SIZE: u32 = 1024;

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn bg_color(x: u32, y: u32) -> Rgba<u8> {
    // Diagonal gradient from #4D8DFF to #2F6BFF with a subtle vertical lift.
    let t = (x as f32 / SIZE as f32 + y as f32 / SIZE as f32) / 2.0;
    let r = lerp(88.0, 42.0, t);
    let g = lerp(146.0, 108.0, t);
    let b = lerp(255.0, 252.0, t);
    Rgba([r as u8, g as u8, b as u8, 255])
}

fn rounded_square_mask(x: f32, y: f32, center: f32, half: f32, radius: f32) -> bool {
    let dx = (x - center).abs() - (half - radius);
    let dy = (y - center).abs() - (half - radius);
    if dx <= 0.0 && dy <= 0.0 {
        true
    } else if dx > 0.0 && dy > 0.0 {
        let d = (dx * dx + dy * dy).sqrt();
        d <= radius
    } else {
        dx <= 0.0 || dy <= 0.0 && dx <= radius || dy <= radius
    }
}

fn circle(x: f32, y: f32, cx: f32, cy: f32, r: f32) -> bool {
    let dx = x - cx;
    let dy = y - cy;
    dx * dx + dy * dy <= r * r
}

fn main() {
    let out_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "assets/icons".to_string());
    std::fs::create_dir_all(&out_dir).unwrap();

    let mut img = RgbaImage::new(SIZE, SIZE);
    let s = SIZE as f32;
    let scale = s / 1024.0;

    for y in 0..SIZE {
        for x in 0..SIZE {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;

            // Background rounded square (full-bleed with 180px corners).
            let in_bg = rounded_square_mask(fx, fy, s / 2.0, s / 2.0, 180.0 * scale);
            if !in_bg {
                img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                continue;
            }
            img.put_pixel(x, y, bg_color(x, y));
        }
    }

    // Chat bubble: rounded rectangle with a tail, drawn in white.
    let bubble_cx = 512.0 * scale;
    let bubble_cy = 470.0 * scale;
    let bubble_half_w = 300.0 * scale;
    let bubble_half_h = 210.0 * scale;
    let bubble_r = 110.0 * scale;

    let tail = |fx: f32, fy: f32| -> bool {
        // Tail: bottom-left region.
        let x0 = 250.0 * scale;
        let y0 = 640.0 * scale;
        let x1 = 380.0 * scale;
        let y1 = 790.0 * scale;
        if fx < x0 || fx > x1 || fy < y0 || fy > y1 {
            return false;
        }
        // Slanted edge.
        let t = (fy - y0) / (y1 - y0);
        let edge = x0 + (x1 - x0) * (1.0 - t) * 0.85;
        fx >= edge
    };

    let in_bubble = |fx: f32, fy: f32| -> bool {
        let dx = (fx - bubble_cx).abs() - (bubble_half_w - bubble_r);
        let dy = (fy - bubble_cy).abs() - (bubble_half_h - bubble_r);
        let inside_rounded = if dx <= 0.0 && dy <= 0.0 {
            true
        } else if dx > 0.0 && dy > 0.0 {
            (dx * dx + dy * dy).sqrt() <= bubble_r
        } else {
            false
        };
        inside_rounded || tail(fx, fy)
    };

    // The "nook" cut-out: a crescent inside the bubble.
    let notch_cx = 512.0 * scale;
    let notch_cy = 470.0 * scale;
    let notch_r = 92.0 * scale;

    for y in 0..SIZE {
        for x in 0..SIZE {
            if img.get_pixel(x, y).0[3] == 0 {
                continue;
            }
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            if in_bubble(fx, fy) {
                let in_notch = circle(fx, fy, notch_cx, notch_cy - notch_r * 0.35, notch_r);
                if !in_notch {
                    img.put_pixel(x, y, Rgba([255, 255, 255, 255]));
                } else {
                    // Inner crescent in bg color for depth.
                    img.put_pixel(x, y, bg_color(x, y));
                }
            }
        }
    }

    let path = format!("{out_dir}/nook-icon-1024.png");
    img.save(&path).unwrap();
    println!("wrote {path}");
}
