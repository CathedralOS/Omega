# Route one diagnostics on must-reject programs

Measurement record, 2026-11-11, for the open question on
[diagnostics](check_on_lowered_form.md#open-questions) in check on the
lowered form, and for the `diagnostics` release gate. No checker behaviour
was changed. It asks: when the checker on the lowered form rejects a program
that must be rejected, could a programmer find the defect and fix it from
that error, as they can from the current checker's?

**Partial result: not yet, and the cause is mostly one prover gap, not
missing information.** This run covers 3,375 of the 8,294 `fail/` fixtures
(41%), in group order (which groups is [below](#what-the-partial-run-covers)),
measured at `main` `c7371d3dc`. The new route rejects 117 of the 1,178 it
lowers. 53 of those 117 point at the defect: the right machine and the
right kind of obligation, and 48 of them at the same source line as the
current checker. 63 send the programmer to a different machine
that is correct. Each of the 63 is the same prover gap: the prover cannot
prove an `ensures` that compares the returned value with a literal or a
formal, such as `machine four() -> u64 ensures result == 4 { 4 }`. The
fixture's real defect sits in a machine the route cannot lower yet, so the
programmer is shown only the false error.

**No message says what failed.** Every rejection reads
`unproved obligation: <kind>: <cause> (<prover note>)`, for example
`requires clause 1 of the called machine: no proof found from the facts
available here (the bounds do not decide the comparison)`. It does not name
the callee, the clause, the operation or the condition. The current checker
says `cannot prove requires contract for call bounded from probe: xored >=
1 && xored <= 2`. So even a well-placed rejection shows the line but not the
condition to establish. Only 1 of the 117 messages contains the fixture's
pinned `expected.txt` fragment.

**Nothing found so far is information the lowered form does not carry.**
The callee and clause of a requirement, the operation of a safety
obligation, the goal and the source spans of the values it names are all in
the lowered program or its source correspondence. The one gap of that kind
is minor: a value carries no binder name, only the span of the expression
that defines it, so a message can quote the source at that span but cannot
say "local `quotient`". Loans could not be measured: the route refuses
borrows, so no `fail/` borrow fixture reaches it.

A rerun of every `fail/` fixture at a later `main` is under way. Seven
lowered-route PRs have landed since `c7371d3dc` (`#2379`, `#2382`, `#2390`,
`#2393`, `#2397`, `#2412`, `#2422`), and `#2379` changes what the route
judges, so these numbers will be replaced, not added to.

## The three counts

A fixture counts as lowered when the new route reaches a verdict on at least
one closure. A fixture that binds no program entry (most `fail/` fixtures)
has no module for the route, so the harness also roots the route at each of
the fixture's own machines ([how](#how-the-numbers-were-taken)). The
current checker rejects 3,256 of the 3,375 fixtures run. 117 pass its check
and fail in a later stage, and 2 (`effects/create_dir_all_requires_*`) were
killed with exit 137 before printing anything, most likely out of memory
with four fixtures running at once. Those 119 are not counted.

| | Entry only | Entry and each own machine |
| --- | ---: | ---: |
| Lowered by the new route | 106 | 1,178 |
| 1. Rejected by the new route | 7 | **117** |
| 2. Message contains the pinned fragment | 0 of 7 | **1 of 117** |
| 2. Names the defect's obligation at its construct | 7 of 7 | **53 of 117** |
| 3. Site matches the current checker's (exact or same line) | 7 of 7 | **49 of 117** |
| 3. Different site | 0 | 43 |
| 3. Current checker gave no position to compare with | 0 | 22 |
| 3. No position from the new route (`Unpositioned`) | 0 | 3, all `ImplicitReturn` |

The second row of count 2 is the coordinator's test after an outside
review: whether a programmer could find the cause, not whether the wording
matches. It was classified per fixture (below). All 22 fixtures without a
current position are among the 63 wrong-machine rejections. One fixture
matches by site but not by defect: in
`arithmetic/implicit_fractional_integer_landing` the route flags a
multiply's overflow on the line where the fixture pins an implicit
fractional landing, a typing rule the route does not own.

The 1,061 lowered fixtures the route does not reject:

| Route verdict | Fixtures | What it means |
| --- | ---: | --- |
| Accepted; some other closure is outside the subset | 659 | Not classified one by one. The current message is a declaration or typing rule in 327, a call's `requires` in 141, an `ensures` in 112, an index or range bound in 58, an operation's safety in 20. A defect in a body can be in a closure the route did not lower. |
| Falls back to the current checker | 341 | Some obligation's kind is outside the route. The harness at this head printed it as `other`: termination or a conserved obligation. |
| Accepted, every closure judged | 52 | The defect is not in a machine body: 44 are declaration rules (a field type, a duplicate declaration, a trait or provider shape, a const argument), the rest literal typing (`7/2` landing in `i32`) or value arguments in a type application. The route does not judge these, so it must not decide a program alone. |
| `Incomplete` | 9 | The prover has no rule for a goal's form. |

## Why each miss happens

Each rejection was classified by its sites and by the machine the current
checker's message names. Each cause says whether it is a missing mapping
that can be added, or information the lowered form does not carry.

| Cause | Fixtures | Kind |
| --- | ---: | --- |
| A trivially true `ensures` over the returned value is unproved, so a correct helper is rejected in place of the defect | 63 | **Prover gap**, not diagnostics. The goals are `result == 4`, `result == value`, `result >= 0u8` and similar, and the prover's note is "the bounds do not decide the comparison". Fixing the prover removes these rejections. |
| The message names the obligation kind and clause number, not the callee, clause text, operation or condition | all 117 | **Missing mapping.** `CallRequirement { invocation, clause }` leads to the callee and its clause's span; `OperationSafety { operation }` names the operation; the goal's values have defining spans. |
| An `ensures` is reported at the return, not at the clause | 2 | **Missing mapping.** A `NormalGuarantee` names its `PublishedGuarantee`, and the clause's span is in the typed trees. Both sites can be given. |
| An `ensures` on a body with no tail expression has no position (`Unpositioned(ImplicitReturn)`) | 3 | **Missing mapping.** The same clause span would do. |
| The message says "not proved" where the current checker says "structurally false" or "disproved" | 9 | **By design.** The route never claims an obligation is false ([`compiler_request.md`](../../spec/build/compiler_request.md#outcomes): "Failure to find a proof is not proof of falsehood"). |
| The prover does not say which facts it had, or which write replaced the value a guard constrained | most | **Missing mapping.** The defining operation of each operand and its span are in the lowered form; the prover reports only a one-line note. |
| A value has no binder name | all | **Not carried.** Only the span of the defining expression. Quoting source at that span locates the value, so this does not block a fix. |

On explanation beyond a position, which the coordinator asked about: an
unclosed invariant window already carries both the opening write and the
consumption point (`UnprovedObligation::consumed`). Which contract
introduced a requirement can be recovered as above. Why a loan is still
live is not measurable here.

## What the partial run covers

Run in alphabetical order of the fixture path, `facts/` last. The run
stopped at the two-hour limit of its second leg.

- **Complete, 45 groups, 2,141 fixtures:** `arrays`, `atomics`, `calling`,
  `compile_time`, `effects`, `expressions`, `filesystem`, `float`,
  `generics`, `host`, `indexes`, `inline_asm`, `layouts`, `machines`,
  `memory`, `modules`, `multiplicity`, `mutation`, `operators`, `optimizer`,
  `ownership`, `packages`, `parse`, `parser`, `places`, `ports`, `proofs`,
  `providers`, `range`, `ranges`, `recast`, `references`, `reflection`,
  `relax`, `relevance`, `resources`, `slices`, `structs`, `structural`,
  `targets`, `tasks`, `terminal_psi`, `termination`, `text`, `time`.
- **Partial, 22 groups, 1,234 of 2,447 fixtures:** `arithmetic` 149 of 174,
  `asm` 52/118, `borrow` 12/92, `borrows` 92/444, `boundary` 2/6, `build`
  135/217, `calls` 111/174, `capabilities` 6/15, `cases` 5/12,
  `collections` 13/51, `comptime` 16/18, `concurrency` 1/7, `constants`
  19/27, `constraints` 6/7, `contracts` 108/181, `control_flow` 24/45,
  `core` 85/165, `data` 22/39, `dependent` 37/70, `domains` 108/302,
  `drops` 52/89, `traits` 179/194. The missing ones are mostly fixtures
  whose current diagnostic has no position, or that have more than one
  source file. Their first results were discarded when the root rule changed
  (below), and they were not run again.
- **Not run, 3,706 fixtures:** `blockexec`, `types`, `wire` and all of
  `facts/`.

So the partial numbers lean towards fixtures with one source file and a
positioned current diagnostic.

## How the numbers were taken

Host: a cloud Linux x86_64 container with 4 CPUs and 15 GB of memory, mbx
1.29.2, toolchain `nightly-2026-09-04`. Measured source: `main` `c7371d3dc`.
Harness: `route_one_comparison` at `effe452d4` (that `main` plus the harness
changes), built with `CARGO_PROFILE_DEV_OPT_LEVEL=3` into its own target
directory. The first fixtures ran with the same harness at opt-level 1;
both builds gave the same output on a test fixture, apart from times.

The harness changes are in
`omega-rust/omega/compilation/compiler/examples/route_one_comparison.rs`.
It checks each fixture through the typed-frontier seam, keeping the current
checker's diagnostics. It judges the bound entry's closure and, with
`--every-machine`, each machine declared in the fixture's own source files
(not its `build.omg`) or in a file a current diagnostic names. For each
program both checkers reject, it prints a `quality` line with the fragment
and site comparison. `--tally` prints the three counts.

```
CARGO_PROFILE_DEV_OPT_LEVEL=3 CARGO_TARGET_DIR=$PWD/target-o3 \
  mbx build -p compiler --example route_one_comparison
# one process per fixture, four at a time, 300 s each, every fail/ main.omg
timeout 300 nice -n 19 target-o3/debug/examples/route_one_comparison \
  --every-machine <fixture>/main.omg < /dev/null > <fixture>.tsv
cat *.tsv | target-o3/debug/examples/route_one_comparison --tally
```

During the run the root rule was widened from "machines in a source file a
current diagnostic names" to also include the fixture's own source files,
and the fallback reason was added. The 1,201 results that could differ
under the new rule were deleted: a fixture with a fallback, a current
diagnostic outside `main.omg` or without a position, or more than one
source file. The remaining results are the same under both rules.

The classification in [why each miss happens](#why-each-miss-happens) was
done by a script over the harness output, then checked by reading the
fixtures. It matches the route's obligation kind with the current message's
family (requirement, guarantee, operation safety, bounds) and compares the
rejected machine with the machine the current message names. A site
counts as matching when the spans are equal, overlap, or start on the same
line.

**Running it on one slice.** A lane can measure its own groups the same way:
pass its fixtures' `main.omg` files, then `--tally`. The tool's header
documents each line it prints. Machine names and sites are on the
`program`, `diagnostic`, `use` and `quality` lines, which is what the
classification above needs.
