# Materialized C scalar-record results

Parent: [NORMALIZED-ABI-LOWERING](../../../../TASKS.md).

**A copyable C quotient/remainder record preserves both signed fields and
independent earlier-call contents through transport and copying into the
consuming operation.** Every place uses this same fact and libc `lldiv`.
The consumer passes the first whole record twice, calls C again with different
inputs, then reads all original, copied and later fields. The comparisons are
runtime oracles; they introduce no static quotient/remainder refinement.
Each reject twin removes only `[copy]` and must reject the second whole-value
use with `holds no value`. Thus the static obligation is copy custody, not a
post-hoc equality guard or an invented opaque-provider guarantee.

The nine places are [local](local/main.omg), [field](field/main.omg),
[parameter](parameter/main.omg), [call result](call_result/main.omg),
[match join](match_join/main.omg), [const name](const_name/main.omg),
[nested machine](other_state/main.omg), [other machine](other_machine/main.omg),
and [target body](target_body/main.omg). Every place has `invalid/main.omg`;
matching copies are pinned under `tests/omega/{pass,fail}/facts/foreign_scalar_record_result_through_*`.
The root repeats the local witness. The const place names the C construction
inputs: a real foreign operation is not a constant initializer.

The existing `providers/foreign_multiregister_result_exit` exposes the baseline:
C returns the correct native fields, while Terminal declines the absent trusted
binding. This row adds no replacement C implementation or source-name dispatch.
Omega's adapter admits the complete normalized libc binding, selected requirement,
ordered scalar signature, public two-i64 record shape and evaluated calling
application. Runtime-ABI invokes actual C `lldiv`; denominator zero and the
unrepresentable `i64::MIN / -1` pair refuse before the call.

Terminal publishes actual scalar contents into its ordinary fresh placed record
storage. The scalar sum-case path uses the same carrier/publication and retains
its real discriminator; a record has none. Complete ordered field IDs, types and
actual value restrictions are checked before publication. The host supplies no
opaque storage identity, reference backing, claim, domain qualification or
projected evidence. Copies and moves then use ordinary record custody.

All four hosted entries are declared. The selected C record is 16 bytes:
macOS ARM64 and Linux x86-64 return it in two registers; Windows x64 uses
caller-owned result memory, shifts the two scalar inputs and returns that
result pointer. The Windows binding is the exact
`api-ms-win-crt-utility-l1-1-0.dll` / `lldiv` import from Microsoft's x64 UCRT
SDK import library, forwarding to the actual `ucrtbase.lldiv` export. Linux
uses `libc.so.6` with the selected architecture's symbol version; macOS uses
`/usr/lib/libSystem.B.dylib` / `_lldiv`. Matching-host observations, producer
identities and retained products are recorded separately in qualification/PR
evidence; declarations and cross-target builds provide no runtime credit.
Linux ARM64 remains unmeasured until a matching host runs the row.

Matching-host evidence for this row: optimized Mac producer `2cffebee3b86`
passed all nine pinned positives and rejects plus the root and nine ordinary
public `run --both` routes. Hosted run
[40201214950](https://github.com/CathedralOS/Omega/actions/runs/40201214950)
built producer `ffb3227a24d6` on Linux x86-64 and Windows x64: each host passed
nine positives with native/Terminal exit zero and agreement, and nine exact
`holds no value` rejects. Windows also resolved the named API-set export to
the loaded UCRT image and called both signed input pairs. The retained ZIPs,
producer profiles, native/Terminal products and original reports are bound
in the scoped qualification evidence. These observations keep their actual
producer identities after documentation and golden-only commits.

This advances real Windows indirect C results when qualified there. It does
not qualify a macOS indirect C result (this pair is register-returned), borrowed
descriptor arguments, stack-slot overflow, or the parent's complete host matrix.
The separate `aggregate_struct_return_abi` Quartet is Omega-produced and cannot
substitute for an actual C return.

Applicable requirement layers:

- Establishment/identity: exact foreign admission and complete field validation
  publish a fresh owned occurrence; the first and later results remain distinct.
- Composition: all nine transports preserve the same fields and copy obligation;
  both match paths and explicit target bodies consume the same operation.
- Soundness: the noncopy twins reject before execution; unavailable bindings and
  unsupported authority refuse. Signed input/field widths remain exact.
- Psi 05–07: actual fields use ordinary record cells, copies and state arrivals;
  existing sum-case discriminators retain their separate field namespace.
- Omega 00–02: the exact normalized C application retains ordered formals,
  complete result geometry and validated ABI placements.
- Omega 03–06: caller homes and live-across-call transport preserve the first
  result, including Windows' hidden result pointer and subsequent scalar inputs.
- Omega 07–09: real matching-host images resolve the admitted libc import and
  execute against Terminal. No unobserved target receives engine credit.

```sh
omega run --both samples/cli/facts/foreign_scalar_record_result/local/main.omg
omega --check samples/cli/facts/foreign_scalar_record_result/local/invalid/main.omg
python3 tools/ci/compiler_foreign_records.py
```
