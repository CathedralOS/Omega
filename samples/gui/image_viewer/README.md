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
