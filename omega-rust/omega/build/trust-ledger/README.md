# Standalone compiler admission custody

[src/lib.rs](src/lib.rs) owns coordinator-facing `omega.admissions` custody.
The sibling [trust-model](../trust-model/src/lib.rs) owns filesystem-free
obligation/report construction. This is separate from the
[package lock](../../../../wiki/spec/packages/locks.md), which records source
pins and project package acceptance.

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
