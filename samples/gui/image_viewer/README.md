# Image viewer

This macOS application loads the supplied 64×64, uncompressed 24-bit BMP files
from disk and decodes them into a framebuffer. Right and Left move through
`img0.bmp`, `img1.bmp`, and `img2.bmp`; Escape or closing the window exits.
Keys act only while the window has focus. Each navigation reloads the file.
The decoder expects the supplied format and a 12,342-byte file; it is not a
general BMP parser.

With the current `omega` executable, from the repository root:

```sh
omega update --project samples/gui/image_viewer --target macos_arm64 --offline
# If review is required, inspect the reported file and decide its pending rows.
omega update --resume --project samples/gui/image_viewer --offline
cd samples/gui/image_viewer
omega run --keep main.omg
```

Run `--resume` only when an update requests review. The run command omits
`--target` so it executes on the host. Its retained `.app` can also be run
from this directory using the reported inner executable:

```sh
/path/reported/by/omega/image_viewer.app/Contents/MacOS/image_viewer
```

Paths use the process working directory, with the existing parent-directory
fallback. Publication does not copy images into the bundle or change the
working directory; Finder/resource-relative launch is outside the current
contract. The implemented providers target macOS ARM64; the declared Windows
entry does not yet have matching GUI providers.

The application owns its `Window` and destroys it on every exit. File reads
use the POSIX service, accumulate short transfers until EOF, and close the
descriptor before decoding. Missing, truncated, and oversized files fail with
exit 1. Interactive rendering, navigation, and normal close require a desktop
session that permits observation and input.

## Native gate (macOS ARM64, 2026-10-13)

`python3 tools/sample_native_gate.py --filter image_viewer` reviews the
package, builds `build/image_viewer.app` natively and reports the sample as
`no-oracle`: the header documents neither `Expected exit` nor
`Expected output contains`, because the viewer waits for input. The gate
therefore scores the build, not a run. Launched from this directory, the
built executable prints `loaded BMP: 64x64 (12342 bytes)` once `img0.bmp`
has been read to EOF and decoded, and stays alive in its window until Escape
or close.

The file service's `read` reaches libSystem's `_read` through the provider's
`&mut buffer[0]` on its `&mut [u8]` parameter: an inline byte element with no
declaration of its own. Omega's descriptor-input guard now rejoins that
endpoint to the canonical `u8` carrier the lowering published (previously it
refused with `SourceCustodyMismatch`); the admitted shape is pinned by
`tests/omega/pass/filesystem/runtime_posix_read_runtime_range_view_exit`.
