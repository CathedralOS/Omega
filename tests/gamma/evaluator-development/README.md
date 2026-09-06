# Direct Beta Gamma evaluator validation

This test-owned gate validates the selected typed scalar/effect Gamma evaluator,
which is implemented directly in addressed Beta:

```text
Beta compiler -> Gamma evaluator tape
Gamma evaluator + Gamma-authored augmenter -> richer Gamma source
Gamma evaluator + expanded source -> result 42
```

The evaluator accepts a little-endian u32 source length, exact source bytes, and
remaining sealed input. It censuses function names, parameter spans, body spans,
and arities, then evaluates expressions directly from source. It implements
explicitly typed scalar functions, lexical `let`, `if`, forward calls, integer
operators, sealed input length/indexing, and byte output. It retains no AST and
emits no Gamma or Alpha code.

## Measurements

```text
1,632-line / 46,482-byte canonical addressed Beta with named control targets
8,355-byte evaluator tape
```

The selected Beta compiler assembles that canonical Beta directly; this gate
pins the Beta and tape hashes and executes the retained tape. The adjacent
81-line label resolver remains only for nonauthoritative
experiments elsewhere under `tests/`; it does not reconstruct or participate in
the selected evaluator.

For comparison, the current Gamma route above the common Beta compiler contains:

```text
753-line Beta Gamma evaluator
725-line Gamma compiler
193-line Gamma1 lowerer
666-line former concatenative seed for the same scalar/effect language
--------------------------------
2,337 authored lines and four semantic/build layers
```

The direct evaluator runs the unchanged 85-line Gamma `const` augmenter,
requires its exact 51-byte Gamma receipt, and evaluates that receipt to byte 42.
It also covers literals, lexical bindings, every scalar operator, true/false
branches, forward and parameterized calls, recursion, compiler I/O, and quiet
invalid/trap outcomes. Exact and adjacent gates pin the 4,096-function census,
255-list syntax depth, and 256-context ordinary-call limits without inflating
the routine gate with multi-megabyte output or heap witnesses.
The separate [heap-boundary gate](../heap-boundary/README.md) owns full selected
pair-capacity and adjacent-refusal controls, including buffered-prefix
suppression. Scalar-forgery controls cover both the previous heap address and
the current first live pair address, `268435456`; a numerically correct address
does not replace the evaluator's pair provenance kind.

`function_lookup.py` supplies 16 additional authored-source controls. Distinct
function results are called in one fixed order while declaration and `main`
placement vary. Missing exact names cover shorter prefixes, extensions, and
equal-length neighbors. Marker controls distinguish first-declaration
application ownership from a later declaration with the same spelling. Reverse
declaration order reaches all 4,096 rows, then distinguishes a duplicate name
from a fresh 4,097th function before provision. The fixtures construct source
bytes and expected observations; they do not model evaluator lookup.

The evaluator keeps physical function rows in authored order and searches a
sorted pointer index by exact name. The index occupies
`0x01228000..0x01230000` inside the existing function partition, under the
unchanged 4,096-row preflight. The 16 controls pin lookup, row payloads, and
failure ownership independently of this implementation. The complete gate
runs 73 evaluator invocations, including the augmentation and capacity pairs.

## Limitations

The remaining admission work is outside evaluator semantics:

- Non-tail calls and source nesting retain explicit bounds.
- Integer parsing and arithmetic are deliberately wrapping Gamma operations;
  Delta owns checked arithmetic.
- The Beta root and the eventual Gamma derivation checker retain their own
  independent admission obligations.

The measured margin remains: the direct evaluator is 1,632 Beta lines versus
2,337 lines across the former concatenative route. More importantly, its state
is held in documented registers and explicit memory regions rather than hidden
behind a generic stack-machine expansion.

## Finding

The former concatenative Gamma is not earned as a permanent bootstrap rung.
Direct Beta already executes the high-level Gamma-authored augmentation workflow
with 1,632 authored Beta lines rather than 2,337 across the former route.
Proper-tail execution, whole-program static
validation, private pair provenance, and exact resource outcomes preserve that
local auditability without another semantic layer.
