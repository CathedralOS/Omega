# Named domain casts

Ratified 2026-11-09. A value could gain or lose a domain without the source
saying so: an annotated `let` attached one to a literal, a predicate-only
domain dropped away wherever a plainer type was expected, and a proved
`requires x in D` could change which `+` an expression meant. A reader could
not tell from a line what its values carried. The rule now: a declared type
gains or loses a domain only through a cast that names that domain, written
`value as =Keep -Remove +Add`, and every domain the value carries appears in
that cast.

```omega
let trip: i32 in Km & Positive = 5 as +Km +Positive;
let length: i32 in Km = trip as =Km -Positive;
let plain: i32 = trip as -Km -Positive;
```

The normative owner is [domain casts](../../spec/language/domains.md#domain-casts),
with touch points in [semantic roles and operators](../../spec/language/domains.md#semantic-roles-and-operators),
[policy domains](../../spec/language/numeric_values.md#policy-domains),
[operator declarations](../../spec/language/expressions.md#operator-declarations)
and [contract-based repair](../../spec/language/dependent_values.md#contract-based-repair).
Track implementation under `NAMED-DOMAIN-CASTS` in [TASKS.md](../../../TASKS.md).
Delete this record when the implementation and acceptance are complete.

## Why

The earlier rule depended on what kind of domain was involved. Predicate-only
domains weakened implicitly, semantic and routed ones needed an explicit `as`
to a bare type, and a domain with neither predicates nor routes could be
attached by an annotation. Authors had to know each domain's kind to know
whether a line changed anything, and adding a predicate to a domain silently
changed which conversions were visible. One uniform rule removes the
classification from the reader's head.

The cast lists the whole set, not only the change. A cast that showed only
removals hid what survived, and a cast that restated the target type hid what
was given up. With `=`, `-` and `+` the line shows both, and a domain added to a
declaration later turns every cast that ignores it into an error at that cast.

## Decisions

1. A declared type gains or loses a domain only through a cast that names it.
   Facts are proved; they never attach, remove or select anything. Nothing
   weakens implicitly, predicate-only domains included.
2. A domain-only cast accounts for every attached domain: `=D` keeps, `-D`
   removes, `+D` adds. Parents, alias constituents and compiler-owned domains
   count. Entries are simultaneous and unordered. Any mismatch rejects.
   "Carries" means attached to the static type, not provable.
3. `+D` obeys the establishment rules of `D`: predicates are proved, routes
   need their evidence. `-D` discards no fact and never widens access; owned
   claims, cleanup selections and access policies cannot be cast away.
4. A carrier cast takes an operand with no domains and may only add:
   `near as -Metres as i64 +Metres`. `near as i64`, `near as i64 =Metres` and
   `near as i64 -Metres +Metres` reject.
5. The whole-type targets `x as T in D` and `x as T::D` are retired.
6. A child domain travels with its ancestors, each named in the cast, and the
   result is ancestor-complete. Naming an alias names its expansion.
7. A literal receives a domain only through a cast. An expected type selects a
   carrier and never inserts a cast, at a `let`, argument, field, return or
   arm. Arms and joins agree on the exact domain set.
8. Builtin operators and comparisons, the suffixed policy operators included,
   take unqualified operands, in code and in contracts. In a domain's own
   `requires`, `self` is the plain carrier value.
9. A declared operator applies only when its parameter types equal the
   operands' complete qualified types. Nothing composes two domains'
   operators. A domain may declare an operator against a plain operand.
10. Through a mutable reference a cast removes a domain exactly when the
    lender could add it again itself. The end of the loan is that unwritten
    `+D`, checked for the current contents under the invariant-window rules.
11. After `as` come an optional carrier and the longest run of entries.
    Parsing uses no name lookup, so arithmetic on a cast result takes
    parentheses.
12. Open remains the default and `access` restricts. Result-domain overloads
    are unchanged: an expected type selects a declared machine and inserts
    no cast.

## What changes for source

- Every literal bound for a qualified place gains a cast: `5 as +Km`.
- Every silent drop of a predicate-only domain gains one: `text as -Utf8`.
  A reviewer's text search of `source/library` counted about 250 machines
  taking `&[u8] in Path`; each caller passing plain bytes, and each body
  passing those bytes to a plain-byte helper, is affected. That count is not
  re-verified here.
- Each whole-type cast is respelled. `x as i32` on a qualified `x` becomes
  `x as -D`, and a carrier change on a qualified value becomes two casts.
- `x as u32 + 1` becomes `(x as u32) + 1`, because `+` and `-` after any cast
  begin an entry.
- Operators and comparisons on qualified operands need a declaration for that
  exact type or a cast. Core declares the policy domains' operators and
  comparisons for two qualified operands and for one qualified with one plain
  operand, so `counter + 1` is unchanged. A domain such as `Km & Wrapping`
  matches none of them.
- Indexing a qualified collection, or with a qualified index, follows the
  same exact-match rule.
- A helper that receives governed storage names the qualified type in its
  parameter; an unqualified write view of a routed domain no longer exists.

## Implementation order

1. Parse cast entries after `as` in both parsers; reject the retired
   whole-type targets with a diagnostic that prints the replacement.
2. Check accounting against the normalized attached set, then route `+`
   through the existing establishment checks and `-` through the existing
   access, custody and cleanup retention checks.
3. Remove implicit predicate-only weakening and implicit attachment at every
   admission site: `let`, assignment, argument, field, return, arm and join.
4. Restrict builtin operators to unqualified operands and make declared
   operator matching exact. Declare the core policy-domain operand shapes.
5. Admit `-D` on a mutable reference where the lender could add `D`, and
   re-establish each removed domain at the end of the loan as an
   invariant-window debt.
6. Sweep `source/library`, then `samples` and `tests/omega`, re-recording
   touched golden rows. `source/psi` is detached from the product path and
   may follow last.

The non-reference build used during design (`eb37f41`, stable toolchain)
attached and dropped a predicate-free, route-free tag silently, dropped a
predicate domain silently and already required `as` to drop a domain that
homes an operator (`tests/omega/fail/domains/implicit_semantic_domain_local_weakening`).
Steps 3 and 4 are therefore where existing programs start to reject.

## Fixtures and acceptance

One pass and fail pair per behavior, within the six-file limit per change:

- Accounting: `=`, `-` and `+` on a two-domain value pass; an unlisted
  attached domain, `+` of a present domain and `-` of an absent one each
  reject and name the domain.
- Literals: `5 as +Km` passes; an annotated `let`, an argument and a field
  initializer supplying a bare literal each reject.
- Carrier: remove, convert, add passes; a carrier cast on a qualified operand
  rejects.
- Ancestors: keeping the parent while removing the child passes; keeping the
  child while removing the parent rejects.
- Operators: a builtin comparison on a qualified operand rejects; the same
  comparison declared by the domain against a plain operand passes.
- Mutable views: removing a tag and a predicate through `&mut` passes when a
  callee guarantee or a later write re-proves the predicate; an unrepaired
  predicate rejects at the next use; removing a routed domain rejects outside
  its authorized routes.

Acceptance: these pass `omega run --both`, the existing weakening fixtures
still reject, and no whole-type cast target remains in `source/library`,
`samples` or `tests/omega`.

## Left open

- The spelling that removes a domain attached to an element type behind a
  view, such as `&[f32 in Km]` read as `&[f32]`.
- A shorthand for a domain that wants every comparison passed through to its
  carrier, in place of one declaration per operator.
- Which qualified operand shapes core declares for indexing.
- The Terminal Psi form of a cast that both removes and adds. Its
  qualification-change edges today are strictly an addition or strictly an
  erasure.
- Whether a generic operator declaration may quantify over a set of domains.
  Today each exact set needs its own declaration.
