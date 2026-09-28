# `validated named operator application has no checked selected use`

Three GUI samples — `gui/window_app`, `gui/window_demo`,
`gui/windowed_calculator` — fail checking with this bare message, six times
each in `window_app`. The message carries no state, statement or symbol, so
it reads as an authored defect and is not one. This file records what the
failure actually is, measured, so the next reader does not re-derive it.

## The site

`04_typed-trees-to-checked-trees/src/operators/applications.rs`. For each
`ValidatedBoundaryOperatorApplication` whose site is an expression, the pass
requires a `CheckedNamedOperatorUseFact` in `operators.named_uses` with **both**
the same `selected_operator_symbol` and the same `expression` handle. When no
row matches, it emits this diagnostic.

## What was measured, at `695b5e05d07`

Instrumented the failure site to print the requirement, the expression, and
every `named_uses` row. In `window_app`:

| | validated application expressions | named use expressions |
| --- | --- | --- |
| requirement `1520` | 128, 133, 138, 143 | 1528, 1533, 1538, 1543 |
| requirement `1524` | 577, 582 | 1975, 1980 |

`named_uses_total = 6`, and for every failing application `same_symbol` is 4
or 2 — **every use matches by symbol** — while `same_expression` is **0**.

So the two sides agree exactly on which operators are applied and how many
times, in the same order, and disagree only on the expression handles. These
are the same six logical calls under two numberings, not a missing fact.

The `named_uses` side is the authored code: each row's origin is a real
`StateStatement`, machine `291` statements 0-3 and machine `292` statements
5-6, role `AssignmentValue`. The validated side's handles are much lower
(128-582 against 1528-1980), and the pass has already confirmed each one
resolves through `expression_table` to an `ExpressionNode::Call` whose
selected symbol equals the requirement. Two distinct call expressions in the
arena therefore resolve to the same operator.

## Reading

This is a cross-stage identity mismatch, not a missing checked use: the
validated application and the checked use are recorded against different
copies of the same call. `ValidatedBoundaryOperatorApplication`'s own doc
says the rows are "retained by one concrete machine specialization", which is
the obvious suspect for the second copy, but nothing here establishes which
snapshot is canonical.

**Do not relax the comparison to make these samples check.** It is a
rejoin check over retained evidence; matching on symbol alone would let an
application rejoin a use it does not name. The repair belongs wherever the
two handle sets diverge, and that was not isolated here.

## Not what the board currently says

SAMPLE-CORPUS assigns `gui/{image_viewer,window_app,window_demo}` to
**BOUNDARY-PRIMITIVE-ARRAY-PARAMETER** for "authored by-value primitive-array
boundary arguments/results". That is a real and separate obligation — this
failure is reached before any of it, and `image_viewer` fails elsewhere
(`call operation: boundary arguments`) while `windowed_calculator`, which the
row does not list, fails here. Whoever takes the row should expect two
blockers, not one.
