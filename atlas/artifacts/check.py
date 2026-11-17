#!/usr/bin/env python3
"""Check the built atlas's navigation, local links, citations, SVGs and hashes."""
import hashlib
from html.parser import HTMLParser
import json
from pathlib import Path
import re
from urllib.parse import unquote, urlsplit

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
OUT = HERE / "out"


class Page(HTMLParser):
    def __init__(self, path):
        super().__init__(convert_charrefs=True)
        self.path = path
        self.ids = set()
        self.links = []
        self.h1 = self.diagrams = self.current = 0
        self.feed(path.read_text())

    def handle_starttag(self, tag, attributes):
        attrs = dict(attributes)
        if "id" in attrs:
            assert attrs["id"] not in self.ids, f"duplicate id {self.path}: {attrs['id']}"
            self.ids.add(attrs["id"])
        if tag == "a":
            assert "href" in attrs, f"anchor without href: {self.path}"
            self.links.append(attrs["href"])
            self.current += attrs.get("aria-current") == "page"
        if tag == "h1":
            self.h1 += 1
        if tag == "svg":
            self.diagrams += 1
            assert attrs.get("role") == "img" and attrs.get("aria-labelledby"), f"SVG lacks accessible title: {self.path}"
        if tag in ("script", "img"):
            assert not attrs.get("src", "").startswith(("https://", "http://")), f"unexpected remote asset: {self.path}"


def check():
    manifest = json.loads((OUT / "build.json").read_text())
    evidence = HERE.parent / "evidence/data/manifest.json"
    assert hashlib.sha256(evidence.read_bytes()).hexdigest() == manifest["evidence_manifest_sha256"], "evidence changed after rendering"
    pages = {(OUT / item["page"]).resolve(): Page(OUT / item["page"]) for item in manifest["pages"]}
    citations = set()
    links = 0
    for item in manifest["pages"]:
        path = OUT / item["page"]
        content = path.read_text()
        page = pages[path.resolve()]
        assert hashlib.sha256(content.encode()).hexdigest() == item["sha256"], f"changed output: {path}"
        assert page.h1 == 1 and page.current == 1, f"page heading/navigation count: {path}"
        assert page.diagrams == item["diagrams"], f"missing diagram: {path}"
        assert not re.search(r"%%[A-Z_]+%%|ATLASDIAGRAM\d+END|class=\"language-mermaid\"", content), f"unrendered content: {path}"
        for href in page.links:
            links += 1
            url = urlsplit(href)
            if url.scheme or url.netloc:
                prefix = f"/CathedralOS/Omega/blob/{manifest['source_revision']}/"
                tree_prefix = f"/CathedralOS/Omega/tree/{manifest['source_revision']}/"
                if url.netloc == "github.com" and url.path.startswith((prefix, tree_prefix)):
                    source = ROOT / unquote(url.path.removeprefix(prefix).removeprefix(tree_prefix))
                    assert source.exists(), f"missing cited path: {path}: {href}"
                    if re.fullmatch(r"L\d+(?:-L\d+)?", url.fragment):
                        last = int(url.fragment.split("L")[-1])
                        assert source.is_file() and last <= len(source.read_text().splitlines()), f"citation outside file: {href}"
                    citations.add(href)
                continue
            target = (path.parent / unquote(url.path)).resolve() if url.path else path.resolve()
            assert target.exists(), f"broken local link: {path}: {href}"
            if url.fragment and target in pages:
                assert unquote(url.fragment) in pages[target].ids, f"missing anchor: {path}: {href}"
    print(f"Checked {len(pages)} pages, {links} links, {sum(p.diagrams for p in pages.values())} SVG diagrams and {len(citations)} pinned source citations")


if __name__ == "__main__":
    check()
