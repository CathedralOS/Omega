Run on macOS AArch64 through the corpus gate:

```sh
python3 tools/corpus_gate.py --both --filter host/runtime_descriptor_record_array_input_exit
```

The native and Terminal engines both return zero and print `<abc` followed by
LF, then `!` and LF. The record owns an initialized seven-byte array and a
separate marker. Empty, forwarded short-read and EOF calls lend array windows;
another EOF call lends the whole array mutably, and the final output call lends
that field as a shared slice. The caller
observes the original bytes, untouched prefix/suffix and unchanged marker.
The record's custody is not the selected array's presentation type.

The calling policy uses canonical std vocabulary and explicitly declares the
pointer/count ABI. Public execution is `omega update`, review the exact
candidate, `omega update --resume`, then `omega run --both main.omg < input.txt`.
Full std package review remains a separate acceptance requirement, as described
in the sibling `runtime_descriptor_mutable_view_input_exit/README.md`; no
unrelated std authority was approved to qualify this compiler regression.
