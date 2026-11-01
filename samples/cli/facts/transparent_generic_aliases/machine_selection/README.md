# Callable obligations through transparent aliases

The imported `Routed<machine Selected> = Erased<Selected>` alias forwards its declaration-owned callable parameter to another alias. Both require the ordinary u64-to-u64 callable contract and expand to `u64`. A generic machine interchanges those aliases with explicit `u64` and invokes its selected callback; two same-shaped declarations return 42 and 17. Alias transparency does not make these erased applications distinct runtime types.

The controls change only an erased alias's supplied callback to a u32-to-u32 declaration or remove Routed's authored callable contract. No finite-layout data is indexed by code: the language restricts machine-indexed data to proof-only families.

The first retained Mac baseline exposed a parser default that preinstalled the trait-only identity contract on data/alias binders. The first changed Linux producer passed that boundary, then exposed the missing alias-header contract lookup. Its initial finite-data witness also violated the ratified proof-only restriction; that false report and source are retained, and the witness was corrected to preserve the rule. Execution of the corrected witness is pending. Full alias/nine-place/Product and other-host acceptance remain open.
