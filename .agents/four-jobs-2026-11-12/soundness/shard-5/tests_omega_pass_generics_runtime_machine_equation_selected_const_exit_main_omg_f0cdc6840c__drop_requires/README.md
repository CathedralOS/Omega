The authored machine equation matches [u8; 4] with its exact selected u64 const argument. Native and interpreted execution must return 0.

Run the public native/Terminal execution gate:

```sh
python3 tools/corpus_gate.py --both --native-target linux_x86_64 --filter generics/runtime_machine_equation_selected_const_exit
```
