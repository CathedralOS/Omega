A mutable reference local receives a Match result whose alternatives each independently supply mutable source access. The selected result retains the original source frame and lifetime.

Two sequential calls choose opposite alternatives and read the original values 7 and 9. Both calls must restore caller loans after their selected reference ends. The program returns normally with exit 0 only when both reads match. The companion invalid control rejects a shared alternative instead of donating access from its exclusive sibling.
