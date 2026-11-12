A child reborrow of a selected reference keeps the selected descriptor and both
immediate input parents alive. After the child ends, both parent descriptors
are read again, and the caller observes the selected field's mutation.
The second call returns only its first input, but its explicit shared output
lifetime still retains both declared input parents until the result ends.

Run `omega run --both main.omg` or `python3 tools/corpus_gate.py --both --filter
borrows/runtime_selected_reference_nested_restoration_exit`.
