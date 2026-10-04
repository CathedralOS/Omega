# Reborrowing preserves a store's address

An explicit reborrow passes a record to a mutating helper. A later assignment
copies its computed field into another field of the same record. The unrelated
signed-byte layout activates concrete encoded-write checking; its presence
must not make the ordinary record's reference origin disappear.

Before the fix, shared-value prefix analysis treated `&mut record` as exposure
of the reference binding while ordinary write analysis treated it as a referent
reborrow. The assignment therefore failed with unknown destination origins.
Both queries now use the same established-origin check. Carrier-leaf replacement,
borrow-access validity and concrete narrow-write fitting remain independent.

On macOS ARM64 the corpus compiles and executes this program with exit 70 in
both engines:

```sh
python3 tools/corpus_gate.py --both --filter runtime_reborrow_preserves_store_origin_exit
```

The public command is:

```sh
omega run --both tests/omega/pass/layouts/runtime_reborrow_preserves_store_origin_exit/main.omg
```

Fresh project package acceptance is a separate prerequisite. The related public
run in `../../slices/runtime_call_endpoint_view_exit` now reaches standard-library
review decisions rather than failing to check `Filesystem::read_all`.
