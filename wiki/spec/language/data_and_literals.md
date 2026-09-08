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
