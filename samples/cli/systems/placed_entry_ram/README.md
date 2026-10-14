This sample lends newly constructed private `Registers` RAM directly to a placed entry parameter. The initial status is 41 and transmit is 3. The entry reads status through the sealed 32-bit accessor at offset 0, prints `41`, and writes 9 through the sealed 8-bit accessor at offset 4. After normal return, the runtime observes the original allocation as `2900000009000000`.

Prepare this ordinary local project through its package review, then run the public CLI:

```sh
omega run --both --keep --entry-ram 1=2900000003000000 main.omg
```

`1` is the authored source parameter position. The bytes encode the complete original scalar record in little-endian order, including initialized zero padding. The runtime validates every scalar's actual codec and complete type, exact retained placement, extent, alignment, access and no-code cleanup before constructing storage. The flag explicitly sponsors a new private guest allocation; it supplies no address, external object or host authority.

With `--keep`, the run directory retains the exact `program.oterm`, `native.object.txt` replay model, `native.identity.txt`, actual `native.entry-ram.frames`, `terminal.stdout`, `terminal.stderr`, `terminal.effects`, `native.entry-ram.1.bin`, and `terminal.entry-ram.1.bin`. Both backing files must contain the eight original bytes `29 00 00 00 09 00 00 00`. The native observer uses a dedicated runtime-owned channel, separately from guest stdout and stderr. Its exact frame binds the canonical artifact digest, placement seal, parameter position and byte extent. The independent image reader checks original allocation disjointness, initialization before lending, exact argument pointers, normal-return ordering, and observation of the same residences. A missing or extra frame and incomplete writes refuse. `omega inspect-terminal --artifact program.oterm` independently admits the retained canonical product and presents its program fingerprint and exact placed row; those must match the captured frame.

The initial physical lender supports Linux x86-64. Other native deployments retain the missing-lender refusal. No recipe is supplied by default. Unsupported qualifications, linear custody, nominal cleanup, predicates, compound and atomic fields, external/MMIO loans, raw host pointers, callbacks, exports and destructive ExternalTake demand remain unavailable.

The frozen files `entry_ram.hex`, `expected_stdout.txt`, and `expected_original_backing.hex` state the input and independent authored oracles before execution. Qualification must retain actual native and Terminal values, including the original backing after loan retirement; matching exit codes alone do not establish this sample.
