# Checked static semantics

The `checking/` tree validates admitted source before lowering. Phase order is
fixed by [pipeline.gamma](../pipeline.gamma): source envelope
([source.gamma](source.gamma)), token admission ([lexical.gamma](lexical.gamma)),
balanced parsing and grammatical roles ([syntax/](syntax/README.md)), the
global identity census ([collection.gamma](collection.gamma)), declaration
resolution ([declarations.gamma](declarations.gamma) and
[declarations/](declarations/)), body checking ([types.gamma](types.gamma) and
[types/](types/)), and selected-profile validation
([profile.gamma](profile.gamma)). Scoped environments
([environments.gamma](environments.gamma)) and the exact-name tries and cursors
([names.gamma](names.gamma), [names/](names/README.md)) serve the census,
parameter catalogs, match coverage, and lexical scopes.

## Traversal and rebuild pairs

This audit charges every pair these phases allocate to an earlier-phase
occurrence, in the same style as the
[normalization audit](../normalization/README.md#traversal-and-rebuild-pairs).
It does not re-charge the parser or grammar worklist: their retained nodes,
spines, and frames are preflighted under the
[syntax provision](syntax/README.md#syntax-storage), which bounds them below
2,857,368 pairs at 40 bytes each — already inside the 40,265,318-pair arena.
Envelope and token admission, accessors, and token predicates allocate no
pairs; each phase's `compiler_complete` or `compiler_failure` outcome is
charged once in the tables below.

Quantities, each an earlier-phase measure bounded by the admitted source
extent `N <= 4,194,304` bytes:

- `S` — retained syntax nodes, `S <= min(N, 714,342)` because each node costs
  at least four syntax-ledger pairs.
- `D`, `C`, `F`, `P`, `Fd` — data declarations, authored constructors,
  functions, parameters, and constructor-field annotations.
- `I`, `L`, `M`, `U`, `B`, `K`, `A` — `if`, `let`, `match`, arm,
  pattern-binder, and application nodes plus checked argument edges.
- `V`, `E_n`, `K_r`, `W_n`, `Q` — name-token occurrences, name events,
  resolved-row replacements, total name-event bytes, and trie lookups, as
  defined in the [name-trie audit](names/README.md#pair-accounting).

The disjoint occurrence classes sum to at most `S`; edge counts are at most
`S` as well.

### Global census

[collection.gamma](collection.gamma) holds one cursor per global namespace.
Raw rows carry only source coordinates and retained nodes; typed metadata is
built during resolution.

| Site | Pairs | Charged per |
| --- | ---: | --- |
| Three `empty_identity_cursor` records + globals triple | 14 | compilation |
| Type metadata row `(pair owner (pair ctor-count payload-total))` | 2 | data declaration |
| Constructor metadata row | 4 | authored constructor |
| Function raw row `(pair owner (pair name-end node))` | 2 | function |
| Updated globals triple + `compiler_complete` | 3 | declaration |
| Constructor-collection outcome | 1 | data declaration |
| Finished-roots triple + outcome | 3 | compilation |
| Declaration-name seek, commit, and cursor finishes | name-event terms | per name event |

### Declaration resolution

[declarations/](declarations/) replaces census rows with typed metadata in
authored order. `identity_cursor_replace` preserves child edges and every
earlier immutable root; lookups are charged through `Q`.

| Site | Pairs | Charged per |
| --- | ---: | --- |
| Two resolution `identity_cursor` records | 6 | compilation |
| Parameter-catalog builder `(pair 0 empty-cursor)` | 5 | function |
| Parameter provision outcome, builder pair, and type spine cell | 3 | parameter |
| Parameter annotation option + outcome | <=2 | parameter |
| Catalog outcome, environment pair, reversed-spine carrier | 3 | function |
| Result annotation option + outcome | <=2 | function |
| Function signature record + outcome | 6 | function |
| Ordered parameter-type spine reversal | 1 | parameter |
| `identity_cursor_replace` of a resolved row | <=8 | function / payload constructor |
| Constructor-field annotation option + outcome + spine cell | <=3 | field annotation |
| Field-resolution outcome | 1 | payload constructor |
| Resolved constructor row + ordered field spine | 4 + arity | payload constructor |
| Constructor-list outcome | 1 | data declaration |
| Finished typed-roots triple + outcome | 3 | compilation |
| Metadata lookups and name seeks/commits | name-event terms | per lookup / name event |

### Body checking

[types/](types/) runs the explicit visit/resume machine. A continuation is
`(pair kind (pair payload previous))`; payload owners are listed per row.
Local and constructor name lookups (`local_environment_lookup`,
`find_function_metadata`, `require_constructor_metadata`,
`lookup_type_option`'s nominal find) are charged through `Q`, and every
`local_environment_bind` insertion through the name-event terms.

| Site | Pairs | Charged per |
| --- | ---: | --- |
| `if` kind-1/kind-2/kind-3 frames + payloads | 8 | `if` node |
| `let` annotation option, bind outcomes, environment pair, kind-4 frame + payload | <=9 | `let` node |
| Builtin signature spine | <=4 | builtin application |
| Kind-5 argument frame + six-field payload | 8 | checked argument edge |
| `match` kind-6 frame + context + coverage cursor | 12 | `match` node |
| First-arm context updates at body and resume | <=4 | `match` node |
| Arm state + kind-7 frame + pattern outcome | 6 | match arm |
| Pattern-binder bind (provision + environment + outcome) | 3 | pattern binder |
| Body and definition outcomes | 2 | function |
| Program and profile outcomes | <=3 | compilation |
| Fixed admission/parse/grammar/publish carriers | <=7 | compilation |
| Name lookups, coverage seek/commits, binder inserts | name-event terms | per lookup / bound name / arm |

### Rollup

Summing the site rows gives

```text
checking site pairs
  <= 36 + 7*D + 17*C + 31*F + 6*P + 4*Fd + 8*I + 9*L + 4*K
    + 16*M + 6*U + 3*B + 8*A
  <= 39*S + 36
```

(plus the at-most-once `compiler_failure` value, five pairs).

Name-trie and cursor work is shared with lowering, so it is bounded once in
the [name-trie audit](names/README.md#whole-compiler-rollup):

```text
name pairs <= 10*S + 46*V + 34*N + 22
```

These envelopes are deliberately coarse: every pair is now charged to an
admitted-source occurrence, but the constants do not establish that the total
stays below the pair arena for maximum-size sources. Branch-level rebuilds no
longer multiply by sibling count, so the former `1150*N` name-rebuild product
is now `34*N`; what remains for a whole-producer pair bound is the residual
name-event envelope together with the capture aggregate, now closed at
`sum(T) <= 32*N + 512*N*N` by the
[normalization audit](../normalization/README.md#capture-allocation-ownership).
