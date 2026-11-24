# Private entry RAM and original backing

The frozen project binds the standard Linux `ProgramEntry` to a machine with a
placed parameter. The current hosted contract supplies zero arguments. Supplying
initial bytes on the command line cannot install a new argument. The
[entry contract](../../../../wiki/spec/build/entry_roots.md#entry-shape-and-arrival-bridge)
identifies which caller can lend each visible argument.

A future run needs an explicitly installed or embedded entry that lends the
original allocation. The files in this directory retain the input and the
independent stdout and backing expectations for that run.

## Historical probe

This probe was written to lend newly constructed private `Registers` RAM directly to a placed entry parameter. The initial status is 41 and transmit is 3. The entry reads status through the sealed 32-bit accessor at offset 0, prints `41`, and writes 9 through the sealed 8-bit accessor at offset 4. After normal return, the runtime observes the original allocation as `2900000009000000`.

Command used with the previous hosted entry contract:

```sh
omega run --both --keep --entry-ram 0=2900000003000000 main.omg
```

`0` is the authored source parameter position of the free entry wrapper. The bytes encode the complete original scalar record in little-endian order, including initialized zero padding. The runtime validates every scalar's actual codec and complete type, exact retained placement, extent, alignment, access and no-code cleanup before constructing storage. The flag explicitly sponsors a new private guest allocation; it supplies no address, external object or host authority.

With `--keep`, the run directory retains the exact `program.oterm` canonical replay product, native executable `omega-program`, `native.identity.txt`, actual `native.entry-ram.frames`, `terminal.stdout`, `terminal.stderr`, `terminal.effects`, `native.entry-ram.0.bin`, and `terminal.entry-ram.0.bin`. Both backing files must contain the eight original bytes `29 00 00 00 09 00 00 00`. The native observer uses a dedicated runtime-owned channel, separately from guest stdout and stderr. Its exact frame binds the canonical artifact digest, placement seal, parameter position and byte extent. The independent image reader checks original allocation disjointness, initialization before lending, exact argument pointers, normal-return ordering, and observation of the same residences. A missing or extra frame and incomplete writes refuse. `omega psi inspect --artifact program.oterm` independently admits the retained canonical product and presents its program fingerprint and exact placed row; those must match the captured frame. Unstructured native-object Debug text is not a replay format and is not retained.

This project binds Linux. The [installed entry storage fact](../../facts/installed_entry_storage/README.md) identifies the compiler revisions and host configurations on which an earlier entry contract ran. No recipe is supplied by default. Unsupported qualifications, linear custody, nominal cleanup, predicates, compound and atomic fields, external/MMIO loans, raw host pointers, callbacks, exports and destructive ExternalTake demand remain unavailable.

The frozen files `entry_ram.hex`, `expected_stdout.txt`, and `expected_original_backing.hex` state the input and independent authored oracles before execution. Qualification must retain actual native and Terminal values, including the original backing after loan retirement; matching exit codes alone do not establish this sample.
