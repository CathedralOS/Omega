# Live storage access

`ResourceProfileGrant` binds a profile to a live extent's address space, root,
lineage, provenance, era and rights. `AdmittedResourceProfile` retains that
authority and restricts it to an actual loan. Failure returns the unconsumed
grant. Its constructors and private fields are unchanged by the move.

`AccessAuthorization` checks an operation against an admitted field's permissions,
observation model, current borrow and source loan. The resulting
`AuthorizedFieldAccess` keeps private fields. Region access first asks
`access-admission` to check the requested geometry, then applies the same live
operation checks.

Descriptive access records and static placement checks remain separate. A
resource receipt ID describes a receipt; it does not issue one. Compiler
consumers of those records no longer depend on live extent authority.
