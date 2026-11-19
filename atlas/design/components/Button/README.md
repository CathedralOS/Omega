# Button

A button for one action, in three weights that follow the Psi and Omega shape rule.

- `solid` is for the action that realizes or runs something: "Run natively", "Build". Use one per view.
- `outline` is for checking and other target-neutral actions: "Check", "Compare".
- `quiet` is for navigation and minor actions: "View evidence".

You provide the label as children, an `onClick`, and `disabled` while the action can't run. Every other button attribute passes through.

Do write the label as the exact action, in sentence case. Don't use a bare "OK" or "Submit", don't put icons alone in a button, and don't color a button `checked` or `trap`. Those colors report results; they are not actions.
