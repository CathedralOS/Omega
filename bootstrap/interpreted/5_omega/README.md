# Epsilon-written bootstrap compiler

This directory contains the bootstrap compiler that must eventually construct
the Omega compiler in [`../../../source/`](../../../source). It currently compiles a
checked scalar and record subset into executable Alpha tapes. It does **not** yet compile
the Omega compiler source closure or close the Omega self-rebuild edge.

The source can run through the sibling [`../4_epsilon/`](../4_epsilon)
evaluator or the source-built compiled peer in
[`../../compiled_vm/`](../../compiled_vm). These are peer construction paths.
The intended remaining edge is:

```text
Epsilon execution + bootstrap compiler + Omega compiler source → first Omega compiler
first Omega compiler + same Omega compiler source              → rebuilt Omega compiler
```

The bootstrap compiler may support a subset sufficient for its successor, but
every supported facility must preserve Omega semantics. Unsupported forms fail
explicitly. The maintained Rust compiler is a development comparator, never a
semantic stage hidden inside the bootstrap route. Formal bootstrap correctness
certificates are outside current acceptance; ordinary language checks and
contracts still apply.

The stable artifact names and task IDs `D` and `C` in older records identify the
bootstrap compiler and Omega compiler respectively. Implementation and remaining
closure work are tracked in [`../../TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).

## Retention inventory

| Retained file | Canonical role | Deletion condition |
| --- | --- | --- |
| `omega_compiler.epsilon.sources` | Canonical ordered membership, byte lengths, and SHA-256 identities of Epsilon-written compiler closure `D`. | Delete only when an adjacent compiler input format carries and verifies the same source-member custody directly. |
| `*.epsilon` | Eight members separated into representations, request/UTF-8 validation, lexical classification, lexing, parsing, Alpha tape construction, scalar compilation, and outcome encoding. | Replace a completed component only atomically with an equally complete final Epsilon implementation and synchronized manifest identity. |

Target identity is an exact immutable compiler-invocation input. `build.omg`
under [`../../source/omega/`](../../../source/omega) does not declare a support
list or repeat target host/boundary policy; exact
multi-target requests fan out into independently checked child compilations.

The manifest, not directory enumeration or path order, selects and orders D's
members. `tools/bootstrap/source_closure.py` validates every
declared length, digest, source byte, path, strictly increasing stable
identity, and exact source inventory before concatenating bytes without separators.
The current ten-member
manifest materializes 21,760 lines / 802,467 bytes with SHA-256
`b822576f03fb8e9ea8868b350fe45cc50c0484e4f6cee590be4ef89d95a5eaa4`.
The manifest itself is 1,674 bytes, SHA-256
`29a5cabc70989102dcfcaf492266b1d1d5e675a330ddc9bd6ed7077f4a460f24`;
`tools/bootstrap/omega/compiler_env.sh` checks both identities against every
materialization and `tests/bootstrap/omega-identity.sh` covers refusals.
A digest binds source custody; it does not prove compiler correctness.
Earlier parser measurements remain historical observations of their original
source subjects, not measurements of the current closure.
The authoring files preserve one Epsilon translation unit and its packed
diagnostic coordinates.

The [source-to-executable gate](../../../tests/bootstrap/omega-executable/README.md)
connects parsed scalar machines to checking and the shared Alpha encoder.
Its public request route validates OCREQ shape and re-walks the bound subject;
the complete OCREQ semantic admission phases remain unfinished as detailed below.
The direct `compile(source, entry)` boundary is an internal compilation customer,
not a replacement public request protocol.

The internal `compile_set(source, entry_unit, entry)` customer accepts a bounded
source set: at most 64 package identities, 256 direct dependency aliases and 512
source units. Each unit has invocation-owned package membership and an exact
byte range. The parser checks that unit independently and reads its optional
specified `module name::path;` header. Module-less files of one package share
one logical module; physical paths supply no namespace. Types and calls retain
their exact declaration identity across units. Source-order changes do not
select a different imported declaration.

Imports are file-local. Exact imports select declarations; namespace imports
bind a namespace without importing its ordinary machines as bare names.
Repeated imports of one identity are allowed, while competing imported names
refuse. Qualified names require the requester's own package or a direct
requester-local dependency alias, with public visibility across packages.
Public field and call-interface types cannot expose private records. The
current subset handles explicitly declared modules, free machines and named
records; it does not add attached declarations, reexports, overload selection,
or full build/request admission. The single-view API is the one-unit case.
Every supplied declaration is checked, including unused units and bodies.

The construction application is [construct_omega.epsilon](../construct_omega.epsilon).
It reads exact package, direct-alias and unit rows followed by up to 4 MiB of
source bytes. Package roles distinguish ordinary packages from the explicitly
trusted core and standard library; spelling alone grants no such role. The
selected entry is a source-unit ordinal and name span. This internal input
context is separate from OCREQ package acquisition and admission. A successful
invocation writes Alpha tape; a refusal writes its source coordinate and exits
nonzero. The implementation remains under construction; Epsilon source
acceptance alone does not establish compilation of the Omega compiler closure.

The emitted subset retains a nullary `u8` program entry. Free and nested
machines may use plain `u8`, `u32`, `u64`, `bool`, `u32 in Trapping`, copy records,
and Unit results. Omitted results independently mean Unit; a nested declaration
never inherits its enclosing machine's result. Initialized and zero-established
locals retain their declared types, and mutable bindings permit checked stores.
It supports
resolved ordinary free calls, nested argument calls, scalar comparisons,
short-circuit Boolean operations, and parameterized same-owner tail transfers.
Boolean transition subjects may depend on runtime values, with Boolean or
wildcard guards and either a machine transfer or a direct Boolean result.
Constant transition subjects retain their specialized code path.

A signature census runs before bodies resolve calls or transfers. Unsupported
overload families refuse; identical admitted signatures reject as duplicates
regardless of binder spelling. Every body and arm is checked, including those
unreachable from the selected entry. Machine clauses, receivers,
other runtime carriers/policies, arithmetic beyond the checked unsigned families below, non-Boolean dynamic
subjects, and ordinary calls to nested declarations remain explicit coverage
boundaries. Nested declarations use canonical `machine`; the retired `state`
alias is no longer accepted. The spelling `state` remains an ordinary identifier.
Callable-name hiding refuses until lexical resolution can support it completely;
a nested declaration never silently falls through to a same-named global.
A terminal transfer naming a visible global is `Incomplete` in this same-owner
subset; only an absent target is reported as missing.

Anonymous constant arithmetic retains its mathematical meaning. The bounded
folder supports integral `+`, `-`, `*`, and `/` when every intermediate lies in
`0..255`, plus comparisons and Boolean operations. This is an implementation
coverage bound, not an Omega carrier inferred from the destination. Fractional,
negative, and wider intermediates return `Incomplete`; division never silently
truncates. Anonymous remainder and width-dependent bitwise/shift operations
also return `Incomplete`. Typed operations need their own checked lowering.
A direct out-of-range `u8` literal and division by zero reject. Full rational
folding, broader explicit landing, and other typed runtime operations remain work.

Private static storage separates parameter, local, pending-argument and
expression-scratch cells within a 16 MiB arena `[0x02000000, 0x03000000)`,
beyond the maximum 16 MiB tape and
below Alpha's return stack. Each call evaluates arguments left to right into
its own pending cells before copying them into destination parameters, including
same-state transfers. This preserves
earlier arguments and caller locals across nested calls. A whole-owner call
DAG, collecting calls from every state, rejects reentrant ordinary-call cycles;
all admitted state transfers stay within the owner, so mixed transfer/call
cycles cannot bypass the check. At most 4,096 source ordinary calls can be live.
No admitted source operation converts these private addresses to numeric values.
Numeric comparisons bias both unsigned words by the high bit before Alpha's
signed comparison, preserving unsigned order. Decimal literals currently cover
`0..4294967295`; larger `u64` spellings remain explicitly
unsupported. Plain `u64` addition by a nonnegative literal is admitted when
checked upper bounds prove the complete result at most 2147483647. The emitted
word addition is then exact; unproved plain arithmetic remains unsupported.
Qualified `u32 in Trapping` supports addition, subtraction and multiplication;
qualified `u64 in Trapping` supports addition and subtraction. Each selected
core operator checks overflow or underflow and realizes an abnormal Alpha Trap.
The qualified operand selects the family; a same-carrier plain peer is allowed,
and a receiving annotation never inserts qualification. `u64` multiplication
and the remaining runtime operators decline explicitly. Named checked-value
variants retain the selected realization; operands evaluate once, left to right,
with private scratch preserving a captured left value across a right-hand call. Comparisons retain carrier/policy identity rather
than treating every word as interchangeable. Ordinary arguments, locals and
results never acquire or drop a domain implicitly: `as +Trapping` and
`as -Trapping` express the supported domain changes. Core comparisons admit a
plain peer of the same carrier, as specified by the policy-domain operator
family; that does not qualify the peer. Carrier casts currently cover plain
identity/anonymous landing and exact `u8` to `u32` widening. Other casts and
context-dependent domain inference inside compound expressions refuse explicitly.

This closed slice projects the primitive operators and the fixed core policy
comparison and arithmetic families specified by [numeric values](../../../wiki/spec/language/numeric_values.md#policy-domains);
it does not claim general
operator-declaration or package/core admission. Imports, custom operators and
user domains remain unsupported. The core Trapping comparison family publishes
`crashes Trap`, even though its emitted value computation is total. The compiler
retains that conservative summary across all ordinary calls and nested states.
Under [effect admission](../../../wiki/spec/language/effects.md), private checked
bodies infer it; public bodies that would need a crash clause
refuse until those clauses are supported. No `NoTrap` claim or complete selected
product effect admission follows from this internal source-to-tape customer.
Visibility uses canonical `public`; retired `pub` is an ordinary identifier,
not a declaration modifier.

Records retain declaration identity, field qualification and copy/affine
multiplicity. The current layout gives every scalar an eight-byte word and
fixed arrays their declared count times the element stride; this is a private
Alpha representation, not an implementation of canonical native layout queries.
A SourceUnit byte array therefore occupies 1 MiB. Layout recursion is bounded
at 64 steps and the complete private storage arena at 16 MiB. By-value cycles,
linear/opaque/authority-bearing types, stored references, selected cleanup and
unsupported domains decline before allocation. An intact affine local with no
contained cleanup may leave scope; it is never implicitly copied.

The first default-domain solver admits direct scalar-field `<=` literal bounds,
with the literal checked against the field's carrier. Zero establishment proves
these predicates recursively. Every field store must preserve the declared
bound immediately, using the same unsigned upper-bound evidence as indexed
access and explicit entry contracts. Unknown predicates and temporary invariant violations decline. A checked
record copy preserves all nested field validity. Calls retain only declared
validity, not stale stronger facts about fields they may mutate.

Copy records support exhaustive literals, nested field projection, owned local
storage, parameter/result transport and captured-place assignment. Literals
evaluate their fields in authored order; arguments evaluate left to right and
copy values into distinct staging storage before the next call can overwrite
its result. No affine record moves or partial initialization are admitted yet.
Fixed arrays may be retained, zero-established and copied inside these records.
Fixed `u8` arrays also support indexed copy reads and writes through a checked
core operation plan. The plan retains the concrete element/extent, collection,
selector, access mode, parent backing and bounds evidence. Explicit shared or
mutable removal of the array shell's `Trapping` domain preserves element types
and closes its empty restoration debt. Reads use shared views and writes use
mutable views; other reference attenuation remains unsupported. Other domains,
range views and structural
equivalence of separately spelled array value types remain unsupported.

Index bounds come from literals, immutable snapshots, declared scalar-field
upper bounds, or explicit checked machine entry requirements. The requirement
profile supports `true`, conjunction, and unsigned formal/projected-field `<` or
`<=` literal bounds. Every call and transfer must discharge the exact target
requirements; unsupported predicates cannot disappear. Pure Boolean guards
establish bounds at nested arrivals, with pure actual expressions required for
contract-bearing calls. No private requirement is inferred from a later access.
An overlapping write or exclusive call invalidates the entry version. Call
mutation ranges are retained before checking the next operand, including within
the same expression; disjoint sibling ranges preserve their evidence.
The collection is captured before
the selector and the resulting destination before the RHS; selector mutation
cannot redirect a pending store. A scoped collection loan remains live during
selector evaluation and, for assignment, through the RHS. Whole-array overlap
is conservative: this slice does not yet prove separate element loans disjoint.

Shared and mutable record-reference parameters carry the original referent
address. Calls support one reference parameter, explicit borrows of owned
records or projected record fields, and projected reborrows. The checker tracks
active argument loans by exact root and field range: overlapping access after
an exclusive loan refuses, while disjoint fields remain usable. The referenced
root cannot be rebound. Nested transfers may forward the enclosing invocation's
original incoming reference as a bare matching parameter; its whole referent
loan remains live while later arguments evaluate. A local-backed reference
cannot enter this path, so repeated tail transfers cannot outlive reused local
storage. Ordinary calls also accept a bare matching incoming reference and
retain the same argument-order loan and mutation range. Escaping/stored references, implicit
shared lending and lifetime syntax decline. Mutation through a shared reference rejects. All these checks finish before
any executable bytes are published.

The `--omega-storage` customer takes exact SourceId, SourceUnit, ModulePath,
`source_unit_clear`, `source_unit_append`, `source_unit_byte_or_nul` and
`module_path_append` declarations from `source/psi/source/source.omg`, adds
`storage_controls.omg`, and executes the emitted result. The real private
helpers now state their existing guarded bounds explicitly and remove the
array-shell policy through checked shared/mutable views. The customer checks
clear's preserved byte, zero/final/full append boundaries, absent reads and
ModulePath capacity. Generic controls check disjoint mutation, captured indexed
assignment, original-reference forwarding and explicit scalar entry contracts.
Refusals cover missing/stale bounds, mutating guard operands, same-expression
call invalidation, overlapping argument loans and local-backed tail transfers.
The parser and compiler contain no special cases for these helper names.

```sh
python3 bootstrap/compiled_vm/source_test.py --omega-storage
```

The `--omega-control` source-built customer extracts the exact canonical
`is_identifier_start` and `is_identifier_continue` declarations from
`source/psi/lex/lexer.omg`, adds the checked-in boundary-calling wrapper, compiles
them with this bootstrap compiler, and executes the emitted tape. Its companion
source-owned controls exercise parallel transfers, skipped looping operands,
nested comparisons, type/arity/range/grammar refusals and reset behavior:

```sh
python3 bootstrap/compiled_vm/source_test.py --omega-control
```

Extraction and request framing supply bytes, not language meaning; no compiler
path recognizes these helper names or source shapes.

The source-owned `controls_values.epsilon` customer exercises parameter/local
semantics, nested argument calls, caller preservation, forward calls, state
transfers, and refusal/reset behavior. Run it through the source-built peer:

```sh
python3 bootstrap/compiled_vm/source_test.py --omega-values
```

This reconstructs the source Gamma compiler and its successor evaluators before
running the manifested bootstrap compiler and executing its emitted artifacts.
It does not use Python to supply language semantics. Python frames and checks
bytes; the native Alpha seed and documented source/tape admission remain trust
inputs. The development comparator can also run these controls with
`python3 bootstrap/compiled_vm/test.py --omega-values` and retains its separately
disclosed Python semantic dependency.

The scalar failure paths also record the canonical OCOUT outcome tuple
(outcome tag, coordinate space, code, coordinate, canonical ordinals, limit,
requested). Parser exhaustion projects onto its assigned resource code, tape
capacity reports under emitted-artifact coordinates, and source refusals
anchor at the retained span. The assigned `Reject` codes 1–12 remain
`malformed_request`, the eight lexical refusals projected from
the scanner's retained diagnostic, `duplicate_name` at the later
declaration's name, `missing_entry` with no coordinate, and
`integer_literal_out_of_range` at the refused literal's first byte. The
embedded projections now cover the full assigned inventory: the syntax
`Reject` codes 13–89 and the scalar checking codes 90–97 all anchor in
canonical source, and the `Incomplete` coverage provisions project
resource codes 15–23 onto canonical source and 24–25 onto the
coordinate-free space. Producers still record the unassigned marker for
the syntax, checking, and coverage refusals themselves, so the encoder
keeps refusing those recorded tuples until the recording side is wired. The
[outcome gate](../../../tests/bootstrap/omega-outcome/README.md) exercises the
embedded tables, exact and refused frame encodings, bounded arithmetic, and
the recorded tuples through the selected evaluator.

The manifested members (`D`) exist but are incomplete; both descriptive compiler
tapes remain absent. The [standalone request contract](../../../wiki/spec/build/compiler_request.md)
owns OCREQ/OCOUT, including complete snapshots, subject binding, validation order,
and shared diagnostic selection. D's request decoder currently validates the
outer envelope and phase-1 declared-extent provisions in `outcome.epsilon`
plus the phase-2/6 subject and invocation field/tag shape pass in
`request_and_utf8.epsilon`; the semantic phases over the decoded fields —
package-key recomputation, canonical ordering, graph, snapshot, admission, and
commitment checks — remain unimplemented. The assigned OCOUT header,
outcome-code, coordinate-space, phase, and scalar-resource tables are embedded
there as projections with the canonical failure-frame encoder. The contract's
lexical `Reject` codes 2–9 and the named declaration, entry, and literal
refusals 10–12 are assigned and implemented in D; the syntax `Reject`
inventory, the remaining checking codes, and the `Incomplete` coverage
provisions are assigned in the contract and still need producer wiring and
the semantic phases over the decoded fields under OMEGA-D/OMEGA-C in
[the board](../../../TASKS_BOOTSTRAP.md).
A raw-single-file stdin convention cannot replace the sealed request.
[Build execution](../../../wiki/spec/build/execution.md) owns the admitted checkpoint,
one-way generated-source continuation, and dependency-bundle no-rerun contract.

Its retained Alpha encoder targets `AlphaBootstrapV4` directly: at most
16,777,212 raw tape bytes, targets below that exact extent, and at most 1,864,134
nine-byte-dominated fixups. These are outer compiler-resource bounds, not Omega
language limits, and move only with the common seed/checker profile.
The four direct address-writing paths admit offsets `0..16777211`, matching
that payload provision; finalization additionally requires every target to lie
on an instruction start inside the actual payload. The
[whole-member D customer](../../../tests/epsilon/interpreted-omega-experiment/README.md)
checks exact forward fixups, sealing, undefined-label failure, and exact/adjacent
address admission through the selected Epsilon evaluator. It also constructs
a 79-byte echo/count tape through D's emitters and executes those exact emitted
bytes with the selected Alpha seed, covering all three symbolic operand shapes,
forward/backward targets, and call/return on empty, NUL, and high-byte input.
This is execution evidence, not a checked compiler refinement. It does not establish
full-size tape realization or a completed D compiler.

Epsilon cannot safely express a reusable validate-once source cursor: machines
and fields are public, while immutable views cannot be stored in data. `D`'s
parser therefore validates once and streams the same source through private
states of one canonical invocation. Its retained slices sequence empty,
trivia-only, ordinary `use path::member;`, basic `[public] data`, and ordinary
machine roots whose current bodies contain neutral path calls and simple
assignments.
The explicit slow [complete-D parser customer](../../../tests/bootstrap/omega-parser/README.md)
checks this manifested implementation through the lower chain. Sum-valued
parser guards use Epsilon case transitions; scalar equality is not a sum test.
One mixed root table preserves authored use/data/machine order. Optionally
public ordinary machines retain an arbitrary name-like path, optional
parentheses containing a comma-separated state-parameter list, and a body from
the retained statement slice. An optional leading target selector is
retained as an exact span on the same machine row; selection and activation
remain later phases. A source-ordered machine-clause ledger retains exact
non-generic `satisfies Trait::requirement` bindings and nonempty `reaches`
ceilings over comma- or plus-separated service identifiers. A satisfying clause
may additionally own the exact `via`, `Binding`, and `CompilerIntrinsic` spans
for the payload-free closed external binding. That form terminates one bodyless
external-leaf machine with its canonical empty implicit entry. The ledger also
retains the private ranking witness `terminates by <path> -> <View::Path>;` as
separate source-ordered subject and view paths plus exact `by` and arrow spans;
it does not confuse that form with bare `terminates;`, which authors the public
eventual-terminal guarantee. Tuple or non-path subjects, an omitted or
argumented view, ranking ranges, the bare guarantee, generic satisfies
arguments, aliases, other external binding cases, and empty or
installation-bound reach rows remain incomplete. The parameter list
retains canonical optional `const` and leading
`mut`, consuming or borrowed `self`, and shared/mutable/write-only
binding-reference forms.
Non-receiver parameters retain `name: Type`. Non-receiver parameter types, data
fields, case-payload fields, and immediate machine return types share one engine
for bare named/Self/Unit, outer references, unqualified domains, inclusive
literal ranges, and nested fixed arrays and slices. References may retain an
exact explicit lifetime, and general `Self` uses the same `SelfType` base as
receivers. Every parameter row records its canonical const/mutable/self flags;
the owning implicit or explicit state owns the optional return node.

The retained nonempty-body slices include ordinary semicolon-terminated call
statements without acknowledgements, evidence arguments, or result discard,
and terminal expression statements over the retained primary path/literal slice,
plus a restricted canonical transition core.
Each call owns a flattened receiver-member span, an exact target
member, and a contiguous span of argument expression handles. Its current
value arguments are exact name/self paths, booleans, unsuffixed nonnegative
decimal integers, or string tokens represented by tagged expression nodes;
the same lane accepts shallow named struct literals over those primary field
values. Zero arguments and trailing commas are supported. Calls may also own a
separate nonempty `<...>` lane of path-only static machine arguments. Qualified
paths are retained exactly; const arguments, evidence projections, nested
static applications, lifetime arguments, and empty or trailing-comma static
lanes remain incomplete. Ordinary assignments retain
separate target and value handles. Their current target grammar is a self/name
place path followed by zero or more indexes, each with a path, decimal, or
canonical range over the retained path/decimal bound slice; current values are
self-member paths, qualified-name paths, booleans, unsuffixed nonnegative
decimal integer literals, or strings. Integer
spelling is retained by source span and is not evaluated during parsing;
strings retain the exact token span and scanner-proven decoded byte length
without a decoded-byte mirror. Assignment values, local initializers, ordinary
call arguments, and transition subjects share one bounded precedence reducer
over `||`, `&&`, equality, comparison, single-domain contextual `in`, `|`, `^`,
`&`, `<<`, `>>`, `+`, `-`, `*`, `/`, and `%`. Every ordinary operator
materializes a source-ordered binary node without evaluation or type guessing;
membership retains its value and domain path directly in the tagged expression
row. Domain composition after `in` remains parser-incomplete and cannot fall
through to the integer bitwise lane. Once formed, an unparenthesized membership
may continue only into comparison, equality, or logical tiers; grouping is
required before a tighter operator can apply to its Boolean result.
Parentheses delimit reduction frames and preserve their exact transient source
extent without manufacturing a group syntax node. Recursive logical `!`,
fixed-width integer complement `~`, and arithmetic negation `-` prefixes wrap
a completed operand from the admitted primary, group, and cast slice before
that value enters the binary frame; borrow/reference and other contextual
unary forms remain incomplete. Postfix indexing may start from a retained path
in any framed-value consumer or continue a completed literal, call, cast, or
indexed value. Repeated brackets retain a left-to-right expression chain; each
bracket independently reuses the canonical index/range representation. Index
operands may themselves contain that same postfix form. A 128-frame context
stack suspends every live outer bracket coordinate while the inner indexed
expression is retained, then rejoins the completed inner handle as the outer
operand. This covers nested range starts and ends without copying syntax rows;
adjacent depth fails as the existing private `ExpressionDepth` resource.
Shallow struct literals
accept an exact one-member record or two-member case type path and a comma-
separated named field list with an optional trailing comma. Canonical adjacent
fields without commas, nested struct literals, and richer field expressions
remain incomplete until the shared postfix and field-expression paths admit
them. The implicit entry state and every
authored `state` own independent parameter, return-type, and contiguous mixed-
statement spans. A retained transition has either no subject or one precedence-
correct subject over path, boolean, and unsuffixed nonnegative decimal integer
primaries, and expands each
boolean, integer, or wildcard arm into an ordinary statement targeting one
named state. Arms also retain bare name paths and qualified two-member case
paths with optional braces. Braced case patterns own a contiguous field ledger
over shorthand bindings and fixed path-expression matches, preserving the
distinction between `Type::Case` and `Type::Case {}` plus exact binding and
field spelling for later resolution. Current named targets own a contiguous
argument-expression span
over self/name/member paths, booleans, unsuffixed nonnegative decimal integers,
strings, shallow struct literals, and path bases followed by one or more path
or decimal indexes. An index may instead use any canonical bound-presence/inclusivity
shape (`..`, `..end`, `..=end`, `start..`, `start..end`, or `start..=end`)
over those retained bounds. It is represented by a separate range expression
rather than folded into the indexed node. Target argument lists retain an
optional trailing comma. A
subjectless block retains absence explicitly and does not manufacture a unit
expression. Computed or multiple subjects, record/tuple patterns, renamed or
waived fields, non-path fixed values, proof selectors, richer guards, richer
target arguments,
terminal/value/self targets, continuations, and `match` remain implementation-
incomplete. Richer expressions and statements likewise stop the root as
incomplete, and no body is skipped as opaque text.
Local-data statements retain canonical `let [mut] name: Type [= expression];`
syntax in a dedicated row. `mut` is contextual, so `let mut: T;` still binds an
immutable local named `mut`. Locals share the borrow-capable type engine and the
same bounded expression reducer as assignments, including repeatable postfix
indexing with path or decimal indexes and the same canonical range shapes;
absent initializers are represented by an explicit presence bit rather than a
valid-looking handle. Ordinary call
initializers share the statement-call target, static-argument, runtime-argument,
and delimiter parser but own a distinct expression row with an explicit
optional receiver handle. Call arguments retain the existing primary slice and
reuse the same reducer.
Path-valued casts reuse the same expression and type engines in assignments,
locals, ordinary call arguments, and transition-target arguments. They retain
the value and target-type handles, exact cast span, and an optional single-name
`in Domain` suffix. Ordinary assignment, local, and call-argument casts may
also wrap a completed grouped primary and then continue through the shared
binary tail. Richer postfix values, recasts, domain arguments, arbitrary bound
expressions, and other richer initializers remain implementation-incomplete.
One terminal expression statement may close a state directly. Its current
values are a self/name/member path, boolean, unsuffixed nonnegative decimal
integer, string, or a path followed by one or more indexes with path, decimal,
or canonical range operands, and such a primary may continue through the shared
binary-operator reducer — including a statement-leading path — with `}`
terminating the statement only when no expression group is open. The statement
points directly to the shared expression node; indexing has no assignment-only
syntax representation.

Each completed machine owns zero or one implicit entry followed by its explicit
states in source order, matching the canonical parser. Outer and nested
`requires` clauses retain shared predicate-expression rows with exact owner
slices, including repeated clauses, conjunctions and calls. The parser does not
infer predicates or resolve their meaning; the compiler's narrower checked
requirement profile is described above. Parameters, a return,
implicit statements, or an otherwise empty machine require the implicit entry;
an explicit-state-only machine without those forms does not manufacture one. A
free machine uses the generated `entry` identity, while an attached machine
names its entry with the final authored declaration-path member. Machine,
termination-witness, and domain paths share the general path-member arena, but
a machine snapshots its
path extent before parameter types can append domain members. A trailing
parameter comma rejects as malformed. Constrained slice elements and the
`Slice<T>` spelling, return types placed after clauses, generics, remaining
machine-clause forms beyond the explicit `requires` profile, and `boundary` forms; target
declarations and public target-scoped combinations, other public roots,
other bodyless declarations, and other body forms
remain incomplete. The parser never skips a body as opaque
syntax. In the current 73-root `C` closure, all 113
machine-header parameter occurrences, all 73 root parameter lists, and all 73
root headers are representable. All 73 roots are complete: all 57 bodies that
reach parsing, plus sixteen target-scoped bodyless external leaves. The bodyful
set includes
four initial call-only roots, seven roots using the retained assignment slice,
four target-provider roots using path-only static call arguments, the string-argument
`psi` package build root, `Lexer::initialize`, the canonical Omega package build
root, `Lexer::{is_whitespace,push_decoded}` through explicit states and
transitions, and `ConsoleNativeProvider::{write,write_line}` through exact
satisfies and reach clauses, plus `Lexer::tokenize` through the same ordinary
call/assignment/transition statement grammar and `Lexer::emit_punctuation`
through retained addition. `Lexer::is_identifier_start`,
`Lexer::is_identifier_continue`, and
`Lexer::hex_digit_value` complete through precedence-aware transition subjects
and subtraction. `SourceUnit::append` and `TokenStream::push_decoded` complete
through retained indexed assignment places. `SourceUnit::byte_or_nul` completes
through the same indexed expression in terminal value position, and
`TokenStream::push` completes once its indexed place and named transition-target
argument are both retained. `Lexer::{retain_token,classify_keyword,
lex_identifier,lex_whitespace,consume_suffix}` complete through canonical local
bindings and the shared additive initializer path. `Lexer::{lex_line_comment,
lex_block_comment,copy_source_to_decoded,is_raw_string_candidate}` complete
through retained call-valued local initializers.
`Lexer::{validate_utf8,dot_starts_float,reject_raw_string_candidate}` complete
through retained additive ordinary call arguments; `Lexer::lex_next` advances
through the same syntax. Path-valued casts complete
`Lexer::{consume_digits,lex_punctuation,lex_next}` and
`Parser::load_current`. Reusing the indexed-expression builder for local
initializers completes `Lexer::span_equals`; the three Parser roots that use the
same form advance to qualified token-pattern guards. Retaining qualified case
paths, shorthand bindings, and fixed path matches then completes `Main::main`,
`Lexer::digit_in_base`, and `Parser::{parse_data,skip_trivia,parse}`. The
shared grouping and multiplicative reducer then completes
`Lexer::{decode_at,lex_cooked_string}`. Recursive logical-not retention completes
`Lexer::lex_number`. The sixteen external leaves are the four
`ConsoleNativeProvider::{read_line,read_byte,write_byte,exit_process}`
realizations for each selected target. `console_write_bytes` completes through
shared indexed transition-target arguments: `bytes[0]` owns an integer index,
while `bytes[1..]` owns an open-ended range index. Closing the current root
census does not imply that the parser implements unexercised Omega syntax or
that `D` has semantic, lowering, or emission closure.

Data syntax retains an optional `[copy]` property, repeated `requires`
predicates, bare named fields,
payload-free cases, contextual `case: Type` fields, structured case payloads
over the same bare named type leaf, one unqualified `Base in Domain`
constraint, recursively nested fixed arrays `[Type; length]`
over bare named leaves with the same unsuffixed decimal length spelling and an
optional outer domain, optional final member/case semicolons, mixed field/case
order, and relative spans in separate live-prefix tables. A case reaches its
contiguous payload-field span in a separate arena; direct and payload fields
share one binding control path. Type references are postorder tagged nodes: a
constrained root points to its base and to one source-shaped constraint, while
an outer Reference points backward to its complete referee tree and retains
shared/mutable/write-only access plus an optional exact lifetime span.
FixedArray and Slice nodes point backward to their element; FixedArray also
retains the exact length span. SelfType and Unit need no payload.
Domain constraints point into the general path arena; array lengths retain exact
spans without interpreting their values. Data predicates retain source-ordered
ordinary expression roots and clause spans; each data row owns its exact
requirement span, including an empty span for a declaration with no predicates.
The common expression builder owns precedence, groups, paths, literals and
operators. Completing a predicate leaves the next `requires` or body-opening
brace to the data-header parser. These are retained syntax, not an assertion
that D already proves the data's default-domain obligations. Scalar range type
suffixes are syntax failures at their opening bracket. Bracket syntax
uses a bounded invocation-local frame stack and emits named, array, and slice
nodes in postorder, so every child index points backward.
Compact kind/index ledgers reach the use/data/machine rows and field/case child
spans instead of duplicating coordinates. Qualified, indexed, intersected,
combined, exclusive, expression-bound, or multiple constraints; constrained
slice elements, the `Slice<T>` spelling, generic types, rich array elements or
lengths; numbered identities;
field relevance; other public roots; and every other unimplemented valid form
stop as implementation-incomplete rather than becoming false Omega rejections.

The provisional backing tables hold 4,096 root/use/data/machine/state rows and
16,384 path-member/data-member/direct-field/payload-field/case/machine-parameter/
machine-clause/data-requirement/type-node/constraint/statement/call-statement/call-expression/
cast-expression/assignment/local-data/transition/expression/binary-expression/
unary-expression/
indexed-expression/
argument/transition-pattern-field/static-machine-
argument/struct-literal/struct-field rows,
plus 128 scratch type-array frames and 128 expression values, binary operators,
unary prefixes, group boundaries, and nested-index contexts.
Only rows below their corresponding count may be inspected after `Complete`;
every other status may leave unowned partial prefixes and authorizes no
syntax-tree consumer. A repeated invocation invalidates old rows by resetting
every count. Root capacity dominates use/data/machine capacity, while `States`
becomes independently exhaustible as soon as one machine may own multiple
states. Data-member capacity dominates direct-field/case capacity. Fields,
parameters, and state returns share the type-node table, making `TypeNodes`
independently exhaustible. Its equal ceiling dominates payload-field,
machine-parameter, and constraint capacity in
the current slice because every retained row of those kinds owns at least one
type node.
Import/domain paths, call receivers, and path expressions share the
independently exhaustible path-member arena after each machine snapshots its
declaration path. Every retained call or transition-target argument owns one
expression row, so `Expressions` dominates the equal argument table. Every
call, assignment, local-data statement, or
transition-arm row owns one statement, so `Statements` dominates all equal
statement-variant tables. Every local owns at least one type node, so
`TypeNodes` also dominates the equal local-data table. Assignments own two
expressions, making the
expression table independently exhaustible. Every retained struct field owns
one value expression
and every struct literal owns its expression node, so `Expressions` dominates
both equal struct tables. Every static machine argument owns at least one
same-capacity path-member row, so `PathMembers` dominates that table as well.
Every retained machine clause owns at least one path member, so `PathMembers`
also dominates the equal clause ledger.
Every retained transition-pattern field owns one path member for its spelled
field, so `PathMembers` also dominates the equal pattern-field ledger even when
a fixed match adds its own expression path.
Every binary row owns its expression node, so `Expressions` dominates that
equal table as well.
Every unary row likewise owns its expression node, so `Expressions` dominates
that equal table as well.
Every indexed row likewise owns its expression node, so it introduces no new
resource distinction.
Every range row likewise owns its expression node, so `Expressions` dominates
the equal range table.
Every call-expression row likewise owns its expression node, and every retained
receiver is another expression node, so `Expressions` dominates the equal call-
expression table.
Every cast-expression row likewise owns its expression node, so `Expressions`
dominates that equal table.
Every terminal expression statement points directly to an existing expression
handle and adds no variant table; the equal statement and expression ceilings
remain independently exhaustible across the other retained forms.
The meaningful
resource distinctions are therefore `Roots`, `States`, `PathMembers`,
`DataMembers`, `TypeNodes`, `TypeDepth`, `ExpressionDepth`, `Statements`, and
`Expressions`.
These are private compiler budgets to profile against the real compiler
closure, not Omega source limits; exhaustion maps to `Incomplete` through
`outcome.epsilon`'s embedded scalar-resource projection: parser resources
occupy wire codes 3..11 under canonical-source coordinates, the request's
declared section extents occupy codes 1..2 under request coordinates, and the
Alpha tape capacities occupy codes 12..14 under emitted-artifact coordinates.

No source identity, package alias, token ledger, decoded mirror, or transferable
preflight fact is retained. The [standalone request contract](../../../wiki/spec/build/compiler_request.md) binds each relative tree to a package-owned
source unit and fixes public diagnostic/outcome framing at the outer compiler
edge. A
public validate/advance split would be false authority, while revalidating the
whole view at every token would be quadratic; neither belongs in the compiler.
