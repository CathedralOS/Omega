# Case rank through a stored reference

Owner: `TASKS.md` — **NESTED-MACHINE-UNIFICATION**. The fact and operation remain:

> A private helper preserves the invocation rank on a tail edge, and every complete cycle strictly descends while advancing the explicit counter once.

`Remaining` holds an explicit mutable reference to `Payload::Pending`. Its
independent case requirement licenses `count`; the helper's positive-count
requirement licenses exact subtraction. Updating the separate counter must not
lose the still-live count observation. The seven-step path advances 3 to 10;
the empty path leaves 10 unchanged. Completion checks the original backing,
payload guard 827 and carrier canary 641. Misreading the tag or pointer,
skipping a decrement, or losing/repeating a counter write traps.

The same production is present at all nine places under
`<place>/case_value/stored_reference/`: local, field, parameter, call result,
match join, const name, nested machine (`other_state`), another machine, and
another target body. Each `invalid/main.omg` changes only the decrement from
one to zero, and must reject the complete-cycle strict-descent obligation.
Call-result and other-machine places consume their producer's public tag and
bound guarantee. All positives bind all four hosted targets.

`compositions/alias_child` mutates through an aggregate alias;
`compositions/scalar_alias` mutates through an explicit scalar borrow.
`compositions/ordered_slot_read` replaces a descriptor in an earlier argument,
then observes its new backing, writes it through a callee, and checks a detached
old scalar snapshot. `compositions/copied_descriptor` reseats the original
reference while its copy retains the original backing. Controls reject stale
bounds after reseating, overlap through scalar and aggregate aliases, missing
or wrong cases, unformed subtraction, and cycles without strict descent.

## Requirements and evidence

- Establishment: independently formed invocation requirements establish the tag
  and bound. A partial operand cannot justify its own formation. Nesting adds
  no ambient premises; each arrival proves its actual contract.
- Identity and lifetime: the frame owner retains the exact loaded slot and
  backing dependencies at the original entry or ordered read cut. Scalar
  observations join only through singleton typed, value-preserving coordinates,
  with completed selectors. Writes retire overlapping live observations;
  detached entry/saved values remain snapshots and cannot revive storage facts.
  Descriptor metadata alone supplies no contents, access or lifetime evidence.
- Composition: every place carries the same bound and operation. Aggregate and
  scalar aliases use the same typed backing join. Earlier operand effects
  supersede old descriptor aliases; a copied descriptor has independent binding
  identity. Every recurrent edge participates in strict-cycle checking.
- Soundness: scalar meaning selection is separate from observation authority.
  Exact arithmetic requires observed operands before adding their bare-carrier
  bounds. Unknown origins and unmatched further reference loads remain unknown.
  Formation, effects, loans, case classification and termination stay separate.
- Psi 05–07: source checking preserves the active payload and explicit reference
  crossing into canonical Terminal Psi; both interpretable counter paths and
  all canary/writeback checks use the original program.
- Omega 00–02: ordinary variant layout, explicit reference carriers and calling
  plans serve all four authored hosted entry bindings. Native realization loads
  the actual descriptor; possible origins never choose a runtime address.
- Omega 03–06: the 100,000-cycle variant exercises live caller backing under an
  actual 256 KiB native stack limit. This is a physical host observation, not
  portable composed WCSU certification.
- Omega 07–09: emitted images execute the same observable counter, tag and
  canary falsifiers and agree with Terminal Psi on macOS ARM64. Other native
  hosts remain unmeasured; their entry bindings alone are not runtime evidence.

The fixed base is `10bd3f15626b6781d30f1d1ac52058c610895fde`: all nine unchanged
positives refuse exact subtraction after the disjoint counter update. Removing
only that update makes the source check; that diagnostic reduction is not a
passing runtime witness. Exact producer revisions, source hashes, diagnostics,
locks, raw engine observations and the physical image are retained in the
session qualification and publication evidence under `build/borrowed-case/`.
Positives and strict twins are pinned under
`tests/omega/{pass,fail}/facts/nested_machine_joint_rank_stored_case_*`;
composition and safety fixtures use `nested_machine_stored_case_*`.

Remaining parent acceptance includes additional independently captured
reference/index loads, declared-view/dependent joint measures, portable WCSU,
complete effect/custody composition, linked application migration and the
Omega-written compiler. This row does not close the parent outcome.
