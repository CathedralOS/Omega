# Composed owned-element reads

Run from the repository root:

```sh
mbx run -p omega -- run --both tests/omega/pass/collections/runtime_composed_owned_element_reads_exit/main.omg
```

Native and interpreted execution exit 0 only when both reads observe their
authored values. The second read evaluates arithmetic in a descendant selector
before reading a declared scalar field. Each selector retains the bound of its
own collection and indexing occurrence.
