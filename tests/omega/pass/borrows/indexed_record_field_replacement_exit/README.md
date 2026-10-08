Run on macOS AArch64 through the public CLI after package acceptance:

```sh
cd tests/omega/pass/borrows/indexed_record_field_replacement_exit
omega update --offline
# Review pending decisions, then: omega update --resume --offline
omega run --both main.omg
```

Native and Terminal execution must exit 70. Whole-record replacement and a
helper's indexed tag write update the caller's original array. The caller checks
every tag and sentinel, including the fields outside the selected element.

Package preparation reviews the current std package as well as this fixture;
the corpus does not supply accepted-package evidence. This fixture does not
publish its generated lock.
