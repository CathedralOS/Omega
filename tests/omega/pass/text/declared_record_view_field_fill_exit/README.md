# Declared record view-field fill

Run `omega run --both tests/omega/pass/text/declared_record_view_field_fill_exit/main.omg`.
Native execution and the checked interpreter must both exit 70.

The reduction declares `Room`, declares an uninitialized `room`, and fills
`room.label` from an established qualified view. Comparing that field with the
literal observes the stored descriptor's actual bytes. No fabricated zero
reference, literal-only initializer, or concatenation is needed to establish
the local member.

Native lowering consumes the copied descriptor through the same readable-home
lookup as record construction. Availability and descriptor shape remain
required; independent custody replay still checks its backing lifetime.
