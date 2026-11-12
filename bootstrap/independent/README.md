# Standalone-language construction experiment

This peer explores successive languages whose implementations become standalone
executables, rather than carrying the existing interpreter stack through every
successor. Early executables may target Alpha; native emission and stage count
are engineering choices to measure against complete construction cost.

Implementation has not started. The first implementation milestone must compile
an actual successor compiler, not stop at a language demo. Use the existing Gamma
compiler source in [`../compiled_vm/`](../compiled_vm/README.md) as the first
concrete customer when choosing facilities and an output representation. A
source recognizer or precomputed compiler output does not satisfy that milestone.

Start with an explicitly documented seed. The existing audited Alpha floor and
admitted Beta tape are available for reuse from [`../interpreted/`](../interpreted/README.md);
reusing them must be disclosed, and does not require retaining later stages.
Any additional trusted host compiler is an accelerator or a different declared
trust profile, rather than an implicit part of cold construction.

Carry a viable route through the bootstrap compiler, construction of the
Omega-written compiler, and reproducible self-rebuild. Keep the product source
outside this directory. It may also be developed through Rust, with those results
identified separately from this construction route.

Additional language facilities must reduce successor implementation cost enough
to earn their audit burden. Record actual source/artifact sizes, execution and
memory costs, safety boundaries and failure evidence. Neither this experiment
nor the existing paths are designated selected.
