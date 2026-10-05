# A build writes one line through the compiler-owned build log

The authored customer for `BuildLog::write_line`, which had none anywhere in
the repository. The build writes one line through the activation's log facet,
and the compiler emits that line with the compile's other output.

## Why this is pending, not in `pass/`

The corpus runner cannot host any fixture that writes a build log line. The
compiler emits the retained build log on its standard output, and the runner
multiplexes its fixture records on the same stream, so the authored line is
parsed as a record and aborts the whole measurement:

    corpus_gate: runner emitted unreadable records:
    line 1: not a fixture record: omega build log witness

That is a corpus-wide harness property, not a property of this source: any
fixture writing any build log line breaks the same way. Separating the
runner's record stream from the compiler's output is a harness change in the
`tests` lane and not part of the facet repair.

Verified through the real CLI on exactly this source, which prints

    omega build log witness

before its publication line and exits 0. The same source printed nothing at
`1ca7226ca9e`, where the facet call never reached the evaluator.

The rejection half of this behavior does fit the corpus, because a refused
call writes no bytes: `tests/omega/fail/build/build_log_counterfeit_facet_refused`
pins that the call reaches the evaluator at all.

Move this directory into `pass/` with its golden row once the runner carries
fixture records on a stream the compiled program's build log cannot reach.
