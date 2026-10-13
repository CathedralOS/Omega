# W2-17 execution evidence

The complete application reads finite bounded stdin for 1..4 horizontal layers
and 0..16 offsets, validates strictly increasing velocities and positive finite
thicknesses, reports critical offsets and all admissible head/direct branch
times, and chooses deterministic first arrivals. No compiler or library patch
was needed: missing Console/float-operations imports and illegal simultaneous
mutable/read-only arguments were corrected in the authored sample.

## Source, executor and independent oracle

The isolated worktree began on audited main
`0294611e8a71a9b98e8a39b53edd87ca46c12121`. The starting checkout and unrelated
work were preserved. Final qualification uses production compiler sources at
`c784626bd8981f911448670105c8ef5abd9003c3`, freshly fetched through the landing
reservation. Exercised collection/store/reference changes triggered release
of that reservation, a compiler rebuild outside the lease, and full affected
numerical/source requalification. The later `b7aaf37278ea0c23a273db7eccae9bacdee1e221`
main commit changes only macOS golden records and TASKS.md; it does not alter
this Linux sample’s governing inputs.

Executor: selected environment
`ccarenv_b64_Y2NhcmVudl9lNjdlZWM4ZWRkNmM4MTkxOTg4MDFhMDg5ZjViZmMzOQ`, Linux
x86_64, 4-CPU quota, 16-GiB memory quota. Actual session model/effort metadata is
unavailable and unverified. Approved repo-local Git identity: Jarod Castillo,
`43973221+Smotto@users.noreply.github.com`. Claim:
`ff7ed763f91e4911a924aba935bc3f6a`; SAMPLE-CORPUS overlap was advisory.

Doctor reported eight checks with no failure/warning. Pinned Rust:
`1.100.0-nightly (a69a63265 2026-09-03)`; mbx 1.21.0; nextest 0.9.146; pinned
link/run probe succeeded. Only one Rust build ran at a time. Cold release CLI
build cost 10m32s; selected-interval integration rebuilt in 4m32s, and the
collection/store/reference integration rebuilt in 5m53s. Reusing a
qualified native image made the 31-case sweep plus interface refusal 0.63s.
The private observer linked the exact verified CLI's 159-unit fingerprint
closure instead of paying another cold Cargo build.

Independent oracle SHA-256:
`d88a8ecaa3b4a8a2494458af2395cbec93020f30702bc4ca91a02bc1465ba61e`.
The final 31-case oracle was frozen before any Omega sample execution. All
original cases were preserved when independent review added extreme-gap and
tiny-time fixtures, before execution. Decimal Fermat minimization uses 70-digit
arithmetic and 240 derivative bisections, with no reuse of the application’s
cosine formula. Eight independent analytic crossover probes assert winners
below/at/above each crossover. Numerical checking requires exact zero or
relative error <=1e-7 for nonzero outputs, including tiny times. Reported
critical offsets, every branch time and first-arrival IDs are checked.

## Execution and authority

The sandbox initially presented `/` as UID65534. Omega rejected this unrelated
custody ancestor before reaching source checking. Automatic approval review
rejected the first exposed host-route request. The user then explicitly
answered “Authorize the exact host route.” Bounded commands on that route saw
real `/` UID0 and passed the unchanged custody checks. No ownership change,
custody bypass, trust fixture, provider override or approval retry after an
unresolved denial was used.

The user separately authorized this sample’s package-review inventory. The
inspected initial proposal had 287 rows: 3 sample Console permissions,
141 bundled std callables, 141 external supplies and 2 dangerous-capability
source declarations (std FilesystemHost and Console). Acceptance changed only
those exact pending decision tokens. Source inventory acceptance grants no
host filesystem authority; the application reaches Console input, output and
termination only. Existing filesystem/runtime authority checks remain enabled.
Observer and integrated review used the normal manager and required no new
pending decisions. Review’s source rendering hit its resource limit; exact
public source files remained available for the independent source review.

Audited-base smoke ran ordinary `omega update` and `omega run --both` on
`tests/omega/pass/calls/runtime_static_entry_context_exit`, with native and
Terminal exits both 0. Actual sample execution passed 31 native fixtures and
31 captured Terminal fixtures. The private observer retains the accepted
native report’s exact Terminal artifact and runtime provisions, then executes
fresh instances with finite input; it compares actual stdout/stderr and
explicit exits against the frozen oracle. The first normal CLI `--both --keep`
run independently retained Terminal stdout equal byte-for-byte to native
stdout. Exit parity alone was not used as acceptance.

The intended-invalid interface project rejected with exit 1 specifically for
`cannot prove index `3` is within length 3`. Runtime controls observed the
authored 64/65 refusals for nonfinite, malformed, overflowing, missing, surplus,
overlong, nonpositive and unordered records. The sample source gate discovers
both the explicit-entry application and the nested intended-invalid project.
The ordinary native gate passed 1/1 programs and 1/1 intended-invalid controls
in 19s, using exact default-input stdout and exit.
`input.txt`, exact checked stdout and expected exit also qualify ordinary
sample-native discovery.

## Independent review and qualification records

Read-only independent reviewer `/root/independent_review` used explicitly
selected `gpt-6.1-sol` / `medium`. Source review found and resolved cancellation
in minimum velocity gaps and absolute-floor bias in tiny-time ties. Parser
integer decimal numerators now align before subtracting velocities; cosine
uses the stable gap product, and ties use only relative roundoff. The oracle
selects the mathematical minimum independently. Pure Python review reproduced
31 frozen cases and eight crossover triples. Arithmetic/formatter emulation
matched all 12 valid cases; 4,723 admitted velocity-pair challenges had maximum
relative branch arithmetic error 4.97e-16. Final review also covers the actual
integrated native/Terminal value records and source gate.

Final ignored records include commands, exits, timing and stream hashes in
`build/w2-17/integrated-records.json`; binary/artifact identities are in
`integrated-identities.json`; numerical outputs and explicit exits are in
this project’s `build/validation/`. Recovery artifacts retain exact source,
oracle, actual value evidence and the qualification manifest, without
credentials or private policy-lock stores. Normal landing uses tools/landing.py
with an exact candidate receipt, followed by a fresh remote main ancestry
check. The publication receipt and final fetched SHA are reported separately.


The local `.gitattributes` preserves raw `.stdin` bytes, including CRLF;
whitespace exceptions apply only to the two intentionally whitespace-sensitive
input fixtures. All 31 staged input blobs were independently hashed against the
frozen oracle before committing. Omega source remains under canonical LF.
