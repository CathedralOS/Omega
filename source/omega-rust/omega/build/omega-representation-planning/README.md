# Omega representation planning

This crate is the compiler-owned entrance for provider-backed opaque value
representations.

`src/lib.rs` harvests `Build::select_representation<Opaque, Conformance>()`
from the authoritative build machine, closes the exact named conformance, and
validates its compiler-owned trait, opaque subject, and concrete carrier. It
does not accept byte sizes, alignments, ABI classes, or movement rules from
source.

D44 requires every v1 selection to carry the explicit role-tagged lifecycle
disposition `Inert`. Admission must derive that disposition from the complete
closed carrier graph: every field, array element type, and sum payload must be
ABI-movable without independently invoked nominal cleanup, nested live linear
debt, or an unjoined opaque/external discharge. A direct no-`drop` test or a
provider assertion is insufficient. An invalid explicit selection rejects even
when unused. Cleanup-owning carriers belong to a future separate, versioned
lifecycle relationship and are not an interpretation of this empty trait.

D26 scopes uniqueness to the compilation activation: one opaque declaration
has at most one selected application, including an unused selection. Current
orchestration evaluates one authoritative build machine, so the local harvest
implements that invariant. `CheckedCompilation` retains the complete validated
collection unchanged, including unused selections, so downstream work need not
rediscover an application from syntax or a calling-plan digest; completed
build-configuration validation must keep it fail-closed if orchestration later
admits more than one. This custody is not a package-review availability row,
consumer demand, or physical ABI commitment. Dependency builds publish
generated-source bundles and their selections are not imported as consumer
policy.

Downstream target consumers derive physical shape from the retained carrier.
Calling-policy closure and general target layout now consume the same exact
selection. General layout substitutes the carrier only while deriving an
opaque value's physical size and alignment; references remain pointer-shaped
without descending into the opaque, and the semantic type remains the opaque
declaration. A direct by-value layout demand without a selected application
rejects. Package review, artifact compatibility, and replacement contracts
must consume the same application rather than re-discovering a carrier.
Package review projects the producer's available declaration/conformance/
carrier surface separately from a consumer demand created by an actual
by-value use. The selecting build source is provenance rather than ABI
identity; independently compiled artifacts compare strong application
commitments at their real by-value composition edges.

`CheckedCompilation` now retains every exact validated boundary calling-plan
realization. Its materialized signature exposes the compiler-derived
`BoundaryOpaqueRepresentationUse` rows for actual by-value crossings, while an
unused selection remains absent from that use list. Package review now rejoins
public producer candidates to canonical opaque, conformance, and carrier
identities without selecting them. Consumer demand still requires the complete
D26/D44 movement and inert-finalization commitment plus the boundary-plan join.
General size/alignment substitution alone is deliberately not published as
demand evidence.
