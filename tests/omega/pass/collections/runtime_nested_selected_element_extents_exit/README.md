A helper captures two nested element descriptor lengths through one borrowed record. Guards on those scalar snapshots prove the corresponding element reads. Unequal counts 2 and 5 must remain element counts, independent of sibling offset, backing byte size and borrowed root.

Run `omega run --both tests/omega/pass/collections/runtime_nested_selected_element_extents_exit/main.omg`.
