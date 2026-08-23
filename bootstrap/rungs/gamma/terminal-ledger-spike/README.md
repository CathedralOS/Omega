# Canonical terminal semantic-ledger spike

This directory is the bounded Gamma feasibility experiment required by the
`PCC-CANONICAL-SEMANTIC-LEDGER` ruling. It is not the production generator and
does not replace the explicit migration trust graph. It establishes that the
authoritative endpoint can begin at canonical terminal-Psi bytes and remain a
small, typed, interpreter-defined program rather than another Rust verifier.
It is artifact-assurance work merely hosted in Gamma: it does not define a
Gamma expression, type rule, evaluation rule, or proof-kernel rule, and its
co-location under the rung during repository migration grants it no authority
over Gamma meaning.

## Trust boundary and inputs

The spike was built against four ordinary `TerminalModule` values encoded at
terminal format 18/vocabulary 20. The live product fixtures have since advanced
to format 22/vocabulary 25, so the frozen decoder now rejects those fixtures and
the standalone spike gate is intentionally tracked as stale until it is rebased
or retired. The Rust fixture builder and `bytes_to_gamma.py` are untrusted test
transport. The typed Gamma program begins with the reusable, PSITERM-neutral
primitives in `../canonical-bytes/`; the spike-specific decoder then:

1. consumes the exact byte list;
2. validates its frozen markers, closed tags/counts/types, and the exact spike
   shape needed by the bounded ledger;
3. rejects truncation, trailing bytes, unknown tags, invalid identities, wrong
   types, wrong section counts, and shape drift;
4. resolves every retained leaf operation through one exact row in either the
   scalar table in `schema.gamma` or the distinct structural/effect table;
5. resolves scalar, Unit, and boundary calls through the separate exact
   three-row composition table in `call_composition.gamma`; and
6. emits and audits a ranked 54-row scalar ledger or 3-row structural/effect
   ledger, or validates exact Unit/boundary composition sites without inventing
   primitive rows.

The format-18 envelope additionally included format-annotated IEEE structural
leaves and atomic float equality/inequality, plus byte-sequence structural
carriers and atomic content equality. That vocabulary is deliberately outside
this bounded fixture: encountering any of those field or proposition tags
rejects instead of widening the spike's claimed ledger coverage. Their exact
byte grammar is independently gated in `../terminal-codec-primitives/` but is
not concatenated into this spike.

The transport packs at most seven bytes into one positive Gamma `Int` solely to
keep the source parser shallow. Typed `unpack_bytes` reconstructs every byte
before `decode_module` sees it. No packed integer is interpreted as terminal
semantics.

## Covered ruling cases

The fixture covers an i8 constant, a Boolean constant, Boolean not/equality,
integer equality and ordering, i8 bitwise not/and/or/xor, strict i8-to-i16
widening, partial i16-to-i8 exact cast, exact and wrapping shifts with an i16
count, exact and wrapping i8 add, exact and wrapping signed divide/remainder,
toward-zero negative division/remainder, and the signed `MIN / -1` policy
distinction. Exact cast owns target-range admission; exact right shift owns
nonnegative in-range count admission; exact left shift additionally owns result
representability. The ledger audit pins each exact scalar, logic, comparison,
conversion, shift, and arithmetic denotation plus twelve safety rows rather than
merely counting them.

Its diamond control flow introduces two branch-local premises, invalidates them
at the join, carries the matching value through both predecessor edges, and
creates one merge row that depends on both edge rows. The asymmetric fixture
changes only the false-arm value and must reject. Every prerequisite rank is
strictly lower than its consumer; the positive branch premise is available in
the true block and unavailable at the join.

The call carries two obligations for two canonically ordered callee
requirements. The ledger audits exact enumeration and capture-free positional
substitution:

- obligation 105: `caller.value11 <= 127`;
- obligation 106: `-128 <= caller.value10`.

The call table contains exactly the three closed terminal variants: scalar
in-module `Call`, structural in-module `CallUnit`, and exact external
`BoundaryCall`. One generic checker keeps target/result custody, positional
binder shape, clause coverage, capture-free substitution, claim or receipt
transfer, guarded outcomes, crash-route composition, evidence lifetime, fuel,
and frontier policy as independent axes. The canonical scalar fixture consumes
its table row end to end. A separate 697-byte canonical fixture decodes an exact
qualified affine resource, structural requirement, claim transfer, `CallUnit`,
boundary signature, completion receipt, and `BoundaryCall`, then exercises the
other two rows through the same checker. Missing, duplicate, cross-kind,
weakened-evidence,
wrong-requirement, weakened-frontier, signature, state-version,
move/reborrow, coverage, substitution, outcome, crash, and evidence-lifetime
drift all reject in both evaluators.

The separate structural/effect fixture covers the three remaining leaf kinds
without pretending that they are scalar equations:

- `BooleanStructuralField` requires one exact live affine record place and one
  exact relevant Boolean field, emits `StructuralFieldEq`, and keeps that place
  live until the exact nominal cleanup;
- `PortWrite` requires one exact service in the machine's published ceiling,
  emits the exact `(service, port, value)` effect, and keeps the place frontier;
- `EstablishTrivialAffineLocal` requires the exact empty-record local, emits its
  establishment fact, adds it to the affine frontier, and requires its exact
  `ReturnUnit` retirement.

The structural/effect decoder and evaluator are separate modules. Mutating
field relevance, field identity, service, port, cleanup machine,
establishment destination, or affine retirement rejects in both evaluators.

## Closed leaf and call schemas

The bounded operation slice no longer dispatches through one hand-written
builder per primitive. `schema.gamma` contains exactly thirty-two declarative
rows: constants, Boolean not/equality, integer equality/order, the complete
scalar bitwise cohort, strict widening and partial exact cast, the complete
exact/wrapping shift cohort,
the complete exact/wrapping/saturating add/subtract/multiply cohort,
the complete exact/wrapping/saturating divide/remainder cohort, and signed
less-than. The rows are grouped
into scalar/logic, bitwise/conversion/shift, exact-arithmetic,
total-arithmetic, and divide/remainder/comparison policy
tables and composed through one generic table operation.
Each row owns its result shape, direct denotation, canonical safety goal,
post-discharge fact, crash policy, and local fuel/frontier behavior. A generic
interpreter emits the rows. Exact lookup rejects both missing and duplicate
rows.

Calls are intentionally not disguised as leaf rows. Their three-row
exact-unique table and generic composition checker live in
`call_composition.gamma`; clause coverage, capture-free substitution,
outcome/control, crash-route, evidence-lifetime, fuel, and frontier
responsibilities remain separate from primitive denotation. The gate runs the
complete fixture with a missing row, a
duplicate row, and an altered row; all three reject while the canonical table
reconstructs the byte-identical 54-row ledger. This is the bounded
thirty-two-kind scalar table. A second exact-unique table covers structural
`EstablishTrivialAffineLocal`/`BooleanStructuralField` and effectful `PortWrite`
with place/frontier/effect vocabulary rather than scalar permutations. Missing,
duplicate, and weakened-frontier structural rows reject. Missing, duplicate,
and altered call rows reject, and all eight dynamic call-custody checks remain
independently mandatory.

The generator carries exact `TerminalScalarType` declarations for every known
value, including unsupported full-width signed, unsigned, and address types;
the bounded operation rows reject types outside their explicit Boolean/i8/i16
policy without collapsing the declaration. The three supported constructors and
that bounded-policy predicate live in `scalar_types.gamma`; shared type equality
remains in the terminal-codec layer. Integer-constant operations likewise retain
the exact signed/unsigned 128-bit payload until the bounded signed-i8 row is
selected; unsigned or non-sign-extended payloads reject at that policy boundary
rather than in byte decoding. Leaf
operands, call arguments, block parameters, and newly introduced results are
checked against that environment; duplicate identities and type drift reject
before any row is published. Boolean-producing leaves publish ordinary result
equations; conditional control consumes that equation without reconstructing
or owning the leaf's meaning. The gate mutates the canonical byte fixture to
reuse an existing value identity, encode an invalid Boolean, redirect a Boolean
operand to an i8 value, narrow a widen result, redirect an exact cast to an i8
operand, and erase its obligation, and requires both evaluators to reject each
case. A schema mutation that weakens exact left shift to count-only admission
also rejects.

## Measured result

Measured on Darwin 25.5 arm64, Apple M4 Pro, after building the Beta programs;
these are feasibility observations, not performance promises:

| Item | Result |
| --- | ---: |
| Canonical scalar fixture bytes | 1,983 |
| Canonical structural/effect fixture bytes | 695 |
| Canonical Unit/boundary call fixture bytes | 697 |
| Assembled typed Gamma core | 4,982 lines / 198,971 bytes |
| Shared canonical-byte layer | 109 lines / 3,831 bytes / 14 functions |
| Imported terminal-codec primitive subset | 302 lines / 12,484 bytes / 19 functions |
| Full shared terminal-codec primitive layer | 592 lines / 25,186 bytes / 35 functions |
| Spike-specific typed core | 4,571 lines / 182,655 bytes / 393 functions |
| Closed data declarations | 182 |
| Typed functions, assembled | 423 |
| Maximum source nesting | 25 |
| Canonical scalar ledger | 54 rows / 3,607 modeled bytes |
| Canonical structural/effect ledger | 3 rows / 185 modeled bytes |
| Prospective scalar reconstruction certificate | 2,984 modeled bytes |
| Prospective structural/effect certificate | 164 modeled bytes |

The certificate estimate is deliberately explicit: 32 bytes of header, then
44 bytes per row plus one 32-byte reference per prerequisite. It is a sizing
model, not an accepted certificate format.

The schema conversion deliberately adds a small typed vocabulary and generic
interpreter while deleting operation-specific builder branches. The important
scaling result is structural: new leaf meaning is one isolated data row, and
the ledger orchestrator is no longer the owner of operation permutations.
`schema.gamma` is 327 lines; call/control/premise orchestration remains separate
from primitive denotation. The 349-line `call_composition.gamma` owns its three
rows and generic axis checker, while `ledger.gamma` sequences the resulting
rows. Structural/effect byte decoding is separate from its schema and ledger
evaluation. Unit/boundary byte decoding and composition evaluation are likewise
isolated in `call_decode.gamma` and `call_ledger.gamma`.

The first literal transport spelling used one constructor per byte and exposed
a parser-stack failure. That did not require weakening the endpoint: a typed,
semantics-free unpacker made the exact same bytes practical. More importantly,
the original ledger orchestration was rejected by the Gamma checker until it
was decomposed into accessors, validators, row emitters, and sequencing helpers.
That is useful evidence for the ruling: the low implementation should grow as
small modules and closed row tables, not as a second monolithic verifier.

The PSITERM-neutral byte cursor, checked `u8`/little-endian `u16`/`u32`, exact
low/high-half `u64`, and exact four-limb `u128` primitives are factored into
`../canonical-bytes/`.
The codec's exact current envelope and bounded canonical UTF-8 rule are a
separate `../terminal-codec-primitives/` responsibility; all three spike
decoders use one header parser instead of duplicating magic/format checks, and
the identity consumers use one UTF-8 parser instead of owning an ASCII-only
parser. Canonical Boolean plus optional and required full-width semantic-ID
decoding are also shared; required identities have exact equality and canonical
unsigned order, and the fixture's zero-high-half limit is applied only by its
explicit consumer adapter. The complete Boolean/fixed-signed/fixed-unsigned/address
scalar-type grammar and exact signed/unsigned 128-bit integer-value payloads are
shared as well; declarations and boundary results retain that complete scalar
type, and integer-constant operations retain the complete signed/unsigned
payload, while the fixture's Boolean/i8/i16 operation subset and canonical
signed-i8 sign-extension rule are separate adapters. Exact fixtures reject format,
vocabulary, type-width, integer tag/sign-extension, and malformed-UTF-8 drift.
The same shared responsibility now has an independently gated v18
structural-leaf module for IEEE kind/format, byte-sequence carriers, canonical
structural paths, and proposition tags `11`/`12`. It preserves full-width
identity/index order but remains outside the assembled bounded spike.
Both layers have independent typed/interpreter gates. The bounded spike
explicitly narrows identities to a zero high half only after the byte layer has
decoded the complete unsigned value. The largest measured audit tax is
mechanical repetition:
Gamma currently has no parametric result type, so the bounded decoder declares a
result ADT for each parsed semantic type. Completing the structural/effect slice
adds 1,128 lines and 42,263 bytes to the assembled core, while canonical
Unit/boundary decoding and evaluation bring the full bounded assembly to 4,982
lines and 198,971 bytes. The code remains bounded, typechecked, separated into
decoder versus schema/evaluator modules, and at nesting depth 25. All three call
variants still traverse one axis checker rather than adding call-kind evaluator
branches. That is an engineering and audit cost, not an actual
language-design blocker or a reason to weaken the canonical-byte endpoint.

## Gate

Run:

```sh
sh bootstrap/rungs/gamma/test-canonical-bytes.sh
sh bootstrap/rungs/gamma/test-terminal-codec-primitives.sh
sh bootstrap/rungs/gamma/test-terminal-ledger-spike.sh
cargo test -p psi-terminal-codec --test ledger_spike
```

The first command checks the shared byte layer independently. The second
typechecks the exact assembled spike source, mechanically erases its
annotations, evaluates matching/asymmetric/malformed fixtures with both the
canonical Beta interpreter and the independent Python evaluator, and pins the
ledger/certificate measurements.
