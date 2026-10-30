# Owned foreign arrays of records and byte fields

Parent: **NORMALIZED-ABI-LOWERING** in [TASKS.md](../../../../../TASKS.md).
This continues the [existing quotient/remainder fact](../README.md) through
fixed arrays in an ordinary owned public record.

**A copyable C quotient/remainder record preserves both signed fields and
independent earlier-call contents through transport and copying into the
consuming operation.** Every witness uses the same two real `lldiv` input
pairs: `-70000000000 / 3` and `70000000000 / 9`.

`Word [copy] { value: i64; guard: [u8; 4]; }` appears twice in
`Division [copy] { parts: [Word; 2]; }`. Index zero is the quotient and index
one the remainder. The first result has quotient guards `[17,34,51,68]` and
remainder guards `[85,102,119,136]`; each byte in the later positive result is
one greater. The consumer passes the first complete result twice, performs the
second foreign call, checks the same six signed values and all twenty-four
guard bytes, then passes both earlier records to the stateless C verifier.
The first and copied results must still contain `-23333333333, -1`; the later
result must contain `7777777777, 7`. Runtime comparisons add no static result
qualification or post-hoc proof guard.

The [root](main.omg) repeats [local](local/main.omg). The other places are
[field](field/main.omg), [parameter](parameter/main.omg),
[call result](call_result/main.omg), [match join](match_join/main.omg),
[const name](const_name/main.omg), [nested machine](other_state/main.omg),
[other machine](other_machine/main.omg), and [target body](target_body/main.omg).
The match join observes distinct true/false markers while preserving the same
foreign operation. The const place names the original numeric inputs; a foreign
operation is not a constant initializer.

Every place and the root has a twin at `invalid/main.omg`. It removes only the
outer `Division`'s `[copy]`; `Word` and its byte array stay copyable. The repeated
whole-value use in `receive` must reject with `holds no value`. Exact nine-place
pairs are pinned under
`tests/omega/{pass,fail}/facts/foreign_owned_array_record_through_*`.
All positives declare `Expected exit: 0` and bind all four hosted entries.

## Portable C provider

[fixture/provider.c](fixture/provider.c) calls actual libc `lldiv`, returns a
corresponding C record containing two `struct word` array elements, and sets
the guard bytes according to the input sign. There is no replacement division
implementation. Its static assertions pin Word size sixteen, Division size
thirty-two, signed and byte field offsets, element count/stride, and alignment.
`arraydiv_verify` accepts both complete records by value and checks both earlier
signed pairs and all their guards. The self-test checks actual division, copied
contents after the later call, every guard byte, and wrong-word and wrong-guard
negative controls.

[fixture/prepare.py](fixture/prepare.py) follows the existing portable fixture
workflow, building and exercising a matching-host shared library while recording
commands and exact hashes. Linux uses `libomega_arraydiv_fixture.so` with
`ARRAYDIV_1.0`; macOS uses `libomega_arraydiv_fixture.dylib`; Windows uses
`omega_arraydiv_fixture.dll`. Public canonical boundary leaves and public
locator producers bind the exact `arraydiv_lldiv` and `arraydiv_verify` exports.
The generic native-provider sponsor consumes their complete admitted calling
applications. No symbol-specific adapter or implicit C pointer decay is added.

## Required evidence

- Establishment and identity: publish a fresh complete owned result only after
  recursive array element types/counts and typed containing paths validate.
  Earlier, copied and later guards and signed scalars keep their own values.
- Composition and soundness: all nine places preserve the same arrays and copy
  obligation. Markers distinguish match paths. Removing only outer copy custody
  rejects; representation grants no reference, claim or qualification authority.
- Psi 05–07: complete checked array/record result geometry and ordinary custody
  survive Terminal publication and canonical artifact inspection.
- Omega 00–02: exact ABI applications retain array counts, scalar element classes,
  record offsets and thirty-two-byte aggregate placements. Arrays remain values,
  with no implicit pointer argument conversion.
- Omega 03–06: caller-owned return memory and by-value argument transport retain
  all byte and signed fields, including earlier live results across calls.
- Omega 07–09: native and Terminal matching-host execution agree for every
  positive; each twin rejects for its own copy obligation. Four declarations
  alone grant no cross-host execution credit.

The governing contracts are [boundary shapes](../../../../../wiki/spec/build/boundary_shapes.md),
[foreign storage](../../../../../wiki/spec/build/foreign_storage.md), and
[Terminal ownership](../../../../../wiki/spec/terminal-psi/ownership.md).
## Measured qualification

Fixed baseline `eab58bf427c48063e3fcf597e4550ca53ac7cb8c` executes the frozen
local program natively at zero while Terminal declines its record-only ABI
admission. Optimized actual producer `7b904dca0385e7b8bad014f6bcdbb0e65b1ac647`
executes the root and all nine places with native exit 0 and Terminal exit 0 on
macOS ARM64. All ten Terminal artifacts pass canonical inspection; all nine
copy-removal twins reject with `holds no value`. The real C self-test also
rejects changed signed words and guard bytes.

The affected comparison records 51 source outcomes and
52 native/Terminal agreements with no lost baseline outcomes.
All authored exit and output oracles hold. It includes flat/nested integer and
floating foreign results, float classification, standard input, atomic case
payloads and retained nested record/array calls. The affected Rust packages
record 83 passing tests and one preexisting skip. New controls challenge exact
indexed rosters, missing/duplicate/out-of-range paths, primitive restrictions,
public element counts/classes/stride and semantic/public cycles.

Publication uses the ordinary constructor storage transaction. Byte-array
subtrees receive one byte backing, while arrays of records retain exact
containing field/index paths. No opaque host identity supplies their contents.
Schema validation visits empty element declarations and suppresses zero-leaf
expansion. This does not establish bounded whole-pipeline empty-array storage:
legacy codec preparation and nested empty-byte-array materialization can still
scale with outer dimensions and remain unqualified performance acceptance.

The original [run record](qualification.json) credits macOS ARM64 only.
The [matching-host record](hosted_qualification.json) now establishes this same
root and nine-place row on all four hosted targets: optimized producer
`33901569f34af6ba272ab5994871921aa6a6072e` on Windows x86-64 and both Linux
hosts, and `e8927c1e8e2d76304342c9cd995646d3f8c3691c` on macOS ARM64.
Each host records ten public native/Terminal zero agreements, ten canonical
reloads, nine pinned corpus agreements and nine intended copy-removal rejects.
The root also passes two inputs through one compilation, retaining its exact
canonical artifact and native identity before publication. Existing observations
retain their original producers.
Raw profiles, frozen inputs, exact C resources, engine outputs, canonical
artifacts and qualification are retained in `build/owned-array-evidence` and
`build/owned-host-evidence`.
Variants, reference arrays and independently qualified descendants remain open
outside this row.
