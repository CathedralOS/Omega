Run `omega run main.omg` from this directory. From the repository root, run
`python3 tools/corpus_gate.py --both --filter runtime_owned_nominal_replacement_exit`
for native/Terminal comparison; both must exit 0. Replacement evaluates the new value before
the old value's nominal cleanup and keeps the new home for subsequent reads and
exit cleanup. The outgoing scalar is captured before the exit hook writes zero.
