# Helper stores and returns a mutable view

The helper replaces a record's mutable element-view descriptor and returns a
view of the installed backing. Its carrier-borrow lifetime is independent of
the returned backing lifetime. Returning the reference produces a value; it
does not itself invoke the replacement authority that reference carries.

The caller observes both replacement elements, their full-width tags and the
new count, while the displaced backing remains unchanged. Expected exit: 0.
Run through the corpus gate with `--both` and this fixture's name.
