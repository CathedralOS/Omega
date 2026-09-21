# BUILD-DIRECTORY-ALIAS-COLLISION — verify + record (z142)

Claims-lane name — appears on TASKS.md only as a fence citation holding
`build-evaluation/src/evidence/filesystem_scope.rs` (:8843, expired
23:07Z); there is no marked board row. Resolves to the
BUILD-DIR-HOST-ALIAS-COLLISIONS / BUILD-DIR-ALIAS-AND-RACE-COLLISION-
DETECTION surface: a host alias (symlink) spelling or planted inside the
admission→write window must not alias a build directory past the
filesystem fence.

## Verified at `3dac85e5cc` (linux x86-64)

Landed, all faces:

- `overlap_key` (:85-88) canonicalizes through the host's canonical
  spelling so a symlinked ancestor names the real path; admission keys
  both source root and build dir (:193-217).
- Post-admission refusal (:522-548): an ancestor aliased after admission
  refuses before `create_dir_all` can follow it.
- `ensure_established_write_root` (:608-627): post-creation re-check —
  `symlink_metadata` rejects a symlink occupant and canonical
  resolution drift.
- Captured-source snapshot backing refuses non-directory/symlink
  occupants (:673).

Fresh witness: `cargo nextest run -p build-evaluation --lib alias` —
**7/7 PASS**, including the race-window legs the board named as the
residual: `write_root_establishment_rejects_an_ancestor_alias_planted_
after_admission`, `…_rejects_a_host_alias`, `…_on_an_ancestor`,
`captured_source_snapshot_rejects_a_host_alias_at_its_backing`,
`captured_source_scope_rejects_host_alias_spellings_of_the_roots`,
`build_write_root_rejects_alias_of_a_named_input_snapshot`.

## Fence map at verification

`filesystem_scope.rs` itself is currently claimed by sibling
BUILD-DIRECTORY-HOST-ALIAS-RACE-ISOLATION (z37, exp 12:26Z) — the
in-window detector/coverage lane this name cited. Build-evaluation
`admission/` + `behavior_exclusions` sit under
BUILD-EXCLUSION-REALIZATION (Zergling-193, exp 15:52Z).

## Verdict

**Resolved / record-only.** The alias-collision machinery this lane
name fenced is landed and freshly witnessed, including the post-
admission race-window pins. Residual detector work belongs to the live
RACE-ISOLATION / RACE-COLLISION-DETECTION lanes.

Field note (review 125798220682..90df29812c00): two unmerged lanes still
extend `filesystem_scope.rs` on this surface — `zergling/z154-build-
directory-alias-collision` (write root vs named-input snapshot roots,
`<backing>.input-N`) and `zergling/z176` (materialized write-root re-
verify against race-window aliases). "Landed, all faces" above predates
them; whichever resolves first should re-check the other for overlap.
