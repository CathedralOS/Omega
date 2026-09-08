# Native products and component publication

[Terminal Psi](../terminal-psi/product.md) is the portable compilation boundary.
This contract distinguishes compilation products from installed component
publication. Writing artifact bytes for later consumption does not establish a
running installation. The deployment requirements below apply to the installed
component claim, not to ordinary artifact serialization. Platform container layouts
are separate, and specified deployment routes are not necessarily implemented.

## Products and authority

| Product | Contains | Does not establish |
| --- | --- | --- |
| Canonical Terminal artifact | Exact semantic, proof, optional debug, and reconstructed manifest sections. | Native target, installed provider, or publication authority. |
| Native artifact | Verified Terminal product, exact selected provider closure/executions, target realization, object/image, and replay evidence. | Output path, installed occurrence, progress receipt, or `InstalledCode` custody. |
| Component candidate | The same native artifact plus exact source-selected provider facts and pending component-progress manifest. | A second lowering route or runnable installation. |
| Installed runnable | Replayed installation and the real installed-code/provider/progress custody for a live component era. | A new installation merely by recompilation. |
| Publication receipt | Exact installation/image/path relationship and required file-mode validation. | Authority inferred from a filename or compact report identifier. |

A source-free native consumer accepts canonical Terminal plus explicit realization
inputs, not checked/typed/source representations. It verifies, lowers, emits,
and replays exact bytes. Unresolved/duplicate settlements, executions outside
the selected requirement closure, target substitution, and object/image drift
reject before a product exists.

Realization retains the exact Terminal authority policy/review and the application
and physical-child evidence required by the
[boundary contract](../terminal-psi/boundary_calls.md#operator-applications-and-physical-children).

A direct native request cannot silently discard pending component progress.
An authority-distinct dynamic ELF request with an exact normalized interpreter
retains its complete Terminal/object/selection/evidence/image relationship in a
separate non-installable product. Producing it grants no loader-policy,
installation, publication, or execution admission.

Opaque callback companions remain exact by-value custody on success and rejection.
Retaining or validating them does not infer source origin, registration,
invocation, address, or lifetime authority. Interpreting an admitted callback
requires its separate exact source/realization join.

## Installed provider and progress closure

Installation seals the complete selected provider-plan set to exact installed
occurrences and a domain-separated strong digest. Include selected plans with no
execution in this image. Compact `u64` summaries are report coordinates, not
identity sufficient for acceptance.

A `ProgressProfile` receipt is admissible only when its exact issuer occurrence
realizes an owner-authorized boundary route and the receipt qualifies its exact
subject occurrence. Issuer and subject need not be the same occurrence.

Component closure checks every pending row and replays the manifest's own
domain-separated digest. Retain the original manifest and exact evidence;
compact-equal but structurally different inputs reject.

External-root summaries likewise retain exact validated roots, boundary/resource
columns, provider exit assurance, installed occurrences, and the strong selected
closure digest. Compact root-policy or execution-summary equality grants no
authority.

## Deployment transaction

Deployment receives the candidate and independently acquired installed code,
provider-occurrence bindings, progress attestations, and profile decision.
Compilation cannot supply substitutes for those authorities.

The transaction proceeds through installation claim, provider closure, progress
closure, canonical installation finalization, and publication. Failure retains
the exact current deployment carrier and every unconsumed later input. A one-shot
registry claim must not become repeatable because a later step failed.

Runnable binding joins the complete Terminal object/image, canonical installation,
real linear `InstalledCode` claim, and opaque acceptance. It compares the selected
provider closure even when no progress rows exist. The live era retains the
registry and runnable custody; successful retirement alone releases them.
Binding or retirement failure preserves exact custody.

## Visible component publication

The output owner consumes a deployment-finalized runnable, not selected plans or
compiler trust labels as a substitute. It derives the requested destination from
the build output and sealed image filename, then delegates consuming publication.

Publishing an installed component as a flat executable replays the
installation/image relation, stages exact sealed bytes and executable mode,
validates before atomic rename, and
replays the visible file before reporting success. Rejection returns the runnable
and requested path for retry. Receipt replay detects later byte or mode drift.

Flat installation v1 retains its fixed `0` destination byte under the existing
digest domain. No destination enum or optional second-copy receipt is required.
Whole [macOS packages](macos_application.md) have their own complete scope;
they cannot reinterpret the old executable-copy receipt. Flat installation,
output-kind, and general report validation remain required independently.

Reports retain the non-clonable published carrier, permitting borrowed inspection,
validated path projection, or consuming transfer to the next owner. They must
not reduce a failed linear transaction to diagnostics while dropping its custody.

Container assembly, metadata, signing, and other platform publication requirements
need their own exact artifact scope. An executable receipt alone does not certify
an application package.
