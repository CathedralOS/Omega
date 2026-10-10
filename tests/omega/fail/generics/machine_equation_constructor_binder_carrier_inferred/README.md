The u64 machine binder cannot occupy Mode's u32 const position even when inferred values fit both widths. Check must reject before Terminal/native admission.

```sh
python3 tools/corpus_gate.py --filter generics/machine_equation_constructor_binder_carrier_inferred
```
