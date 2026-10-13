# Ordered nested length observations

`omega run --both main.omg` must exit 0. Two effectful selectors run once,
left-to-right, before reading the selected nested descriptor's live count.
Different backing lengths distinguish the selected descriptor from its sibling.
A helper also observes a literal selection through a runtime view.
