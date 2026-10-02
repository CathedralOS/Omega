# Heat grid

This program walks a 3×4 grid using `cells[y * 4 + x]`, with dependent state
contracts bounding both coordinates. It adds each cell's coordinates to its
heat; the seeded cell at `(2, 3)` goes from 40 to 45. The required process exit
is **45**. The source uses ordinary initialization, indexed reads/writes,
conversion calls and state transfers—none needs a sample-specific compiler path.

The static entry owns a local `Grid`. Each state receives an explicit borrow;
coordinates and per-cell calculations are parameters or locals, not persistent
receiver scratch fields.

From the repository root on macOS ARM64:

```sh
python3 tools/sample_native_gate.py --cli target/release/omega --target macos_arm64 --filter collections/heat_grid --jobs 1
target/release/omega run --both samples/cli/collections/heat_grid/main.omg
```

The first command completes this checkout's package review for measurement and
checks the native exit oracle. It records acceptance, not a dependency audit.
The second compares native execution with the checked interpreter; both must
exit 45. Omit `--target` from `omega run` to execute on the current host.
The former receiver-based sample produced native and interpreted exit 45 on
macOS ARM64. That result is not execution evidence for the static migration.

The migrated sample passes `omega --check` on Windows. `omega run --both`
currently stops at missing local package acceptance before execution; its review
includes the standard library's `FilesystemHost` authority. Complete the explicit
package review before running either execution check above; do not infer approval
from the earlier sample's result. Native and interpreted exit 45 remain required.

On Windows PowerShell, use `python`, `target/release/omega.exe`, and the gate's
`--target windows_x86_64`. Matching-host Windows and Linux execution remain
separate validation requirements; a macOS run does not establish them.
