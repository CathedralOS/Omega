Compilation must refuse the alias: its sole element is 3, outside the declared range 0..=2. On ccf1a63b5e2106325c9fca3a3c50f59aceaf2980, --check incorrectly succeeds. Carrier correspondence supplies no qualification proof.

```sh
omega --check tests/omega/pending/modules/selected_constant_array_alias_false_qualification/main.omg
```
