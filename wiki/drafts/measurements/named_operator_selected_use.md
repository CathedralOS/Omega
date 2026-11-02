# `validated named operator application has no checked selected use`

Three GUI samples — `gui/window_app`, `gui/window_demo`,
`gui/windowed_calculator` — were observed failing checking with this bare
message, six times in `window_app`. The message carries no state, statement
or symbol. The original diagnosis below confused distinct authored calls;
the current diagnosis records the missing direct import and remaining route.

## The site

`05_typed-trees-to-checked-trees/src/operators/applications.rs`. For each
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
The import exposed anonymous arithmetic incorrectly losing builtin meaning in
the presence of visible float overloads. The numeric-values specification gives
wholly anonymous arithmetic exact meaning; wildcard float candidates are not
selected meanings. With that repaired, `window_demo` at `7c17208d59` on macOS
ARM64 passes its `self.info[2] = 0 - 64` initialization and reaches
`call operation: boundary arguments` at state 0, statement 22:
`self.gui.window_create(self.cls, self.title, ...)`. This is the authored
by-value array call, before later native float conversions or GUI publication.
The isolated Console-bearing array-store regression executes with exit 70.
**SAMPLE-CORPUS** retains the full native program acceptance.

## Remaining sample coverage

**SAMPLE-CORPUS** retains the authored by-value primitive-array boundary
arguments/results under **BOUNDARY-PRIMITIVE-ARRAY-PARAMETER**. The repaired
`window_demo` now reaches that obligation. Its progress does not establish
the outcomes of `window_app`, `image_viewer`, or `windowed_calculator`;
reproduce each full program with its own direct imports and provider selection.
