# Operation arithmetic policy

Ratified 2026-11-09. An integer overflow policy could only be chosen by
qualifying a binding's type, so an author who wanted one checked addition had
to mark the operands, and the mark then spread to every field, parameter and
local those values passed through. The rule now: a suffix on the operator
chooses the policy for that one operation (`+%` wraps, `+|` saturates, `+!`
traps) and the values stay plain integers. `Wrapping`, `Saturating` and
`Trapping` remain as `omega::core` domains for values whose every operation
should use one policy, defined by forwarding to the suffixed operators.

The normative owner is [policy operators](../../spec/language/numeric_values.md#policy-operators),
with touch points in [operator declarations](../../spec/language/expressions.md#operator-declarations),
[semantic roles](../../spec/language/domains.md#semantic-roles-and-operators)
and [proof terms](../../spec/proofs/contracts.md#mathematical-values-and-quantification).
Track implementation under `OPERATION-ARITHMETIC-POLICY` in
[TASKS.md](../../../TASKS.md). Delete this record when the implementation and
acceptance are complete.

## Why

Trapping is the case that spreads. A wrapping `u32` is a different kind of
number, a residue modulo 2^32, so carrying that on the type says something
about the value. A trapping add that returns has produced the exact sum:
"trapping" only records that its author chose a runtime check over a proof,
which is a fact about the operation.

A text search of six `source/psi` files at `1155b38948` (`lex/lexer.omg`,
`resolve/resolver.omg`, `resolve/names.omg`, `parse/statement.omg`,
`source/source.omg`, `checking/domains.omg`) found 992 `in Trapping`
annotations against 311 arithmetic operators; `parse/statement.omg` alone has
120 annotations for 5 operators. The operator count is a rough match on
spaced `+ - * / %` and leaves out conversions, so read it as the direction,
not an exact ratio.

## Decisions

- Suffixes `%`, `|` and `!` on `+`, `-`, `*`, `/`, `%`, `<<` and `>>`. The
  bare operator stays Exact.
- A suffixed operator means exactly what the unsuffixed operator means under
  that policy today. No new arithmetic is specified; the existing overflow,
  quotient, remainder and shift-count rows are the single definition.
- Operands are one bare fixed-integer carrier and the result is that bare
  carrier. An operand carrying any domain is first cast under
  [named domain casts](named_domain_casts.md).
- A `!` form is a `Trap` site that contributes a crash route, removable by
  path facts. Its normal return establishes that the result equals the exact
  mathematical result.
- The suffixed forms are compiler-owned integer primitives: not bindable by a
  `machine` declaration, no float forms, no compound assignment.
- The three policy domains stay, with unchanged role. Their unsuffixed
  integer operators are declared for two qualified operands and for one
  qualified with one plain operand, as the suffixed operation on the operands
  with the domain removed and the result requalified.
- Integer `as` is untouched. It was already Exact under every policy, and the
  wrapping, saturating and trapping narrowings are already named machines in
  `core/numeric_conversion.omg`. Array reads are untouched too: they always
  owe a bounds proof and never trap.

## Lexical notes

- Six base operators gain single tokens (18 in all). Longest match applies, so
  `a +!b` is a trapping add and `a + !b` needs its space.
- Right shift has no `>>` token today, so that nested static argument lists
  can close one `>` at a time. Its suffixed forms keep that: the suffix is the
  `%`, `|` or `!` immediately after the second adjacent `>`.
- `>>|` means the same as `>>`, and on unsigned carriers `/%`, `/|`, `%%` and
  `%|` mean the same as `/` and `%`. They exist so the suffix set is uniform.

## Implementation

1. Lex and parse the forms in both compilers: the Rust source-to-tokens and
   tokens-to-syntax stages, and `source/psi/lex` with
   `source/psi/parse/expression.omg`.
2. Type them as the builtin integer operation with a fixed policy, whatever
   the operands' qualifications, with the operand and result rules above.
3. Check them: the `%` and `|` forms owe what their policy rows owe; the `!`
   forms are crash sites with a route, path-fact disproof and the exact-result
   fact on normal return.
4. Lower them to the existing Wrapping, Saturating and `TrappingInteger`
   operations. No new Terminal operation is expected; if one is needed it is
   implementation work under the Psi delegation, not an owner question.
5. Declare the three policy domains' integer operators in `omega::core` in
   terms of the suffixed forms and remove the separate compiler definition of
   their arithmetic, with no observable change for existing programs.
6. Sweep `source/psi` and the `tests/psi` harnesses: plain carriers, a `!`
   operator where a runtime check is intended, and an Exact operator where
   existing facts already prove the bound. The domain stays legal elsewhere;
   the corpus and samples are not part of this sweep.

## Fixtures

Each with a negative twin, within the repository's fixture caps:

- Every suffix on `+`, `-`, `*` at a signed and an unsigned carrier, agreeing
  with the matching policy domain under `--both`.
- Signed `MIN / -1` and `MIN % -1` under each suffix; a zero divisor rejects
  for `%` and `|` forms and traps for `!` forms.
- Shift counts: reduced, proved and trapped.
- A `!` operation publishes `Trap`; the same operation behind a guard that
  rules out overflow publishes none.
- The exact-result fact from a `!` operation discharges a following Exact
  operation's obligation.
- An operand carrying any domain rejects until a cast removes it; a wholly
  anonymous tree rejects.
- `a + !b` and a nested static argument list closing with `>>` still parse as
  before.

## Left open

- The generic spelling for a caller-selectable policy, which was already
  unsettled.
- Whether a package may ever bind a suffixed token for its own types.
- Whether the arithmetic-policy role should dissolve into ordinary domains.
  That needs a rule for which `+` applies when a unit and a policy both home
  one, so the role stays compiler-owned for now.
