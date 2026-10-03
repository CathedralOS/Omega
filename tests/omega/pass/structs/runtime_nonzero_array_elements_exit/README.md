# Nonzero array elements

`omega run --both main.omg` must exit 70 on both engines after package review.

Each `Point.value` belongs to `u64::Positive`, which excludes zero. The program
moves an already completed local owner and an ordinary call result into the
array field of `Envelope`, then observes the authored values 30 and 40.
Complete array construction must preserve both values and their field-domain
contracts without requiring a valid zero initializer for either element.

The build binds the same entry to Windows x86-64, Linux x86-64, Linux ARM64,
and macOS ARM64.
