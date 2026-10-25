# Indirect C floating record result

Parent: **NORMALIZED-ABI-LOWERING** in [TASKS.md](../../../../TASKS.md).

A copyable indirect C affine record preserves all six floating fields and
independent earlier-call contents through transport and copying into the
consuming operation.

The macOS ARM64 provider is the actual CoreGraphics `CGAffineTransformMake`
export. Its C result contains six ordered `double` fields, 48 bytes with
8-byte alignment. This exceeds the homogeneous floating aggregate register
return limit and exercises the evaluated indirect-result calling plan.
`consume` makes a second call, then checks every field of the first result,
its copy, and the second result against their distinct constructor inputs.

Fresh baseline `c1fcc42fe4f1` executed the root natively with exit 0; Terminal
admission declined because no trusted adapter matched the exact foreign
binding. The copy-invalid root rejected for reuse after consumption. The new
adapter calls the real C export and publishes the complete typed fields through
the existing fresh-owned scalar-record result materializer. Its shared admission
retains ordered formals, exact field codecs, native geometry, and the complete
admitted calling application. Pending target layouts refuse rather than being
treated as default C storage.

The same consumer is reached through [a local](local/main.omg),
[a field](field/main.omg), [a parameter](parameter/main.omg),
[a call result](call_result/main.omg), [a match join](match_join/main.omg),
[constant names](const_name/main.omg), [a nested machine](other_state/main.omg),
[another machine](other_machine/main.omg), and
[another target body](target_body/main.omg). The constant names are the six
immutable constructor arguments; this does not claim compile-time foreign
execution or a constant C record. Each place has an `invalid/main.omg` twin
that removes only `[copy]` from the result type and must reject whole-value
reuse. The root has the same control.

The row establishes the following requirements on the measured Mac host:

- **Establishment:** the real C constructor supplies all six typed coefficients;
  the consumer checks all 18 first/copy/second fields.
- **Identity and lifetime:** a later call cannot overwrite the first result's
  owned storage, and an allowed copy preserves all six fields.
- **Composition:** all nine transports feed the same consumer and operation.
- **Soundness boundaries:** removing copy permission rejects; exact normalized
  library/symbol/target, signature, field storage and application admission
  govern the host adapter, without inventing grants or reference authority.
- **Psi 05–07:** checked multiplicity and calls lower to an unchanged typed
  Terminal artifact whose aggregate publication owns ordinary fresh storage.
- **Omega 00–02:** Build selects the actual CoreGraphics binding, exact f64
  layout and evaluated native indirect-return application.
- **Omega 03–06:** floating argument registers, caller result memory, copies,
  and live first-result homes survive the second call.
- **Omega 07–09 and emission:** the emitted Mach-O resolves the external C
  symbol and must agree with actual Terminal execution at exit 0.

Sample Builds declare all four hosted entry slots. The matching positive corpus
pins select only `macos_arm64` and carry `expected_native_exit.macos_arm64.txt`;
their source bodies match the samples. CoreGraphics availability and execution
on another host are not claimed. No substitute operation supplies portability.
The parent's Linux ARM64, other-host indirect result, borrowed-descriptor and
retained-backing requirements remain open.

Current implementation and witness checkpoint: final optimized public runs,
bounded corpus validation, independent review and publication are pending.
