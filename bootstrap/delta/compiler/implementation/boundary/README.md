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

The implemented source-owned outcomes are:

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

The source envelope, lexical tokens, complete global identity census, and
post-frontend entry schema additionally own these Reject results (tag 1, zero
limit/requested):

| Code | Meaning | Coordinate space | Coordinate |
| --- | --- | --- | --- |
| 3 | invalid_source_byte | 1 Delta source | first forbidden byte |
| 4 | invalid_syntax (lexical token coverage) | 1 Delta source | malformed token start |
| 5 | integer_literal_out_of_range | 1 Delta source | out-of-range decimal token start |
| 6 | duplicate_type | 1 Delta source | later type name |
| 7 | duplicate_constructor | 1 Delta source | later constructor name |
| 8 | duplicate_function | 1 Delta source | later function name |
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

Collection visits globals in authored order across their distinct namespaces,
without resolving a declaration type. The complete type/constructor catalogs
and raw function declaration custody then feed declaration-type resolution;
only its complete typed metadata reaches body checking. A duplicate therefore
precedes unknown declaration types, including an earlier unknown function
parameter or constructor payload type. Schema runs only after the ordinary
frontend accepts: an invalid body cannot turn into missing-entry or
wrong-signature rejection. Empty source remains invalid Delta syntax, not an
otherwise valid program missing an entry. Profile 2 and schema code 21 remain
retired.

The common layout and IDs follow the D13/D30/D33 contract. Their retained
historical table is recoverable at
`78d8f51053^:source/delta/compiler/dcout-v1.tsv`; the shared field layout is at
`50bb6afe20:source/beta/compiler/README.md`. Neither retired implementation nor
detached table participates in execution. D125 removes profile 2, not the
request-failure identities.

This is partial frontend-boundary coverage, not full DCOUT closure. Remaining
structural syntax, local-name, type, arity, and match failures still reach the shared
evaluator trap, and later resource/internal outcomes do not yet carry
compiler-owned evidence. Those empty-output evaluator
statuses must not be decoded as DCOUT or synthesized into frames by a runner.
The generated ConformanceBytesV1 program's statuses are separately owned by
its adapter. Successful compiler output remains the exact unwrapped Gamma
receipt; the canonical entry explicitly writes the emitter's final LF because
marked Gamma applications do not append a scalar return byte.
