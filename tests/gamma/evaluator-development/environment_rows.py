"""Physical environment bounds without a quadratic full-width signature."""


def fixtures():
    # One 512-parameter function retains 512 rows per non-tail activation.
    # Distinct lengths make most exact-name comparisons stop at their length;
    # there is no special evaluator path for these names or this source shape.
    names = [b"n"] + [b"p" * width for width in range(1, 512)]
    parameters = b" ".join(b"(" + name + b" Int)" for name in names)
    last = names[-1]
    arguments = b" ".join([b"(- n 1)"] + [b"0"] * 510 + [last])
    initial = b" ".join([b"0"] * 510 + [b"(pair 65 66)"])
    observed = b"(eq (+ (first " + last + b") (second " + last + b")) 131)"
    cases = []
    for name, levels, overflow, application, expected in (
        ("old exact environment", 128, False, False, (0, b"AB")),
        ("cross former environment limit", 129, False, False, (0, b"AB")),
        ("exact expanded environment", 256, False, False, (0, b"AB")),
        ("adjacent expanded environment", 256, True, False, (3, b"")),
        ("application exact environment", 256, False, True, (0, b"A")),
        ("application adjacent environment", 256, True, True, (250, b"")),
    ):
        body = b"(let extra Int 0 " + observed + b")" if overflow else observed
        result = (b"(+ (write 65) (loop " + str(levels - 1).encode("ascii")
                  + b" " + initial + b"))")
        if application:
            result = b"(pair (- " + result + b" 66) 1)"
        source = (b"(def $application () Int 1)\n" if application else b"") + (
            b"(def loop (" + parameters + b") Int (if (eq n 0) " + body
            + b" (+ 0 (loop " + arguments + b"))))\n"
            + b"(def main () Int " + result + b")\n"
        )
        cases.append((name, source, expected))
    return cases
