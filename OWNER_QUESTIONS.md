# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`.

Last pruned: 2026-07-20.

## Authored ProviderPlan spelling (the P4a-flip / P4b gate; 2026-07-20)

The PRV4 end state moves the built-in host lowerings into "ordinary
omega::core/std target packages" selected by build.omg -- but the ruling
does not specify the AUTHORED spelling for plan rows that today only exist
in the Rust populate tables: lowering SEQUENCES (write_line = get_std_handle
then write_file) with call-shaping policy (first_text_argument+newline,
single_byte_read, constant_result:N). The retiring `provides` grammar's
closed Binding sum cannot spell either. Questions:

1. The row spelling for an operation SEQUENCE + call shape (e.g.
   `write_line -> [Stdout::get_std_handle, Stdout::write_file] shaped
   first_text_argument+newline`?), and whether plans are one block per
   (target, trait) like provides or one plan VALUE with a name.
2. The slot-owner SELECTION spelling in build.omg: the target-default set
   ("this target uses std's default plan set") + per-slot overrides
   ("but console binds plan X").
3. Whether the interim Rust-constructed plan values (the lossless oracle's
   derivation) should ship as the merge source BEFORE the authored surface
   exists, or the flip waits for authoring (today's tables stay the source
   either way -- the oracle guarantees equivalence when the flip happens).

Until ruled: P4a's plans-as-source flip and P4b's filesystem re-authoring
hold; P4d (keyword retirement) proceeded -- platform blocks are retired
(boundary traits are the surface) and the provides retirement follows the
same path once plans can spell what provides spells today.
