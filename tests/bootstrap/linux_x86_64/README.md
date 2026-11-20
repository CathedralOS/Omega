# Cold bootstrap test launcher for Linux x86-64

Run the checked-in binary with the source tree and a new output directory.
The default `gamma-compiler` takes Gamma source on stdin and writes a
standalone Linux x86-64 executable on stdout.

```sh
tests/bootstrap/linux_x86_64/bootstrap . /tmp/omega-gamma
```

The command runs offline with an empty `PATH`. It needs Linux x86-64, space
for the stage files, permission to execute programs in the new directory,
and the virtual memory provision of the Alpha seed.

### Build a supplied Gamma program in the same command

A third argument supplies one complete Gamma source file. The driver snapshots
its bytes in `program.gamma` before constructing the compilers, then runs
`gamma-compiler` on that snapshot. Successful compilation publishes the native
`program` executable. An invalid program leaves the rebuilt Gamma compiler
available and returns the producer's failure status.

### Build the Delta compiler from its source tree

Use `--delta` to read the Delta compiler entry and the implementation manifest
from the supplied source root. Each member path resolves relative to the
manifest. The driver writes the entry and the members in the declared order
with no separator. It saves the result as `program.gamma` and compiles it to
`program`.

```sh
tests/bootstrap/linux_x86_64/bootstrap --delta . /tmp/omega-delta
```

The driver checks the manifest header and row format, strictly increasing
identities, unique relative Gamma paths, regular member files, source text
bytes and each member's declared length. It rejects symlinks in member paths.
It checks the spelling of each digest. To verify the SHA-256 values and the
full source inventory independently, materialize the same input and compare
it with the driver's snapshot.

```sh
mbx xtask source-closure bootstrap/delta/compiler/implementation/implementation.gamma.sources \
    /tmp/audited-delta.gamma --prefix bootstrap/delta/compiler/delta_compiler.gamma
cmp /tmp/audited-delta.gamma /tmp/omega-delta/program.gamma
```

[Delta source ownership](../../../bootstrap/delta/compiler/README.md)

### Compile Gamma source emitted by the supplied program

A final optional argument supplies input to the `program` built in the previous
step. The driver saves the bytes in `program.input`, runs `program` on them,
then gives its successful output to the rebuilt Gamma compiler. The output
directory retains the intermediate bytes as `producer.output`. When Gamma
accepts them it writes the native executable `successor`.

For example, the Delta compiler can emit the Gamma source for the Epsilon
evaluator. This invocation takes its canonical Delta compilation request.

```sh
tests/bootstrap/linux_x86_64/bootstrap --delta . /tmp/omega-epsilon /path/to/epsilon-request.bin
```

That request contains the 16-byte DCREQ header, the ordered Delta source of
the Epsilon evaluator and its entry, and the Gamma support bytes. The
[successor test](../../../wiki/development/bootstrap/native.md#native-successor-customers) writes
this input to `build/independent-successors/epsilon-request.bin`. A source
distribution can supply the same file alongside the checkout. The invocation
uses those bytes directly and runs offline. The native `successor` already
contains the Gamma program, so its stdin starts at the sealed `EEREQ` header.
[Epsilon evaluation request](../../../bootstrap/epsilon/interpreter/EVALUATOR_ENTRY.md)

### Run the compiled Alpha tape route

Pass `--alpha` before the source root to run the same construction with
the Gamma-to-Alpha backend. For example, build the Epsilon evaluator from
the same Delta source tree and request as above.

```sh
tests/bootstrap/linux_x86_64/bootstrap --alpha --delta . /tmp/omega-epsilon-alpha /path/to/epsilon-request.bin
```

The Gamma compiler source uses `gamma/compiler/alpha/alpha_backend.gamma`. The driver
stamps its output into the original Linux Alpha seed at every executable
stage. The evaluator emits `gamma0.tape`. Two self-compilations produce
`gamma1.tape` and `gamma2.tape`. The driver compares all three stamped
executables byte for byte. The resulting `gamma-compiler` accepts ordinary Gamma source
on stdin and writes Alpha tape. The optional inputs produce `program.tape`
and `successor.tape`, together with the independently executable `program`
and `successor`. For the illustrated inputs those are the Delta compiler and
Epsilon evaluator. Run `successor` with the same sealed Epsilon request as
the native route. Each output tape must fit the seed's 16 MiB field including
its four-byte length. [Compiled VM](../../../wiki/development/bootstrap/compiled-alpha.md)

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
3. The driver joins `gamma/compiler/compiler.gamma`, `gamma/compiler/frontend.gamma`
   and the selected backend with one newline between files. The native route
   uses `gamma/compiler/linux_x86_64/native.gamma`. The Alpha route uses
   `gamma/compiler/alpha/alpha_backend.gamma`. The driver writes
   the four-byte little-endian source length, those exact source bytes and
   a second copy as input for the Gamma program. The evaluator runs this
   request and produces the first compiler, `gamma0`, after stamping its tape
   when the Alpha target is selected.
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
five supplied source files are the inputs for constructing the Gamma compiler.
Self-reconstruction checks the Beta bytes. The finite Beta audit and Alpha
semantics explain what those bytes do. The driver handles file I/O, process
invocation, framing, byte comparisons and optional Delta manifest assembly.
A replacement driver can substitute stages, so its executable is also part
of the native trust root.
[Alpha](../../../bootstrap/alpha/README.md) · [Beta audit](../../../bootstrap/beta/AUDIT.md) · [Gamma compiler](../../../wiki/development/bootstrap/native.md)

The test launcher is freestanding C with an assembly entry for Linux system calls.
For a source-to-binary comparison, run this command from the repository root
using GCC 14.2.0 and GNU binutils 2.44. These development tools participate
when reproducing the driver itself. The checked-in binary needs only the OS
during compiler construction.

```sh
gcc -std=c11 -Os -ffreestanding -fno-stack-protector -fno-asynchronous-unwind-tables \
    -fno-pie -no-pie -nostdlib -static -Wl,--build-id=none,-s -Wall -Wextra -Werror \
    tests/bootstrap/linux_x86_64/entry.s tests/bootstrap/linux_x86_64/construct.c -o /tmp/omega-bootstrap
cmp /tmp/omega-bootstrap tests/bootstrap/linux_x86_64/bootstrap
```

For a function-labeled disassembly, omit `,-s` from the linker option and run
`objdump -d /tmp/omega-bootstrap`. Its `.text` bytes match the stripped build.

The Linux test supplies the five compiler source files, checks every stamped image
against the audited seed, checks each static executable, compiles two changed
Gamma programs, runs both on changed bytes and exercises malformed inputs.
Run `python3 tests/bootstrap/linux-seed-construction.py` for native images and
`python3 tests/bootstrap/linux-seed-construction.py --alpha` for Alpha tapes.
Add `--delta` to either test command to exercise the canonical Delta source
tree, malformed manifests, and compilation and execution of a Delta customer.
