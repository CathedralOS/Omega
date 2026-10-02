# Mandelbrot zoom

Renders 12 frames of 48 by 22 ASCII characters, zooming into the seahorse-valley
boundary. Each pixel runs up to 40 binary64 escape-time steps. Frames begin
with the ANSI cursor-home sequence `ESC [ H` and use `Clock.sleep(70)` for pacing.

After completing the sample's package review, run from the repository root:

```sh
OMEGA_INTERP_STEP_BUDGET=100000000 omega run --both samples/cli/rendering/mandelbrot_zoom/main.omg
```

Press Enter at the final prompt. A POSIX shell can redirect stdin from
`/dev/null` for unattended execution. The named entry and explicit state
arguments execute in both engines on macOS ARM64 and exit 0. The interpreter
needs more than its default 10-million-step allowance for all 12 frames; the
command provisions 100 million steps through its existing environment setting.
The checked sleep adapter runs normally, and its selected foreign wait advances
virtual time without invoking the host import. Native execution retains real
frame pacing.

Application stdout starts with
`mandelbrot_zoom: 12 animated frames zooming into seahorse valley`, contains
all 12 frames, and ends with `PASS: 12 zoom frames rendered (exit 0)` and
`[press Enter to close]`. Each of the 264 image rows retains all 48 columns,
including spaces. The complete 13,099-byte stdout matched an independent
binary64 implementation of the authored algorithm; its SHA-256 is:

```text
6e3b80d1651491dd0f9289b4fc38db3161fe07001237773953f5a51536b59955
```

This digest includes every cursor-home sequence, the banner, image frames,
success message, prompt, and LF line endings. The CLI's native exit and
interpreter status reports are on stderr; a successful command status alone
does not establish that interpretation ran.
