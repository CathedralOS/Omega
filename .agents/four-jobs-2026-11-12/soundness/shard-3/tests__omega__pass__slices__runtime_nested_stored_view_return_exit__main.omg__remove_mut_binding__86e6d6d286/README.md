# Return a descriptor through an outer slice

`omega run --both main.omg` must exit 0 in both engines. A helper replaces an
element through a record-carried view and returns that view through its outer
slice argument. The caller checks the returned records and untouched backing
sentinels.

The inner descriptor is reached through an outer runtime index. Its contents
describe the returned backing; the address of the enclosing record does not.
