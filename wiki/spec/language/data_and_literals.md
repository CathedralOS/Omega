# Data, literals, and lexical framing

`data` declares fields, cases, or both. Fields belong to the value. A record has
only fields; a sum has only cases; a mixed declaration has common fields plus
one active case and its payload. Machines access values through their explicit
parameters, not implicitly owned machine fields.

## Construction and case identity

Fields have no declaration-site default initializers. Construction may omit an
ordinary runtime field only when its zero value satisfies the complete default-domain obligations.
Nonzero/computed defaults are ordinary constructor machines. See
[establishment](dependent_values.md#default-domains-and-zero-initialization).
Erased fields instead follow [erased construction](../proofs/contracts.md#explicit-erased-bindings):
an accessible nullary constructor may supply an omitted term without requiring
runtime layout or a zero value.

Field and case-payload relationships belong to the data declaration's ordinary
`where`; case-dependent projections require established case knowledge. Field
and payload type spellings do not carry bracketed scalar range suffixes. Construction and every
restoring write prove the owning default-domain clauses.

Case-bearing values use a case literal, not record-only construction. Common
and payload fields may be named together and evaluate exactly once in authored
order. Common fields are accessible independently of the active case; payload
fields require the corresponding case knowledge. Partial construction and
disposal follow [ownership order](ownership.md#construction-and-disposal-order).

A case implicitly declares its same-named domain. `Type::Case` therefore denotes
case membership in domain position. A payload-free case can also denote a
constructed value; a payload-bearing case needs its payload in value position.
Cases and attached domains retain their member namespace; data does not own
machine declarations. Collisions reject rather than selecting by priority.
Pattern rules are defined
in [dispatch](patterns.md).

Case values always use their carrier-qualified member path. Expected type,
imports, and local context do not infer a case owner for a single-segment name:
write `Light::On`, not `On`. A payload-bearing value uses its qualified
construction form, such as `Command::Move { dx: 1 }`; `Command::Move` without
its payload remains the case domain, not a partially constructed value. This
rule is `bare-case-value-names` and applies uniformly in locals, arguments,
returns, comparisons, and nested expressions.

Cases are nominal, not foreign integers. `#N` is schema identity, not a runtime
tag assignment. Foreign codes cross in their declared integer carrier and use
ordinary checked mapping machines, including an explicit unknown-code outcome.
No raw integer silently establishes a sum value.

The ordinary case-bearing zero representation selects the first declared case
and recursively zeroed common fields/payload. That is accessible only after the
active value's default-domain obligations hold; inactive payloads need no
establishment. Zero does not imply semantic emptiness or authority. A payload-free
first case is not required. Layout optimization cannot use invalid payload niches
to remove the tag or alter zero's meaning. Stable external placement uses
[layout policies](../layouts/plans.md), not inferred native offsets.

Value equality and domain membership are distinct. For executable structural
equality, compare common fields, case identity, and the active payload, never
inactive storage/padding. Primitive and payload-free-sum equality is intrinsic;
records and payload-bearing sums select the declared synthesis/conformance
required by [Equatable](conformances.md#core-equality-acquisition). Adding a
payload requires that declaration rather than silently changing tag equality
into field equality. Separately, `==` between values of one identical
non-primitive data type in a fact denotes
[proof-relevant identity](../proofs/contracts.md#data-identity-in-facts) and
does not structurally compare fields or select `Equatable`.

## Case constraints

Named decision: `case-constraints-through-default-domain`, settled 2026-10-24.
Cases have no `where` clause. Case-dependent validity is an ordinary predicate
of the whole value in the data declaration's default-domain `where`. Its `self`
subject follows [predicate subjects](domains.md#declaration-and-membership):

```omega
data Value<T>
    where ((self in Value::Integer) && T == i32)
       || ((self in Value::Boolean) && T == bool)
{
    case Integer(value: i32);
    case Boolean(value: bool);
}

data Interval
    where (self in Interval::Empty)
       || ((self in Interval::Range) && self.lo <= self.hi)
{
    case Empty;
    case Range(lo: u64, hi: u64);
}
```

The data clause uses ordinary well-formed contract propositions, including
Boolean combinations, exact type equality, and value couplings. It may name
enclosing generic parameters, common fields, `self`, and otherwise lexically
available contract names. Accessing `self.lo` or `self.hi` above requires the
Range case established by the preceding conjunct. Use ordinary guarded formation
and case reasoning: inactive payloads cannot be projected, and a predicate
cannot assume the default domain it is establishing to justify its own terms.
A field's qualification constrains that field, not the enclosing sum's case.

This introduces no case-local generic or hidden existential binders. A conformance
obligation does not select an implementation or manufacture evidence; ordinary
[explicit selection](conformances.md) and [proof-term rules](../proofs/contracts.md)
still apply.

Let Fields describe common-field validity, Payload(i) the selected payload's
validity, and Predicate(self) all data-level `where` clauses. The complete
default-domain condition is:

```text
Fields AND Predicate(self) AND
    OR over cases i (active_case == i AND Payload(i))
```

Omitted data predicates contribute true, not a waiver of payload validity.
Inactive payloads have no establishment or ownership obligation. In a
case-conditional predicate, an inactive alternative cannot satisfy a conjunction
requiring its own case. Named generic domains can express the same predicate on
a carrier; data `where` makes it mandatory for every established carrier value.

Construction proves the complete default domain before producing an established
value. Matching combines that domain with the selected case fact through ordinary
[arm-local fact rules](patterns.md). For Value<T>, selecting Integer therefore
establishes T == i32; selecting Range in Interval establishes self.lo <= self.hi.
A type equation reveals an equality; it does not reassign a generic parameter, reinterpret bytes, or
insert a runtime type test. Constructing `Integer` as `Value<bool>` rejects; matching
that combination establishes a contradiction, not a conversion.

Value couplings obey the same [establishment and mutation rules](dependent_values.md)
as type-wide invariants. Replacing a case or changing its payload invalidates
dependent place facts as appropriate; snapshots keep their own exact subjects.
Before observation, the active value's complete conditions must hold again.
Zero storage and generic bodies follow the existing
[default-domain gate](dependent_values.md#default-domains-and-zero-initialization).
This retains constrained-case GADT behavior through ordinary domains, without
case-level `where`, a wrapper, a new data species, implicit boxing, runtime type
tests, or general existential packaging. It grants no new layout optimization.

The mathematical interpretation uses the selected
[W-based inductive profile](../proofs/inductive_profile.md). Case constraints are
not limited to nonrecursive examples. An equality-payload encoding may handle
some cases; it is not by itself a justification for arbitrary indexed recursion.
Checked encodings preserve the case conditions and induction/computation rules
without selecting physical layout or merging nominal source identities.

## Lexical profile

Source is valid UTF-8, with ASCII syntax. Identifiers match
`[A-Za-z_][A-Za-z0-9_]*` and compare by exact bytes. Syntactic whitespace is only
space, tab, carriage return, and line feed. Punctuation, operators, keywords, and
numeric spellings use ASCII; non-ASCII bytes occur only in comments and literal
bodies. Other Unicode whitespace rejects. Host Unicode tables, normalization,
or confusable-name heuristics cannot change tokenization.

The current profile has no Unicode identifiers or raw-payload literal form.
Those require separately specified byte/identifier rules, not host-language
syntax reuse.

## Quoted bytes

A quoted literal contains bytes, has ordinary shared byte-view type `&[u8]`,
and establishes no encoding domain. Its runtime shared view refers to immutable
image storage, not a state-local owner. Source UTF-8 framing does not make the
value text or normalize/transcode its contents.

The lexer copies literal-body source bytes except these byte escapes:

| Escape | Byte |
| --- | --- |
| `\n` | `0x0A` |
| `\r` | `0x0D` |
| `\t` | `0x09` |
| `\0` | `0x00` |
| `\\` | `0x5C` |
| `\"` | `0x22` |
| `\xNN` | The byte specified by two hexadecimal digits. |

Codepoint escapes and raw newlines inside quotes reject. Explicit byte escapes
keep literal values independent of source checkout line endings. Encoders and
normalizers are ordinary library machines; build-resource inclusion requires
admitted build inputs. Explicit qualification proves the chosen encoding's
predicates, not a compiler-inferred text interpretation.

In an exact-width owned fixed-array constant or evaluator result, a literal
copies its bytes into the array. Byte length must match exactly: no truncation,
padding, or hidden live-length field. Temporary evaluator views cannot escape;
only an owned value snapshot crosses its result boundary. Constant interning is
an unobservable optimization, not source storage identity. See
[constants](constants.md) and [semantic evaluation](evaluation.md).
