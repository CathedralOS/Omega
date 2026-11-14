# Data, literals, and lexical framing

`data` declares a record or a sum. A record has only fields. A sum has only
cases, one of which is active with its payload. Machines access values through
their explicit parameters. Runtime callable fields store explicit machine values; they do not
attach machine declarations or an implicit receiver to data. See
[machine values](machine_values.md) and [some data binders](some_packages.md).

`data Name<Parameters> = TypeExpression;` instead declares a
[transparent type alias](generics.md#transparent-type-aliases). It introduces
no fields, cases, nominal carrier or storage wrapper.

## Records, sums and nested declarations

Named decision: `record-or-sum-data`, settled 2026-11-10;
[the design draft](../../drafts/designs/record_or_sum_data.md) records the
migration. A data body that declares both a field and a case rejects. Common
fields and alternatives are combined by holding a sum in a record's field:

```omega
data Command {
    command_id: u64;
    op: Operation;

    data Operation {
        case GetUser(user_id: u64);
        case CountUsers;
    }
}

machine targets_one_user(op: &Command::Operation) -> bool { ... }

let command = Command {
    command_id: 17,
    op: Command::Operation::GetUser { user_id: 42 },
};
```

`command.op` is an ordinary field and `Command::Operation` an ordinary sum: it
is constructed, moved, borrowed, returned, stored and passed as a generic
argument under the rules for any other type.

**Nested declarations.** A data body may contain `data` declarations after its
fields or cases, to any depth, in a record or a sum. A nested declaration
introduces a name and nothing else: no field, case, constructor argument,
positional pattern component or cleanup action. It is visible throughout the
enclosing declaration, including before its text, and elsewhere as
`Owner::Name`. Nesting only scopes the name. The nested type has its own
default domain and cleanup selection, neither owes nor satisfies its owner's
invariants, captures no value of its owner, and may be held by any declaration
that can name it. A type that several declarations hold is declared at module
scope and held in a field the same way.

**A field declared with its type.** `name: data Type { ... };` in a field or
case-payload position means exactly the member `name: Type` together with the
nested declaration `data Type { ... }` in the enclosing data declaration:

```omega
data Command {
    command_id: u64;

    op: data Operation {
        case GetUser(user_id: u64);
        case CountUsers;
    };
}
```

A type declared in a case payload is named under the enclosing data, as in
`Source::Node`; a case is not a scope. The inline form introduces no parameters
of its own, so a nested type that takes its own arguments is declared
separately and applied at the field. Each name is declared once, and a later
member refers to it by name.

**Names.** Nested data shares its owner's one member namespace with cases and
attached domains, and a collision rejects. A nested
[alias](generics.md#transparent-type-aliases), `data Header = SharedHeader;`,
is transparent under the owner's name.

**Visibility.** A nested data declaration inherits its owner's visibility,
recursively, as [fields and cases do](modules.md#visibility), and takes no
`public` of its own. Referring to a separately declared type in a field does
not change that type's visibility. Domain-owned access restrictions govern
operations on a nested type's values and do not hide its name.

**Generic context.** A nested declaration may use its owner's ordinary static
parameters, and they are part of its identity under the rule for
[nested machines](generics.md#applications-and-specialization):

```omega
data Envelope<T> {
    body: data Body {
        case Value(value: T);
        case Empty;
    };
}

let body: Envelope<u32>::Body = Envelope<u32>::Body::Value { value: 7 };
```

`Envelope<u32>::Body` and `Envelope<bool>::Body` are distinct types whether or
not the body mentions `T`, and no `Envelope` value is needed to construct one.
Type identity selects no representation; see
[schema and layout](reflection.md#schema-and-layout). An owner's
[`some` binder](some_packages.md#declaration-and-construction) is a per-value
witness, not a parameter of the owner's type, and is not inherited. A field
passes it to a parameter the nested type declares:

```omega
data Packet<'a, some S> {
    payload: Payload<S>;

    data Payload<U> {
        value: &'a U;
    }
}
```

The nested type is `Packet<'a>::Payload<U>`. Runtime-value indices are likewise
passed explicitly.

A sum held in a field has the same representation freedoms and obligations as
any other field; nesting adds no packing restriction and no size guarantee. A
[layout plan](../layouts/plans.md#home-layout-realization) preserves ordinary
references, reads, writes, loans and representation commitments for it, a
reference to the field denotes its
[original storage](../terminal-psi/structural_access.md#referent-identity),
and an optional packing that cannot preserve an otherwise valid use is not
selected. Nesting and holding by value add no schema member, number or
discriminator beyond the field itself.

## Construction and case identity

Named decision: `prefix-case-identities`, settled 2026-10-27. A numbered
member writes its identity before its declaration kind:

```omega
data Reading {
    #1 sensor_id: u32;
    #2 recorded_at_ms: u64;
    #3 sequence: u32;
    #4 state: State;

    data State {
        #1 case WarmingUp;
        #2 case Sample(#1 temperature_celcius: i32);
        #3 case Fault(#1 code: u32);
    }
}
```

The spelling is `#N case Name`, not `case #N Name`. Unnumbered alternatives
remain `case Name`; there is no shared `case { ... }` declaration block.
Cases remain distinct mutually exclusive schema members, not simultaneously
present record fields. Each declaration and each payload has its own identity
scope, so a payload's `#1` identifies neither a case nor a field of the record
that holds the sum. Existing numbering,
retirement, declaration-order and stable-identity rules are unchanged by this
prefix migration. There is no extra case-group field, number or discriminator.

### Explicit field omission

Named decision: `explicit-zero-field-omission`, ratified 2026-11-06.

A record or case literal without `..` supplies every ordinary runtime field of
the constructed value. A final standalone `..` explicitly requests zero values
for omitted runtime fields. Each omitted value and the complete aggregate must
satisfy their default-domain, qualification, establishment and custody
obligations. The marker supplies no authority, borrowed reference, callable or
proof witness merely because storage can be zeroed.

```omega
data Options {
    timeout_ms: u32;
    retries: u32;
}

let exact: Options = Options { timeout_ms: 1000, retries: 0 };
let defaults: Options = Options { timeout_ms: 1000, .. };
let missing: Options = Options { timeout_ms: 1000 }; // Rejected: retries missing.
```

Adding a runtime field makes an exhaustive literal require an update. A literal
with `..` accepts the new zero value only if the complete construction remains
valid. A required nonzero or computed value must still be supplied explicitly.

There is at most one marker, after all authored field initializers; it has no
operand and evaluates nothing. `Type { .. }` requests zero for every runtime
field. `Type { }` is exhaustive only when there are no runtime fields.
The marker is neither record-update/spread syntax nor a pattern rest binding;
it changes no array length, range endpoint or positional-call rule.

For a case literal, the same marker covers omitted fields of the explicitly
selected active payload. It does not select a case, initialize inactive
payloads or bypass case constraints. Nested literals each govern
their own omissions; an outer marker does not authorize omissions inside an
authored inner literal.

Fields have no declaration-site default initializers. Nonzero/computed defaults
are ordinary constructor machines. See
[establishment](dependent_values.md#default-domains-and-zero-initialization).
Erased fields instead follow [erased construction](../proofs/contracts.md#explicit-erased-bindings):
an accessible nullary constructor may supply an omitted term without requiring
runtime layout or a zero value. That rule does not require or derive from `..`.

Field and case-payload relationships belong to the data declaration's ordinary
`requires`; case-dependent projections require established case knowledge. Field
and payload type spellings do not carry bracketed scalar range suffixes. Construction and every
restoring write prove the owning default-domain clauses.

A sum value is written as a case literal, and a record holding a sum supplies
it as an ordinary field initializer. Initializers evaluate exactly once in
authored order. A record's fields are accessible independently of the case
active in any sum it holds; payload fields require the corresponding case
knowledge. Partial construction and
disposal follow [ownership order](ownership.md#construction-and-disposal-order).

A case implicitly declares its same-named domain. `Type::Case` therefore denotes
case membership in domain position. A payload-free case can also denote a
constructed value; other case values use a case literal, including `..` when
runtime fields are omitted.
Cases, nested data and attached domains share their owner's member namespace;
data does not own machine declarations. Collisions reject rather than selecting by priority.
Pattern rules are defined
in [dispatch](patterns.md).

Case values always use their carrier-qualified member path. Expected type,
imports, and local context do not infer a case owner for a single-segment name:
write `Light::On`, not `On`. A payload-bearing value uses its qualified
construction form, such as `Command::Move { dx: 1, .. }` when other fields
have valid zero values; `Command::Move` without its literal remains the case
domain, not a partially constructed value. This rule is `bare-case-value-names` and applies uniformly in locals, arguments,
returns, comparisons, and nested expressions.

Cases are nominal, not foreign integers. `#N` is schema identity, not a runtime
tag assignment. Foreign codes cross in their declared integer carrier and use
ordinary checked mapping machines, including an explicit unknown-code outcome.
No raw integer silently establishes a sum value.

The ordinary zero representation of a sum selects the first declared case
and a recursively zeroed payload. That is accessible only after the
active value's default-domain obligations hold; inactive payloads need no
establishment. Zero does not imply semantic emptiness or authority. A payload-free
first case is not required. [Home-layout realization](../layouts/plans.md#home-layout-realization)
may encode case identity in the payload representation and omit a separate tag
when it preserves the language's semantics, including zero's meaning and legal
mutation, observation and custody. A repairable payload predicate alone does not
establish that its excluded patterns remain unavailable throughout those uses.
Stable external placement uses
[layout policies](../layouts/plans.md), not inferred native offsets.

Value equality and domain membership are distinct. For executable structural
equality, compare a record's fields, and a sum's case identity and active
payload, never inactive storage/padding. Primitive and payload-free-sum equality is intrinsic;
records and payload-bearing sums select the declared synthesis/conformance
required by [Equatable](conformances.md#core-equality-acquisition). Adding a
payload requires that declaration rather than silently changing tag equality
into field equality. Value `==` retains that selected comparison in facts.
Explicit `core::Identity<T>(left, right)` states
[proof-relevant identity](../proofs/contracts.md#data-identity-in-facts) without
requiring an executable equality algorithm. Structural comparison exposes
checked component comparisons; conversion to identity requires the applicable
checked bridge law and cannot silently reinterpret float fields.

## Case constraints

Named decision: `case-constraints-through-default-domain`, settled 2026-10-24.
Cases have no `requires` clause. Case-dependent validity is an ordinary predicate
of the whole value in the data declaration's default-domain `requires`. Its `self`
subject follows [predicate subjects](domains.md#declaration-and-membership):

```omega
data Value<T>
    requires ((self in Value::Integer) && T == i32)
       || ((self in Value::Boolean) && T == bool)
{
    case Integer(value: i32);
    case Boolean(value: bool);
}

data Interval
    requires (self in Interval::Empty)
       || ((self in Interval::Range) && self.lo <= self.hi)
{
    case Empty;
    case Range(lo: u64, hi: u64);
}
```

The data clause uses ordinary well-formed contract propositions, including
Boolean combinations, exact type equality, and value couplings. It may name
enclosing generic parameters, fields, `self`, and otherwise lexically
available contract names. Accessing `self.lo` or `self.hi` above requires the
Range case established by the preceding conjunct. Use ordinary guarded formation
and case reasoning: inactive payloads cannot be projected, and a predicate
cannot assume the default domain it is establishing to justify its own terms.
A field's qualification constrains that field, not the enclosing sum's case.

This introduces no case-local generic or hidden existential binders. A conformance
obligation does not select an implementation or manufacture evidence; ordinary
[explicit selection](conformances.md) and [proof-term rules](../proofs/contracts.md)
still apply.

Let Fields describe a record's field validity, Payload(i) a sum's selected
payload validity, and Predicate(self) all data-level `requires` clauses. The
complete default-domain condition is:

```text
record:  Fields AND Predicate(self)
sum:     Predicate(self) AND
             OR over cases i (active_case == i AND Payload(i))
```

A record that holds a sum relates the sum's case to its other fields and
parameters by testing the field place:

```omega
data Command<T>
    requires ((self.op in Operation::GetUser) && T == User)
       || ((self.op in Operation::CountUsers) && T == u64)
{
    command_id: u64;
    op: Operation;
}
```

A case predicate takes any place as its subject, and the fact it establishes
concerns that exact place. Matching `command.op` of an established
`Command<T>` combines the selected case with `Command`'s predicates, so the
`GetUser` arm holds `T == User`. A constraint about the sum alone belongs on
the sum; one relating it to its holder's fields belongs on the holder.

Omitted data predicates contribute true, not a waiver of payload validity.
Inactive payloads have no establishment or ownership obligation. In a
case-conditional predicate, an inactive alternative cannot satisfy a conjunction
requiring its own case. Named generic domains can express the same predicate on
a carrier; data `requires` makes it mandatory for every established carrier value.

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
case-level `requires`, a wrapper, a new data species, implicit boxing, runtime type
tests, or general existential packaging. It grants no new layout optimization.

The mathematical interpretation uses the selected
[W-based inductive profile](../proofs/inductive_profile.md). Case constraints are
not limited to nonrecursive examples. An equality-payload encoding may handle
some cases; it is not by itself a justification for arbitrary indexed recursion.
Checked encodings preserve the case conditions and induction/computation rules
without selecting physical layout or merging nominal source identities.

## The uninhabited type `Never`

Named decision: `published-no-return`, ratified 2026-11-13.

Core declares one ordinary uninhabited data type:

```omega
public data Never requires false { }
```

Its default-domain predicate is `false`, so no value of it can be established.
Construction, zero storage, `..` omission, reflected construction,
representation recasts and foreign boundaries each owe that predicate and cannot
discharge it. The compiler does not recognize the name: the rules below apply to
any type proved uninhabited through its default or named domains. Proof is
automatic for `Never` itself and for a record, or every case of a sum, with a
`Never` field; other uninhabitedness needs explicit evidence, and failure to
prove a type inhabited is not proof that it is empty.

**Results.** A machine declared `-> Never` cannot return normally; see
[published no-return](state_contracts.md#published-no-return). An expression
whose declared type is proved uninhabited is accepted wherever any type is
expected. No value is produced or converted, because its evaluation never
completes normally. A declared inhabited result does not gain that
compatibility because a postcondition excludes return: such a call can end a
statement sequence, but it cannot stand where a value is required.

**Coverage.** A case whose payload is proved uninhabited needs no arm under
[coverage](patterns.md#coverage-and-domain-tests):

```omega
data Outcome<T, E> {
    case Ok(value: T);
    case Err(error: E);
}

machine take_value(outcome: Outcome<u32, Never>) -> u32 {
    match outcome {
        Outcome::Ok { value } -> value
    }
}
```

A body generic over `E` still covers `Err` unless its own contract establishes
that `E` is uninhabited; a caller's choice of `Never` does not exempt it.

## Lexical profile

Source is valid UTF-8, with ASCII syntax. Identifiers match
`[A-Za-z_][A-Za-z0-9_]*` and compare by exact bytes. Syntactic whitespace is only
space, tab, carriage return, and line feed. Punctuation, operators, keywords, and
numeric spellings use ASCII; non-ASCII bytes occur only in comments and literal
bodies. Other Unicode whitespace rejects. Host Unicode tables, normalization,
or confusable-name heuristics cannot change tokenization.

`some` introduces a per-value type binder in data parameter lists; it is not a
visibility modifier or an alternative spelling of an ordinary type argument.
The retired `dyn` spelling has no source type or conversion form.

This profile has no Unicode identifiers or raw-payload literal form.
Those require separately specified byte/identifier rules, not host-language
syntax reuse.

## Integer radix spellings

Integer literals support decimal, hexadecimal `0x`, binary `0b`, and octal `0o`
spellings, digit underscores, and explicit carrier suffixes. For example:

```omega
let mask: u16 = 0xFFFFu16;
let byte_divisor: u32 = 0x1_0000u32;
```

Radix and grouping affect source notation, not numeric meaning or overflow
policy. The second value is `65536`, not the maximum of `u16`. Carrier-limit
queries belong to [core integer/float APIs](numeric_values.md#core-integer-and-float-apis);
hex literals need no limit-name lookup. Bit layouts and masks should use clear
radix spellings or ordinary named protocol constants instead of unexplained
decimal encodings. Quoted-byte `\xNN` escapes below are separate literal syntax.

Concrete integer suffixes accept the positive decimal widths of
[width-indexed integers](numeric_values.md#width-indexed-integers), such as
`17u13` or `41u257`. The abbreviated carrier name is identical to its indexed
application. Symbolic widths use an expected indexed type or an explicit exact
conversion; bare `i`/`u` suffixes remain unbounded computational carriers.

## Quoted bytes

A quoted literal contains bytes, has ordinary shared byte-view type `&[u8]`,
and establishes no encoding domain. Its runtime shared view refers to immutable
image storage, not a invocation-local owner, so the view is valid at whatever
lifetime its use requires while that image is retained. A machine may return
one at a [caller-chosen result lifetime](lifetimes.md#caller-chosen-result-lifetimes).
Source UTF-8 framing does not make the value text or normalize/transcode its
contents.

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
