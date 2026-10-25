# Returned linear residual

Seed.code == 7 survives each place into the unconsumed second field of a
returned Pair, so consuming first by value preserves the original sibling
claim through normal cleanup.

This CML4 row exercises the operation
`consume(issue(seed.code).first) - 7`, followed by an ordinary zero-result
check and a Trap alternative. `issue` returns two distinct linear Receipt
children. `consume` receives the first by value; its normal cleanup must settle
that child's original claim. The original returned Pair retains the second
child and owes its eligible residual cleanup. Pair deliberately has no nominal
hook: this does not authorize partial moves through a whole-value hook.

Receipt's owner declares `drop` with fixed prerequisite `code == 7`; the hook
writes code 0. `consume` promises result 7, so its outgoing scalar must be
captured before that mutation. The final subtraction checks that captured value.
This scalar result check alone does not observe how many sibling hooks ran;
acceptance also inspects the exact claim maps, residual actions and original provenance.

`SEED_CODE` is the source value used to establish the Seed, not a claim identity
or a transfer receipt. Every invalid twin changes only its declaration from
7 to 8. Transport helpers' matching requires/ensures still hold for that supplied
value, `issue` remains unconstrained, and `issue.first` remains 7. The unchanged
second-child cleanup prerequisite must fail. No fallback to an unsupported-
projection diagnostic is an acceptable negative oracle.

## Places

| Place | Transport before the same returned-child operation |
| --- | --- |
| root / local | local Seed initialization; root repeats local |
| field | copy Envelope's Seed field |
| parameter | Seed passed into `through` under its explicit source-value fact |
| call result | `supplied` returns Seed with its source-value guarantee |
| match join | two Seed-producing arms; main calls both Boolean choices |
| const name | named Seed constant |
| a nested machine | Seed bound at `next` under its arrival fact |
| another machine | forwarding helper passes Seed into the operation's owner |
| another target body | each hosted `across` body forms Seed and performs the operation |

Each place has `main.omg`, `build.omg`, and an `invalid/` twin. Exact copies are pinned as
`tests/omega/pass/facts/returned_linear_residual_through_<place>_exit` (with
`build.omg` and authored native exit 0) and
`tests/omega/fail/facts/returned_linear_residual_through_<place>_rejected` (with
the twin's automatic-cleanup fragment in `expected.txt`). The root
plus nine positives bind all four hosted ProgramEntry catalogs and author native
and Terminal exit 0. Binding targets is not execution on those hosts. Rejects
retain the automatic-cleanup and Receipt::drop diagnostic fragments. Qualification
records the actual compiler, host, source hashes and engine observations.

## Pinned Fixtures

The eighteen pins for the nine standard places live at
`tests/omega/pass/facts/returned_linear_residual_through_<place>_exit` and
`tests/omega/fail/facts/returned_linear_residual_through_<place>_rejected`.
Recorded rows: 9 `checked` (`check-target:macos_arm64`), 9 `rejected` carrying
the expected fragment, 9 native `built ... exit:0`, and 9 both-leg rows reading
`exit:0 interpreter-exit:0 interpreter:agrees`.

Completed CML4 requirement: selected by-value linear-child consumption and
original-owner residual cleanup. Producer `b457b1978b87` runs all nine pinned
places with native and Terminal exit 0 on macOS ARM64, Linux x86-64 and
Windows x86-64; every residual-hook reject twin matches its diagnostic.
The [matching-host run](https://github.com/CathedralOS/Omega/actions/runs/40189006798)
also covers ordered edges, abandoned prefixes and field replacement.
Linux ARM64 native execution remains unmeasured.

## Eight-layer acceptance obligations

1. **Establishment:** issue constructs first.code 7 and second.code from its
   actual scalar input. Its result clauses preserve those field equations.
2. **Identity and lifetime:** each returned child continues its exact original
   claim. Selecting first transfers only that child; second remains under the
   same returned owner until its normal continuation disposes it.
3. **Composition:** the nine places transport the same Seed fact into the same
   nested producer/consumer expression. The match rung exercises both joins.
4. **Soundness:** the one-literal twin supplies 8 only to the residual sibling;
   fixed consume/drop obligations remain 7. Required cleanup must reject without
   manufacturing a caller requirement or a replacement claim.
5. **Checked-to-Terminal:** retain producer completion, by-value argument
   transfer, outgoing scalar capture and exact residual cleanup/retirement.
   Canonical artifact inspection checks the actual result bindings, selected
   transfer, residual hook and separate home retirement.
6. **Providers, layout and ABI:** the existing ProgramEntry route must carry
   Pair's result and the Receipt argument with their checked identities. This
   row asserts no particular size, field offset, register or stack location.
7. **Instructions and frames:** realize the nested result, consuming call and
   hook mutation in their required order. The scalar comparison is an execution
   oracle, not a proof of a particular instruction or frame allocation.
8. **Images and execution:** native and Terminal must independently complete
   with the authored result and checked cleanup frontier. Target bindings alone
   establish no foreign-host execution. This row does not measure fuel/resume;
   the projected nominal cleanup fixture's portable provision covers that contract.

Authority: [source ownership](../../../../wiki/spec/language/ownership.md),
[claim paths and residuals](../../../../wiki/spec/terminal-psi/ownership.md), and
[calls and normal cleanup](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md).
