# Mandelbrot

Renders the Mandelbrot set as 28 rows of 64 ASCII characters. Each pixel runs
up to 28 binary64 escape-time steps and selects from ` .:-=+*#`.

After completing the sample's package review, run from the repository root:

```sh
omega run --both samples/cli/rendering/mandelbrot/main.omg
```

Press Enter at the final prompt. A POSIX shell can redirect stdin from
`/dev/null` for unattended execution. The named entry and explicit state
arguments run natively and in the checked interpreter on macOS ARM64, with
both engines exiting 0.

Application stdout starts with
`mandelbrot: escape-time fractal rendered as 64x28 ASCII`, followed by the
complete 28-row image, `PASS: 28 rows rendered (exit 0)`, and
`[press Enter to close]`. Every row retains all 64 columns, including spaces.
The complete 1,931-byte stdout matched an independent binary64 implementation
of the authored algorithm; its SHA-256 is:

```text
c2426e3116ab434670b86aed7144875ab885ea9c75d1cfd32bb0328dcf009ca8
```

This digest covers the banner, image, success message, prompt, and LF line
endings. The CLI's `native exit 0` and `interp exit: 0` reports are on stderr.
