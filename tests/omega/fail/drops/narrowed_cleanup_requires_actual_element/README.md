# A narrowed receiver cannot borrow its sibling's prerequisite

Index 0 of the local view selects original element 1, whose value is 4.
Original element 0 has value 9, but that does not establish the selected
old element's cleanup requirement. Checking must reject.
