# file_journal

The journal imports the portable `Filesystem` interface and the `Path`
domain from the system package. Its Build grants that dependency content read
and write, metadata query and mutation, namespace mutation and process exit.
Each string passed as a path must satisfy the path domain. The entry declares
blocking because the filesystem calls can wait for the host.
[Provider selection](../../../../wiki/spec/build/platform_services.md#provider-closure-transport)

From this directory, run the engines separately on the same host.

```sh
omega run --engine native build.omg
omega run --engine psi build.omg
```

Success exits **7**. The journal creates `/tmp/omega_journal_dir`, writes the
17 bytes `omega fs journal\n`, checks their size and reads them back, renames
the file, then removes the file and directory. It exits with the number of
completed steps on failure. The sample owns that fixed directory and removes
its known files at startup. Run one engine at a time.

On Linux x86-64 native and Psi both returned 7. Psi charged 18,517 work units.
A syscall trace confirmed the exact bytes on read and write and the final
directory removal. A pre-existing nonempty directory containing an unrelated
file made the application exit 0 and preserved that file.

The Windows x86-64 qualification used a writable `\tmp` on the current
drive. The earlier native and Psi runs both exited 7.
[Windows run](https://github.com/CathedralOS/Omega/actions/runs/41444385907) ·
[Qualification record](../../../../wiki/drafts/measurements/windows_file_journal_2026_11_02/README.md)

The [host re-cast rung](../../probes/file_journal/host_recast/main.omg) isolates the bounded original
backing required by Darwin filesystem metadata writes. It passes a 144-byte
window starting eight bytes into a larger array through a checked foreign
call twice. A preceding shared whole-array comparison establishes the canaries
in that same native residence without lending them mutably. Its oracle checks that both returned addresses are nonzero and
identical, and checks the first, interior and last bytes and both adjacent
canaries. Between calls, an ordinary guest array copy retains byte 17 while
an in-place write changes the original byte to 29; the next native call must
write through that same original home. The [reject twin](../../probes/file_journal/invalid_host_recast/main.omg) shortens only that
window to 143 bytes and must fail the same minimum-length contract. This
macOS host witness uses the existing native binding surface while the
Build-owned binding migration remains open. It establishes original-cell
transport, not filesystem service supply or the parent task's completion.
