# Data, literals, and lexical framing

`data` declares fields, cases, or both. Fields belong to the value. A record has
only fields; a sum has only cases; a mixed declaration has common fields plus
one active case and its payload. Machines access values through their explicit
parameters and receiver, not implicitly owned machine fields.

## Construction and case identity

Fields have no declaration-site default initializers. Construction may omit an
ordinary runtime field only when its zero value satisfies the complete default-domain obligations.
Nonzero/computed defaults are ordinary constructor machines. See
[establishment](dependent_values.md#default-domains-and-zero-initialization).
Erased fields instead follow [erased construction](../proofs/contracts.md#explicit-erased-bindings):
an accessible nullary constructor may supply an omitted term without requiring
runtime layout or a zero value.

Case-bearing values use a case literal, not record-only construction. Common
and payload fields may be named together and evaluate exactly once in authored
order. Common fields are accessible independently of the active case; payload
fields require the corresponding case knowledge. Partial construction and
disposal follow [ownership order](ownership.md#construction-and-disposal-order).

A case implicitly declares its same-named domain. `Type::Case` therefore denotes
case membership in domain position. A payload-free case can also denote a
constructed value; a payload-bearing case needs its payload in value position.
Cases, attached domains, and attached machines share a member namespace;
collisions reject rather than selecting by priority. Pattern rules are defined
in [dispatch](patterns.md).

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

Value equality and domain membership are distinct. For structural equality,
compare common fields, case identity, and the active payload, never inactive
storage/padding. Primitive and payload-free-sum equality is intrinsic; records
and payload-bearing sums select the declared synthesis/conformance required by
[Equatable](conformances.md#core-equality-acquisition). Adding a payload requires
that declaration rather than silently changing tag equality into field equality.

## Case constraints

A case may carry an ordinary `where` clause after its payload list (or its name
when payload-free) and before its terminating semicolon:

```omega
data Value<T> {
    case Integer(value: i32) where T == i32;
    case Boolean(value: bool) where T == bool;
}

data Interval {
    case Empty;
    case Range(lo: u64, hi: u64) where lo <= hi;
}
```

The clause uses ordinary well-formed contract propositions, including Boolean
combinations, exact type equality, and value couplings. It may name enclosing
generic parameters, common fields, its own payload bindings, and otherwise
lexically available contract names; another case's payload is not in scope.
It introduces no case-local generic or hidden existential binders. A conformance
obligation does not select an implementation or manufacture evidence; ordinary
[explicit selection](conformances.md) and [proof-term rules](../proofs/contracts.md)
still apply.

Case constraints are indexed by the actual active case, not predicates that can
be satisfied by an inactive alternative. Let Common include type-wide `where`
constraints and common-field validity, and Payload(i) and Constraint(i) describe
case i. The complete default-domain condition is:

```text
Common AND OR over cases i (
    active_case == i AND Payload(i) AND Constraint(i)
)
```

An omitted case clause contributes true, not a waiver of payload validity.
Inactive payloads have no establishment or ownership obligation. This extends
the existing default domain rather than replacing type-wide constraints.

Construction proves the selected case's complete conditions before producing an
established value. Matching that value contributes the selected case's conditions
through the ordinary [arm-local fact rules](patterns.md). A type equation reveals
an equality; it does not reassign a generic parameter, reinterpret bytes, or
insert a runtime type test. Constructing `Integer` as `Value<bool>` rejects; matching
that combination establishes a contradiction, not a conversion.

Value couplings obey the same [establishment and mutation rules](dependent_values.md)
as type-wide invariants. Replacing a case or changing its payload invalidates
dependent place facts as appropriate; snapshots keep their own exact subjects.
Before observation, the active value's complete conditions must hold again.
Zero storage and generic bodies follow the existing
[default-domain gate](dependent_values.md#default-domains-and-zero-initialization).
This supplies constrained-case GADT behavior without a new data species, implicit
boxing, or general existential packaging.

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
