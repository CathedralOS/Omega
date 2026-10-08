# Returned byte-field snapshot

After ordinary package review, run:

```sh
omega run --both tests/omega/pass/borrows/returned_owned_byte_field_snapshot_exit/main.omg
```

Both engines must exit 70. The helper returns an owned snapshot of an established
UTF-8 field. Replacing the original field must change neither that snapshot nor
the record's sibling. Its field predicate proves the return domain; identical
qualification labels on the physical source home are not required.

`fail/text/byte_field_snapshot_domain_widening` rejects a return that claims ASCII
from a field guaranteed only to contain UTF-8.
