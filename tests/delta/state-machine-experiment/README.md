# Typed state-machine Delta experiment

This experiment asks whether Gamma can implement the expensive core of a Delta
state-machine compiler without an intervening functional language.

The latest compiler implementation is retained at
[`../../../source/delta/compiler/experiments/state_machine/delta_compiler.gamma`](../../../source/delta/compiler/experiments/state_machine/delta_compiler.gamma).
This test owner retains only the customer, executable gate, and measurements.

It is not the normative Delta language or canonical compiler edge. The current
functional Delta contract remains unchanged while this evidence is compared.

## Surface

The whitespace-tokenized source admits:

```text
sum TYPE CASE+ end
record TYPE (FIELD TYPE)+ end
array TYPE word LENGTH
machine NAME PARAM TYPE RESULT TYPE
  (local NAME TYPE)*
  (state NAME | statement)*
end
entry MACHINE STATE
```

Statements are:

```text
const DEST NAT
add DEST LEFT RIGHT
sub DEST LEFT RIGHT
construct DEST SUM CASE
field-set RECORD FIELD SOURCE
field-get DEST RECORD FIELD
index-set ARRAY CONSTANT_INDEX SOURCE
index-get DEST ARRAY CONSTANT_INDEX
index-set-dyn ARRAY INDEX_VARIABLE SOURCE
index-get-dyn DEST ARRAY INDEX_VARIABLE
copy DEST SOURCE
read DEST
write SOURCE
call DEST MACHINE ARG
return SOURCE
goto STATE
brzero CONDITION ZERO_STATE NONZERO_STATE
switch VALUE SUM (CASE STATE)+ endswitch
halt SOURCE
```

The implementation deliberately uses one exact global namespace, one-word sum
values, statically allocated machine variables, constant array indexes, and
nonrecursive machine calls. These restrictions keep the experiment finite;
they are not proposed Delta semantics.

## Expensive essentials exercised

- complete ASCII tokenization and exact names;
- duplicate declaration rejection;
- nominal sums, records, and fixed arrays;
- typed parameters, results, locals, fields, and array elements;
- machine-local variable and state ownership;
- constructor/family agreement;
- exhaustive declaration-order sum transitions;
- machine calls and returns;
- explicit state jumps and conditional transitions;
- direct Alpha address assignment and emission; and
- deterministic rejection of unknown syntax, names, types, bounds, and control.

## Measurement

| Subject | Size |
| --- | ---: |
| Gamma-written experiment compiler | 636 lines / 25,533 bytes |
| Native compiler produced by Gamma | 22,339 bytes |
| Representative Delta source | 38 lines / 733 bytes |
| Generated Alpha tape | 523 bytes |
| Nested-scope parser source | 109 lines / 2,312 bytes |
| Nested-scope parser Alpha tape | 1,919 bytes |

The compiler line spend is approximately:

| Concern | Lines |
| --- | ---: |
| named compiler state | 69 |
| tokenizer and exact keyword checks | 70 |
| identifiers and integers | 37 |
| symbols and nominal types | 74 |
| declaration/state census | 77 |
| statement sizing | 56 |
| Alpha emitters and type helpers | 39 |
| typed replay and lowering | 214 |

The canonical Gamma evaluator and self-hosted Gamma compiler independently
execute/compile this Gamma source. Their resulting native Delta compiler agrees
with interpreted execution on the exact 523-byte state sample, 1,919-byte parser
sample, and eight malformed twins, including identical failure prefixes. The
compiled parser passes nested-scope, shadowing, duplicate-offset, malformed
scope, and arena-exhaustion cases.

## Reading the result

The experiment does not justify an intervening functional rung. Its hardest
implementation defects were implicit Gamma stack-temporary clobbering and manual
state naming. The state-machine CFG and fixed-storage representation remained
small and direct. This suggests that typed/named locals in Delta address the
observed pain more directly than another permanent functional language.

V2 tested three former reopen conditions: variable-length logical collections,
nested scopes, and deterministic source-offset diagnostics. Indexed fixed arenas
handled them with 72 additional Gamma lines. The remaining credible functional
challenge is rich recursive syntax transformation across many node variants,
not parsing, scopes, or bounded collection storage alone.
