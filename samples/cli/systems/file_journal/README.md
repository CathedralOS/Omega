# file_journal

The settled
[provider-closure transport](../../../../wiki/spec/build/platform_services.md#provider-closure-transport)
keeps one portable `Filesystem` binding while its selected target provider
retains the exact POSIX or Windows raw-family closure.

Acceptance: `omega run samples/cli/systems/file_journal/main.omg` exits 7
after creating, writing, inspecting, reading, renaming and removing the file
and its directory. macOS ARM64 and both Linux targets select the POSIX portable
provider; matching-host execution remains separate evidence.

Windows selects the portable provider over its retained raw-family closure.
On windows_x86_64, `omega run` exits 7. macOS and Linux execution remain
separate evidence under **FILESYSTEM-PLATFORM-SERVICES**.
Terminal execution still declines the selected foreign bindings until trusted
host adapters supply them.

The authored `/tmp/omega_journal_dir` is rooted, not relative to the working
directory. A Windows runner must explicitly provision a writable `\tmp` on
its current drive. The library must not silently reinterpret that path.

The [host re-cast rung](host_recast/main.omg) isolates the bounded original
backing required by Darwin filesystem metadata writes. It passes a 144-byte
window starting eight bytes into a larger array through a checked foreign
call twice. A preceding shared whole-array comparison establishes the canaries
in that same native residence without lending them mutably. Its oracle checks that both returned addresses are nonzero and
identical, and checks the first, interior and last bytes and both adjacent
canaries. Between calls, an ordinary guest array copy retains byte 17 while
an in-place write changes the original byte to 29; the next native call must
write through that same original home. The [reject twin](host_recast/invalid/main.omg) shortens only that
window to 143 bytes and must fail the same minimum-length contract. This
macOS host witness uses the existing native binding surface while the
Build-owned binding migration remains open. It establishes original-cell
transport, not filesystem service supply or the parent task's completion.
