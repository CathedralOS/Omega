# Declared float comparison route

`left == right` on `f64` selects the declared operator without importing
`omega::language::core::float_operations`. A copy of an `f64` is not this fact.
The values are 212.0 and 2.0, so the comparison is false.

Owning task: **OPERATOR-BOUNDARY-CALLS**. This folder is the std-free witness.
`tests/omega/pending/arithmetic/float_computation_no_route` stays unchanged and
still depends on std.

Every place copies that fact sentence and the same operation: the program
compares the two `f64` values 212.0 and 2.0 with `==` and reaches `done()`
(exit 0) only when the comparison is false; a true comparison reaches
`machine bad() { crash Trap; }`. Only the transport of the operands and of the
comparison result changes. No place imports
`omega::language::core::float_operations`, and no place adds a `Build::depend`
line, so the declared route is exercised without a float library in the
program's package closure.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | the entry machine holds both operands and the result as locals |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both operands and the comparison result stay entry-machine locals |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | both operands arrive as record fields |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | both operands arrive as explicit machine parameters |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | both operands arrive as call results |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms compare and one successor observes |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants are the compared operands |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives both operands and compares |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `Comparator::equal` receives both operands through borrowed caller storage |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | one comparison body per hosted target |

Every positive place (and root `main.omg`) declares `Expected exit: 0` and binds
`ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and
`linux_arm64`. Each twin changes one thing: the right operand is 212.0 instead
of 2.0, so the same comparison is true and the program takes its `crash Trap`
route. Nothing else in the program moves.

The same nine places also carry the route's format boundary as a second valid
control: `<place>/controls/mixed_format/main.omg` is the place program with one
change — the right operand is declared `f32` instead of `f64` — so `left ==
right` has no declared operator and no builtin route. Each control is refused by
checking with ``builtin `==` cannot combine `f64` and `f32` `` rather than
silently widening.

## Execution

```sh
omega run --both samples/cli/facts/float_comparison_route/main.omg
omega run --both samples/cli/facts/float_comparison_route/<place>/main.omg
omega --check samples/cli/facts/float_comparison_route/<place>/invalid/main.omg
```

Measured on this Intel macOS x86_64 host with an opt-level 1 producer built from
`4d917f5ebf6` (the four authored hosted targets cannot execute here, so each
scratch copy added `macos_x86_64::ProgramEntry`, and `target_body` also got an
identical `macos_x86_64` comparison body; no fact source carries a fifth
binding, because that is host pinning, not a language gap). All ten positives
report `native exit 0` and `interp exit: 0`; all nine twins report
`native signal 4` (SIGILL) and `interp crash: Trap (host presentation agrees)`.
The bare public command on this host stops at
`selected target macos_x86_64 has no bound required root slot
macos_x86_64::ProgramEntry`; `omega run --target` is compile-publication only
and supplies no runtime claim. A Windows x86-64 host previously reported
`native exit 0` and interpreter exit 0 for the root at `30fbbc91026`.

All ten positives and all nine twins check clean for `macos_arm64` (the realized
target the corpus check leg reads from these bindings).

## The declared route is load-bearing

- **The comparison is emitted, not folded.** The retained Terminal Psi artifact
  (`omega run --both --keep`, then `omega inspect-terminal --artifact
  <dir>/program.oterm`) contains `IeeeFloatCompare { comparison: Equal }` for
  every place, even where both operands are literals or named constants
  (`local`, `field`, `const_name`, `target_body`). A compile-time fold of
  `212.0 == 2.0` would leave no comparison operation to execute.
- **That operation is only produced from a selected declared operator.**
  `OperationKind::IeeeFloatCompare` is emitted from
  `emission/selected_comparison.rs` only when
  `CheckedOperatorFacts::selected_float_comparison` accepts the use, and that
  classifier requires the selected operator declaration to be a boundary
  operator (`operator.is_boundary && candidate.is_boundary`). The source imports
  nothing, so the selected boundary declaration is the language core's
  `== Float::equal(left: f64, right: f64)`. A comparison with no selected
  operator has no exact Terminal meaning and does not lower.
- **The declared operator is demanded, and only when floats are compared.** The
  artifact's boundary roster carries `path(Float::equal)` rows for this program;
  a control program that compares two `u64` values and uses no float at all has
  zero `Float::equal` rows in its artifact.
- **The operand format is the route's boundary.** A control program that
  compares an `f64` left operand with an `f32` right operand is refused by
  checking with ``builtin `==` cannot combine `f64` and `f32` `` rather than
  silently widening, pinned as
  [`float_comparison_route_mixed_format_rejected`](../../../../tests/omega/fail/facts/float_comparison_route_mixed_format_rejected/main.omg)
  for the root and transported across all nine places
  (`<place>/controls/mixed_format/`, pinned as the nine
  `tests/omega/fail/facts/float_comparison_route_through_<place>_rejected`
  fixtures).
- **The value decides the observable.** The nine twins keep the route and the
  program and move only the right operand to 212.0; the comparison becomes true
  and both engines abort instead of exiting 0.

## Pinned fixtures

Nine pass fixtures `tests/omega/pass/facts/float_comparison_route_through_<place>_exit`
are byte-identical copies of the place programs and record the `0` native exit
oracle. Nine twin fixtures
`tests/omega/run/facts/float_comparison_route_through_<place>_traps` are
byte-identical copies of the place twins and record the `trap` oracle, following
`tests/omega/run/facts/double_indexed_rmw_through_local_traps`: a twin whose
only authored obligation is a runtime comparison result checks clean and rejects
at runtime, so its pin is the runtime oracle, not a `fail/` diagnostic row.

Nine further fixtures
`tests/omega/fail/facts/float_comparison_route_through_<place>_rejected` pin the
place transport of the format boundary: each is a byte-identical copy of
`<place>/controls/mixed_format/main.omg` (the place program with the right
operand declared `f32`) and each rejects with ``builtin `==` cannot combine
`f64` and `f32` ``. The root's form of the same control is pinned as
[`float_comparison_route_mixed_format_rejected`](../../../../tests/omega/fail/facts/float_comparison_route_mixed_format_rejected/main.omg).

The nine `_rejected` fixtures and the nine `_exit` fixtures were re-recorded
at `114906953e1` with the corpus runner built at that base (sha256
`f14e17c7521b980e193ba6929baec279dd843ec8144214f42e19e4e94cacfe5a`): nine
`rejected ... expected` rows in `tests/omega/corpus_outcomes.txt`, each
followed by the full refusal ``builtin `==` cannot combine `f64` and `f32`; a
landed float retains its format, so changing format requires an explicit
conversion`` (target_body carries it once per bound body); nine
`built ... exit:0` rows in `tests/omega/corpus_native_macos_arm64.txt`; and
nine `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows in
`tests/omega/corpus_both_macos_arm64.txt`. The nine run-tier `_traps` twins
gained their host rows at the same base: `built ... exit:signal signal:5` in
the native golden and `built ... exit:signal signal:5 interpreter:trapped` in
the combined golden. Before recording, the filtered re-measure at the same
base read 46 fixtures with 0 structural moves.

## Requirements and evidence

- **Owning contract:** [numeric values](../../../../wiki/spec/language/numeric_values.md),
  [boundary applications](../../../../wiki/spec/terminal-psi/boundary_calls.md#operator-applications-and-physical-children)
- **Parent outcome:** OPERATOR-BOUNDARY-CALLS (`TASKS.md`). This witness
  discharges the declared `f64` equality route without the float-operations
  import, across all nine transport places. It does not close the rest of that
  item.

1. **Establishment:** The comparison itself establishes the observed truth: the
   selected `== Float::equal(left: f64, right: f64)` declaration carries
   `ensures result == FloatSemantics::equal(Float::meaning64(left),
   Float::meaning64(right))`, and the entry transitions on that boolean. No
   place re-establishes the result in a caller with a post-hoc equality guard;
   the twins break the compared value, not a downstream assertion.
2. **Identity and lifetime:** The compared values keep their exact IEEE 754
   bit patterns (212.0 and 2.0) across a local, a record field, an explicit
   parameter, a call result, both match arms, named constants, a successor
   state, `Comparator::equal` through borrowed caller storage, and four
   target-scoped bodies. Each twin differs from its positive only in the right
   operand's value, so the outcome tracks the compared values and not a stale or
   substituted one.
3. **Composition:** The consuming operation is the crash guard reached from the
   comparison result. The result composes through all nine transports, and both
   engines agree on the exit (0) and on the twins' `crash Trap`.
4. **Soundness boundaries:** A true comparison must not complete as exit 0. All
   nine twins abort natively (SIGILL) and in Terminal Psi (`crash: Trap`) with
   host presentation agreement. The mixed-format control fails closed at
   checking, so the route cannot silently absorb an operand outside its
   declared format.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The declared comparison
   lowers without loss to target-neutral Terminal Psi (`PSITERM\0`) as
   `IeeeFloatCompare { comparison: Equal }` in every place, proof-only float
   meaning terms are erased, and the Terminal Psi interpreter executes every
   positive to `interp exit: 0` and every twin to `interp crash: Trap`.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):**
   Each positive joins `ProgramEntry` for the four hosted targets. `f64` layout
   is `size_of = 8`, `align_of = 8`; the declared `Float::equal` boundary rows
   and the selected `FloatNativeProvider::equal64` family are present in the
   artifact, and `f64` operands occupy floating-point argument registers in the
   `CallPlan`/`StatePlan` of each place, including the twins.
7. **Instruction selection, register homes, and activation frame realization
   (`Omega 03–06`):** The executed x86-64 images select the IEEE ordered
   equality path for the `f64` operands, assign register homes and disjoint
   frame slots for the entry machine and its helper, and route the true arm to
   the trap. The four authored hosted targets are checked through their entry
   bindings and bodies; this host executed only `macos_x86_64`.
8. **Form encoding, branch relaxation, relocations, and native image emission
   (`Omega 07–09` + emission):** The emitted images execute and agree with
   Terminal Psi, including the twins' trap presentation, so encoding and
   relocations preserve both comparison outcomes.

### What remains open

Native execution on the four authored hosted targets: this host pins
`macos_x86_64`, and `omega run --target` is compile-publication only, so it
supplies no runtime claim. The rest of OPERATOR-BOUNDARY-CALLS is untouched
here — the std-dependent `pending/arithmetic/float_computation_no_route` matrix
still does not execute, and the float/receiver signature, recovered
specialized-operator, directed fused-multiply-add, receiver-bearing
requirement, and checked-interpreter multi-satisfier frontiers remain. No place
in this row refused, so this row needed no compiler repair and changed nothing
in `omega-rust/`.