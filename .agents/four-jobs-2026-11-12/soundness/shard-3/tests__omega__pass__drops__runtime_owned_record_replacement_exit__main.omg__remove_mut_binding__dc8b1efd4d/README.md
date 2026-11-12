Run `omega run main.omg` from this directory. From the repository root, run
`python3 tools/corpus_gate.py --both --filter runtime_owned_record_replacement_exit`
for native/Terminal comparison; both must exit 0. Whole-owned assignment replaces the binding
without borrowed-storage extraction or restoration debt; a later read observes 9.
