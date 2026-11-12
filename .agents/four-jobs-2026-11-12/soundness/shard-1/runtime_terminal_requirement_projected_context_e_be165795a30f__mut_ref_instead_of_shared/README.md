# Terminal requirement with projected context

Both guarded arms invoke `ItemCount::get_count`, a one-member boundary trait
with an empty reach, passing an explicit shared view of a nested record field.
The selected provider reads that projected record. Distinct requests return 70
and 71, and the program exits 0 in both engines.
