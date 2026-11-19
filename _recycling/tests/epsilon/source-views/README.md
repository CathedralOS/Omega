# Epsilon compiler source windows

Run `sh tests/epsilon/source-views/run.sh`. The entrance materializes the exact
compiler and control closures through the selected Delta/Gamma route;
`gate.py` frames inputs and compares observations against authored expectations.
It does not parse Epsilon or infer language judgments.

The small Delta entrance is `controls/main.delta`. It selects checking routes
in `checking.delta` and direct factory/accessor controls in `extents.delta`.
`support.delta` owns only private framing, byte copying, and observation coding.
The ordered `controls/source_views.delta.sources` manifest includes every
control member. `receipt.tsv` pins the generated Gamma application.

Ten fixtures run through three routes: the retained raw-Bytes checker entrance,
a view over flat sealed input, and a view over constructed Delta Bytes. The raw
route copies away its test-mode header and is not a flat-input performance
baseline. Both window routes exclude a header, a non-ASCII prefix, and a suffix
containing token/escape continuations and non-ASCII sealed input. The fixtures
cover successful checking, strings, an unknown name at source-relative offset
277, EOF keyword/comment boundaries, unterminated strings, incomplete escapes,
invalid source bytes, and empty source. Fixture identities cover the authored
files; `.hex` files are decoded only to transport otherwise forbidden bytes.

Fourteen direct extent outcomes cover valid endpoints and empty windows on flat
and constructed backing, negative origin/length, backing overflow, and maximum
signed integer operands. These check the validating factory, not forged raw
representation constructors.

Three independent invalid-index invocations cover negative, past-end, and empty
window reads. Readable backing bytes exist outside each window, so underlying
backing bounds cannot accidentally satisfy the control. Each requires ordinary
Delta failure status 249, empty stdout, and empty stderr. A pending output byte
must not be published when source access fails. These are internal compiler
accessor controls, not new Epsilon observations or language bounds.
