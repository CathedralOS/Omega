# Validation protocol and evidence

The application changes only its assigned namespace. Its initial base was
b4008bb3c7894aebad47eaccf8af12bb4a8006b1. No compiler or standard-library
source was authored. The complete initial qualification used main producer
0e7af1e5722c8ff56db5a5c1824610ce99c444b2 and binary SHA-256
f0ca1c145894905944e873820362f8fc25b66a852ee59c8d4c55725868d63935.
Integration uses a separately built compiler from main
e259521ddc2d5d7d0c5fbf026ee6acf6d8613e7e; the external review bundle records
its identity and actual integration outcomes without circular source hashes.

Target: linux_x86_64. Executor: Linux x86-64, UID 1000, 5 visible logical CPUs,
a 4-core cgroup quota, 16 GiB cgroup memory limit, build jobs capped at 4.
Toolchain: nightly-2026-09-04. Build command:
`CARGO_BUILD_FINGERPRINT=content CARGO_UNSTABLE_CHECKSUM_FRESHNESS=true mbx build --locked -p omega --release -j 4`.
One Rust build and one sequential Omega operation run at a time per executor.

The default sandbox exposed synthetic root ownership, causing cache custody
rejection. Supported per-command escalated execution retained UID 1000 and saw
actual root ownership. No ownership, permissions or custody guards changed.
The initial blanket package-acceptance request was automatically denied before
execution. After new explicit authorization of the exact complete baseline,
one fresh original-project retry succeeded. That accepted transaction remains
archived. Subsequent normal identity refreshes have no new decision meanings;
complete framed payload comparison covers all 285 accepted rows. Evidence
locks and proposals are never transplanted into a consumer project.

All handwritten expectations and tolerances were frozen before Omega execution.
`data/FROZEN.sha256` SHA-256 is
e20f04440957dd37437d66861f25f2447923a352ca920813024f595645e9ab7c.
The independent reference confirms 12 hand-counted rational histories and never
creates expected output. `validate.py` verifies the freeze and checks all 26
histories against complete stdout, empty stderr and exact exit status. Tolerance
is zero. The initial complete qualification passed source check, native compile,
26/26 native cases and 26/26 Native/Terminal termination comparisons without
unsupported cases. The native executable SHA-256 was
49e23c90fe5262fd0258e838b83814ead463c61e19e17c4179247e262b03adfa.

`omega run --both` compares process termination. It does not establish equality
of Terminal stdout or a durable serialized Terminal artifact. Complete native
stdout is independently checked against the frozen oracle. Kept native artifacts,
comparison logs and exact route status are recorded in the review bundle.

Integration qualification was declared before execution: source check, native
compile, all 26 exact native cases, fresh separate bounds rejection and 12
Native/Terminal comparisons selected from the changed compiler inputs. Selected
cases: nested, capacity, constant, minimum, byte_boundary, zero_curve,
bad_character, byte_limit, integer_overflow, linear, monotone and descending.
This supplements the original complete 26-case comparison.

The complete intended-invalid program has its own Build entry and no external
package. Require exit 1 and the sole diagnostic
``cannot prove index `2` is within length 2``. A package, CLI, type or custody
failure does not pass. The initial qualification met this exact requirement.

Read-only independent review checked all frozen bytes, hand oracles, bounds and
existing sample overlap; compared stack reduction against independent list
reduction on 349,520 synthetic histories; and verified actual native outputs and
the exact invalid diagnostic. Final review is recorded with the final candidate.

The supported Library review bundle contains the final source, source hashes,
patch and Git bundle, complete commands and logs, compiler/source/resource
identities, all runtime outputs, accepted-row continuity and publication receipt.
Its `evidence/OUTCOME.md` is authoritative for final candidate and publication
state. Runtime outputs are evidence, never source inputs. Consumer materialization
and archive member hashes are checked independently of local path availability.
