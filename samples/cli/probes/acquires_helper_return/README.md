# acquires_helper_return

A helper machine ACQUIRES fresh host authority at the `Desktop`
boundary and hands it onward through its capability-typed return;
`Backup::stage` never touches the boundary — authority propagates
`acquires` up the call graph with the helper recorded as provenance
(`Backup::stage acquires via Vault::pick`).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
