Run `omega run main.omg` from this directory. The authored result is exit 0:
the scalar consumer borrows one complete temporary, and its normal continuation
must retire the two owner-authorized linear fields in reverse declaration order.
The false branch establishes no temporary or obligation.

`omega --check main.omg` succeeds. On macOS ARM64, `omega run main.omg` now
reaches Terminal production but the retiring verifier rejects a claim-bearing
child result with `UnknownStructuralArgument`. Its operation-result argument
policy only admits claim-free affine results. OMEGA-READS-TERMINAL-UNVERIFIED
and TERMINAL-VERIFIER-DELETED own removal of that replay, not an exception for
this program. This fixture stays pending until native and Terminal execution
produce the authored result.

CML4 owns the exact temporary disposition; STATE-LOCAL-VALUE-FRONTIER owns
the shared produced-value frontier. Scalar materialization of moved linear
places and claim-bearing call results is still incomplete. A returned owner
without a unique conserved claim mapping must reject, never receive invented
replacement identities.

The invalid controls require a disposition for the whole anonymous owner,
including siblings outside a borrowed projection. No lowering omission grants
permission to forget an undisposable linear field.
