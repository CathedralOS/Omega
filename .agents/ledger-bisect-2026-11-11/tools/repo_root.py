from pathlib import Path


def repo_root() -> Path:
    here = Path(__file__).resolve().parent
    for p in [here, *here.parents]:
        if (p / "tests/omega/corpus_outcomes.txt").is_file():
            return p
    raise RuntimeError("could not find repository root")
