#![feature(portable_simd)]

use std::simd::prelude::*;

use raylib::{consts::*, prelude::*};
use rayon::prelude::*;

const ZOOM_SPEED: f32 = 0.2;

const ITER_LIMIT: u32 = 300;
const THRESHOLD: f64 = 4.0;

#[derive(Debug, Clone, Copy)]
struct ViewBox {
    min: Vector2,
    max: Vector2,
}

impl ViewBox {
    fn new(top_left: Vector2, size: Vector2) -> Self {
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

fn main() {
    let (mut rl, thread) = raylib::init()
        .size(1200, 800)
        .title("Mandelbrot Set Viewer")
        .resizable()
        .build();
    rl.set_target_fps(60);

    let mut width = (rl.get_screen_width() as usize).next_multiple_of(8);
    let mut height = rl.get_screen_height() as usize;
    let mut screen_size = Vector2::new(width as _, height as _);
    let mut view_box = ViewBox::new(Vector2::new(-2.5, -1.5), Vector2::new(4.0, 3.0));
    let mut buffer = vec![0u32; width * height];

    mandelbrot(view_box, &mut buffer, screen_size);
    let mut texture = rl
        .load_texture_from_image(&thread, &render_to_image(&buffer, screen_size))
        .unwrap();

    while !rl.window_should_close() {
        if rl.is_window_resized() {
            width = (rl.get_screen_width() as usize).next_multiple_of(8);
            height = rl.get_screen_height() as usize;
            let new_size = Vector2::new(width as _, height as _);
            let size_diff = (new_size - screen_size) * view_box.range() / screen_size;
            view_box.max += size_diff * 0.5;
            view_box.min -= size_diff * 0.5;
            screen_size = new_size;
            buffer.resize(width * height, 0);
        }
        let mouse_pos = rl.get_mouse_position() * view_box.range() / screen_size + view_box.min;
        let mouse_wheel = rl.get_mouse_wheel_move();
        let mouse_delta = if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
            rl.get_mouse_delta() * view_box.range() / screen_size
        } else {
            Vector2::zero()
        };

        if mouse_delta != Vector2::zero() || mouse_wheel != 0.0 || rl.is_window_resized() {
            view_box
                .translate(mouse_delta)
                .zoom_around(mouse_pos, Vector2::one() - ZOOM_SPEED * mouse_wheel);

            mandelbrot(view_box, &mut buffer, screen_size);
            texture = rl
                .load_texture_from_image(&thread, &render_to_image(&buffer, screen_size))
                .unwrap();
        }

        let fps = rl.get_fps();
        let mouse_screen_pos = rl.get_mouse_position();
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::LIGHTSALMON);
        d.draw_texture(&texture, 0, 0, Color::WHITE);
        d.draw_text(&format!("{fps}"), 20, 20, 48, Color::YELLOW);
        d.draw_text(
            &format!("{}, {}", mouse_pos.x, mouse_pos.y),
            mouse_screen_pos.x as i32 + 10,
            mouse_screen_pos.y as i32 + 10,
            24,
            Color::YELLOW,
        );
    }
}

fn mandelbrot(view_box: ViewBox, buffer: &mut [u32], screen_size: Vector2) {
    let w = screen_size.x as usize;
    let d = view_box.range() / screen_size;
    buffer.par_chunks_mut(8).enumerate().for_each(|(n, chunk)| {
        let x = n * 8 % w;
        let y = n * 8 / w;
        let points = ComplexSimd {
            real: f64x8::from_array(std::array::from_fn(|i| {
                view_box.min.x as f64 + d.x as f64 * (x + i) as f64
            })),
            imag: f64x8::splat(view_box.min.y as f64 + y as f64 * d.y as f64),
        };
        get_count_simd(&points).copy_to_slice(chunk);
    });
}

fn render_to_image(buffer: &[u32], screen_size: Vector2) -> Image {
    let w = screen_size.x as usize;
    let h = screen_size.y as usize;
    let mut image = Image::gen_image_color(w as i32, h as i32, Color::BLANK);
    for y in 0..h {
        for x in 0..w {
            let t = buffer[y * w + x] as f32 / ITER_LIMIT as f32;
            // let t = (buffer[y * WIDTH + x] as f32).log(ITER_LIMIT as f32);
            image.draw_pixel(x as i32, y as i32, Color::BLACK.alpha(t));
        }
    }
    image
}

#[derive(Debug, Clone)]
struct ComplexSimd {
    real: f64x8,
    imag: f64x8,
}

fn get_count_simd(start: &ComplexSimd) -> u32x8 {
    let mut current = start.clone();
    let mut count = u64x8::splat(0);
    let threshold = f64x8::splat(THRESHOLD);
    for _ in 0..ITER_LIMIT {
        let rr = current.real * current.real;
        let ii = current.imag * current.imag;
        let undiverged_mask = (rr + ii).simd_le(threshold);
        if !undiverged_mask.any() {
            break;
        }
        count += undiverged_mask.select(u64x8::splat(1), u64x8::splat(0));
        let ri = current.real * current.imag;
        current.real = start.real + (rr - ii);
        current.imag = start.imag + (ri + ri);
    }
    count.cast()
}
