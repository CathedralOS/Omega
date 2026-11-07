# W2-14 qualification

This workload exposed general compiler gaps; the sample is their end-to-end
verification vehicle. The candidate was integrated onto main
`cfd9c3d5cfb1c854c112513156a30c5df5104dc1` and its release producer was built
from `0755e2c06ec14907237351525547d4beb88302d0` in an isolated worktree.

## Canonical contracts and failing witnesses

[State contracts](../../../../../wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms)
classifies transition arms by resolved target and requires ordinary argument,
result and completion rules. A bodyless named boundary operation in a direct
arm failed ordinary selection: `machine caller(value: i32) -> i32 {
transition { _ -> probe(value) } }`, with `boundary operator probe(value: i32)
-> i32;`, rejected with `validated named operator application has no checked
selected use`. Qualified `F64::from_i32` variants likewise lost ordinary call
semantics. `transition_calls.rs` rejoins authored operands and exact occurrence
identity to ordinary call checking before result overload selection;
`declarations/operators/uses.rs` no longer confuses a two-component numeric
adapter path with the existence of an ordinary selected use. The symbol resolver
preserves lexical heads when a suffix fails, so a local named `F64` cannot be
laundered into a static namespace. Wrong arity, operand, result, unproved
requires and namespace-shadowing controls still reject.

The natural field variant `machine field(value: &Value) -> f64 { transition {
_ -> F64::from_i32(value.integer) } }` passed checking but failed native product
production with `scalar computation needs one checked expression and one source
binding`. The typed call retained provisional target zero after operand-directed
selection chose the exact checked declaration. `scalar_computations/boundary_calls.rs`
uses that checked identity only for the existing strict namespace-receiver query.
It preserves source IR and custody evidence. The
[boundary call contract](../../../../../wiki/spec/terminal-psi/boundary_calls.md#call-and-requirement-identity)
requires matching ordered argument identities and independent realization;
provider or namespace spelling alone grants no execution authority.

[Ownership](../../../../../wiki/spec/language/ownership.md#borrows-and-aliases)
permits exclusive mutation of established affine referents and requires loans to
denote original storage. A default-initialized or constructed mutable `Record`
local lent to a successor rejected with `terminator: jump successor: loan
transfer`; automatic cleanup-at-return was incorrectly treated as current
retirement. After retaining the live local home, the sample's `InputReader`
containing an established `Binding<Console>` still rejected because a copy/recast
content predicate was used to decide lending. `state_graph/successors.rs` now
retains the current ordered home and applies the existing recursive enrolled
no-reference shape/type checks. It keeps mutable access, multiplicity, nominal
drop and lifetime checks. Binding establishment remains intrinsic under
[authority](../../../../../wiki/spec/resources/authority.md); no host authority
is manufactured. The positive fixture mutates original default and constructor
locals then observes them after a shared successor loan. Duplicate mutable loans
of the same root reject. Actual InputReader execution is covered by every sample
case. These fixes do not alter dispatch-coverage boundaries.

## Actual execution evidence

`reference.py --freeze` fixed independent 27-image Decimal squared-distance
counts and separate high-precision shell volumes before scientific execution.
The precision adversary was independently modeled and frozen before execution,
never fitted to observed output. `data/FROZEN.sha256` verifies all oracle inputs.

The public SDK sponsor compiled one accepted native product, retained its Terminal
payload/provisions and published the same report through normal native custody.
Each Terminal case had its own finite stdin cursor, isolated exit domain and
100000000 fuel. Native subprocesses received the identical frozen stdin.
All **39 cases** passed both engines: **15 valid, 24 refused**. Counts were exact;
max absolute normalized-value error on both engines was
**7.610484419790457347623501064E-11**, below 1e-8. Every stdout/stderr/exit byte
matched between engines. Wrapping, translated and permuted outputs were equal.
Sample capture preparation plus all Terminal executions took 19.247s; maximum
input and exact interior/final edges were included. See capture-receipt.json,
terminal-verified.json, native-verified.json and retained representative stdout.

Integrated corpus: three runtime fixtures (named conversion through parameter,
local and field; affine successor loans; existing result-domain conversion)
completed native and Terminal with exit0. Source corpus: four checked and six
rejected, no missing expected fragments or lost successes. The separate
unchecked_snapshot source control exited1 specifically because the index
`3 * distribution.snapshots * distribution.particles` could not be proved
within length768. Invalid runtime inputs printed exactly `INVALID molecular
input` and exited2. These are actual value and diagnostic checks, not exit parity.

Independent read-only review by /root/review found no remaining code defect and
independently verified all frozen hashes, all39 values/refusals from both engines,
producer/link dependency hashes and source controls. Goldens add only the nine
new source and three new both-engine rows; existing rows remain verbatim.

## Execution and authority

Cloud executor preflight passed omega doctor8 checks and package-free native/
Terminal smoke exit0. mbx doctor had zero failures, one absent Cargo-shim warning.
One Rust build was active at a time: cold audited release build10m20s, subsequent
repair iterations33-60s; integrated-main release rebuild6m38s. The sponsor uses
exact Cargo JSON full metadata plus matching libraries, avoiding repeated Cargo
builds and stale cross-worktree artifacts. The actual native compile frontier was
resolved rather than replaced with host-side numerics.

Normal package update/review admitted exactly finite Console process_input,
process_output and process_termination; reviewed std source-risk declarations
are separate and grant no filesystem or subprocess authority. Compiler identity
changes required normal package refresh. Root-controlled native runtime custody
was checked through the approved executor host route; no ownership check was
weakened. The advisory claim attempt timed out with may_have_published:true;
registration is not represented as confirmed and no other session was blocked.

The current source and evidence are preserved in recovery artifacts. Publication
uses tools/landing.py with an exact-SHA passing affected-package receipt and
remote ancestry verification. Final landing identity is recorded by the normal
landing registry/commit metadata; this document cannot predict its own commit SHA.

Final current-main requalification: producer `5def3ef4c8f1fb78bbadaf204b70dc454dae9cdf` on
base `658da8228c736fba8b460dad4382117a8e30e0d4` repeated all39
native/Terminal cases with the identical maximum error and engine bytes. Both
focused corpus gates were clean (three runtime exits0, six expected rejections).
See final-requalification.json for the rebased producer hashes and receipts.
Its release rebuild took29.06s, Terminal capture18.657s and exact-SHA affected
package check69s. Scoped Clippy completed with pre-existing warnings; strict
Clippy and whole-tree formatting identified baseline warnings/format differences,
with no formatting difference in the touched compiler files.

Later integration on `5b32c4eb36ffa2a6161794b068e9e0282b0bb482` qualified producer
`c5b04a6f69483271b100aceebbe5c7931b56fdb1` after deferred-binding cleanup,
computed-case ordering and library-proof changes. All39 cases again passed
both engines with the same exact counts, error bound and identical bytes; both
focused gates remained clean and the snapshot bounds control rejected correctly.
See latest-integration.json. The broader IEEE conversion and qualified-tail
native regression gate also passed their authored exit70 expectations. The
latest release build took136s, package identity refresh64s, full Terminal capture
19.241s and affected-package check19.58s. A corpus-record merge conflict was
resolved by preserving incoming main records verbatim and appending scoped rows.
