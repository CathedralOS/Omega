"""Physical row construction shared by finite authored diagnostic recipes."""

from lexical import record


class LiteralRows:
    """Intern physical terms and append authored proof rows; no rule inference."""

    def __init__(self):
        self.terms = []
        self.references = {}
        self.proofs = []

    def term(self, tag, symbol, *children):
        encoded = record(tag, symbol, len(children), *children)
        if encoded not in self.references:
            self.terms.append(encoded)
            self.references[encoded] = len(self.terms)
        return self.references[encoded]

    def proof(self, rule, left, right, *fields):
        self.proofs.append(record(rule, left, right, *fields))
        return len(self.proofs)
