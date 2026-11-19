# Indexed storage-claim issuance

Expected exit: 0 on both native and Terminal execution.

Run `omega run --both samples/cli/memory/indexed_storage_issuer/main.omg`
after ordinary package preparation.

An authorized generic machine issues a ticket indexed by a placement type.
The exact qualification follows the returned ticket through another generic
machine (including a nested consuming call), a record field, and an ordinary postcondition. Free issuers and
issuers attached to generic data share the same declaration-authority rule.
The program checks three independently supplied values after state transfer.

This is a source-authority prerequisite for BUMP-ALLOCATOR-CANARY, not an
allocator or a claim that addresses grant memory access. Actual backing,
initialization, resident cleanup and vacant-extent return remain unfinished.

