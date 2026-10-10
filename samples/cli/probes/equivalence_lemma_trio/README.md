# equivalence lemma trio

Compile-scope probe. No standard-library dependency.

- case_call_premises: `requires v in T::Case` call premises +
  ensures-position calls discharged on literals, gated symbolic
  subjects, and `requires v == alias` aliases.
- equivalence_lemma_trio: `proposition equivalent(a,b) = a == b`
  definitional body + refl/sym/trans lemma triple.
- match_domain_results: every match arm `as`-grants into a bare
  domain; the match types `i64 in Km` and feeds a domain result.
- interleaved_match_locals: match moves two of three interleaved
  locals; the one declared between them survives.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/equivalence_lemma_trio/main.omg
mbx run -p omega -- update --project samples/cli/probes/equivalence_lemma_trio
```
