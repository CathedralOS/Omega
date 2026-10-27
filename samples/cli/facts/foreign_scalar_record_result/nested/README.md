# Recursive owned foreign record results

Parent: **NORMALIZED-ABI-LOWERING** in [TASKS.md](../../../../../TASKS.md).
This is another production of the [existing quotient/remainder fact](../README.md).

**A copyable C quotient/remainder record preserves both signed fields and
independent earlier-call contents through transport and copying into the
consuming operation.** Every witness repeats this sentence and the original
input pairs: `-70000000000 / 3` and `70000000000 / 9`.

`Word [copy] { value: i64; }` is embedded twice in
`Division [copy] { quotient: Word; remainder: Word; }`. The consumer passes the
first whole `Division` twice, invokes the second foreign operation, then checks
all original, copied and later signed fields through `.quotient.value` and
`.remainder.value`. The first and copied results must still contain
`-23333333333, -1`; the later result must contain `7777777777, 7`.
These comparisons are runtime oracles, not static quotient/remainder promises.
After all six Omega field checks, the same consumer passes both complete earlier
records to the stateless C `nesteddiv_verify` function. It checks the same
`-23333333333, -1` pair in each argument, confirming nested owned input transport
and copied contents as well as the foreign result publication. It contributes
no new arithmetic or static guarantee.

The [root](main.omg) repeats the [local](local/main.omg) witness. The other places
are [field](field/main.omg), [parameter](parameter/main.omg),
[call result](call_result/main.omg), [match join](match_join/main.omg),
[const name](const_name/main.omg), [nested machine](other_state/main.omg),
[other machine](other_machine/main.omg), and [target body](target_body/main.omg).
The match join retains distinct true/false markers alongside the same foreign
result and checks both paths. The const place names the original construction
inputs; it does not evaluate a foreign call as a constant initializer.

Each place and the root has an `invalid/main.omg` that removes only the outer
`Division`'s `[copy]` property. `Word` remains copyable. The repeated whole-value
use in `receive` must reject with `holds no value`; the input pairs, nested
layout, operation, comparisons and branch markers are unchanged. The nine
positive/twin pairs are copied exactly into
`tests/omega/{pass,fail}/facts/foreign_nested_scalar_record_result_through_*`.
Every positive declares `Expected exit: 0` and all four hosted `ProgramEntry`
bindings. External leaves and their locator-producing helpers are public;
terminal helpers use nested `machine` declarations and explicit arguments.

## Real portable provider

[fixture/provider.c](fixture/provider.c) calls the actual libc `lldiv` and
returns the corresponding nested C `struct division`. It does not calculate a
replacement quotient/remainder implementation. Its assertions pin signed
64-bit operands, the eight-byte `word`, the sixteen-byte `division`, both
containing offsets, and scalar alignment. The same `word` type appears beneath
two distinct outer fields. The C self-test copies the first result, makes the
second call, then checks all six signed values. Undefined division input pairs
refuse before calling libc.
The self-test also invokes the whole-record verifier and confirms that changing
one copied remainder makes it return false. Both exported functions use the
same ordinary nested C layout and exact library binding.

[fixture/prepare.py](fixture/prepare.py) follows the existing portable C fixture
workflow: it builds a matching-host shared library and self-test, executes the
self-test, and records the exact commands and file hashes. Linux exports the
exact `NESTDIV_1.0` version; macOS uses its declared install name; Windows
exports the same wrapper from its DLL. The ordinary generic native-provider
sponsor must execute this exact library and complete admitted calling
application. No symbol-specific Terminal adapter or authored byte decoder is
part of this row.

## Required evidence

- Establishment and identity: complete public nested result shape and typed
  containing paths must validate before publishing a fresh owned result home.
  The earlier home and its copy remain distinct from the later call's result.
- Composition: every place transports the same whole record and copy obligation.
  Match markers make branch inversion observable without changing the arithmetic.
- Soundness: removing only outer copy custody rejects; representation creates
  no reference, claim, lifetime or descendant qualification authority.
- Psi 05–07: normal completion retains the complete recursive result type and
  ordinary structural custody; canonical artifact inspection must accept it.
- Omega 00–02: recursive public shape, exact foreign application and target ABI
  placement remain authenticated. Equal flattened bytes cannot erase containing
  paths or conflate repeated child types.
- Omega 03–06: register results or indirect return homes preserve every nested
  scalar and the earlier result across the second call.
- Omega 07–09: real matching-host native execution agrees with Terminal on every
  positive; each reject twin fails for its own copy obligation. Four declarations
  alone supply no execution credit.

The governing contracts are [boundary shapes](../../../../../wiki/spec/build/boundary_shapes.md),
[foreign storage](../../../../../wiki/spec/build/foreign_storage.md), and
[Terminal ownership](../../../../../wiki/spec/terminal-psi/ownership.md).
## Measured qualification

Fixed current-main baseline `abfac3bb8812961984f703d77e6cabbcda681ae5`
executes the real nested-result/owned-input program natively at zero while
Terminal declines its direct-scalar-field requirement. The initial macOS loader
setup failure remains separate from that corrected observation.

Optimized actual producer `77f259b46d0d2da38796a9f00f6a350049c10d34`
executes the root and all nine places with native exit 0 and Terminal exit 0
on macOS ARM64. All ten produced Terminal artifacts pass canonical inspection;
the nine copy-removal twins reject for `holds no value`. The real C self-test
also verifies both whole-value arguments and rejects a changed copied field.
The containing paths remain exact even when both children share the `Word` type.

The same producer supplies 43 actual native/Terminal agreements in the affected
cohort, including existing flat integer/floating results, float classification,
standard input, atomic outcome payloads and retained nested record/array calls.
The 33-case source comparison and 43-case runtime comparison lose no baseline
outcomes; authored exits and output oracles also hold. The two affected Rust
packages record 67 passing tests and one preexisting ignored test. Earlier
flat-row observations remain separate evidence.

The code also admits owned nested inputs through the same public-shape join;
each place checks both by-value C arguments after its six Omega observations.
Publication uses ordinary fresh placed storage after validating the complete
ordered scalar roster and every typed containing path. It grants no reference,
descendant qualification, content or claim authority from equal bytes.

The authored bindings cover four hosted targets; these observations establish
macOS ARM64 execution only. Matching-host qualification on Windows and both
Linux targets remains under the parent acceptance. See [the run record](qualification.json).
Raw compiler profiles, C provider identities, frozen inputs, engine outputs,
canonical artifacts and qualification are retained in
`build/foreign-frontier-evidence`.

The flat record row's earlier observations do not establish recursive owned
result publication. Arrays, variants, references and qualified descendants
remain separate acceptance.
