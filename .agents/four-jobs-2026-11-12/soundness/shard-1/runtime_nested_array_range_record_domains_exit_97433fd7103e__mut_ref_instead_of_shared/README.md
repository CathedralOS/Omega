# A window retains its nested array dimension

Run `omega run --both main.omg`. The selected outer window preserves both
inner records' Utf8 obligations. The helper reads value 70 from the selected
inner record; both executions must exit 0. The byte-field store crosses two
array indices and a following record field through ordinary static geometry.
