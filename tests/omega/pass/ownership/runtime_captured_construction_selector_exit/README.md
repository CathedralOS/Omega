# Construction preserves the captured assignment selector

Run `omega run --both tests/omega/pass/ownership/runtime_captured_construction_selector_exit/main.omg`.

The authored exit is 0. A scalar call inside the constructed replacement changes
the index from 0 to 3. Cleanup and store still name the originally selected
element, and the sibling remains unchanged. Replaying the selector would access
outside the array, even though the authored assignment target was in bounds.
