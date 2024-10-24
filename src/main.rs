#![feature(portable_simd)]

use std::simd::prelude::*;

use crate::options::Options;
use clap::Parser;
use raylib::{consts::*, prelude::*};
use rayon::prelude::*;

mod options;

const ZOOM_SPEED: f32 = 5.0;

const ITER_LIMIT: u32 = 300;
const THRESHOLD: f64 = 4.0;

const NUM_LANES: usize = 8;

fn main() {
    let opts = Options::parse();

    let (mut rl, thread) = raylib::init()
        .size(opts.window_size.0 as i32, opts.window_size.1 as i32)
        .title("Mandelbrot Set Viewer")
        .resizable()
        .build();
    rl.set_target_fps(60);

    let mut canvas = Canvas::from_options(&opts);

    mandelbrot(&mut canvas);
    let mut texture = canvas.render_to_texture(&mut rl, &thread);

    while !rl.window_should_close() {
        if rl.is_window_resized() {
            canvas.resize(
                rl.get_screen_width() as usize,
                rl.get_screen_height() as usize,
            );
        }
        let mouse_pos = canvas.screen_to_world(rl.get_mouse_position());
        let mouse_wheel = rl.get_mouse_wheel_move();
        let mouse_delta = if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
            canvas.screen_to_world(rl.get_mouse_delta()) - canvas.view_box.min
        } else {
            Vector2::zero()
        };

        if mouse_delta != Vector2::zero() || mouse_wheel != 0.0 || rl.is_window_resized() {
            canvas.pan(mouse_delta);
            canvas.zoom(mouse_pos, mouse_wheel * rl.get_frame_time());
            mandelbrot(&mut canvas);
            texture = canvas.render_to_texture(&mut rl, &thread);
        }

        let fps = rl.get_fps();
        let mouse_screen_pos = rl.get_mouse_position();
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::LIGHTSALMON);
        d.draw_texture(&texture, 0, 0, Color::WHITE);
        draw_shadowed_text(&mut d, &format!("{fps}"), rvec2(20, 20), 48);
        if mouse_screen_pos != Vector2::zero() {
            let text = format!("{:.6}, {:.6}", mouse_pos.x, mouse_pos.y);
            draw_shadowed_text(&mut d, &text, mouse_screen_pos, 24);
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ViewBox {
    min: Vector2,
    max: Vector2,
}

impl ViewBox {
    fn new_centered(center: Vector2, size: Vector2) -> Self {
        let top_left = center - size * 0.5;
        Self {
            min: top_left,
            max: top_left + size,
        }
    }

    fn translate(&mut self, v: Vector2) -> &mut Self {
        self.min -= v;
        self.max -= v;
        self
    }

    fn scale(&mut self, factor: Vector2) -> &mut Self {
        self.min *= factor;
        self.max *= factor;
        self
    }

    fn zoom_around(&mut self, v: Vector2, factor: Vector2) -> &mut Self {
        self.translate(v).scale(factor).translate(-v)
    }
    fn range(&self) -> Vector2 {
        self.max - self.min
    }
}

struct Canvas {
    buffer: Vec<u32>,
    image: Image,
    width: usize,
    height: usize,
    view_box: ViewBox,
}

impl Canvas {
    fn from_options(opts: &Options) -> Self {
        let view_size = rvec2(opts.window_size.0, opts.window_size.1) / opts.zoom / 100.0;
        Canvas::new(
            opts.window_size.0 as usize,
            opts.window_size.1 as usize,
            ViewBox::new_centered(opts.center.into(), view_size),
        )
    }

    fn new(width: usize, height: usize, view_box: ViewBox) -> Self {
        let width = width.next_multiple_of(NUM_LANES);
        Self {
            buffer: vec![0; width * height],
            image: Image::gen_image_color(width as i32, height as i32, Color::BLANK),
            width,
            height,
            view_box,
        }
    }

    fn resize(&mut self, width: usize, height: usize) {
        let old_size = self.size();
        self.width = width.next_multiple_of(NUM_LANES);
        self.height = height;
        let new_size = self.size();
        let size_diff = (new_size - old_size) * self.view_box.range() / old_size;
        self.view_box.max += size_diff * 0.5;
        self.view_box.min -= size_diff * 0.5;
        self.buffer.resize(self.width * self.height, 0);
        self.image = Image::gen_image_color(self.width as _, self.height as _, Color::BLANK);
    }

    fn pan(&mut self, delta: Vector2) {
        self.view_box.translate(delta);
    }

    fn zoom(&mut self, pos: Vector2, value: f32) {
        self.view_box
            .zoom_around(pos, Vector2::one() - ZOOM_SPEED * value);
    }

    fn size(&self) -> Vector2 {
        Vector2::new(self.width as _, self.height as _)
    }

    fn screen_to_world(&self, v: Vector2) -> Vector2 {
        v * self.view_box.range() / self.size() + self.view_box.min
    }

    fn render_to_image(&mut self) -> &Image {
        for y in 0..self.height {
            for x in 0..self.width {
                let t = self.buffer[y * self.width + x] as usize;
                self.image.draw_pixel(x as i32, y as i32, COLORS[t]);
            }
        }
        &self.image
    }

    fn render_to_texture(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread) -> Texture2D {
        rl.load_texture_from_image(thread, self.render_to_image())
            .unwrap()
    }
}

const COLORS_LEN: usize = 1 + ITER_LIMIT as usize;
const COLORS: [Color; COLORS_LEN] = {
    let mut colors = [Color::BLANK; COLORS_LEN];
    let mut i = 0;
    while i < COLORS_LEN {
        let t = i as f32 / ITER_LIMIT as f32;
        colors[i] = color_from_hsv(15.0 + t * 360.0, 0.7, 1.0 - t);
        i += 1;
    }
    colors
};

const fn clamp(x: f32, a: f32, b: f32) -> f32 {
    if x < a {
        a
    } else if x > b {
        b
    } else {
        x
    }
}

const fn min(a: f32, b: f32) -> f32 {
    if a < b {
        a
    } else {
        b
    }
}

/// https://en.wikipedia.org/wiki/HSL_and_HSV#HSV_to_RGB_alternative
const fn color_from_hsv(hue: f32, saturation: f32, value: f32) -> Color {
    const fn f(n: f32, h: f32) -> f32 {
        let k = (n + h / 60.0) % 6.0;
        clamp(min(4.0 - k, k), 0.0, 1.0)
    }
    let r = ((value - value * saturation * f(5.0, hue)) * 255.0) as u8;
    let g = ((value - value * saturation * f(3.0, hue)) * 255.0) as u8;
    let b = ((value - value * saturation * f(1.0, hue)) * 255.0) as u8;
    Color { r, g, b, a: 255 }
}

const fn range_array<const N: usize>() -> [f64; N] {
    let mut arr = [0.0; N];
    let mut i = 0;
    while i < N {
        arr[i] = i as f64;
        i += 1;
    }
    arr
}

fn mandelbrot(canvas: &mut Canvas) {
    const ROW_DELTAS: Simd<f64, NUM_LANES> = Simd::from_array(range_array());
    let delta = canvas.view_box.range() / canvas.size();
    let base = canvas.view_box.min;
    canvas
        .buffer
        .par_chunks_mut(NUM_LANES)
        .enumerate()
        .for_each(|(n, chunk)| {
            let x = n * NUM_LANES % canvas.width;
            let y = n * NUM_LANES / canvas.width;
            let points = ComplexSimd {
                real: Simd::splat(base.x as f64)
                    + Simd::splat(delta.x as f64) * (Simd::splat(x as f64) + ROW_DELTAS),
                imag: Simd::splat(base.y as f64 + delta.y as f64 * y as f64),
            };
            get_count_simd(&points).copy_to_slice(chunk);
        });
}

fn draw_shadowed_text(
    d: &mut RaylibDrawHandle,
    text: &str,
    screen_position: Vector2,
    font_size: i32,
) {
    const TEXT_PAD: i32 = 12;
    const SHADOW: i32 = 2;
    let text_size = d.measure_text(text, font_size);
    let mut px = screen_position.x as i32;
    let mut py = screen_position.y as i32;
    if px + text_size + TEXT_PAD >= d.get_screen_width() {
        px -= text_size + TEXT_PAD;
    } else {
        px += TEXT_PAD;
    }
    if py + font_size >= d.get_screen_height() {
        py -= font_size;
    }
    d.draw_text(text, px + SHADOW, py + SHADOW, font_size, Color::BLACK);
    d.draw_text(text, px, py, font_size, Color::YELLOW);
}

#[derive(Debug, Clone)]
struct ComplexSimd {
    real: Simd<f64, NUM_LANES>,
    imag: Simd<f64, NUM_LANES>,
}

fn get_count_simd(start: &ComplexSimd) -> Simd<u32, NUM_LANES> {
    let mut current = start.clone();
    let mut count = Simd::splat(0u64);
    let threshold = Simd::splat(THRESHOLD);
    for _ in 0..ITER_LIMIT {
        let rr = current.real * current.real;
        let ii = current.imag * current.imag;
        let undiverged_mask = (rr + ii).simd_le(threshold);
        if !undiverged_mask.any() {
            break;
        }
        count += undiverged_mask.select(Simd::splat(1), Simd::splat(0));
        let ri = current.real * current.imag;
        current.real = start.real + (rr - ii);
        current.imag = start.imag + (ri + ri);
    }
    count.cast()
}
