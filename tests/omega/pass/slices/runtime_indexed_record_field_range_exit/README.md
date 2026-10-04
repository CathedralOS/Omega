Slices borrowed from a runtime-indexed record field retain their exact backing.
An ordinary call selects row one and a successor state selects row zero.
Writes through the two mutable slices update the selected elements and preserve
all sibling rows, elements, and leading marker fields. Both record levels
place the selected storage at a nonzero offset. Both engines must exit zero.
