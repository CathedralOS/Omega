# Build the native Gamma compiler on Linux x86-64

Run the checked-in binary with the source tree and a new output directory.
The resulting `gamma-compiler` takes Gamma source on stdin and writes a
standalone Linux x86-64 executable on stdout.

```sh
bootstrap/linux_x86_64/bootstrap . /tmp/omega-gamma
```

The command runs offline with an empty `PATH`. It needs Linux x86-64, space
for the stage files, permission to execute programs in the new directory,
and the virtual memory provision of the Alpha seed. The first compiler that
can build Omega and the subsequent Omega self-build remain open.

## Follow the construction

1. The binary carries the audited Linux Alpha executable and the admitted
   1,773-byte Beta tape. Its assembly source includes both parts of the
   original ELF around the 16 MiB zero-filled tape section. The driver
   restores that section as a sparse file, inserts the tape and runs the
   resulting `beta-seed` on `beta_compiler.beta`. Every byte of its output
   must match the admitted tape before the rebuilt Beta can run.
2. The rebuilt Beta compiles `gamma_evaluator.beta` to `gamma.tape`. The
   driver puts those produced bytes in the same Alpha executable to obtain
   `gamma-evaluator`.
3. The driver joins `compiled_vm/compiler.gamma`, `compiled_vm/frontend.gamma`
   and `independent/native.gamma` with one newline between files. It writes
   the four-byte little-endian source length, those exact source bytes and
   a second copy as input for the Gamma program. The evaluator runs this
   request and produces the first native compiler, `gamma0`.
4. `gamma0` reads the joined Gamma source and emits `gamma1`. That compiler
   reads the same source and emits the final executable. The driver compares
   both rebuilds byte for byte before publishing `gamma-compiler`. The
   output directory retains the tapes, compiler request and each constructed
   executable for inspection.

Each producer writes to a blocking pipe. The driver checks the disk writes
and waits for the producer before renaming a completed stage. A failed
command leaves the newly created directory available for diagnosis, including
any `stage.partial`. A fresh invocation requires its own new directory.

## Audit the native entry

The Linux kernel, the checked-in driver, its embedded Alpha and Beta, and the
five supplied source files are the inputs. Self-reconstruction checks the
Beta bytes. The finite Beta audit and Alpha semantics explain what those
bytes do. The driver source owns file I/O, process invocation, framing and
comparison. A replacement driver can substitute stages, so its executable
is also part of the native trust root.
[Alpha](../interpreted/0_alpha/README.md) · [Beta audit](../interpreted/1_beta/AUDIT.md) · [Gamma compiler](../independent/README.md)

The driver is freestanding C with an assembly entry for Linux system calls.
For a source-to-binary comparison, run this command from the repository root
using GCC 14.2.0 and GNU binutils 2.44. These development tools participate
when reproducing the driver itself. The checked-in binary needs only the OS
during compiler construction.

```sh
gcc -std=c11 -Os -ffreestanding -fno-stack-protector -fno-asynchronous-unwind-tables \
    -fno-pie -no-pie -nostdlib -static -Wl,--build-id=none,-s -Wall -Wextra -Werror \
    bootstrap/linux_x86_64/entry.s bootstrap/linux_x86_64/construct.c -o /tmp/omega-bootstrap
cmp /tmp/omega-bootstrap bootstrap/linux_x86_64/bootstrap
```

The Linux test supplies only the five source files, checks every stamped image
against the audited seed, checks each static executable, compiles two changed
Gamma programs, runs both on changed bytes and exercises malformed inputs.
Run `python3 tests/bootstrap/linux-seed-construction.py`.
