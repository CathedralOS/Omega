# Checked Delta to expanded Gamma

Start at [program.gamma](program.gamma). It consumes the complete checked
program and resolved catalogs after selected-profile schema validation and
builds every function definition in authored order. Data declarations select
whether one shared projection definition is needed; they emit no Delta types.
No lowering helper publishes receipt bytes. Its result is the complete
[expanded Gamma program](../representation/README.md).

[expressions.gamma](expressions.gamma) owns the shared visit/resume machine.
Concept-owned helpers handle bindings, applications, constructors, checked
arithmetic, and matches. They consume completed grammar, name, arity, and type
judgments rather than deciding those judgments again.

## Continuations and binding custody

The common operations are:

```text
lowering_visit(node, locals, depth, frames, globals)
lowering_resume(value, depth, frames, globals)
frame = (pair kind (pair payload previous))
```

The resumed value is an expanded Gamma expression. Frame depth counts pending
compiler work, not source depth or generated expression height. Resume pops
exactly one frame before dispatch; handlers receive the decreased depth and
previous stack. Payload owners retain completed child values and the lexical
environment needed for the next child. Counts govern pair-spine projections.

[bindings.gamma](bindings.gamma) reuses the existing exact-name trie, storing
original Gamma binding atoms rather than types. Immutable snapshots preserve
lexical scopes; only parameters retain a separate declaration-order spine. A let
initializer is lowered in its outer environment; only the body receives the
new source binding atom. Parameter and pattern references likewise reuse their
established atoms. Looking up that custody does not introduce a second conflict
or type policy after checking.

## Concept ownership

Application arguments and constructor fields are lowered left to right before
their enclosing Gamma expression is built. Checked arithmetic binds each
operand once before constructing its existing overflow guard; generated marker
and coordinate identities remain shared by the guard's references.

[matches.gamma](matches.gamma) coordinates the subject and arm bodies.
[matches/arms.gamma](matches/arms.gamma) owns authored arm order, tag selection,
and the final exhaustive fallback.
[matches/bindings.gamma](matches/bindings.gamma) owns pattern binding and
payload projections. The completed subject appears once under its generated
binding; arm bodies use the common continuation machine. Constructor metadata
supplies checked representation and tags.

### Wide payload projections

Patterns with at most 256 fields keep their existing inline projection chains;
each individual chain then has at most 255 Gamma expression lists. Wider
patterns call one ordinary generated Gamma function, `$dp(payload, index)`,
which tail-recursively takes `second` until the index reaches zero. Nonfinal
fields take `first` of that tail; the final field is the unwrapped tail itself.
Indices are generated in `0..arity-1`, so each reached `second` has a pair
under the already-checked product representation. The helper neither allocates
product nodes nor changes field order, provenance, or authored trap behavior.
Its recursive call is proper-tail, but each initializer's initial call consumes
one ordinary runtime context; this is not a promise of unchanged exhaustion
thresholds for arbitrary application stacks.

The checked data-before-functions order lets `program.gamma` install `$dp` once
when the first constructor wider than 256 fields is encountered. During that
prefix the definition count is zero or one. Later wide constructors reuse it,
even across nominal owners; a wide declaration without a matching use retains
the same small definition instead of requiring another usage-analysis pass.
The helper's two parameters and height-three body are ordinary Gamma structure.
Its reserved name cannot collide with renamed authored functions or generated
`$hN` extraction names. It is not a new primitive, adapter, or language rung.

This avoids rebuilding and printing quadratic projection chains for wide
patterns. It does not change the product layout or make evaluation of all `k`
fields subquadratic: the runtime still walks each requested prefix. Enclosing
binding chains and wide constructor products still undergo normal height
normalization. Programs whose constructors all fit 256 fields retain their
previous plan and receipt bytes.

Calls, lets, products, and guards are ordinary Gamma plan nodes. Their
constructors compute expanded expression-list heights, including generated
wrappers, and cache exact serialization extents through the serializer's
count-only formatting helpers. The later serializer does not need Delta constructor, pattern,
arithmetic, or lexical-environment knowledge.

## Traversal and rebuild pairs

This audit charges every pair lowering allocates to a retained-source
occurrence, in the same style as the
[normalization audit](../normalization/README.md#traversal-and-rebuild-pairs).
Gamma constructors cost their fixed shapes: `gamma_node` is four pairs plus
any payload pair and `gamma_call`/`gamma_let` add two; `gamma_word` and
`gamma_generated` cost five, `gamma_first`/`gamma_second` twelve,
`gamma_pair`/`gamma_eq`/`gamma_lt` thirteen, `gamma_if` fourteen, and
`gamma_trap` twenty-one; `gamma_definition`/`gamma_program` are three and
one. Name lookups and `lowering_bind` insertions are charged through the
shared [name-trie audit](../checking/names/README.md#pair-accounting); the
quantities `N`, `S`, `V`, `W_n`, `E_n`, and `Q` are defined in the
[checking audit](../checking/README.md#traversal-and-rebuild-pairs).

| Site | Pairs | Charged per |
| --- | ---: | --- |
| Kind-1 remaining-argument frame + payload | 6 | lowered argument edge |
| Argument result cell + ordered-spine reversal | 2 | lowered argument edge |
| Ordinary `gamma_call` + head atom (`gamma_literal`/`gamma_function`/`gamma_word`) | <=11 | call-like node (application, `if`, `eq`, `lt`, `/`, `%`, builtin) |
| Constructor tag + outer `pair` + product spine/reversal | <=21 + 14*a | constructor atom or application with `a` fields |
| Checked-arithmetic template (3 generated atoms, head literal, operation call, guard, 3 lets) | <=178 | `+`/`-`/`*` node |
| Literal / local-reference / constructor atom | <=21 | atom occurrence |
| `let` binder atom + kind-2 and kind-3 frames + `gamma_let` | 17 | `let` node |
| `match` kind-5 frame + generated context atoms | <=22 | `match` node |
| Kind-6 arm frame + match-state payload + bound/reversed rows | 13 | match arm |
| Arm-selector `eq`/`if`/tag chain | 31 | non-final match arm |
| Payload unwrap (`first`/`second` + at most three lets) | <=42 | `match` node |
| Pattern-binder atom + binding row | 6 | pattern binder |
| Field projection (inline `first`/`second` pair or `$dp` call) | <=29 | pattern binder |
| `gamma_let` projection wrapper | 6 | pattern binder |
| Parameter atom + declaration-order cell + parameter spine reversal | 6 | parameter |
| Function name atom + definition record | 7 | function |
| Parameter-list carrier `(pair locals reversed)` | 1 | function |
| Program definition cell + ordered-definition reversal | 2 | function |
| Program root + outcome | 2 | compilation |
| Shared `$dp` definition + its program cell | <=89 | compilation with a wide constructor |
| Local/constructor lookups and binder inserts | name-event terms | per lookup / bound name |

A call-like node with `a` lowered arguments therefore allocates `8*a + 11`
site pairs; a constructor application `22*a + 21`; an arithmetic node 194;
a `let` 17; a `match` at most `64 + 44*U + 41*B`; a function `9 + 6*P_f`;
and the program `F + 2` plus at most 89 for `$dp`. With the disjoint
occurrence classes summing to at most `S` retained nodes and `Ce <= A`
constructor-field edges inside the `A` argument edges,

```text
lowering site pairs
  <= 21*At + 11*K + 21*Ca + 14*Ce + 178*R + 17*L + 64*M + 44*U + 41*B
    + 10*F + 6*P + 8*A + 91
  <= 178*S + 22*S + 91
  <= 200*S + 91
```

where `At`, `K`, `Ca`, `R`, `L`, `M`, `U`, `B`, `F`, and `P` count atom,
non-constructor call, constructor-application, arithmetic, `let`, `match`,
arm, pattern-binder, function, and parameter occurrences.

### Plan size handed to normalization

The same sites construct the `G` plan-node occurrences the
[normalizer](../normalization/README.md#traversal-and-rebuild-pairs) charges
against. Counting node-producing rows — atoms and shared binding atoms, call
and let nodes, constructor products, arithmetic templates, match contexts,
selector chains, projections and wrapper lets, parameters and the `$dp`
definition — gives the deliberately loose bound

```text
G <= 40*S + 15
```

so the normalizer machine's `45*G + 7*F + 1` reduces to an admitted-source
term. That reduction does not itself bound capture merges, which the
normalization audit closes separately through its incidence charge.

## Remaining boundaries

This phase deliberately returns the expanded, pre-normalization plan. The
private lowering-height diagnostic observes those original heights, including
heights above 255. Normal compilation passes the plan to
[normalization](../normalization/README.md) before publication; that separate
owner handles body nesting without changing Delta lowering rules.

Allocated plan and continuation pairs consume the evaluator's finite immutable
arena; the [traversal-and-rebuild accounting](#traversal-and-rebuild-pairs)
above charges each one to a retained-source occurrence and bounds the produced
plan size `G` without establishing whole-producer arena containment.
Resource/internal DCOUT closure and full generated-profile admission
remain open. Body-height normalization does not by itself bound helper count,
live runtime contexts, or cumulative storage. Exact checking and execution
receipts remain explicit regression obligations.
