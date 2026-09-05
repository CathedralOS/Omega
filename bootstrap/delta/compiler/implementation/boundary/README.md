# Delta request boundary

`request.gamma` implements D30/D33 admission. `outcome.gamma` owns the private
failure value and its complete DCOUT V1 publication. The outer
`../../delta_compiler.gamma` entry consumes admission before invoking the
compiler pipeline. No admission function writes output or observes Delta
source. Failure publication returns the generic Gamma application result
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

The common layout and IDs follow the D13/D30/D33 contract. Their retained
historical table is recoverable at
`78d8f51053^:source/delta/compiler/dcout-v1.tsv`; the shared field layout is at
`50bb6afe20:source/beta/compiler/README.md`. Neither retired implementation nor
detached table participates in execution. D125 removes profile 2, not the
request-failure identities.

This is request-boundary coverage, not full DCOUT closure. Frontend/schema
failures still reach the shared evaluator trap, and later resource/internal
outcomes do not yet carry compiler-owned evidence. Those empty-output evaluator
statuses must not be decoded as DCOUT or synthesized into frames by a runner.
The generated ConformanceBytesV1 program's statuses are separately owned by
its adapter. Successful compiler output remains the exact unwrapped Gamma
receipt; the canonical entry explicitly writes the emitter's final LF because
marked Gamma applications do not append a scalar return byte.
