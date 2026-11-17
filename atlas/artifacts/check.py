#!/usr/bin/env python3
"""Check generated pages against the deployed subset and their pinned sources.

Default: require repository objects, verify the atlas content pin, and check
GitHub citations against the exact cited Git tree. --local-only checks the
static site without claiming source verification, for a copied Pages subset.
"""
import argparse
from functools import lru_cache
import hashlib
from html.parser import HTMLParser
import json
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote, urlsplit

ATLAS = Path(__file__).resolve().parents[1]
ROOT = ATLAS.parent
sys.path.insert(0, str(ATLAS))
from build_pages import check_content_revision, require_commit


class Page(HTMLParser):
    def __init__(self, path):
        super().__init__(convert_charrefs=True)
        self.path, self.ids, self.links, self.assets = path, set(), [], []
        self.h1 = self.diagrams = self.current = 0
        self.feed(path.read_text())

    def handle_starttag(self, tag, attributes):
        attrs = dict(attributes)
        if 'id' in attrs:
            assert attrs['id'] not in self.ids, f'duplicate id: {self.path}: {attrs["id"]}'
            self.ids.add(attrs['id'])
        if tag == 'a':
            self.links.append(attrs['href'])
            self.current += attrs.get('aria-current') == 'page'
        if tag == 'h1': self.h1 += 1
        if tag == 'svg' and attrs.get('aria-hidden') != 'true':
            self.diagrams += 1
            assert attrs.get('role') == 'img' and attrs.get('aria-label'), f'SVG needs its claim: {self.path}'
        if tag == 'link' and attrs.get('rel') == 'stylesheet': self.assets.append(attrs['href'])
        if tag in ('script', 'img') and 'src' in attrs: self.assets.append(attrs['src'])
        if tag in ('polygon','path','text','ellipse'):
            assert not re.search(r'#[a-fA-F0-9]{3,8}\b|\bblack\b|\bwhite\b', ' '.join(attrs.get(a,'') for a in ('fill','stroke'))), f'hard-coded SVG color: {self.path}'


@lru_cache(maxsize=None)
def check_commit(revision):
    require_commit(revision)


@lru_cache(maxsize=None)
def git_object(revision, path):
    if not re.fullmatch('[0-9a-f]{40}', revision):
        raise AssertionError(f'unpinned repository URL: {revision}:{path}')
    check_commit(revision)
    name = revision + (':' + path if path else '')
    kind = subprocess.check_output(['git','cat-file','-t',name], cwd=ROOT, text=True).strip()
    data = subprocess.check_output(['git','show',name], cwd=ROOT) if kind == 'blob' else b''
    return kind, data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--site', type=Path, default=ATLAS)
    parser.add_argument('--local-only', action='store_true')
    args = parser.parse_args()
    site = args.site.resolve()
    manifest = json.loads((site/'pages/build.json').read_text())
    if not args.local_only:
        if not manifest['content_revision']:
            raise SystemExit('Draft has no atlas content pin; use --local-only for a static check.')
        try:
            check_content_revision(manifest['content_revision'])
        except (subprocess.CalledProcessError, FileNotFoundError) as error:
            raise SystemExit('Pinned source verification needs the repository and recorded Git objects. --local-only explicitly skips it.') from error
        assert (ATLAS/'content-revision.txt').read_text().strip() == manifest['content_revision']
        evidence = ATLAS/'evidence/data/manifest.json'
        assert hashlib.sha256(evidence.read_bytes()).hexdigest() == manifest['evidence_manifest_sha256'], 'evidence changed after rendering'
        assert json.loads(evidence.read_text())['source_revision'] == manifest['source_revision']
    pages = {(site/p['page']).resolve(): Page(site/p['page']) for p in manifest['pages']}
    citations, assets = set(), set()
    links = 0

    def local_target(owner, url):
        target = (owner.parent / unquote(url.path)).resolve() if url.path else owner
        assert target.is_relative_to(site), f'link leaves deployed subset: {owner}: {url.geturl()}'
        relative = target.relative_to(site)
        assert relative == Path('index.html') or relative.parts[0] in ('pages','design'), f'file is not deployed: {owner}: {relative}'
        assert target.exists(), f'broken local link: {owner}: {url.geturl()}'
        if url.fragment and target in pages:
            assert unquote(url.fragment) in pages[target].ids, f'missing anchor: {owner}: {url.geturl()}'
        return target

    for item in manifest['pages']:
        path = (site/item['page']).resolve()
        content = path.read_text()
        page = pages[path]
        assert hashlib.sha256(content.encode()).hexdigest() == item['sha256'], f'changed generated page: {path}'
        assert page.h1 == 1 and page.current == (item['page'] != 'index.html'), f'heading/navigation count: {path}'
        assert page.diagrams == item['diagrams'], f'missing diagram: {path}'
        assert not re.search(r'%%[A-Z_]+%%|ATLASDIAGRAM\d+END|class="language-mermaid"',content), f'unrendered source: {path}'
        assert not re.search(r'#[0-9a-fA-F]{6}\b', re.search(r'<style>(.*?)</style>', content, re.S).group(1)), f'page duplicates color tokens: {path}'
        assert ('design/tokens.css' if item['page']=='index.html' else '../design/tokens.css') in page.assets
        for href in page.links:
            links += 1
            url = urlsplit(href)
            if url.scheme or url.netloc:
                if url.netloc == 'github.com' and url.path.startswith('/CathedralOS/Omega/'):
                    pieces = url.path.split('/',5)
                    if len(pieces)>=5 and pieces[3] in ('blob','tree'):
                        revision, source = pieces[4], unquote(pieces[5]) if len(pieces)>5 else ''
                        if not args.local_only:
                            kind,data = git_object(revision,source)
                            assert kind == ('blob' if pieces[3]=='blob' else 'tree' if source else 'commit'), href
                            if re.fullmatch(r'L\d+(?:-L\d+)?',url.fragment):
                                bounds = [int(n) for n in re.findall(r'\d+',url.fragment)]
                                assert 0 < bounds[0] <= bounds[-1] <= len(data.splitlines()), f'citation outside pinned blob: {href}'
                        citations.add(href)
                continue
            local_target(path,url)
        for href in page.assets:
            url=urlsplit(href)
            assert not url.scheme and not url.netloc, f'external page asset: {path}: {href}'
            assets.add(local_target(path,url))
    for path in list(assets):
        if path.suffix=='.css':
            for href in re.findall(r'url\([\'\"]?([^\)\'\"]+)',path.read_text()):
                assets.add(local_target(path,urlsplit(href)))
    verdict = 'source verification NOT RUN' if args.local_only else f'{len(citations)} pinned repository links checked against Git objects'
    print(f'{len(pages)} HTML pages; {links} links; {sum(p.diagrams for p in pages.values())} SVGs; {len(assets)} local assets; {verdict}')


if __name__ == '__main__':
    try:
        main()
    except ValueError as error:
        raise SystemExit(str(error)) from error
