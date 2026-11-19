# Bootstrap on Linux x86-64

Run the checked-in binary with the source tree and a new output directory.
The default `gamma-compiler` takes Gamma source on stdin and writes a
standalone Linux x86-64 executable on stdout.

```sh
bootstrap/linux_x86_64/bootstrap . /tmp/omega-gamma
```

The command runs offline with an empty `PATH`. It needs Linux x86-64, space
for the stage files, permission to execute programs in the new directory,
and the virtual memory provision of the Alpha seed. The first compiler that
can build Omega and the subsequent Omega self-build remain open.

### Build a supplied Gamma program in the same command

A third argument supplies one complete Gamma source file. The driver snapshots
its bytes in `program.gamma` before constructing the compilers, then runs
`gamma-compiler` on that snapshot. Successful compilation publishes the native
`program` executable. An invalid program leaves the rebuilt Gamma compiler
available and returns the producer's failure status.

A checkout carries the complete readable Delta compiler source as `delta.gamma`.
It comprises the canonical entry followed by every implementation member in
the existing manifest order. To construct the native Delta compiler, run

```sh
bootstrap/linux_x86_64/bootstrap . /tmp/omega-delta bootstrap/linux_x86_64/delta.gamma
```

The executable is `/tmp/omega-delta/program`. With a previously constructed
Gamma compiler, `gamma-compiler < bootstrap/linux_x86_64/delta.gamma > delta`
emits the same bytes. The cold command reads the distributed file directly.

Edit the canonical Delta entry and members in `bootstrap/interpreted/3_delta/`. When
those sources change, refresh the distribution from the repository root.

```sh
mbx xtask source-closure bootstrap/interpreted/3_delta/implementation/implementation.gamma.sources \
    bootstrap/linux_x86_64/delta.gamma --prefix bootstrap/interpreted/3_delta/delta_compiler.gamma
```

The Delta identity test reruns the existing materializer and compares its
output with the distributed file byte for byte. This adds one readable copy of
the compiler source to the checkout and keeps manifest parsing, inventory
checks and hashing in distribution maintenance.
[Delta source ownership](../interpreted/3_delta/README.md#selected-staged-delta-compiler)

### Compile Gamma source emitted by the supplied program

A fourth argument supplies input to the native `program` built in the previous
step. The driver saves the bytes in `program.input`, runs `program` on them,
then gives its successful output to the rebuilt Gamma compiler. The output
directory retains the intermediate bytes as `producer.output`. When Gamma
accepts them it writes the native executable `successor`.

For example, the Delta compiler can emit the Gamma source for the Epsilon
evaluator. This invocation takes its canonical Delta compilation request.

```sh
bootstrap/linux_x86_64/bootstrap . /tmp/omega-epsilon \
    bootstrap/linux_x86_64/delta.gamma /path/to/epsilon-request.bin
```

That request contains the 16-byte DCREQ header, the ordered Delta source of
the Epsilon evaluator and its entry, and the Gamma support bytes. The
[successor test](../independent/README.md#native-successor-customers) writes
this input to `build/independent-successors/epsilon-request.bin`. A source
distribution can supply the same file alongside the checkout. The invocation
uses those bytes directly and runs offline. The native `successor` already
contains the Gamma program, so its stdin starts at the sealed `EEREQ` header.
[Epsilon evaluation request](../interpreted/4_epsilon/EVALUATOR_ENTRY.md)

### Run the compiled Alpha tape route

Pass `--alpha` before the source root to run the same construction with
the Gamma-to-Alpha backend. For example, build the Epsilon evaluator from
the same distributed Gamma source and Delta request as above.

```sh
bootstrap/linux_x86_64/bootstrap --alpha . /tmp/omega-epsilon-alpha \
    bootstrap/linux_x86_64/delta.gamma /path/to/epsilon-request.bin
```

The Gamma compiler source uses `compiled_vm/alpha_backend.gamma`. The driver
stamps its output into the original Linux Alpha seed at every executable
stage. The evaluator emits `gamma0.tape`. Two self-compilations produce
`gamma1.tape` and `gamma2.tape`. The driver compares all three stamped
executables byte for byte. The resulting `gamma-compiler` accepts ordinary Gamma source
on stdin and writes Alpha tape. The optional inputs produce `program.tape`
and `successor.tape`, together with the independently executable `program`
and `successor`. For the illustrated inputs those are the Delta compiler and
Epsilon evaluator. Run `successor` with the same sealed Epsilon request as
the native route. Each output tape must fit the seed's 16 MiB field including
its four-byte length. [Compiled VM](../compiled_vm/README.md)

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
   and the selected backend with one newline between files. The native route
   uses `independent/native.gamma`. The Alpha route uses
   `compiled_vm/alpha_backend.gamma`. The driver writes
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

For a function-labeled disassembly, omit `,-s` from the linker option and run
`objdump -d /tmp/omega-bootstrap`. Its `.text` bytes match the stripped build.

The Linux test supplies the five compiler source files, checks every stamped image
against the audited seed, checks each static executable, compiles two changed
Gamma programs, runs both on changed bytes and exercises malformed inputs.
Run `python3 tests/bootstrap/linux-seed-construction.py` for native images and
`python3 tests/bootstrap/linux-seed-construction.py --alpha` for Alpha tapes.
