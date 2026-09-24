# Longest equal-byte run

The complete scanner walks `aaabbbbccdaa`, prints its result, consumes one input
line, and exits with **4**. Preserve the runtime byte loop and its bounded cursor;
unrolling the input or changing its types does not establish this sample.

From the repository root on macOS:

```sh
OMEGA_SAMPLE_RUNTIME_FILTER=longest_run RUST_MIN_STACK=67108864 \
  cargo nextest run -p compiler --test samples_compile --no-fail-fast \
  -E 'test(=samples_with_documented_exit_run_correctly)'
```

PowerShell:

```powershell
$env:OMEGA_SAMPLE_RUNTIME_FILTER = 'longest_run'
$env:RUST_MIN_STACK = '67108864'
cargo nextest run -p compiler --test samples_compile --no-fail-fast `
  -E 'test(=samples_with_documented_exit_run_correctly)'
```

Use `mbx` instead of `cargo` when available. The harness supplies the input line
and checks the exit code; running only source checking is not acceptance.

## Compiler dependencies

The outer command currently fails before execution with
`selected ProgramEntry establishment rejoins 0 Terminal attachment identities; expected one`:
the complete scanner body does not yet produce its Terminal entry. This is not
an observed wrong native exit.

Bounded scalar field writes carry independently checked pre-write range proofs.
The native regression `bounded_integer_field_stores_run_natively` checks signed
literal initialization followed by guarded `i32` addition and negative `i8`
subtraction in one body under a conjunctive guard, with updated values observed
by the caller after return. Disjoint writes preserve the other guard facts;
short-circuit control reaches the writes without merging Boolean outcomes first.
Native selection uses its existing checked edge bridges when conditional
fallthrough destinations conflict. The source
regression
`guarded_bounded_integer_field_increment_publishes_checked_terminal` checks the
combined updates through Terminal production and rejects redirected guard edges.
Neither closes this scanner.

Live bounded byte-field lengths reach verified Terminal interpretation through
ordinary scalar operands. The field-length regression observes a nested field's
length after a helper shrinks it and after empty replacement, without substituting
capacity or changing sibling contents.

Native field-length observations preserve the exact metadata operation through
optimization, selection and publication. The regression
`record_reads::byte_lengths::bounded_byte_field_lengths_observe_exact_direct_and_nested_live_extents`
publishes for all four hosted targets and executes on macOS ARM64. Direct and
nested fields with equal capacities return their distinct live lengths, including
empty storage; reads compose with ordinary calls and leave backing unchanged.
Native bounded fields use an aligned leading `u64` length followed by inline
capacity bytes, not a synthesized borrowed-view descriptor. Metadata observation
does not grant byte-content read or replacement authority.

Whole-field byte replacement now copies the exact live source span before
writing length metadata. Run `cargo nextest run -p compiler --test
byte_field_replacement --no-fail-fast`: authored empty/short/full literals and
direct/nested fields publish for all four hosted targets and execute on macOS
ARM64. Verified Terminal runtime-view cases also execute with null empty backing
and short read-only backing ending at a protected page; source, neighbors and
unused destination bytes remain unchanged. Oversized input follows the guard's
non-writing branch. Equal-capacity field/source substitutions and altered copy
operands or footprints reject independent replay.

Runtime-view RHS and write-only bounded-domain assignment still need their
authored Psi producers in `04_typed-trees-to-checked-trees/src/flow`; the runtime
and write-only tests construct verified Terminal explicitly, not pretend that
those source forms already compile. The native copy itself is shared across
literal and runtime views, with ISA-local loops rather than source CFG edges.

Remaining scanner dependencies include field-backed byte reads. After those
operations compose, the complete state
loop must still pass its termination/proof and native execution checks. Keep this
command as the outer acceptance check rather than deriving completion from a
passing isolated store test.

Native scalar access beside bounded byte fields now uses the existing record
layout. `record_reads::scalar_updates_preserve_neighboring_bounded_byte_storage`
in `omega-native-differential-test --test scalar_case_results` publishes for all
four hosted targets and executes on macOS ARM64: shared reads around a nested
mutable call observe signed updates, while byte-buffer regions, padding, and the
sibling record remain unchanged. Its function harness supplies valid empty
buffers; it does not provision a hosted receiver or execute byte replacement.

Hosted receivers now admit direct and nested bounded byte fields whose complete
understood source constraints accept empty storage. `bounded_integer_field_stores_run_natively`
executes the ordinary ProgramEntry with UTF-8, ASCII and no-NUL fields beside the
updated integers. Source eligibility rejoins exact fields, nominal paths and
capacities, using existing resolved-domain predicate denotation; nonempty and
unknown requirements reject. Native image replay separately checks owned
zero-filled storage and the existing root-backed receiver partition. This adds
neither borrowed-view initialization nor authority from a carrier's layout.
Byte reads and the complete scanner loop remain
the next dependencies; receiver initialization alone does not close them.
