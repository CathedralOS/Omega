"""The corruption-check sentence names every registered family. Run from the V2 workspace."""
import unittest

import product_controls


class ControlSummaryTests(unittest.TestCase):
    def test_every_registered_family_is_named_and_only_verified_checks_count(self):
        controls = {label: {"test": test, "verified": True} for label, test in product_controls.CONTROLS.items()}
        controls["join-arrival/same-field"]["verified"] = False
        text = product_controls.summary(controls)
        self.assertIn(f"rejected {len(controls) - 1} of {len(controls)} registered changes", text)
        # The hand-written sentence named seven families and omitted both cycle families.
        for family in ("branch-target", "cycle-claim", "cycle-cleanup", "join-arrival", "premised-selector", "residual", "restoration", "runtime-contract", "runtime-dispatch",
                       "runtime-joined", "scalar-operand"):
            self.assertIn(family, text)
        self.assertIn("in 11 families", text)

    def test_an_unrecorded_control_is_not_counted(self):
        self.assertIn("rejected 0 of 1 registered changes in 1 families: residual",
                      product_controls.summary({"residual/Pair::left/omitted": {"test": "t"}}))


if __name__ == "__main__":
    unittest.main()
