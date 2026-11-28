# Handoff: explicit identity (PROOF-KERNEL-CORE, TASKS.md)

## Done (merged to main)
- #6610 psi: read equality evidence through an implementation parameter at a closed call
- #6613 docs: owner question on integer `==` giving Identity (answered by Jarod in #6620: yes)
- #6624 psi: prove record identity from a true same-type integer comparison (integer bridge, both directions)

## In flight
Branch `claude/project-thread-jgz49a`, pushed, **no PR opened yet**. Head commit:
"psi: synthesize equality evidence laws and prove them from field evidence" (merged with main at 7f2f66fc+).
It is complete and verified; only the PR is left.

What it does:
- Stage 02 `declarations/trait_defaults.rs`: an `X satisfies EqualImpliesIdentity<T> { }` /
  `IdentityImpliesEqual<T>` block that authors no `equals` gets the synthesized structural
  `equals` (marked `reaches none crashes none terminates`) and each missing law as a
  `ConformanceMember::Synthesized` with an empty body (`synthesized_evidence_law`).
- Stage 03 `expressions/equality_preparation.rs`: `refuse_ieee_evidence` refuses float fields
  (directly or nested through synthesized comparisons). `has_new_synthesized` covers evidence.
- Stage 05 `read/clause_terms.rs` `synthesized_comparison` + `read/clause_calls.rs`
  `unfold_synthesized_comparison`: a clause call to a synthesized `equals` row (direct,
  receiver `CoinEq::equals`, sibling/witness requirement, or `E::` dispatch) is read as its
  body (field comparisons). `read/contracts.rs` `copied_requirement`: Identity in a copied
  requirement signature.
- Stage 06 `lib.rs`: same-type integer / Boolean comparison goals (or conjunctions) left open
  by arithmetic are retried via `equality::discharge` when an Identity is known
  (`Discharged::Identities`). `proofs/equality.rs` `paired`: pair equality gives components.
- Tests: 6 new in `pipeline/06_lowered-program-to-checked-program/tests/explicit_identity.rs`
  (`synthesized` helper). READMEs for stages 05/06, TASKS.md updated.

## Next steps
1. Delete this HANDOFF file from the branch (`git rm HANDOFF-explicit-identity.md`, commit, push).
2. Open the PR from `claude/project-thread-jgz49a` to main. Title (73 chars):
   `psi: synthesize equality evidence laws and prove them from field evidence`.
   Body draft in project files `handoff/explicit-identity-pr-body.md`. Validate with
   `mbx xtask check-rules --base origin/main --title "$T" --body-file F`.
3. Squash-merge after the `admission` check passes (full 40-char expectedHeadSha).
4. Remaining TASKS items (in the explicit identity bullet): replay the integer bridge in the
   receiving verifier; cover plain `i`/`u`; payload-less sum fields in clauses (refused as
   "data value compared whole in a clause"); a generic template's own check over `E::equals`
   (refused "clause application has no record in scope", pre-existing; only copies are judged).
   Any wire-schema/spec change needs Jarod's approval via OWNER_QUESTIONS (no wiki/spec edits).

## Commands (run from source-rust/, always `< /dev/null`)
- `mbx test -p source-compiler --features test-support --test checking_explicit_identity` (47 pass)
- `mbx test --no-fail-fast -p typed-trees -p symbol-resolved-trees -p typed-trees-to-prepared-typed-trees -p prepared-typed-trees-to-lowered-program -p lowered-program-to-checked-program -p syntax-trees-to-symbol-resolved-trees -p symbol-resolved-trees-to-typed-trees`
- `mbx test -p source-compiler --features test-support --no-fail-fast`
- `mbx clippy --all-targets -p <crates>`; `mbx xtask fmt --check --base origin/main`

## Gotchas
- Pre-existing failures on main (not ours): 5 `provider_choices` tests (stale fixtures) and
  `tests::type_parameters::generic_fields_form_their_qualifications_without_assuming_the_predicate`.
- Never run `rustfmt` on a lib.rs/mod file: it reformats every child module. Use
  `mbx xtask fmt --check --base origin/main` and fix only the reported lines.
- PR body needs the attribution header, Gate/Spec/Proof/Agent lines, and footer; titles carry a
  lane prefix (`psi:`/`docs:`) and stay within 85 characters.
- Force push is denied; after a merge, `git checkout -B claude/project-thread-jgz49a origin/main`.
