# Delta request boundary

`request.gamma` implements D30/D33 admission. `outcome.gamma` owns the private
failure value, phase-success carrier, and complete DCOUT V1 publication. The
outer `../../delta_compiler.gamma` entry consumes request admission before
invoking the compiler pipeline. No request-admission function writes output or
observes Delta source. Compiler phases return an explicit success carrying the
next phase's data, or the unchanged owned failure. Failure publication returns
the generic Gamma application result
`(pair tag 1)` after writing exactly one frame.

Admission order is complete 16-byte header, first incorrect magic/version/
reserved byte, full profile ID, declared source provision, then body extent and
exact end. Profile 1 alone is admitted. In particular, a declared oversized
body yields source incompleteness without reading that body, and profile 2
remains retired. A truncated fixed header reports its first missing byte even
when an earlier available header byte is incorrect.

The implemented request-admission outcomes are:

| Tag | Code | Meaning | Request coordinate | Limit/requested |
| --- | --- | --- | --- | --- |
| 1 Reject | 1 | malformed_request | first missing, incorrect, or trailing byte under the admission order | zero/zero |
| 1 Reject | 2 | unknown_profile | 8 | zero/zero |
| 2 Incomplete | 1 | source_bytes | 12 | 4,194,304 / exact declared u32 length |

All use coordinate space 4. The fixed 40-byte frame contains the eight bytes
`ff 44 43 4f 55 54 01 00`, tag at byte 8, space at byte 9, two zero reserved
bytes, little-endian u32 code at byte 12, and little-endian u64 coordinate,
limit, and requested fields at bytes 16, 24, and 32. Tag equals process status.
This is a projection of embedded compiler constants, not a runtime host table.

The source envelope, lexical tokens, structural syntax, complete global identity
census, declaration and body typing, and post-frontend entry schema additionally
own these Reject results (tag 1, zero limit/requested):

| Code | Meaning | Coordinate space | Coordinate |
| --- | --- | --- | --- |
| 3 | invalid_source_byte | 1 Delta source | first forbidden byte |
| 4 | invalid_syntax | 1 Delta source | malformed token/node, unexpected closing delimiter, or exact EOF |
| 5 | integer_literal_out_of_range | 1 Delta source | out-of-range decimal token start |
| 6 | duplicate_type | 1 Delta source | later type name |
| 7 | duplicate_constructor | 1 Delta source | later constructor name |
| 8 | duplicate_function | 1 Delta source | later function name |
| 9 | active_local_conflict | 1 Delta source | later parameter, `let`, or outer-conflicting pattern binder |
| 10 | duplicate_pattern | 1 Delta source | later binder repeated within one pattern |
| 11 | unknown_type | 1 Delta source | unresolved type-name token |
| 12 | unknown_constructor | 1 Delta source | unresolved constructor token |
| 13 | unknown_function | 1 Delta source | unresolved function-head token |
| 14 | unknown_local | 1 Delta source | unresolved local-value atom |
| 15 | type_mismatch | 1 Delta source | offending expression start, or wrong-owner pattern constructor token |
| 16 | arity_mismatch | 1 Delta source | application start, constructor atom, or pattern start |
| 17 | duplicate_match_case | 1 Delta source | later arm's constructor token |
| 18 | nonexhaustive_match | 1 Delta source | match-expression start |
| 19 | missing_entry | 0 none | zero |
| 20 | entry_schema_mismatch | 1 Delta source | present `main` declaration name |

Whole-source byte validation precedes token validation. Token validation skips
comments and accepts only parentheses, ASCII identifiers, single-byte arithmetic
operators, and signed decimal integers. A complete decimal spelling is checked
before its range: an oversized digit prefix followed by a nondigit is malformed
syntax, not an out-of-range integer. The first failing token wins before any
global collection, but a forbidden source byte anywhere wins in the earlier
envelope phase. Reserved-word positions and balanced forms still belong to the
structural frontend; a bare minus is a valid operator token, not an integer atom.

Balanced parsing precedes grammar-role checking: an unmatched closing delimiter
or missing close is reported before a role defect in an otherwise completed
earlier form. Both traversals are explicit counted worklists. They do not grow
Gamma call depth with source nesting. Grammar then checks declarations and their
children in authored order, with each required list shape checked before its
child roles. A malformed present child anchors at its start; a missing required
child anchors at the encountered closing delimiter; an unfinished form anchors
at exact source EOF. Empty and data-only programs lack the required function
declaration and reject at source EOF. These are explicit phase and coordinate
rules, not a claim that every frontend category chooses the smallest offset.

Grammar also accounts for D30's `parse_depth` resource. A function body starts
at expression level 1; each expression child, including an atom, is one level
deeper. Match subjects and arm bodies are children of the match expression;
declarations, parameter lists, patterns, and arm wrappers add no levels. Before
judging an expression's grammar, the worklist refuses level 1,025 with halt/tag
2 (`Incomplete`), code 8, coordinate space 1, that node's start, limit 1,024,
and requested 1,025. The check precedes grammar judgment and expansion, not
retained-node or queue-entry allocation. This is a compiler-profile refusal,
not invalid Delta.
It follows complete balanced parsing and the preceding grammar work: a later
unmatched delimiter still precedes depth refusal, and a previously encountered
grammar defect is not replaced by a depth failure at a later node.

This expression-level accounting is distinct from parenthesis nesting in the
generic parser and from the selected Gamma evaluator's physical heap or stack.
It does not establish full syntax-arena accounting or successful emission for
every input within the selected depth limit.

Function/constructor applications, constructor patterns, and recognized
arithmetic/Bytes call-like heads retain their argument lists for semantic arity
checking. Their signature/payload disagreements do not become structural syntax
failures. Unknown names, duplicate local binders, and match coverage also remain
semantic judgments after the complete global census.

Collection visits globals in authored order across their distinct namespaces,
without resolving a declaration type. The complete type/constructor catalogs
and raw function nodes then feed declaration-type resolution;
only its complete typed metadata reaches body checking. A duplicate therefore
precedes unknown declaration types, including an earlier unknown function
parameter or constructor payload type. Declaration resolution visits declarations,
constructors, and fields in authored order. For each function, it visits
parameters in order and checks each parameter's name conflict before resolving
that parameter's annotation; it resolves the result type after all parameters.
An earlier unknown annotation therefore precedes a later parameter conflict,
while a conflicting parameter precedes its own unknown annotation. The entire
declaration phase completes before any body checking, and failures propagate
unchanged through the phase outcome. Schema runs only after the ordinary
frontend accepts: an invalid body cannot turn into missing-entry or
wrong-signature rejection. Empty source is invalid Delta syntax, not an
otherwise valid program missing an entry. Profile 2 and schema code 21 remain
retired.

## Body traversal and coordinates

Body typing visits retained nodes with explicit continuations and immutable
local environments. It checks functions in authored order after all declaration
signatures succeed. A failed child returns its unchanged failure; later checks
do not replace it with a parent or schema judgment.

An application resolves its head before checking arguments. It checks each
available expected argument and its type in order, reports a missing argument
when reached, and reports extra arguments only after all expected arguments
pass. It does not check an extra argument's body before reporting arity. All
arity failures anchor at the application start; a constructor used as an atom
anchors at that atom. Argument type mismatches anchor at the argument node.

An `if` checks its condition, true branch, and false branch in order. A
noninteger condition anchors at the condition; branch-type disagreement anchors
at the false branch. A function result disagreement anchors at its body.
A body `let` resolves its annotation before checking an active-name conflict,
then checks the initializer in the outer environment and only its body in the
extended environment. This differs from declaration parameters, whose conflict
check precedes their own annotation. Initializer disagreement anchors at the
initializer, not its binder or annotation.

A match checks its subject, then each arm in authored order. A nonnominal
subject is a type mismatch at the subject. Each arm resolves its constructor,
checks owner, payload arity, duplicate-case status, binders, body, and agreement
with preceding arm types, in that order. A wrong owner anchors at the constructor
token; payload arity anchors at the pattern start. A binder conflicting with
the saved outer environment is code 9; a name repeated within this pattern is
code 10. Both anchor at the later binder. An arm-result disagreement anchors at
that arm's body. Only after all arms pass does final constructor coverage
produce code 18 at the match start. These traversal rules, not a global sort of
candidate offsets, select the published failure.

## Remaining boundary work

The common layout and IDs follow the D13/D30/D33 contract. Their retained
historical table is recoverable at
`78d8f51053^:source/delta/compiler/dcout-v1.tsv`; the shared field layout is at
`50bb6afe20:source/beta/compiler/README.md`. Neither retired implementation nor
detached table participates in execution. D125 removes profile 2, not the
request-failure identities.

Canonical frontend rejection and the owned source-byte/parse-depth refusals
are not full DCOUT or Delta-edge closure. Other resource/internal outcomes do
not yet carry compiler-owned evidence. Lowering constructs a complete expanded
Gamma plan before publication, and records the height of every generated
expression. A separate normalizer extracts over-height fragments under the
selected evaluator's 255-list body budget before serialization. It introduces
no new refusal code or profile limit. Generated helpers count toward the
existing function limit; non-tail calls and immutable allocation retain their
separate context and storage bounds. Successful full generated-profile
admission throughout Delta's 1,024-level profile remains open. Those empty-output evaluator
statuses must not be decoded as DCOUT or synthesized into frames by a runner.
The generated ConformanceBytesV1 program's statuses are separately owned by
its adapter. Successful compiler output remains the exact unwrapped Gamma
receipt; the canonical entry explicitly writes the emitter's final LF because
marked Gamma applications do not append a scalar return byte.
