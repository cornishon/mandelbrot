# A simple parallel [Mandelbrot Set](https://en.wikipedia.org/wiki/Mandelbrot_set) explorer powered by [SIMD](https://en.wikipedia.org/wiki/SIMD)

## Quickstart
Requires glfw, cmake, curl and Rust (nightly) installed on the system to build

```console
RUSTFLAGS="-C target-cpu=native" cargo +nightly build --release
./target/release/mandelbrot
```

Run `./target/release/mandelbrot --help` for extra options.
Inside the application you can pan and zoom with the mouse. Resizing the window updates the viewport accordingly.

You can omit the `target-cpu` flag at the cost of performance.
Nightly is currently required for generic SIMD support and for const float operations.
