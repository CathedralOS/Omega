# RepresentationSpecialization across ordinary leaf copies, selected leg

`build.omg` authors `Optimization::RepresentationSpecialization` by exact
name.

`main.omg` mints three ordinary-path `StructuralLeafCopy` results over one
`EstablishRecord` chain: `copied` copies `whole`'s `inner` field,
`copied_lease` copies `copied`'s `lease` field (a copy of a copy), and
`deep_lease` copies `whole` at the two-segment `inner.lease` projection.
Each copy's ordinary projection spells source-level field identities; the
specializer resolves each name to the reached roster's verifier
`StructuralFieldId`, prefixes the canonicalized projection to the path
still to resolve, and lands on `whole`'s establishment, where `inner` is
stored whole and `inner.lease` is stored whole as `Lease::Live`. The
`copied_lease in Lease::Live` and `deep_lease in Lease::Live` memberships
then fold to `BooleanConstant(true)` and the `copied.level` field read
folds to `IntegerConstant(7)` -- the ordinary-path leaf-copy crossing both
specialization families share.

This fixture carries no `rollback.txt`. `RepresentationSpecialization`
executes in the post-Terminal abstract-operations stage, so the check
leg's Terminal-artifact pair cannot observe its folds: the unrolled and
rolled-back modules are byte-identical regardless of what the pass
rewrote, and a `module:same` row on a selected-rule fixture only restates
that the overlay proved nothing.

On the native leg the authored program exits `0`: every folded observation
returned the authored value, so the transition takes `done`. The `bad`
arm is a `crash Trap`, so a projection resolved to the wrong roster field
-- `flag` for `lease`, `lease` for `level` -- cannot pass.
