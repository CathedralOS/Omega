Slices borrowed from a runtime-indexed record field retain their exact backing.
An ordinary call selects row one and a successor state selects row zero.
Writes through the two mutable slices update the selected elements and preserve
all sibling rows and elements. Both engines must exit zero.
