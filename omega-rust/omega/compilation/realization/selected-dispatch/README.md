# Selected execution

A product of one target reads two settled things: which target it is for and
which provider plans supply the boundary requirements its accepted Terminal
module reaches. [selected_execution.rs](src/selected_execution.rs) holds exactly
those.

`SelectedExecution::settle` is its only constructor. It takes the target, the
supply that [provider settlement](../provider-planning/src/target_supply.rs)
chose for it and what the plans of that supply are formed from (the accepted
module and the schema of each chosen slot), so no product can be formed for a
target whose supply was never settled, and a chosen supply whose provider plan
cannot be formed refuses there by requirement. Each plan keeps the provider
declaration its checked adapters are joined to the artifact by.

Everything else a product needs is derived where it is used, from these plans
and the accepted Terminal artifact:

- `SelectedExecution::provider_proposal` joins the settled plans to one
  produced artifact: each checked adapter row to the provider candidate the
  artifact states for it. The compiler checks the program's target-local
  scalar conditions against that proposal. A target with no plan proposes the
  empty selection of that artifact; a row the artifact states no candidate for
  refuses the selection inside the proposal.
- Task activations are realized against `plans()` by the compiler
  (`execution_settlement.rs`).
- Root trust obligations resolve grants against `plans()` and the trust claims
  of the source census (`CheckedCompilation::admit_trust`).

The record carries no program entry, callback roots, native provider
declarations, calling applications or review provenance. An executable product
needs those; nothing on the lowered route produces them yet, so the compiler
refuses such a product by name instead of reading a retained copy.
