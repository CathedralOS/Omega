# Route one diagnostics on must-reject programs

Measurement record, 2026-11-11, for the open question on
[diagnostics](check_on_lowered_form.md#open-questions) in check on the
lowered form, and for the `diagnostics` release gate. No checker behaviour
was changed. It asks: when the checker on the lowered form rejects a program
that must be rejected, could a programmer find the defect and fix it from
that error, as they can from the current checker's?

**Where the new route rejects, it points at the defect, but it never says
what failed.** Over every `fail/` fixture at `main` `7f1316ed6`, the route
lowers 1,840 programs the current checker rejects and rejects 293 of them.
In 274 of the 293 it rejects the same kind of obligation at the same
construct as the current checker (265 at the exact span, 10 on the same
line), and 14 more report the same `ensures` at its other end, the return
instead of the clause. But every message reads
`unproved obligation: <kind>: <cause> (<prover note>)`, for example
`operation safety: no proof found from the facts available here (the
operands' bounds exceed the carrier)`, where the current checker says
``exact arithmetic in machine `run` state `entry` local `diff1` may
overflow `u64` ``. No message names the callee, the clause, the operation or the
condition. Only 1 of the 292 fixtures with a pinned `expected.txt` fragment
gets a route message containing it.

**What is missing is mapping, not information.** The callee and clause of a
requirement, the operation of a safety obligation, the goal, and the source
spans of the values it names are all in the lowered program or its source
correspondence. The one gap of that kind is minor: a value has no binder
name, only the span of the expression that defines it, so a message can
quote the source there but cannot say "local `diff1`". Loans could not be
measured: the route refuses borrows, so no borrow fixture reaches it.

**Two accepted programs are real soundness holes.** Of the 71 programs the
route accepts with every closure judged, 67 have their defect outside any
machine body (a declaration, a specification, a Build entry), which the
route does not judge. Of the other 4, two are wrong programs the route
accepts with a changed meaning: `7/2` and `4097/2` in an `i32` machine
are lowered as integer division
([list](#programs-the-route-accepts-with-every-closure-judged)). So the
route must not decide a program alone yet. Both holes are fixed by `#2441`,
merged after `7f1316ed6`.

The first, partial measurement at `c7371d3dc` found 63 of 117 rejections on
a correct helper, because the prover could not prove trivially true
`ensures` such as `result == 4`. That gap is closed at `7f1316ed6`: on the
same 3,375 fixtures, 57 of the 65 rejections are now the same obligation at
the same site, and none blames a helper the current checker does not name.

## The three counts

A fixture counts as lowered when the new route reaches a verdict on at least
one closure. A fixture that binds no program entry (most `fail/` fixtures
outside `facts/`) has no module for the route, so the harness also roots the
route at each of the fixture's own machines
([how](#how-the-numbers-were-taken)). Of the 8,294 fixtures, the current
checker rejects 7,145. 1,002 stop before the typed frontier (parse or
resolution errors), 145 pass the check and fail in a later stage, and 2
produced no output (one timed out at 300 s, one was killed with exit 137).
Only the 7,145 are counted.

| | Entry only | Entry and each own machine |
| --- | ---: | ---: |
| Lowered by the new route | 190 | 1,840 |
| 1. Rejected by the new route | 183 | **293** |
| 2. Message contains the pinned fragment | 0 of 183 | **1 of 292** |
| 2. Same obligation at the defect's construct | not classified | **288 of 293** |
| 3. Site matches the current checker's (exact or same line) | 176 | **275** |
| 3. Different site | 5 | 12 |
| 3. Current checker gave no position to compare with | 2 | 2 |
| 3. No position from the new route (`Unpositioned`) | 0 | 4, all `ImplicitReturn` |

The second row of count 2 asks whether a programmer could find the cause,
not whether the wording matches. It counts the 274 same-obligation,
same-site rejections and the 14 that report the same `ensures` at the
return. One fixture has no `expected.txt`, so count 2 has 292 rows.

The 1,547 lowered fixtures the route does not reject:

| Route verdict | Fixtures | What it means |
| --- | ---: | --- |
| Accepted; some other closure is outside the subset | 1,470 | The defect can be in a closure the route did not lower. The current message is a declaration or other rule in 677, a call's `requires` in 327, an `ensures` in 203, an index or range bound in 201, an operation's safety in 58, a literal's typing in 4. |
| Accepted, every closure judged | 71 | 67 have the defect outside a machine body; 4 do not. Listed [below](#programs-the-route-accepts-with-every-closure-judged). |
| `Incomplete` | 6 | The prover has no rule for the goal's form. By fixture name, five are about Boolean or expression-statement returns and one about an abstract-signature constant initializer. |

The other 5,305 rejected fixtures are not lowered at all: some closure the
fixture needs is outside the route's subset (generic machines, borrows,
assembly, data domains, Build bindings and others).

## Why each miss happens

Each rejection was classified by its sites and by the machine the current
checker's message names. Each cause says whether it is a missing mapping
that can be added, or information the lowered form does not carry.

| Cause | Fixtures | Kind |
| --- | ---: | --- |
| The message names the obligation kind and clause number, not the callee, clause text, operation or condition | all 293 | **Missing mapping.** `CallRequirement { invocation, clause }` leads to the callee and its clause's span; `OperationSafety { operation }` names the operation; the goal's values have defining spans. |
| An `ensures` is reported at the return, not at the clause | 14 | **Missing mapping.** A `NormalGuarantee` names its `PublishedGuarantee`, and the clause's span is in the typed trees. Both sites can be given. |
| An `ensures` on a body with no tail expression has no position (`Unpositioned(ImplicitReturn)`) | 4 | **Missing mapping.** The same clause span would do. |
| Besides the right error, another error at a site the current checker never names | 59 | **Mixed.** 29 are an `ensures`, and the two read by hand are true contracts the prover cannot prove: `result == a9` after `(a1 + ... + a8 - 360) + a9` with every `a` pinned, and `result == 7` through a nested machine whose `requires` is `value == 7`. 27 are an operation's safety, and some are a second real defect the current checker does not report after the first (`local_joined - 20u64` beside the pinned `60u64 - local_joined`). 6 are a call's `requires`. |
| Every route error is in another machine | 2 | `build/generated_source_undeclared_call_refused` pins an unresolved call; the route rejects `seed() + 1` in a generated helper for overflow. `facts/copied_call_argument_snapshot_through_other_state_rejected` pins the caller's `requires`; the route rejects the callee's true `ensures`, the prover gap above. |
| The route flags overflow where the fixture pins a literal that does not fit | 1 | `constraints/guarded_integer_return_range`: `256` returned as `u8`. A typing rule the current checker owns. |
| The message says "not proved" where the current checker says "structurally false" or "disproved" | some | **By design.** The route never claims an obligation is false ([`compiler_request.md`](../../spec/build/compiler_request.md#outcomes): "Failure to find a proof is not proof of falsehood"). |
| The prover does not say which facts it had, or which write replaced the value a guard constrained | most | **Missing mapping.** The defining operation of each operand and its span are in the lowered form; the prover reports only a one-line note. |
| A value has no binder name | all | **Not carried.** Only the span of the defining expression. Quoting the source at that span locates the value, so this does not block a fix. |

On explanation beyond a position: an unclosed invariant window already
carries both the opening write and the consumption point
(`UnprovedObligation::consumed`). Which contract introduced a requirement can
be recovered as above. Why a loan is still live is not measurable here.

## Programs the route accepts with every closure judged

None of these may be accepted, so each needs a stage that owns its rule
before the route decides a program alone.

**Route soundness holes, 2.** Elaboration lowers an anonymous fractional
quotient as an `i32` `ExactIntegerDivide`, so the route proves a different
program from the one written. The rule belongs to Psi 05 typing (`#2232`),
but the route accepts without it.

- `arithmetic/fractional_integer_landing`: anonymous value `7/2` is not an
  integer and cannot land in `i32`.
- `arithmetic/fractional_integer_operand`: anonymous operand `4097/2`
  cannot land exactly in `i32`.

Both are fixed on a later `main` by `#2441`, which lowers wholly anonymous
arithmetic exactly and refuses it when the landing rule does not allow it.
It also refuses `7 % 2` and `(n / 2) * 2` below.

**Body rules another stage owns, 2.** The lowered program has the meaning
the author intended; the language rejects the form.

- `arithmetic/anonymous_remainder_rejected`: `7 % 2` needs an
  integer-typed operand. Same Psi 05 literal typing as above; here the
  integer remainder agrees with the exact value.
- `calls/non_unit_compound_expression_requires_use`: an expression
  statement discards its non-Unit value.

**Declaration, specification and Build rules, 67.** The defect is not in a
machine body, and per-machine rooting does not judge it:

- data, field, trait and provider shapes (33), for example a field typed
  by a boundary trait, a duplicate declaration, an unknown data type;
- specification calls, predicates and operators (14), for example ``cannot prove
  requires contract for specification call `guarded` ``;
- fixed-array extents and const or value arguments of type applications
  (15), including ``exact arithmetic in value argument `Count` of `Index`
  may overflow `u8` ``;
- Build entry shape and selection (5).

With a bound entry, elaboration would claim these declarations and refuse
the program (`route=outside`), not accept it. That was not run, because
these fixtures bind no entry.

## How the numbers were taken

Host: a cloud Linux x86_64 container with 4 CPUs and 15 GB of memory, mbx
1.29.2, toolchain `nightly-2026-09-04`. Measured source: `main` `7f1316ed6`.
Harness: `route_one_comparison` as landed in `#2431`, applied to that
`main` (commit `7cd87de5b`), built with `CARGO_PROFILE_DEV_OPT_LEVEL=3`.
Every `fail/` `main.omg`, 8,294 fixtures, one process each, three at a
time, `timeout 300 nice -n 19`, in two legs.

The harness is
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
# one process per fixture, three at a time, 300 s each, every fail/ main.omg
timeout 300 nice -n 19 target-o3/debug/examples/route_one_comparison \
  --every-machine <fixture>/main.omg < /dev/null > <fixture>.tsv
cat *.tsv | target-o3/debug/examples/route_one_comparison --tally
```

The classification in [why each miss happens](#why-each-miss-happens) was
done by a script over the harness output, then checked by reading fixtures.
It matches the route's obligation kind with the current message's family
(requirement, guarantee, operation safety, bounds) and compares the rejected
machine with the machine the current message names. A site counts as
matching when the spans are equal, overlap, or start on the same line. The
groups of the 67 declaration rules were assigned by reading each pinned
fragment.

**The partial run at `c7371d3dc`.** 3,375 fixtures in alphabetical order,
`facts/` last, with the harness at `effe452d4`. It lowered 1,178 and
rejected 117, of which 53 named the defect and 63 rejected a correct helper.
On the same fixtures at `7f1316ed6` the route lowers 777 and rejects 65.
Most of the drop is fixtures with a closure that fell back at discharge on
`c7371d3dc`: 300 of those 394 are now refused at lowering
(`route=outside`).

**Running it on one slice.** A lane can measure its own groups the same way:
pass its fixtures' `main.omg` files, then `--tally`. The tool's header
documents each line it prints. Machine names and sites are on the
`program`, `diagnostic`, `use` and `quality` lines, which is what the
classification above needs.
