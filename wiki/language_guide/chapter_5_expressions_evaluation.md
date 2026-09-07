# Chapter 5: Expressions And Evaluation

Expressions compute values. Statements perform work in a machine or state.

This chapter records the expected evaluation shape so machines, proofs, and
lowering agree.

## Literals

Numeric literals are typed by suffix or context.

```omega
let machine_value: i32 = 3i32;
let proof_value: Nat = 3nat;
let machine_float: f64 = 3.0f64;
```

Machine numeric types such as `i32`, `u64`, `f32`, and `f64` carry
representation, rounding, and overflow obligations. Proof numeric types such
as `Nat`, `Int`, and `Real` are mathematical values. `Real` is an ordinary
opaque `omega::core` declaration, currently supplied by
`omega::language::core::real` during the axiomatic N5 stage. It is neither a
compiler numeric primitive nor a standard-library/runtime literal format;
values and laws enter through that package's explicit proof surface.

A fixed-array literal evaluates and establishes its element expressions exactly
once in increasing index order. That order is semantic: if an ordinary
cleanup-bearing edge abandons a partially constructed array, the successfully
established prefix is cleaned in reverse order. A trap or nuclear abort has no
successor edge and performs no cleanup.

## Evaluation Schedule

The existing multi-child expression forms governed here, plus adjacent
transition dispatch, have one closed evaluation schedule:

| Form | Semantic evaluation order |
| --- | --- |
| attached call | receiver, then authored arguments left to right |
| free or path-qualified call | authored arguments left to right |
| strict binary operator | left operand, then right operand |
| `left && right` | left; right only when left is `true` |
| `left || right` | left; right only when left is `false` |
| index | collection, then index |
| range | present start, then present end |
| fixed-array literal | increasing index |
| record or case literal | authored field-expression order |
| `transition` | subject exactly once before dispatch; only the selected arm runs |

Each evaluated child runs exactly once. Unary operations, borrows, casts,
membership tests, and member access have only one immediate runtime child and
therefore introduce no relative-order choice. The `&&`, `||`, and transition
rows are the complete current selective set within this expression grammar; a
future lazy or selective form must extend this table explicitly rather than
inherit an open exception.

Named aggregate fields are scheduled where the literal writes them, not where
the data declaration places them. For `data Pair { first: T; second: T; }`, the
literal `Pair { second: make_second(), first: make_first() }` calls
`make_second()` first. Successfully evaluated values are then installed into
their named fields. Declaration order still defines completed-value canonical
identity and structural cleanup, while selected physical layout defines byte
placement; neither reschedules the source computation.

If an ordinary cleanup-bearing edge abandons a partially staged call or
aggregate, only the established prefix exists and it is cleaned in reverse
establishment order. Once an aggregate is complete, its fields instead follow
the recursive reverse-declaration cleanup rule in
[Chapter 17](chapter_17_drops_and_cleanup.md). A trap or nuclear abort still has
no cleanup successor. Reordering is legal only after proving
that results, effects, failures, moves, and cleanup remain observationally
unchanged.

## Assignment

Assignment writes a value into a place.

```omega
self.health = self.health - damage;
```

The assignment is accepted only if the place is mutable, the value is compatible
with the place type, and the resulting facts satisfy the place invariants.

## Calls

Calls evaluate arguments, enter a machine, and receive its result or output
effects.

```omega
let command: Command = self.parser.resolve(&self.line);
self.view.render_room(&self.room);
```

An attached receiver evaluates first, followed by every authored argument from
left to right. A free or path-qualified call evaluates its arguments from left
to right. Each receiver and argument is evaluated exactly once; ordinary
abandonment of partial staging follows the evaluation-schedule cleanup rule
above.

Calls whose statically known operational envelope may pause execution while
live state remains held require an exact acknowledgement:

```omega
ordinary_call();                 // guaranteed neither
suspend may_park();              // may suspend the activation
block may_block_worker();        // may block the current worker
suspend block may_do_either();   // may do either
```

`suspend` and `block` are contextual prefixes recognized immediately before a
call; they do not become global declaration modifiers. These prefixes
acknowledge possibilities, not events guaranteed to happen.
They do not force waiting, create a task or future, alter the call's contract,
change its result, or select an implementation. Missing, partial, redundant,
or out-of-order markers reject against the call's statically known envelope.
The combined spelling is always `suspend block`.

Suspension creates a continuation boundary. A call carrying `suspend` must
therefore be the complete operation in one of these positions:

```omega
suspend inbox.wait();                    // statement
let event: Event = suspend inbox.take(); // simple let right-hand side
transition suspend inbox.take() { ... }  // transition subject
suspend compute_result()                 // terminal expression
```

It may not be nested inside another argument, operator, aggregate, condition,
or other partially evaluated expression:

```omega
// Rejected: the left operand would become hidden continuation state.
let total: u64 = prefix + suspend source.next();

// Bind first.
let next: u64 = suspend source.next();
let total: u64 = prefix + next;
```

A blocking-only call retains the ordinary stack and creates no continuation
boundary, so it may nest:

```omega
let total: u64 = prefix + block source.next();
```

A separate binding is still often clearer, especially under a held guard. The
checker uses local checked summaries where available and pinned envelopes for
imports, requirements, generic calls, and boundary operations. Dynamic calls
use the per-requirement envelope statically retained by the value.
Consequently, transparent `suspends false` and `blocks false` refinements remove
the corresponding marker requirement.

## Operators

Operators are typed operations.

```omega
let next: u64 = index + 1;
let weak: bool = self.health <= 25;
```

The same operator spelling may exist for machine numbers and proof numbers. The
operand types decide which rules and obligations apply.

Operators should be understood as shorthand for resolved semantic operations,
not as syntax with a completely separate meaning model. In that sense,
`left + right` is conceptually like a call to the appropriate add/concat
operation for the operand meaning in scope.

A fixed operator token such as `+`, `[]`, or the range slice `[..]` resolves to
a named `operator` declaration. A declaration that binds a fixed token writes
that literal token immediately after `operator` and before its descriptive
path:

```omega
operator + i32::add(left: i32, right: i32) -> i32;
```

The token comes from a closed compiler-owned vocabulary with fixed lexical
spelling, precedence, associativity, fixity, allowed arities, and source-position
mapping. It is not a string, name, or user-defined punctuation sequence. The
resolved declaration -- its path plus normalized signature/overload identity --
remains canonical. Checked and terminal calls retain that declaration rather
than a bare token, while the token binding remains public source-compatibility
surface. Adding, removing, or changing it is a breaking source revision.

The arithmetic portion of that fixed vocabulary has this precedence, from
tightest to loosest:

| Tier | Forms | Binary associativity |
| --- | --- | --- |
| grouping and prefix | `(expression)` and prefix unary operators | not applicable |
| multiplicative | `*`, `/`, `%` | left |
| additive | `+`, `-` | left |

This table fixes expression grouping. The evaluation-schedule table above
separately fixes strict binary operands left to right and names `&&` and `||` as
the two current short-circuit connective forms.

One declaration binds at most one fixed token. Several declarations may bind
the same token when their normalized operand/domain shapes distinguish them;
unary and binary `-`, or `f32` and `f64` `+`, are ordinary separate
declarations. Two participating declarations with the same token and operand
shape are ambiguous and reject. A second surface spelling requires a second
declaration, which may forward to the same implementation. A named `operator`
with no fixed-token surface remains callable by its path.

A path-qualified named call such as `Token::ordered(left, right)` selects the
same operator declaration and overload model. `Token` is a static namespace in
that form, not a value receiver; an attached call on a value retains its actual
receiver. Compiler consumers must use the checked named-operator resolver and
authored selection occurrence rather than infer identity from the leaf spelling
`ordered`.

For the currently implemented generic named-operator cohort, all static
parameters must be type parameters and the runtime operands must infer one
complete closed application. The call may omit the static arguments or
explicitly repeat that same application; an explicit argument that differs
from operand inference rejects. Each inferred type must satisfy its declared
copy, linear, and carry property bounds through the same structural judgment
used by ordinary generic instantiation. Open type applications and const,
lifetime, machine, and proposition applications remain outside this closed
exact-application evidence cohort; they do not acquire a concrete row. A
Unit-returning named call written as a statement preserves the same declaration
selection when the compiler normalizes it into value form.

A selected nongeneric checked-body provider may realize a boundary operator
without changing that public declaration identity. The checked execution path
currently redirects exact two-operand arithmetic/comparison tokens and exact
indexing uses to the selected checked adapter. The selected plan fingerprint
and its strong commitment must both rejoin the checked use before redirection.
Range forms, unsupported arities, aliases, and generic or lifetime-bearing
realizations remain rejected rather than acquiring an approximate dispatch.

An operator is an independently nameable declaration and is package-private by
default. `pub operator` permits another package to select it. A qualified
spelling such as `Vector::add` does not inherit `Vector`'s visibility; the
operator owns its own source visibility.

So `left + right` resolves to `i32::add` for `i32` operands. The public
signature and any proof obligations stay visible on the declaration; only the
primitive lowering hides behind `boundary` when the operator is a boundary
operator. The former `spelling` clause is retired bootstrap syntax, not an
alternate accepted form.

This model also applies to privileged syntax. `items[index]` should be
understood as an indexing operator, not as raw pointer syntax. `items[1..]`
should be understood as a range-slice operator. Both resolve to a spelled core
`Slice`/`Array`/`Vec` operator whose `requires` clause is the bounds proof
obligation:

```omega
boundary operator [] Slice::index<T>(items: &[T], index: u64) -> T
requires
    index < items.len;

boundary operator [..] Slice::range<T>(items: &[T], start: u64, end: u64) -> &[T]
requires
    start <= end && end <= items.len;
```

Those operators have a semantic home that users and tools can inspect, while
their boundary primitive implementation is bound through the compiler/runtime
layer.

An operator may use an attached receiver or explicit operands. `self` is a
distinguished receiver with ordinary ownership meaning, not merely shorthand
for the enclosing type:

```omega
operator + Vec2::add(self, right: Vec2) -> Vec2;
operator == Vec2::equals(&self, other: &Vec2) -> bool;
operator + Float::add(left: f32, right: f32) -> f32;
```

The receiver occupies normalized position zero; without one, the first
ordinary parameter does. Fixed-token resolution uses the resulting complete
operand telescope, so attached and static/free declarations share one model.
`+` may consume or borrow its operands while producing a new value; mutation
comes from an `&mut self` or other mutable operand on an operation whose fixed
token admits that shape, not from being an operator.

Trait requirements may themselves declare an operator binding:

```omega
trait Ranked<T> {
    operator < compare(left: T, right: T) -> bool;
}
```

Conformances supply that requirement's implementation and never rebind its
token. A trait-backed token use requires one exact conformance already selected
by an explicit proof-static binder in the surrounding machine. It never picks
the only visible conformance. No selected binder rejects even when one matching
conformance is visible; several applicable binders are ambiguous. The named
requirement call with an explicit conformance application is the escape.

A concrete type may publish one direct wrapper as its canonical token meaning,
but a second wrapper over the same token and operand shape rejects. Other
meaningful conformances remain available permanently through named explicit
calls. Direct core and user operators such as integer addition require no
conformance selection.

This chapter only defines ordinary evaluation. Domain-sensitive operator
resolution belongs to the domains chapter because it depends on statically
selected semantic qualifications rather than raw expression syntax or
flow-established facts.

## Core Collections And Views

Omega should distinguish user-facing core concepts from the low-level carriers
the compiler uses to lower them.

Core collection and text concepts:

- `Array[T; N]`: fixed-size owned inline storage.
- `Vec[T]`: owned dynamic contiguous storage.
- `Slice[T]`: borrowed contiguous view over elements.
- `[u8; N]::Utf8`: bounded owned text storage.
- `Vec<u8>::Utf8`: growable owned text storage once allocation is available.
- `&[u8]::Utf8`: borrowed text window (`&mut [u8]` plus an establishment
  contract when mutation must preserve the encoding).

The surface uses the ordinary carrier plus domain spelling:

```omega
let fixed: [Item; 4];
let view: &[Item] = fixed.as_slice();
let text: [u8; 64]::Utf8 = "hello";
let text_view: &[u8]::Utf8 = text;
```

`Array`, `Vec`, and `Slice` are visible core concepts, not just implicit
compiler behavior. `Array` and `Vec` are owners. `Slice` is the common borrowed
view they can produce. Text does not introduce another owner/view hierarchy:
the `Utf8` fact qualifies the byte carrier or byte view that already owns or
borrows the storage.

Projecting a bounded carrier or vector to a slice is descriptor-preserving and
does not copy its initialized bytes. Mutating or reallocating the owner while
the view remains live is rejected. Native lowering and the interpreter agree
on the pointer-and-live-length descriptor exposed by the view.

The implementation can still use privileged internal carriers. A slice view is
likely lowered as a descriptor such as pointer plus length. A vector is likely
lowered as an owned buffer with pointer, length, and capacity. Those carriers
belong near the boundary/primitive layer, while the public proof and operator
surface belongs to core concepts such as `Slice`.

This means names such as `Slice::Length` should be browsable and documented as
core semantic declarations even if their runtime representation is compiler
managed.

## Indexing And Slices

Indexed access is an ordinary expression form with proof obligations.

```omega
let item: InventoryItem = items[index];
let first: InventoryItem = items[0];
```

Working interpretation:

- `items[index]` resolves to the appropriate indexing operator for the
  collection or view.
- The compiler must prove the index is in bounds for the accessed view.
- Fixed arrays, vectors, and slice views can use the same `[]` surface while
  obtaining the proof from different sources.
- `items[0]` is valid when the current facts prove the view is non-empty.
- Mutable indexing and mutable subslicing have the same bounds proof
  obligations, plus the ordinary borrow-checking obligation that the selected
  element or view is uniquely writable.
- Slice ranges such as `items[1..]` resolve to a range-slice operator that
  creates a new view with a narrower extent and updated facts.
- Borrow overlap uses those normalized extents. A live `items[1..]` view does
  not prevent writing `items[0]`, while writing `items[1]` still conflicts;
  dynamic bounds remain conservative unless current facts prove disjointness.
- Text windows use ordinary slice byte indexing and ranges. Character or
  grapheme indexing must be a separate semantic
  operation because UTF-8 byte positions are not the same as user-visible
  characters.

Omega uses `a..b` for an exclusive range and `a..=b` for an inclusive range.
The open-ended forms `a..`, `..b`, and `..` are also supported. An inclusive
range `a..=b` is defined to mean the same as `a..(b+1)` and is normalized to the
exclusive form internally.

This normalization fixes the proof obligations for a range end:

- An exclusive end `b` must satisfy `b <= len`.
- An inclusive end `b` must satisfy `b < len`. Inclusive-end validity is exactly
  index-`b` validity, because `b` is an indexed position.
- An inclusive non-empty range additionally establishes a `non_empty` fact.

The overflow edge of an inclusive range, such as `..=len-1` when `len` is `0`,
or `..=MAX` where `b+1` would overflow, is a proof error, not a runtime panic.

Omega loops often look like repeated transitions over either:

- a bounded index carried in state parameters, or
- a shrinking slice window where `[0]` remains valid until the window is empty.

The important point is that indexing is not magical pointer syntax. It is a
normal operation guarded by proof of a valid range. For built-in core
collections, the operator contract and named measures should be visible as part
of the core language surface; the low-level pointer/descriptor work belongs to a
boundary primitive implementation layer below that surface.

The visible core declaration should therefore look like a normal contract on a
`boundary operator`:

```omega
boundary operator [] Array::index<T>(items: &Array<T>, index: u64) -> T
requires
    index < items.len;

boundary operator [] Vec::index<T>(items: &Vec<T>, index: u64) -> T
requires
    index < items.len;

boundary operator [] Slice::index_mut<T>(items: &mut [T], index: u64) -> &mut T
requires
    index < items.len;

boundary operator [..] Slice::range_mut<T>(items: &mut [T], start: u64, end: u64) -> &mut [T]
requires
    start <= end && end <= items.len;

boundary operator [..] Slice::from<T>(items: &[T], start: u64) -> &[T]
requires
    start <= items.len;

```

The proof checker owns `start <= items.len`. The boundary primitive owns the
descriptor/pointer rewrite that actually constructs the narrower view.
Owned dynamic storage is obtained through an explicit allocator package over
qualified storage authority; `Vec` has no ambient `with_capacity` shortcut. A
growable text owner is that same `Vec<u8>` qualified by `Utf8`; append
operations carry both the capacity/domain proofs and the ordinary unique-borrow
obligation.

Operator declarations form overload sets by call signature. Overload resolution
keys on the operator path plus parameter types; return type alone never
distinguishes overloads. Generic signatures are compared by structure, not by the
spelling or declaration order of type parameters, so these two declarations
describe the same candidate and must be rejected as a duplicate:

```omega
operator Slice::index<T>(items: &[T], index: u64) -> T;
operator Slice::index<U>(items: &[U], index: u64) -> U;
```

Likewise, `combine<T, U>(left: T, right: U)` and
`combine<A, B>(left: B, right: A)` are the same broad generic call signature:
two independently chosen type parameters.

Distinct parameter types may coexist as an overload set, but resolution must
eventually choose one unique candidate from operand types and proof context.
This is the operator-specific rule: fixed spellings remain operand-directed.
Explicit named machine and requirement calls may additionally use the
result-domain lookup specified in chapter 3; ordinary return carriers and
predicate refinements never distinguish those overloads.

## Numeric Semantics

Machine numbers and proof numbers are different kinds of values.

Working categories:

- `UInt` is a proof-level natural number.
- `Int` is a proof-level integer.
- `Real` is the ordinary core package's proof-level real-number carrier.
- `i32`, `u64`, `f32`, and similar types are concrete machine representations.

Proof-level numbers are useful for specifications, constraints, and generic
numeric reasoning. Lowering to native code must erase proof-only numbers or
replace them with proven machine representations.

Machine integer arithmetic is **exact by default**: every operation must be
PROVEN free of overflow, underflow, division by zero, and invalid shifts. If
the compiler cannot prove an operation safe, it is a **compile error** — there
is no unexpected arithmetic and no silent wraparound. (Decided 2026-06-14; this
is the Ada/SPARK model — range types plus a prover — not a build-mode flag.)

For casts and overflow-prone Exact operations, the checked question is always
the direct one: does the operation's unbounded mathematical result lie within
the selected machine carrier? The proof system can describe that mathematical
result without first constructing the possibly overflowing machine value.
Compiler automation may use ranges, affine forms, aliases, or a solver to find
a derivation, but the artifact carries that derivation and the verifier checks
it against the same canonical carrier bounds. The verifier does not act as an
arithmetic oracle or choose whichever sufficient fact it happens to discover.

Fixed-width integer `/` truncates its quotient toward zero. Integer `%` is the
corresponding remainder, so a nonzero signed result has the dividend's sign;
it is not Euclidean modulo. Both operations require a nonzero divisor, and the
signed `MIN / -1` and `MIN % -1` cases are outside Exact because their common
quotient is not representable in the carrier.
Proof-level `Int` uses the same quotient/remainder convention without a
fixed-width bound. Anonymous `%` instead requires an integer-typed operand;
see [typed integer quotient and remainder](#typed-integer-quotient-and-remainder).

To perform arithmetic that *can* overflow, the value lives in an explicit
primitive **domain** that defines the behavior:

- `Wrapping`: wraps modulo the fixed-width representation.
- `Saturating`: clamps to the representable minimum or maximum.
- `Trapping`: checks at runtime and traps on overflow — the escape hatch when
  safety cannot be proven and neither wrap nor saturate is wanted.

For signed division's sole overflow, `Wrapping` defines `MIN / -1 == MIN`,
while `Saturating` defines `MIN / -1 == MAX`; the corresponding remainder is
zero in both policies. Division or remainder by zero is not overflow and is
never licensed by either policy.

The AArch64 runtime backend realizes the 64-bit Saturating case explicitly:
architectural `SDIV` wraps the quotient at `MIN / -1`, so the emitted guarded
`-1` path detects that one wrapped negation and selects `MAX`; remainder selects
zero. Every other nonzero divisor continues through `SDIV`/`MSUB`, and unsigned
division remains `UDIV`. This target fixup consumes the already-checked policy;
it does not weaken the independent nonzero-divisor formation rule.

These three domains occupy one closed **arithmetic-policy semantic role**.
Exactly one policy may govern an operation. The role composes with independent
domain roles: `Km & Wrapping` combines dimensional meaning with modular
overflow, while `Wrapping & Trapping` rejects as two policies for one role.
Attaching a policy changes no payload and performs no work; the later
arithmetic operation supplies the wrap, clamp, or trap behavior.

A non-Exact policy may be erased to an unqualified integer binding only by an
explicit same-carrier `as`. The cast performs no runtime work and does not
change the payload; it makes the loss of arithmetic-policy meaning visible.
Subsequent arithmetic is Exact and must prove its safety from the callee's
contracts and the current value facts. For example, a wrapping value whose
payload is `-1` becomes the exact integer `-1`, not the mathematical pre-wrap
result. Selecting a non-Exact policy in the other direction is likewise
explicit because it changes the behavior of future operations. Predicate-only
facts may weaken implicitly, but arithmetic policy and other semantic meaning
may not.

A machine that names no arithmetic policy therefore publishes Exact behavior.
Its implementation must prove its primitive arithmetic safe, select a
different policy explicitly, return a checked failure, or quantify over an
arithmetic policy. A caller-selectable policy must be visible in the machine's
contract; the generic spelling for that choice remains open. Omission never
silently chooses wrapping, saturation, or trapping.

Fixed-width integer bitwise operators are representation operations, not
overflowing arithmetic. Binary `&`, `|`, and `^` and unary `~` are total,
retain the operand's exact integer carrier, and do not select an arithmetic
policy. `~` complements exactly the carrier width, including two's-complement
signed carriers. Boolean negation remains the distinct `!` operator; neither
operator coerces between Boolean and integer values.

Shift counts follow the same rule: under Exact, a
shift's count must be **proven** nonnegative and below the operand width (a
literal out-of-range shift is an immediate compile error); under `Wrapping` the
count is reduced by Euclidean modulo of the shifted value's width. For the
current 8/16/32/64-bit source carriers this is exactly `k & (width - 1)`,
which is also what the native targets compute; under `Trapping` an
out-of-range count traps. `Saturating` adds no count meaning: it governs
value overflow, not operand validity, so its count obligation is Exact's.
The compiler never adopts the ISA's silent count-masking under Exact —
`x << 64 == x` is an invented number.

### Float-to-integer conversion

A float-to-integer conversion is also proof-or-policy:

- The unqualified named conversion truncates toward zero and must prove that
  the operand is finite and its truncated result lies inside the target
  integer's half-open interval. A declared float range or dominating guard may
  supply that proof; `x == x` is the explicit witness that excludes NaN.
- The `Saturating` result overload truncates toward zero and clamps at the
  target width on every integer target. NaN converts to zero.
- The `Trapping` result overload truncates a finite value and traps on NaN,
  infinity, or either out-of-range direction.
- `Wrapping` has no overload: floats have no modular conversion reading.

An `as` conversion is narrower still: it requires proof that the float denotes
an integral value representable by the target, so the cast preserves
denotation. Fractional toward-zero conversion uses the named family below.

These rules are identical in the interpreter and the x86-64/AArch64 bindings;
ISA-specific invalid-conversion sentinels are never language-visible.

### Public numeric-conversion requirements

Non-exact numeric conversion uses destination-owned named requirements. The
ordinary family begins with:

```omega
F32::from_f64(value)   // binary64 to binary32, nearest-even
F32::from_i64(value)   // exact integer meaning to binary32, nearest-even
I32::from_f64(value)   // binary64 to i32, toward-zero
```

Equivalent destination families exist for the other primitive carriers. These
names identify the conversion direction; result-domain overloads select
same-shape arithmetic policy without multiplying names:

```omega
let exact: i32 = I32::from_f64(value);
let checked_at_runtime: i32 in Trapping = I32::from_f64(value);
let clamped: i32 in Saturating = I32::from_f64(value);
```

With no expected result type, the unqualified overload is selected. Its
finite/range precondition must then be proved, and a failed proof diagnostic
lists the available qualified overloads. `Wrapping` has no float-conversion
meaning and supplies no candidate. A caller that wants saturation only for the
conversion may explicitly erase the resulting policy before later Exact
arithmetic.

Float-to-float conversion to a bare IEEE carrier is total; infinity is an
ordinary result. A destination such as `f32 in Finite` adds a predicate proof
obligation after conversion selection rather than creating another overload.
For float-to-integer conversion, fractional inputs are lossy but not failing:
toward-zero determines the integer. Non-finite inputs and a truncated result
outside the destination range are the proof, trap, or saturation edges.

Directed one-step conversions, when published, use separate requirement names
such as `F32::from_f64_toward_positive`; there is no ambient or runtime rounding
mode. Libraries may compose ordinary rounding and conversion machines when
their contracts prove equivalence. They must use the corresponding one-step
`FloatSemantics` operation where composition would introduce double rounding.

A failure-returning conversion has a different result shape and therefore a
different requirement identity. Its public name and carrier remain deferred to
the checked-result arithmetic decision rather than being guessed here.
Same-format qualification is not a conversion requirement: `as` adds or
explicitly removes an erased domain without invoking a machine.

Two rules keep it honest:

- **No implicit widening.** `u8 + u8` is a `u8` and must be proven to fit a
  `u8`; computing in a wider type uses the explicit widening conversion.
- **No mixed-domain arithmetic.** An exact value and a `Wrapping` value cannot
  be combined directly; select the intended policy with explicit
  qualification or first weaken the Wrapping operand to Exact. Explicit always
  wins.

> **Exact coercion and conversion policy.** `as` is the ordinary explicit
> coercion when the compiler proves one unique transformation preserves the
> value's denotation; an explicitly bare target may instead erase non-owning
> semantic meaning. Integer widening and representable narrowing therefore
> use `as`; the proof may come from the complete source range, a dominating
> guard, or a retained contract fact. The same rule covers direct domain
> qualification.
>
> A transformation that wraps, saturates, traps, rounds, can fail, allocates,
> or otherwise selects policy is an ordinary named machine or an explicitly
> selected policy domain. `core::numeric_conversion` retains those named
> surfaces and may also offer named exact helpers, but callers do not need a
> value-machine call merely to express a proved exact cast. `as` never invokes
> arbitrary user code.

Unit-scale conversion is domain-library behavior, not an intrinsic cast.
Libraries expose it through ordinary named machines or heterogeneous operator
conformances with their own `requires` and `ensures`.

Weaker behavior is therefore always visible at the value, and overflow is a
proof obligation like any other in the language.

### Where the wrap applies: at each node, at the declared width

In a compound expression, a domain-bearing operation produces its
declared-width result **before** the enclosing operation consumes it. With
`a: u32::Wrapping` holding `0 - 2` (that is, `0xFFFF_FFFE`):

```omega
let b: u32 = a >> 1;    // 0x7FFF_FFFF -- shifts the WRAPPED 32-bit value
let d: u32 = a / 3;     // 1431655764  -- divides the wrapped value
```

The shift and division see the 32-bit wrapped value, never a wider
intermediate (a 64-bit register image of `0 - 2` would shift to garbage).
This holds identically in the interpreter and native emission on both ISAs,
in every operand position, and is pinned by
`arithmetic/runtime_wrapping_operand_truncation_exit`. Exact values need no
such rule (they are proven in-range), and `Saturating`/`Trapping` clamp or
trap at the node before the parent consumes the result.

Two node-level corollaries for constants: a constant stored into a
`Saturating` target clamps to the target's range, and a constant that
provably overflows a `Trapping` target still compiles and traps at runtime
-- Trapping overflow is a runtime event, not a compile error.

## Constants: Two Phases

A constant is either anonymous or landed—never both and never neither.

- **Anonymous (pre-landing):** a literal is an exact mathematical value
  with no type. `100` is one hundred; `3.14` is 157/50. Arithmetic between
  anonymous constants is exact: integers remain unbounded, and decimals and
  non-integral quotients have exact rational values. Anonymous `/` is rational
  division, not truncating integer division.
  No width, no signedness, no domain, no format — deliberately: the value
  is chosen, the machine rendering is not.
- **Landing:** the first site that requests a type renders the value ONCE
  — checked for integrality and range into an integer type, rounded once into
  a float format.
  The same literal lands as `u8`, `u64`, `f32`, or any future format with
  no suffix; a suffix (`0u32`) merely lands the literal where it stands. A
  contract comparison inherits this destination from a typed named operand or
  from the owning callable's declared `result` type; review never guesses one.
- **Landed:** from that point the constant IS a value of its type, and the
  type/domain/format ride with it. All further compile-time folding
  happens at the landed type's semantics — width, signedness, domain
  (a constant that provably overflows a `Trapping` target still compiles
  and traps at runtime, per the node rule above), format rounding.

Nothing is ever both (an anonymous value with a width) or neither (a landed
value stripped of its type). Constant folding must preserve the landed type,
domain, and format; it cannot regress a landed value to an untyped integer.

### Exact anonymous division and landing

The numeric value of an anonymous expression is chosen before its destination
representation. For nonzero `b`, anonymous `a / b` retains the exact rational
quotient, even when both operands are integer literals. Thus `7 / 2 / 2`
evaluates as `(7 / 2) / 2` to `7/4`, and `7 / 2 * 2` evaluates to `7`.
Grouping does not itself cause landing, truncation, or rounding.

An exact rational can be represented by an unbounded integer numerator and a
positive denominator, reduced to lowest terms; zero is `0/1`. The expression
may nest, but its value does not require nested rational carriers. This is
compile-time value representation, not an implicit runtime `Rat` conversion or
a requirement to emit a rational evaluator. Division by zero has no anonymous
numeric value and cannot successfully land. It does not acquire a value from
the destination's arithmetic policy. Already-landed division retains its
existing policy and fault rules.

An integer destination accepts only an integral exact value within its range.
It does not silently truncate a fraction. A floating destination rounds the
exact result once according to its format. Neither destination retroactively
types the operators inside an anonymous expression.

```omega
let exact_integer: i32 = 7 / 2 * 2;    // 7; fractional-intermediate warning
let fractional: i32 = 7 / 2;          // error: 7/2 is not an integer
let typed_integer: i32 = 7i32 / 2 * 2; // 6; integer division, no warning
let exact_float: f64 = 7 / 2 / 2;     // 1.75; no integer-landing warning
```

An already-typed operand establishes the operation's typed semantics. The
anonymous operand must land to the required type before that operation, not
after it. Assuming `i: i32`:

```omega
let mixed: i32 = i * 4097 / 2;          // (i * 4097) / 2, typed integer operations
let fraction_operand: i32 = i * (4097 / 2); // error: 4097/2 cannot land as i32
let truncated_operand: i32 = i * (4097i32 / 2); // i * 2048
```

The first multiplication still obeys its applicable overflow rules. A compiler
must not reassociate it into the third expression or propagate the final
`i32` destination inward to make the second expression accept. Runtime values
already have types; mixing them with constants does not introduce runtime
rational arithmetic. The same landing boundaries apply in constant arguments
and proof evaluation. An exact rational identity does not prove the analogous
identity for truncating integer operations. Ordinary machine parameter types
remain landing boundaries even when a call is evaluated during compilation.

### Fractional-intermediate diagnostic

Integer landing of a fractional final value is a compile error. The diagnostic
reports the exact value and destination type, and explains that an explicitly
integer-typed operand requests integer division before the fraction arises.

When an anonymous expression instead has a fractional intermediate but an
integral final value that successfully lands in an integer, emit a default-on,
suppressible warning. Alignment calculations illustrate why this matters:

```omega
let base: u32 = (4097 / 4096) * 4096;        // 4097; warning, not align-down
let aligned: u32 = (4097u32 / 4096) * 4096; // 4096; explicit integer division
```

The first expression can look like integer align-down while calculating an
exact cancellation. Point to `4097 / 4096`, explain that its fractional value
is preserved and the complete expression lands as `4097`, and suggest typing
an operand if integer division was intended. The smaller `7 / 2 * 2` example
likewise lands as `7` with a warning. Both are valid expressions, not type
errors or miscompilations.

The trigger is the exact intermediate value in the authored anonymous
calculation, not a comparison against a hypothetical C evaluation. Retain that
diagnostic origin through simplification so cancellation cannot erase the
warning. Already-typed integer division and floating-point landing do not
trigger this warning. Suppressing it changes no arithmetic or landing rule.

This is a correctness warning about likely author intent, not a style lint.
There is no fractional-landing error when the final value is integral, so
blanket suppression removes this diagnostic assistance for cases such as
align-down. It does not weaken compiler or proof soundness: the specified
exact value is unchanged. Ordinary suppression remains allowed; there is no
expression-only suppression restriction.

> **Implementation status:** these are the required language and diagnostic
> rules, not a claim that every compiler path implements them. The shared
> anonymous integer landing evaluator retains exact rational intermediates.
> Fixed-integer return, local, assignment, cast, supported mixed-operand, and
> resolved machine/named-state argument paths use this value and diagnose
> fractional final values. Parameter proof consumes the same exact value;
> the destination's arithmetic policy cannot truncate an anonymous fraction or
> wrap an out-of-range initial value. Successful integral
> landings report the fractional-intermediate warning through the current
> validation diagnostic route. Closed builtin integer const arguments for data
> and domains retain exact anonymous intermediates before normalization chooses
> the canonical integer argument. Normalization reports their fractional
> origins through the current stderr warning channel; an integral final value
> must still fit its declared const parameter. Closed builtin generic facts and
> const-domain predicates compare anonymous rational values without integer
> landing: `7 / 2 > 3` holds, while `7 / 2 == 3` is false. Typed or named integer
> peers and integer domain membership require integral anonymous operands;
> successful integer landing retains fractional-intermediate warnings. Facts
> with potentially authored operators remain for typed declaration selection,
> not builtin evaluation by spelling. At supported declared `f32`/`f64`
> destinations, wholly anonymous integer-literal arithmetic uses the same exact
> rational value and rounds once, without an integer-landing warning. Retained
> but unused operands do not impose runtime integer-width limits after this
> folding; another executable use still has its own width obligation.
> Decimal-float folding and comparisons also require builtin operator meaning;
> authored operations retain their nodes and independently chosen operand
> destinations for checked selection, rather than inheriting the result format.
> Caller result proofs can also
> transport a callee's builtin `result == immutable_parameter` guarantee from retained closed
> fixed-integer operands, without replaying their source expressions. General runtime argument
> snapshots, generic/evidence-adapted and boundary destination custody, aggregate
> elements, remaining mutable parameter carriers and Unit-body storage, numeric policies,
> mixed integer/decimal anonymous trees and remaining float destinations,
> remaining authored-operator/const-proof consumers, and ordinary warning
> suppression/report transport remain on [the execution board](../../TASKS.md).

### Typed integer quotient and remainder

Builtin `%` requires at least one operand that already has an integer type.
An anonymous operand then lands to the type required by that operation.
Anonymous `%` has no value: a destination annotation, proof context, positive
operands, or a provably zero remainder does not supply the missing operand
type. Anonymous `/` remains exact rational division.

```omega
let anonymous: i32 = -3 % 2;    // error: % needs an integer-typed operand
let remainder: i32 = -3i32 % 2; // -1; typed dividend-sign remainder
let positive: u32 = 7 % 2;      // the same missing-operand-type error
let explicit: u32 = 7u32 % 2;   // 1
```

The rejection explains that an integer-typed operand is required and suggests
typing an operand, not merely the destination. It offers only supported
source spellings; it does not suggest a hypothetical Euclidean helper. A
fractional anonymous operand cannot land for typed `%`, just as it cannot
land for other integer operations. Already-typed incompatible operands still
follow the ordinary operator-resolution rules; this introduces no implicit
conversion between integer types.

This contract includes the builtin proof-level `Int`, not just machine-width
integers. `Int` division returns the unbounded integer quotient truncated
toward zero. Its remainder is zero or has the dividend's sign. For an operand
`a: Int`, `a % 2` and `a / 2` select those integer operations without an `i32`
suffix or runtime representation. Being inside a proof alone does not type
the literal-only expression `-3 % 2`.

For nonzero divisor `b`, the paired mathematical quotient `q` and remainder
`r` satisfy:

```text
a = q*b + r
|r| < |b|
r = 0 or sign(r) = sign(a)
```

For `a: Int` with value `-7`, `a / 2` is `-3` and `a % 2` is `-1`;
a negative divisor changes the quotient's sign, not the remainder's
dividend-sign rule. Both operations
require a nonzero divisor. There is no width-overflow case for `Int`.
Fixed-width integers retain their existing nonzero-divisor requirements,
Exact admission, and other arithmetic-policy behavior. The mathematical law
does not itself prove intermediate machine operations in its authored
expression safe, nor does it override wrapping or saturating overflow cases.

An exact anonymous rational quotient is not the `q` in that integer law.
Proofs and algebra contracts must retain the operand kinds and the associated
operator semantics. The source library's constructed `IntPair` is distinct
from builtin `Int`; this decision does not add operators to that nominal type
or bypass proof-only runtime-consumption restrictions.

A separately named Euclidean modulo operation is deliberately deferred, with
no spelling selected here. It must not appear as a second, context-selected
meaning of builtin `%`. There is no anonymous Euclidean-remainder warning:
the anonymous operation rejects rather than evaluating under a second sign
convention. The fractional-intermediate warning remains unchanged.

> **Implementation status:** this settles the meaning, not the completeness of
> builtin `Int` division/remainder support. Known-builtin literal-only `%` is
> checked before value/proof admission and closed-const normalization; const
> argument parsing preserves the operation for that check. Authored const
> operator selection, `Int` evaluation and proof support, and the remaining
> numeric landing boundaries remain execution-board work.

### Landed target-semantic dependencies

A landed constant may also retain target-semantic dependencies. Before target
closure those observation applications stay symbolic. After closure the value
may fold, including inside an array length or const-generic application, but the
normalized signature, cache key, proof certificate, and artifact keep the exact
observation and selected-realization dependencies. Erasing that dependency
because the folded node now looks like an ordinary integer is a soundness bug.

## Float Facts

> A float is a format-parameterized approximation carrier: every operation is
> an executable special-value theory plus "exact signed-rational arithmetic,
> then round once" for its finite branch. `f32`/`f64` permanently name the
> binary32/64 core formats; a target provider realizes those meanings but never
> chooses them. Every finite nonzero float is exactly a dyadic signed rational.
> Signed zero, infinity, and NaN use the separate `FloatMeaning` cases below.
> A future format (posits, bf16) arrives as a new name plus a format record plus
> provider bindings — zero grammar and no rebinding of existing names.

Float constraints describe correctness facts, not optimization permissions.

### Operators are requirements

Primitive float operations use the ordinary operand-driven operator resolver.
The complete static carrier/domain tuple selects a named boundary-operator
requirement such as `Float::add`; the target package supplies a selected
satisfier and normalized `ProviderPlan`.

```text
a + b
  -> Float::add selected from the static operand tuple
  -> x86 ADDSS, AArch64 FADD, or a checked software realization
```

Selection is static. There is no float vtable, operation-kind argument, ambient
format, or special compiler-only operator family. Arithmetic, comparison,
conversion, classification, fused operations, and directed-rounding variants
remain separate named requirements.

The contract of each requirement is an equality against an executable core
semantic function:

```omega
data FloatMeaning {
    case FiniteNonZero(value: Rat::NonZero);
    case Zero(sign: Sign);
    case Infinity(sign: Sign);
    case NaN;
}

ensures meaning(result)
    == FloatSemantics::add(Binary32, meaning(left), meaning(right));
```

`Rat::NonZero` prevents rational zero from overlapping the two signed-zero
cases; core's checked `nonzero_rat` constructor establishes it from the signed
Nat coordinates and a positive denominator. Subnormals inhabit
`FiniteNonZero`; signed zero is separate because operations such as reciprocal
observe the sign. NaN payloads do not enter the base proof meaning, although
the concrete runtime value retains its honest bits. `FloatMeaning` is
proof-only and therefore has no runtime tagged-union ABI.

`FloatMeaning` equality is structural equality of this sum. Payload erasure is
performed by `meaning32`/`meaning64`, not by equality: all runtime NaN payloads
reach the one reflexive `NaN` proof case, while `Zero(Sign::Positive)` and
`Zero(Sign::Negative)` remain unequal. Atomic IEEE comparison remains a
separate runtime relation. The PCC term retains a verifier-reconstructed source
carrier and exact recognized projection contract; none of that metadata emits
a runtime value or check.

The current core declarations live in
`omega::language::core::float_operations`. They publish pure
`FloatSemantics` identities and contracted f32/f64 boundary requirements for
the arithmetic/comparison spellings, multiply-then-add versus FMA,
classification, and directed rounding. Checked operator evidence records the
primitive identity selected at each use. All f32/f64 primitive arithmetic and
comparison overloads now select explicit x86-64/AArch64 target satisfiers and
retain their exact `ProviderPlan` identities through lowering. The named F32/F64
`minimum`, `maximum`, `square_root`, `negate`, `is_nan`, `is_finite`,
`is_infinite`, `is_normal`, `is_subnormal`, `classify`, and
`multiply_then_add` requirements likewise select explicit target satisfiers.
Their checked plan identity
authorizes compiler-known execution lowering without replacing the source
requirement in proof evidence. Negate preserves the expression root while
lowering to multiplication by a format-landed negative one. The NaN predicate
uses an internal unary operation, evaluates its argument once, and retains the
argument's binary32/binary64 width. The remaining boolean classification
predicates and enum-valued `classify` inspect the raw IEEE bits without reading
ambient floating-point control state. `classify` returns the source-order
`FloatClass` tag plus the sign payload carried by infinity, normal, subnormal,
and zero cases. Multiply-then-add rewrites to an unnameable, format-specific
ternary compiler operation that survives state-local expression copying. Both
engines execute a separate multiply followed by add, preserve all three
authored operands for final result-policy adaptation, and never contract it
into the separately named fused operation. Nearest-even F32/F64 FMA selects
explicit AArch64 satisfiers and lowers to one scalar `FMADD`; its interpreter
path consumes the same `FloatSemantics::fused_multiply_add` identity. Generic
x86-64 remains SSE2-baseline and therefore does not claim FMA3: that target
requires a feature-qualified or checked software satisfier. The first opt-in
x86 hardware carrier now admits only an exact deployment profile plus AVX+FMA3,
binds both generic FMA slots to the scalar VEX instructions, and retains
raw-bit fused-versus-unfused cancellation receipts through final image replay.
An exact-target build can now select the closed `AvxFma3` deployment value and
retain that admitted carrier on the checked compilation; targetless, non-x86,
and cross-profile substitution reject. This selection is not native execution
evidence and does not change generic target selection. For the bounded
attached-Unit lane, source `ProviderPlan` selection and ordinary native
lowering now consume the carrier through exact Terminal FMA occurrences,
assignment-owned XMM homes, canonical MXCSR custody, and final artifact replay.
Independent literal FMA locals may be followed by ordinary receiver-attached zero-result internal
Unit calls; each retained call interval remains inside the function-level
MXCSR envelope. The bounded Windows x86 lane also admits a zero-argument,
zero-result source-evaluated PE leaf after those locals; its complete nested
MXCSR custody must remain inside the same outer envelope. Scalar foreign
arguments/results, wider mixtures, and other operations remain pending. AArch64 also
selects exact F32/F64 FMA-toward-zero/positive/negative satisfiers. Each
directed ternary operation changes FPCR only around one scalar `FMADD`, while
the interpreter consumes the corresponding directed `FloatSemantics` identity;
half-ULP edges distinguish all three results and prove control restoration.
Generic Linux x86-64 retains a separate baseline semantic-edge suite for 36
exact nearest-arithmetic, comparison, classification, min/max, square-root,
negate, and multiply-then-add plans. Every supported host builds its explicit
Linux-x64 root twice and requires identical ELF bytes; only a Linux x86-64 host
adds native execution evidence. FMA and directed operations are absent from
that baseline suite, so its evidence cannot authorize either family.
The cross-target directed cohorts supply F32/F64
add/subtract/multiply/divide/square-root-toward-zero/positive/negative
satisfiers on all four native targets. Each operation saves the complete
floating control state, installs its requested direction for one scalar
operation, and restores the prior state before returning the result; directed
rounding never becomes an
ambient mode. Other named operation families remain on bootstrap target
lowering until their own satisfiers and execution paths replace it.

Named boundary operators now share a checked-software provider route. An
ordinary machine body satisfying the exact operator without `via` is admitted
only when its proved equality/`&&` guarantees cover the requirement and its
requires contract is no stronger. Requirement parameters substitute onto
provider parameters positionally; arbitrary role-swapping is not refinement.
Selection retains a `CheckedAdapter`
`ProviderPlan` identity on the named use; execution then calls that Omega body
in both engines without replacing the boundary operator's proof identity. This
route does not itself manufacture a software algorithm: generic x86-64 FMA
remains unavailable until such a checked implementation (or an honestly
feature-qualified hardware satisfier selected by the build) is supplied.

### Value domains — wellness facts

```omega
data Particle {
    x: f64;                            // bare: may hold NaN/±inf quietly
    speed: f64::Finite;                // NaN and ±inf forbidden
    alpha: f32 [0.0..=1.0];            // range fact — implies Finite for free
    mass: f64::Finite & Positive;      // domains conjoin with the landed `&`
}
```

- `Finite` (core domain; ch5's original `finite` constraint, promoted):
  the value is not `NaN`, `+inf`, or `-inf`. Enforcement is the
  invariant-window machinery (chapter 11): writes are free, the window
  closes at consumption points.
- A float range (`0.0f..=100000.0f`) is a window-checked value fact — and
  every range implies `Finite`: NaN fails every comparison, so no range
  admits it.
- A domain chain `::A & B & C` carries any number of value domains and
  **at most one** policy domain (two policies is the existing
  mixed-domain rejection).
- Float constraints are not runtime metadata.

### Policy domains — operation behavior

Operand-driven and exclusive per operation; float failure modes are non-finite
production, not overflow into wraparound:

- **default**: the format's quiet semantics — correctly rounded, inf/NaN
  propagate silently; `Finite` windows catch them wherever wellness was
  claimed.
- **`in Trapping`**: producing a non-finite value traps.
- **`in Saturating`**: overflow clamps to the format's largest finite
  magnitude — **overflow only**: division by zero and
  invalid operations (`0.0/0.0`, `inf - inf`, `sqrt` of a negative) still
  produce non-finite values; those routes remain `Finite` obligations.
  `Finite & Saturating` is therefore the ergonomic pairing: magnitude
  proofs vanish into the clamp, wellness stays proven via the cheap
  discrete obligations (divisor nonzero, operand signs).
- **`in Wrapping`**: compile error — there is no modular reading of a
  float (the float-to-int cast ruling's precedent, generalized).

For example, finite operands alone do not prove finite division:

```omega
machine divide(a: f32::Finite, b: f32::Finite)
    -> f32::Finite
{
    return a / b;
}
```

The selected `/` requirement requires `b` to exclude both signed zeros and the
rounded quotient to remain within the finite range. Underflow to signed zero is
finite and needs no exclusion. `Saturating` discharges the magnitude-overflow
branch but not the nonzero-divisor obligation. A result-checked `Trapping`
qualification may instead trap before a non-finite result returns.

### Float comparisons and NaN

Float comparisons follow the format's partial order on both engines (IEEE
under the binary32/64 bindings):

- The ordered comparisons `<`, `<=`, `>`, `>=` are **false** whenever either
  operand is NaN (natively: aarch64 condition codes chosen to fail on
  unordered; x86_64 `ucomis*` with parity-aware sequencing).
- `==` is false and `!=` is **true** when either operand is NaN — so
  `f != f` is the IEEE binding's isNaN idiom. The constant folder
  deliberately refuses to fold float self-comparisons (`x == x`, `x != x`)
  for exactly this reason. `is_finite(x)` is the portable wellness
  spelling; the idiom is what implements it under IEEE bindings.

`min` and `max` follow the **hardware contract**:
return the second operand on unordered-or-equal — exactly `a < b ? a : b`,
matching `minsd`/`maxsd` and the aarch64 FCSEL lowering. This is
order-dependent under NaN (`min(NaN, 5)` is `5`; `min(5, NaN)` is `NaN`)
and deliberately differs from both Rust (non-NaN wins — which silently
launders a poisoned value) and IEEE-2019 `minimum` (NaN wins — which costs
a compare-and-blend on x86). Under `Finite` operands all three contracts
agree, so proven code cannot observe the choice; unproven code gets the
fastest true-to-silicon lowering.

Comparison results in value position (`let ok: bool = a > b`) use the same
lowering as guards and are pinned for ordinary values — including negative
operands, where a bit-pattern comparison would invert the order — by
`arithmetic/runtime_float_compare_bool_exit`. NaN-operand differential legs
are now pinnable (runtime `0.0 / 0.0` under the quiet default constructs
NaN portably).

NaN payload bits are absent from the base arithmetic promise and never
observable through `FloatMeaning`. A representation-sensitive consumer must
prove non-NaN, canonicalize NaN, or require an exact raw-NaN refinement from
the selected realization. A runtime recast (`&self.f as &u32`) still reads
whatever bits are honestly there and makes no reproducibility promise.

A compile-time NaN with an unfixed payload remains usable through
`Float::meaning`, but a `const` use that would materialize it into runtime/image
bytes rejects. The author must prove non-NaN, canonicalize the NaN, construct
explicit bits, or select an exact raw-NaN realization. This check occurs at
materialization rather than at the const declaration, so proof-only constants
do not acquire a representation obligation they never use.

> **Implementation checkpoint (August 2026):** the bounded closed-copy-
> aggregate materialization path accepts exact non-NaN binary32/binary64 values,
> including signed zero and infinity, with target byte order retained. Binary32
> values must already round-trip exactly through binary32. Canonical or selected
> exact raw-NaN materialization remains pending.

### Two orders

Arithmetic comparison is the partial order above — floats never pretend to
be totally ordered in arithmetic position. Sorting and keying use a total
order spelled as a named conformance (chapter 14):

```omega
sort_by<F64::TotalOrder>(&mut samples);   // IEEE totalOrder — a sign-magnitude
                                          // integer compare, explicit at the site
```

The core spelling is provided by
`omega::language::core::float_order`: `F32::TotalOrder` and
`F64::TotalOrder` are ordinary complete `Order` conformances selected through
a static machine parameter. Their `before` members are library machines over
honest recast bits, not privileged comparison operators; arithmetic `<` keeps
its IEEE partial order independently.

### No ambient relaxation

- `a * b + c` is two roundings, on every target, always — the compiler
  never contracts multiplies into fused ops on its own.
- `fma(a, b, c)` is the single-rounding spelling.
- There is no fast-math mode, flag, or build option, and none is planned.
  Optimization permissions, where they ever exist, are per-operation
  spellings — never ambient.

### Literals and compile-time arithmetic

A float literal parses to its exact rational value (`9.80665` IS
`196133/20000`); anonymous compile-time decimal arithmetic is exact `Rat`
arithmetic, and the result rounds **once** at the landing site to the landing
type's format. The same literal lands correctly as `f32`, `f64`, or any future
format with no suffix — a constant is unitless until a site requests a type.
After landing, compile-time operations invoke the same executable
`FloatSemantics` functions as the interpreter and target contracts.

Semantic results therefore agree across build time and runtime. Exact raw NaN
bits are a stronger promise: build-time recast of a computed possibly-NaN value
requires proof of non-NaN, canonicalization, or an exact target refinement.

### Canonical floating control state

Checked Omega code runs under one canonical semantic floating-control
configuration: nearest-even, gradual underflow, and the target's corresponding
masked-exception policy. The invariant covers only semantic control bits, not
sticky status flags.

Directed rounding is a distinct operation (`add_toward_zero`, for example),
not a temporary control-mode change. `Trapping` checks the semantic result
rather than unmasking hardware floating exceptions. Consequently the scheduler
does not switch rounding modes between Omega activations.

Foreign code is the restoration seam. A foreign binding either proves it
preserves the relevant MXCSR/FPCR controls or its trampoline saves and restores
them. A callback entering Omega establishes the canonical controls before
checked code runs and restores the foreign controls on return.

## Temporaries

Temporaries have lifetimes like locals generated by the compiler.

```omega
self.view.render_line(RoomFormatter::title(&room));
```

The checker must ensure temporary storage outlives every borrow derived from it
and is cleaned up on every exit edge.

Shared lending may be implicit when an expression is supplied to a shared-
reference parameter. Mutable and write-only lending are explicit expression
forms (`&mut value` and `&write value`) and retain that access distinction in
checked semantic identity. Compiler consumers which persist public contracts
must rejoin an explicit reference argument with the declared parameter type;
they may not reconstruct access from source text or erase it as diagnostic
spelling.

When lifetime or proof facts become hard to read, prefer explicit locals.

## Blocks

A block groups straight-line work.

```omega
{
    let room: Room;
    self.lookup.find_room(self.level, self.current_cell, &mut room);
    self.view.render_room(&room);
}
```

The block can introduce locals and cleanup edges. It does not create a state or
machine by itself.

## Terminal Expressions

In a typed machine, a terminal expression completes the current machine
invocation.

```omega
state shutdown(&mut self) {
    0
}
```

A transition target uses call-shaped syntax. A bare value is a terminal value.
