# CLI admission-policy custody

[admission_policy.rs](src/admission_policy.rs) owns reading and explicitly
replacing `omega.admissions`, including format validation.
The independent [trust-model](../../realization/trust-model/README.md) owns filesystem-free
obligation/report construction. This is separate from the
[package lock](../../../../wiki/spec/packages/locks.md), which records source
pins and project package acceptance.

Compile and run commands use this package to load project policy. It remains an
independent library, not part of compiler pipeline execution. Command orchestration
alone opts into replacement through `--accept-admissions`.

The compiler consumes one complete in-memory admission set, reconstructs required
obligations, and reports consumed, unresolved, and unused rows. Ordinary checking
does not approve missing policy. Only explicit `--accept-admissions` replaces
standalone compiler policy with the exact reconstructed set; it cannot overwrite
package pins. Trust reports are diagnostics, not policy inputs.

Domain names and unmatched strings cannot become accepted facts or compact-hash
receipts. Exact selected-provider grants retain their own semantics. The temporary
standalone accepted-machine selector route is not package claim acceptance:
package-aware compilation requires comparison of the complete claim inventory.

An old compiler-policy file stored at `omega.lock` requires explicit relocation
to `omega.admissions`. A package-format lock must never be renamed or interpreted
as compiler policy. No extra install/update audit or certification step follows
from keeping the file owners distinct.
