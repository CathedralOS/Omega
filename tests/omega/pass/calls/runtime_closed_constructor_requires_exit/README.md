Closed constructor calls now share the existing structural body evaluator.
The eight native and Terminal checks cover expression and transition returns,
parameter and local substitutions, a retained caller local, an exact stored
field, a copied field value that survives an overwrite, and a namespaced call
projection. The authored output is `constructor checks=8` and exit is 0.

Computation is bounded to exact selected, pure, observation-free, crash-free
entry calls with no Requires and complete closed integer/constructor values.
The ordinary call-formation and termination obligations remain required.
Symbolic unequal forms remain unknown. IEEE values, reference descriptors,
and overloaded relations acquire no identity-equality authority.

Runtime prefixes first try the independently established current state's
Requires and executed prefix; acyclic incoming-path proof remains available
as a fallback. Exact stores capture their RHS once and authenticate the
canonical declaration destination and complete direct write frame. Only
ordinary static names/fields with reference-free destinations and
value-preserving operands are admitted. Recasts, ambiguous aliases, dynamic
selectors and unknown writes decline; later writes retire live store facts.
Earlier detached copies keep their own completed identities.

The fail/calls/closed_constructor_requires_* controls retain refusal coverage
for symbolic inputs, overwritten storage, alias writes, stale entry facts and
nested restricted calls. closed_constructor_store_recast_not_identity checks
that an unsigned backing coordinate cannot authorize a signed value premise.

Closed computation evaluates unused local initializers and every constructed
field before selecting a result or projection. Divergent or unformed work
therefore remains unknown and retains its ordinary termination refusal.
Completed scalar facts move between stored places only when their declared
referent carriers agree; a reference recast supplies no value equality.
