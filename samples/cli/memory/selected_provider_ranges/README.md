# Selected provider ranges

`omega run samples/cli/memory/selected_provider_ranges/main.omg` must exit 0.
It counts an owned record's dynamic byte window, the same projection through a
shared parameter in another state, an established view, and an empty window.
Expression and Unit calls retain the selected target-scoped implementation.

The provider checks the first byte and surrounding scalar arguments. A separate
call records prefix, range-start, range-end and suffix evaluation as `1234`;
forming its view must not depend on an unrelated field read materializing the
owned record. Native and Terminal execution must both exit 0.
