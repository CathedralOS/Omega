# Acquired dependency and completed output-set reuse

This is the Linux completed-export admission witness for
**BUILD-SNAPSHOT-OUTPUTS**. The root acquires a generator by `Source::Path` in
both Build and Product contexts. Its Build reads `template.txt` from captured
source and passes only six bytes to the package-local `build_support` helper. The helper produces the
companion payload and a later-stratum source declaration. Both files must seal
and complete before their shared output-set manifest is published. Authored
product source calls the ordinary product helper, not a declaration generated
by its own Build.

The previous direct `use generator::main` in Build relied on the corpus's
retired static dependency projection. Under
[Build declarations](../../../../../wiki/spec/build/declarations.md), requesting
a dependency does not grant ordinary Build imports. The generation and
completed-export obligations here are unchanged; dependency acquisition still
joins both purposes. Explicit cross-package execution is pinned separately by
`explicit_provider_readiness_exit` and its private-backing refusal control.
This fixture does not establish a shared custom generator interface.

`omega run --both main.omg` exits 0 in native and Terminal execution. The
publication oracle requires exactly `companion.txt` and `generated.omg` under
the committed set's `files/` directory. The Linux host workflow is exercised by
`python3 tests/product/completed_outputs/check.py --omega <fresh-omega>`:
valid repeated publication succeeds; interrupted, incomplete, extra, altered,
executable and linked prior exports reject without a successful publication
report. Restoring the complete export permits a retry. The existing
`fail/build/acquired_generator_output_final_check_refused` control separately
establishes that later generated-source failure exports no set.

This discharges admission of earlier host exports against retained manifest and
file content. Atomic source capture, staging isolation against another process
with the same authority, and Windows/macOS isolation qualification remain open;
this Linux witness makes no claim about those premises.
