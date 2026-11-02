# Reflection completion qualification

The scoped Linux x86-64 observation is source commit
`f0eff27c64cd8025b87fc82d75914cd03bdde4f4`, based on
`bc2c778d57d90b75587d484a6a882ad2cecac9a4`. The exact source is retained at
`refs/corpus-observations/f0eff27c64cd8025b87fc82d75914cd03bdde4f4`.
The PR links the separately retained source bundle and raw run reports. In a
clone containing the base, restore the bundle with `git bundle unbundle FILE`.

Both producers use pinned nightly-2026-09-04 and opt-level 1. The candidate was
built before the source commit from the identical compiler/SDK bytes; its Git
label is not relabeled as a new build. The unpublished commit metadata was
rewritten to the authenticated public email; the source tree is identical. SHA-256:

- CLI: `7c14d1b622f17dd34adb3dcc7855376316fefbca0fdd6fee1444a6a497453544`
- Corpus runner: `4f38fe2ab62e25153875bf8147424f842377469f57ec4ccbc3a045b3678f4250`
- Baseline CLI: `f4731503d8e2dc240091d181ac75560accfda2c0e1fd15dc11b65f6a30099b5b`

`sample_native_gate.py` observes the nine transport places and four positive
controls at exit 0 in both engines. Nine exclusion twins plus foreign-authority,
loan-escape, open-generic exclusion and Unit-value controls reject their intended
obligations. A matching-u32 callback body checks; changing the first expected
runtime result from 2 to 3 traps natively (signal 4) and in Terminal (Trap).
The valid terminal conditional also refuses on the retained baseline. Earlier
Unit-valued-match authoring and compiler-identity/cache setup failures retain
their failed observations; they are not described as successful execution.

The canonical prefix `typed_reflection_mutation_completion_` records 26 source
outcomes and 13 native/Terminal outcomes in the maintained corpus goldens.
Six existing statement/ordinary-callback anchors meet their own oracles; four
have no baseline golden row, so that comparison reports additions. Their source
and goldens are unchanged here. Strict Clippy stops on existing dependency and
validation lints; optimized builds, formatting and architecture checks succeed.
Raw reports remain under `build/advance/` in the retained workbench. This is a
bounded reflection repair, not other-host qualification or closure of the full
`WIRE-CODEC-LIBRARY` outcome.
