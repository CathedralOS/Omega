This authored result remains required. The old compiler ran this program by
rewriting typed source with native layout before checking. That route is removed:
it made the common Psi program depend on a selected target (or the compiler host).

The current `unimplemented:` diagnostic names missing portable Terminal layout
queries. Restore this through the selected-layout consumer and explicit target
custody, not another source fold. Both the constant and runtime query must work;
the authored native exit remains zero.
