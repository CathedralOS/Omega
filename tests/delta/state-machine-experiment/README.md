# Typed state-machine Delta experiment

This experiment asks whether Gamma can implement the expensive core of a Delta
state-machine compiler without an intervening functional language.

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
| Gamma-written experiment compiler | 564 lines / 22,601 bytes |
| Native compiler produced by Gamma | 19,872 bytes |
| Representative Delta source | 38 lines / 733 bytes |
| Generated Alpha tape | 453 bytes |

The compiler line spend is approximately:

| Concern | Lines |
| --- | ---: |
| named compiler state | 64 |
| tokenizer and exact keyword checks | 76 |
| identifiers and integers | 49 |
| symbols and nominal types | 84 |
| declaration/state census | 88 |
| statement sizing | 45 |
| Alpha emitters and type helpers | 36 |
| typed replay and lowering | 122 |

The canonical Gamma evaluator and self-hosted Gamma compiler independently
execute/compile this Gamma source. Their resulting native Delta compiler agrees
with interpreted execution on the exact 453-byte sample and eight malformed
twins, including identical failure prefixes.

## Reading the result

The experiment does not justify an intervening functional rung. Its hardest
implementation defects were implicit Gamma stack-temporary clobbering and manual
state naming. The state-machine CFG and fixed-storage representation remained
small and direct. This suggests that typed/named locals in Delta address the
observed pain more directly than another permanent functional language.

Reopen the functional-rung comparison when a larger slice needs recursive syntax
values, variable-length collections, nested scopes, or rich diagnostics. Those
are deliberately absent here and may change the result.
