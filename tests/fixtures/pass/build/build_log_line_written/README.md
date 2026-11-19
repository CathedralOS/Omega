# A build writes one line through the compiler-owned build log

The authored customer for `BuildLog::write_line`. The build writes one line
through the activation's log facet, and the compiler emits that line with
the compile's other output on standard output:

    omega build log witness

The corpus runner writes its fixture records to the file
`OMEGA_CORPUS_RECORDS` names, which `tools/corpus_gate.py` sets, so the
build log reaching the runner's stdout cannot be read as a record.

The rejection half of this behavior is
`tests/omega/fail/build/build_log_counterfeit_facet_refused`, which pins that
the call reaches the evaluator at all.
