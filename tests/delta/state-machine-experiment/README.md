# Typed state-machine Delta experiment

This experiment asks whether Gamma can implement the expensive core of a Delta
state-machine compiler without an intervening functional language.

The latest compiler implementation is retained here as `compiler.gamma`, beside
its customers, executable gate, and measurements.

It is not the normative Delta language or canonical compiler edge. The current
functional Delta contract remains unchanged while this evidence is compared.

## Surface

The whitespace-tokenized source admits:

```text
sum TYPE CASE+ end
record TYPE (FIELD TYPE)+ end
array TYPE ELEMENT_TYPE LENGTH
machine NAME ARITY (PARAM TYPE){ARITY} RESULT TYPE
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
mul DEST LEFT RIGHT
div DEST LEFT RIGHT
construct DEST SUM CASE
field-set RECORD FIELD SOURCE
field-get DEST RECORD FIELD
index-set ARRAY CONSTANT_INDEX SOURCE
index-get DEST ARRAY CONSTANT_INDEX
index-set-dyn ARRAY INDEX_VARIABLE SOURCE
index-get-dyn DEST ARRAY INDEX_VARIABLE
index-field-set ARRAY INDEX_VARIABLE FIELD SOURCE
index-field-get DEST ARRAY INDEX_VARIABLE FIELD
copy DEST SOURCE
read DEST
write SOURCE
call DEST MACHINE ARITY ARG{ARITY}
return SOURCE
goto STATE
brzero CONDITION ZERO_STATE NONZERO_STATE
brlt LEFT RIGHT LESS_STATE NONLESS_STATE
switch VALUE SUM (CASE STATE)+ endswitch
halt SOURCE
```

Top-level types and machines use the global namespace. Cases and fields are
scoped to their nominal owner; parameters, the result, locals, and states are
scoped to their machine. Duplicate spellings reject only within the same owner.
Calls admit at most thirteen one-word parameters and one one-word result.
Fixed-size software frames grow upward from Delta's 1 MiB static base while a
shadow of Alpha's return stack grows downward. A call halts with status 2 before
the two extents collide. Aggregate call arguments, closures, heap allocation,
and ambient unbounded recursion remain outside this experiment.

## Expensive essentials exercised

- complete ASCII tokenization and exact names;
- duplicate declaration rejection;
- nominal sums, records, and fixed arrays;
- typed parameters, results, locals, fields, and array elements;
- machine-local variable and state ownership;
- owner-local member names and repeated spellings across owners;
- constructor/family agreement;
- exhaustive declaration-order sum transitions;
- typed zero-through-thirteen-argument calls, returns, and direct recursion;
- deterministic software-frame and return-stack collision refusal;
- explicit state jumps and conditional transitions;
- direct Alpha address assignment and emission; and
- deterministic rejection of unknown syntax, names, types, bounds, and control.

## Measurement

| Subject | Size |
| --- | ---: |
| Gamma-written experiment compiler | 815 lines / 32,916 bytes |
| Native compiler produced by Gamma | 29,105 bytes |
| Pre-call-frame compiler baseline | 709 lines / 28,913 bytes |
| Pre-call-frame native baseline | 25,104 bytes |
| Representative Delta source | 57 lines / 1,224 bytes |
| Generated Alpha tape | 1,357 bytes |
| Epsilon parser-helper source | 71 lines / 1,658 bytes |
| Epsilon parser-helper code only | 48 lines / 10 states |
| Corresponding Functional Delta helpers | 9 lines |
| Epsilon parser-helper Alpha tape | 1,802 bytes |
| Exact scalar recursion counterpart | 29 lines / 771 tape bytes |
| Nested-scope parser source | 109 lines / 2,314 bytes |
| Nested-scope parser Alpha tape | 2,278 bytes |
| Recursive AST transform source | 427 lines / 10,776 bytes |
| Recursive AST transform Alpha tape | 11,038 bytes |
| Symbolic Alpha encoder source | 552 lines / 13,879 bytes |
| Symbolic Alpha encoder tape | 14,505 bytes |
| Retained functional Alpha backend declarations/implementation | 834 lines / 39,426 bytes |

The scoped-call increment over the 709-line baseline is:

| Concern | Source lines | Native bytes |
| --- | ---: | ---: |
| owner-scoped lookup | 8 | 362 |
| arity, frames, recursion, and one-word checks | 98 | 3,639 |
| total | 106 | 4,001 |

The canonical Gamma evaluator and self-hosted Gamma compiler independently
execute/compile this Gamma source. Their resulting native Delta compiler agrees
with interpreted execution on the exact 1,357-byte state sample, 1,802-byte
Epsilon helper, 2,278-byte parser, 11,038-byte recursive-transform sample,
14,505-byte encoder, and malformed twins,
including identical failure prefixes. The compiled parser passes nested-scope,
shadowing, duplicate-offset, malformed-scope, and arena-exhaustion cases. The
transform customer passes mixed and fully folded trees, a surviving conditional,
arity and framing errors, exact source offsets, and node/depth exhaustion.
The encoder covers every Alpha opcode and target shape, forward labels,
exact-ended input, high-bit immediate bytes, labels above 255,
duplicate/missing/extra labels, undefined targets, malformed records,
fail-before-output behavior, its retained 1,048,572-byte comparison payload,
and adjacent oversize rejection.

The call gate additionally executes zero- and thirteen-argument calls, rejects arity
fourteen, rejects mismatched call arity and aggregate parameters, and runs a
recursive 80,000,024-byte frame. Three entries fit; the fourth deterministically
halts at the software/hardware stack boundary. The Epsilon helper accepts
`123` as byte `0x7b`, rejects a nondigit as `0xff`, and exercises repeated
`c`, `value`, and `result` spellings in separate machine scopes.

## Reading the result

The experiment does not justify an intervening functional rung. Its hardest
implementation defects were implicit Gamma stack-temporary clobbering and manual
state naming. The state-machine CFG and fixed-storage representation remained
small and direct. This suggests that typed/named locals in Delta address the
observed pain more directly than another permanent functional language.

V2 tested three former reopen conditions: variable-length logical collections,
nested scopes, and deterministic source-offset diagnostics. Indexed fixed arenas
handled them with 72 additional Gamma lines. The recursive customer tests the
last D73 challenge with five nominal node variants, variable-arity child chains,
explicit postorder traversal, in-place rewrites, and canonical output. Its 105-line
transform is direct; declarations and parsing dominate at 260 lines.

The earlier result weakened the implementation-necessity case for Functional
Delta, but did not make state-machine syntax free. The complete customer needs 427 lines and
80 named states for a deliberately small tree language. A functional challenger
could compress traversal and reconstruction, at the cost of a larger trusted
compiler and implicit allocation/recursion machinery. The next decision should
compare those whole-edge costs against the actual Epsilon compiler rather than
adding richer values speculatively.

The symbolic backend supplies that first actual-customer comparison. The
state-machine encoder is 552 lines and 106 states; the corresponding retained
functional declarations and implementation occupy 834 lines. The smaller
version also avoids a persistent trie and balanced byte rope because it uses
fixed indexed item/label arenas and streams only after complete validation.
One twelve-word item row arena and one two-word label row arena consume
117,440,064 bytes; with Delta's 1 MiB static base they end at byte 118,488,640,
below Alpha's then-selected 256 MiB memory bound. Row fields carry nominal
owner/type checks instead of relying on fourteen parallel-array conventions.
The customer retains a 1,048,572-byte bounded comparison envelope; scaling its
fourteen-word-per-item fixed arenas to the selected 16 MiB Alpha profile would
exceed Alpha's then-selected 256 MiB memory and is not evidence against the
selected Functional representation. It does not independently replay emitted bytes. Connecting actual Epsilon lowering
and deciding whether symbolic prevalidation is sufficient remain required before
selecting normative Delta.

Scoped bounded calls do not reverse that conclusion. They grow trusted Gamma by
15 percent and its native artifact by 16 percent, which is material but does not
erase the implementation-size advantage by itself. The stronger discriminator
is customer code: the closest representable Epsilon parser kernel expands from
nine Functional Delta lines to 48 state-machine lines, and the exact immutable
`Bytes` recursion is not expressible in this slice. State-machine Delta remains
strong backend evidence, but this call model does not justify replacing the
normative Functional Delta language.
