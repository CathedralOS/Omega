# Selected execution

A product for one target needs two settled facts: which target it is for, and
which provider plans meet the boundary requirements its accepted Terminal
module reaches. [selected_execution.rs](src/selected_execution.rs) holds
exactly those.

`SelectedExecution::settle` is the only constructor. It takes the target, the
supply that [provider settlement](../provider-planning/src/target_supply.rs)
chose for it, and the inputs the plans of that supply are built from (the
accepted module and the schema of each chosen slot). So no product can exist
for a target whose supply was never settled, and a chosen supply whose provider
plan cannot be built refuses there, naming the requirement. Each
plan keeps the provider declaration that joins its checked adapters to the
artifact.

Everything else a product needs is derived where used, from these plans
and the accepted Terminal artifact:

- `SelectedExecution::provider_proposal` joins the settled plans to one
  produced artifact, matching each checked adapter row to the provider
  candidate the artifact names for it. The compiler checks the program's
  target-local scalar conditions against that proposal. A target with no plan
  proposes the empty selection for that artifact; a row for which the artifact
  names no candidate makes the proposal refuse the selection.
- The compiler realizes task activations against `plans()`
  (`execution_settlement.rs`).
- Root trust obligations resolve grants against `plans()` and the trust claims
  of the source census (`CheckedCompilation::admit_trust`).

The record has no program entry, callback roots, native provider declarations,
calling applications or review provenance. An executable product needs those,
and nothing on the lowered route produces them yet, so the compiler refuses
such a product by name instead of reading a retained copy.
