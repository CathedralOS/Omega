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

Equal operator symbols and counts do not establish that the two sets name the
same authored calls. Each GUI sample defines six conversions in its own provider
bodies and also imports `std::macos_gui`, which defines another six. The original
inference that these were copies of the same calls was incorrect.

The `named_uses` side is the authored code: each row's origin is a real
`StateStatement`, machine `291` statements 0-3 and machine `292` statements
5-6, role `AssignmentValue`. The validated side's handles are much lower
(128-582 against 1528-1980), and the pass has already confirmed each one
resolves through `expression_table` to an `ExpressionNode::Call` whose
selected symbol equals the requirement. Two distinct call expressions in the
arena therefore resolve to the same operator.

## Current diagnosis

At `5e729e14dc4`, unchanged `window_demo` reproduces all six refusals. Adding
its direct `core::float_operations` import clears them and reaches a later indexed-store omission. Validation resolves
named operators without the source-visibility predicate applied by checked
named-use collection. `std::macos_gui` already imports the operations; the
sample did not. Imports are nontransitive under
[import exposure](../../spec/language/modules.md#import-scope-and-exposure).

Application rows are freshly produced after specialization on the same frozen
program used to collect checked uses. No lost remapping was established.
Keep both the exact expression-identity comparison and the source-visibility
check; matching by symbol alone would wrongly borrow another call's evidence.
The diagnostic should eventually identify the inaccessible authored selection.
The sample's next barrier concerns anonymous arithmetic in the presence of
visible float overloads. The numeric-values specification gives wholly anonymous
arithmetic exact meaning; wildcard float candidates are not selected meanings.
**SAMPLE-CORPUS** retains the full native program acceptance.

## Not what the board currently says

SAMPLE-CORPUS assigns `gui/{image_viewer,window_app,window_demo}` to
**BOUNDARY-PRIMITIVE-ARRAY-PARAMETER** for "authored by-value primitive-array
boundary arguments/results". That is a real and separate obligation — this
failure is reached before any of it, and `image_viewer` fails elsewhere
(`call operation: boundary arguments`) while `windowed_calculator`, which the
row does not list, fails here. Whoever takes the row should expect two
blockers, not one.
