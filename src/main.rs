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

struct Canvas {
    buffer: Vec<u32>,
    width: usize,
    height: usize,
    view_box: ViewBox,
}

impl Canvas {
    fn new(width: usize, height: usize, view_box: ViewBox) -> Self {
        Self {
            buffer: vec![0; width * height],
            width: width.next_multiple_of(8),
            height,
            view_box,
        }
    }

    fn resize(&mut self, width: usize, height: usize) {
        let old_size = self.size();
        self.width = width.next_multiple_of(8);
        self.height = height;
        let new_size = self.size();
        let size_diff = (new_size - old_size) * self.view_box.range() / old_size;
        self.view_box.max += size_diff * 0.5;
        self.view_box.min -= size_diff * 0.5;
        self.buffer.resize(self.width * self.height, 0);
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

    fn render_to_image(&self) -> Image {
        let mut image = Image::gen_image_color(self.width as i32, self.height as i32, Color::BLANK);
        for y in 0..self.height {
            for x in 0..self.width {
                let t = self.buffer[y * self.width + x] * 255 / ITER_LIMIT;
                image.draw_pixel(
                    x as i32,
                    y as i32,
                    Color {
                        r: 0x18,
                        g: t.try_into().unwrap(),
                        b: 0x18,
                        a: 0xFF,
                    },
                );
            }
        }
        image
    }
}

fn mandelbrot(canvas: &mut Canvas) {
    const ROW_DELTAS: f64x8 = f64x8::from_array([0., 1., 2., 3., 4., 5., 6., 7.]);
    let d = canvas.view_box.range() / canvas.size();
    canvas
        .buffer
        .par_chunks_mut(8)
        .enumerate()
        .for_each(|(n, chunk)| {
            let x = n * 8 % canvas.width;
            let y = n * 8 / canvas.width;
            let points = ComplexSimd {
                real: f64x8::splat(canvas.view_box.min.x as f64)
                    + f64x8::splat(d.x as f64) * (f64x8::splat(x as f64) + ROW_DELTAS),
                imag: f64x8::splat(canvas.view_box.min.y as f64 + d.y as f64 * y as f64),
            };
            get_count_simd(&points).copy_to_slice(chunk);
        });
}

fn main() {
    let (mut rl, thread) = raylib::init()
        .size(1200, 800)
        .title("Mandelbrot Set Viewer")
        .resizable()
        .build();
    rl.set_target_fps(60);

    let mut canvas = Canvas::new(
        rl.get_screen_width() as usize,
        rl.get_screen_height() as usize,
        ViewBox::new(Vector2::new(-2.5, -1.5), Vector2::new(4.0, 3.0)),
    );

    mandelbrot(&mut canvas);
    let mut texture = rl
        .load_texture_from_image(&thread, &canvas.render_to_image())
        .unwrap();

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
            canvas.zoom(mouse_pos, mouse_wheel);
            mandelbrot(&mut canvas);
            texture = rl
                .load_texture_from_image(&thread, &canvas.render_to_image())
                .unwrap();
        }

        let fps = rl.get_fps();
        let mouse_screen_pos = rl.get_mouse_position();
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::LIGHTSALMON);
        d.draw_texture(&texture, 0, 0, Color::WHITE);
        draw_shadowed_text(&mut d, &format!("{fps}"), rvec2(20, 20), 48);
        if mouse_screen_pos != Vector2::zero() {
            let text = format!("{}, {}", mouse_pos.x, mouse_pos.y);
            draw_shadowed_text(&mut d, &text, mouse_screen_pos, 24);
        }
    }
}

fn draw_shadowed_text(
    d: &mut RaylibDrawHandle,
    text: &str,
    screen_position: Vector2,
    font_size: i32,
) {
    d.draw_text(
        text,
        screen_position.x as i32 + 12,
        screen_position.y as i32 + 12,
        font_size,
        Color::BLACK,
    );
    d.draw_text(
        text,
        screen_position.x as i32 + 10,
        screen_position.y as i32 + 10,
        font_size,
        Color::YELLOW,
    );
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
