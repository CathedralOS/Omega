# Outputs and drivers triage receipts

In progress. This packet gives each claim in the [outputs-and-drivers boundary note](../../boundaries/outputs_and_drivers.md) a disposition at the current source. Rows name the original handoff section and item (for example H1.2), the code path, the contract or test, and the outcome.

Evidence baseline: `24ad2700685e6cc6062026c87ed5fd24ed415265`. The survey read `4f3c255d0559`. Path shorthands follow the boundary note (`app/`, `cmp/`, `build/`, `pm/`, `int/`, `drv/`, `S2P/`, `con/`).

## Receipts

| Claim | Current code | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- |
| H1.2, first bullet: the CLI calls `.expect("an acquired target lease opens a Product")` on the plan set. | `app/commands/compile.rs:157-170` | `active` holds exactly the profiles whose lease succeeded. `open_products` runs whenever `active` is nonempty. Each row applies `lease.as_ref().map_err(output_error)?` before the `expect`, so a failed lease returns its error first and the `expect` runs only when `plans` is `Some`. `plans.select(profile)?` returns an error for a profile without a plan. | Intended |
