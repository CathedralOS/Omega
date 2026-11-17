#!/usr/bin/env python3
"""Exercise the local atlas explorer; requires Playwright and Chromium.

Use with build_pages.py --draft and a local HTTP server. This checks reading
controls, not compiler behavior or publication status.
"""
import argparse
import json
from pathlib import Path
import shutil
import sys

from playwright.sync_api import sync_playwright

ATLAS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ATLAS))
from artifacts.explorer import models


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base-url', default='http://127.0.0.1:8765/artifacts/out/')
    parser.add_argument('--browser', default=shutil.which('chromium'))
    args = parser.parse_args()
    base = args.base_url.rstrip('/') + '/'
    url = base + 'pages/architecture.html'
    output = ATLAS / 'artifacts/out/interactive-review'
    output.mkdir(parents=True, exist_ok=True)
    graph = json.loads((ATLAS/'evidence/data/graph.json').read_text())
    pin = json.loads((ATLAS/'evidence/data/manifest.json').read_text())['source_revision']
    data = models(pin, graph)
    dependency = data[-1]
    names = {n['id'] for n in dependency['nodes']}
    expected = [e for e in graph['edges'] if e['kind']=='normal' and e['from'] in names and e['to'] in names]
    assert [e['declaration'] for e in dependency['edges']] == expected
    scenarios = ['dependency view equals every normal declaration among its selected packages']
    errors, failures, observations = [], [], []

    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path=args.browser, headless=True, args=['--no-sandbox'])
        for theme in ('light', 'dark'):
            for width in (1440, 400):
                context = browser.new_context(viewport={'width':width,'height':1000},color_scheme=theme,has_touch=width==400)
                page = context.new_page()
                page.on('pageerror', lambda error: errors.append(str(error)))
                page.on('requestfailed', lambda request: failures.append(request.url))
                page.goto(url); page.locator('.secondary-routes > summary').click(); page.evaluate('document.fonts.ready')
                for view in data:
                    key = view['id']
                    page.locator(f'[data-view="{key}"]').click()
                    panel = page.locator(f'[data-view-panel="{key}"]')
                    assert page.locator('.flow-view:visible').count() == 1
                    info = panel.evaluate('''el => ({
                      pageWidth: document.documentElement.scrollWidth,
                      minimumText: Math.min(...[...el.querySelectorAll('svg text')].map(t => parseFloat(getComputedStyle(t).fontSize)*t.getScreenCTM().a)),
                      graphTop: el.querySelector('.diagram-frame').getBoundingClientRect().top,
                      animated: [...el.querySelectorAll('.edge path')].some(e => getComputedStyle(e).animationName !== 'none'),
                      fonts: [...document.fonts].filter(f=>f.family==='Omega Serif'&&f.status==='loaded').map(f=>f.weight)
                    })''')
                    assert info['pageWidth'] <= width + 1, info
                    assert info['minimumText'] >= 11.5, info
                    assert not info['animated']
                    assert '400' in info['fonts']
                    assert panel.locator('.tour').is_visible() == bool(view['steps'])
                    observations.append({'view':key,'theme':theme,'width':width,**info})
                    page.screenshot(path=str(output/f'{key}-{theme}-{width}.png'))
                context.close()
        scenarios.append('12 view/theme/viewport observations: fonts, legible labels, no page overflow, motion initially stopped')

        context = browser.new_context(viewport={'width':1440,'height':1000})
        page = context.new_page(); page.on('pageerror',lambda e:errors.append(str(e)))
        page.goto(url); page.locator('.secondary-routes > summary').click(); page.clock.install()
        native = page.locator('[data-view-panel="native"]')
        assert native.locator('[data-action="previous"]').is_disabled()
        handler = native.locator('[data-node="handlers"]')
        handler.focus(); handler.press('Enter')
        assert handler.get_attribute('aria-pressed') == 'true'
        assert 'transitive body demand' in native.locator('.flow-inspector').inner_text()
        assert 'if needed' in native.locator('.flow-inspector h3').inner_text()
        related = native.locator('.edge.is-related').evaluate_all('(edges)=>edges.map(e=>e.dataset.edge).sort()')
        assert related == ['handler-demand','measured']
        neighbor = native.locator('.flow-inspector button').filter(has_text='Bind installations')
        neighbor.focus(); neighbor.press('Enter')
        assert page.evaluate('document.activeElement.textContent') == 'Bind installations'
        assert native.locator('[data-node="bind"]').get_attribute('aria-pressed') == 'true'
        scenarios.append('keyboard node selection and neighbor navigation preserve useful focus and exact adjacent edges')
        for _ in range(8): native.locator('[data-action="next"]').click()
        assert 'Step 8 of 8' in native.locator('.tour-position').inner_text()
        assert native.locator('[data-action="next"]').is_disabled()
        assert 'entry and adapter machine code' in native.locator('.flow-inspector').inner_text()
        native.locator('[data-action="previous"]').click()
        assert 'Step 7 of 8' in native.locator('.tour-position').inner_text()
        native.locator('[data-action="play"]').click(); page.clock.fast_forward(5000)
        assert 'Step 8 of 8' in native.locator('.tour-position').inner_text()
        assert not page.locator('.flow-explorer').evaluate("e=>e.classList.contains('is-playing')")
        native.locator('[data-action="play"]').click(); page.clock.fast_forward(5000)
        assert 'Step 2 of 8' in native.locator('.tour-position').inner_text()
        native.locator('[data-action="play"]').click(); page.clock.fast_forward(15000)
        assert 'Step 2 of 8' in native.locator('.tour-position').inner_text()
        native.locator('[data-action="play"]').click()
        page.locator('[data-view="dependencies"]').click(); page.clock.fast_forward(15000)
        assert not page.locator('.flow-explorer').evaluate("e=>e.classList.contains('is-playing')")
        assert not page.locator('[data-view-panel="dependencies"] .tour').is_visible()
        scenarios.append('Previous/Next, finite Play, explicit Pause and view-switch cancellation work with a controlled clock')
        page.goto(base+'pages/omega.html')
        figure = page.locator('figure.diagram').first
        node = figure.locator('.node').first; node.focus(); node.press('Space')
        reset = figure.get_by_role('button',name='Show all'); reset.focus(); reset.press('Enter')
        assert not reset.is_visible()
        assert page.evaluate("document.activeElement.classList.contains('node')")
        assert figure.locator('.is-related').count()==0
        scenarios.append('subject diagram selection resets without losing keyboard focus')
        context.close()

        context = browser.new_context(viewport={'width':400,'height':844},has_touch=True)
        page = context.new_page();page.goto(url); page.locator('.secondary-routes > summary').click()
        native = page.locator('[data-view-panel="native"]')
        for _ in range(3): native.locator('[data-action="next"]').tap()
        assert native.locator('.diagram-frame').evaluate('e=>e.scrollLeft') > 0
        page.locator('[data-view="psi"]').tap();page.locator('[data-view="native"]').tap()
        assert native.locator('.diagram-frame').evaluate('e=>e.scrollLeft') == 0
        for _ in range(2): native.locator('[data-action="next"]').tap()
        summary = native.locator('.inspector-disclosure > summary')
        assert 'Provider closure' in summary.inner_text()
        native.locator('.diagram-frame').scroll_into_view_if_needed()
        rect = summary.bounding_box()
        assert rect and 0 <= rect['y'] < 600, rect
        summary.tap()
        assert native.locator('.inspector-disclosure').evaluate('e=>e.open')
        assert native.get_by_role('link',name='Console binding example').is_visible()
        page.screenshot(path=str(output/'mobile-selected-provider.png'))
        scenarios.append('touch stepping pans inside the map; switching back resets pan; sticky mobile details expose the Console source')
        context.close()

        context = browser.new_context(viewport={'width':400,'height':844},reduced_motion='reduce')
        page=context.new_page();page.goto(url); page.locator('.secondary-routes > summary').click()
        native=page.locator('[data-view-panel="native"]')
        assert native.locator('[data-action="play"]').is_disabled()
        native.locator('[data-action="next"]').click()
        assert 'Step 2 of 8' in native.locator('.tour-position').inner_text()
        assert native.locator('.edge path').evaluate_all("es=>es.every(e=>getComputedStyle(e).animationName==='none')")
        scenarios.append('reduced motion disables automatic play while manual steps remain usable')
        context.close()

        context=browser.new_context(viewport={'width':400,'height':844},java_script_enabled=False)
        page=context.new_page();page.goto(url); page.locator('.secondary-routes > summary').click()
        assert page.locator('.flow-view:visible').count()==3
        assert not page.locator('.view-switch').is_visible()
        page.locator('.flow-reference summary').first.click()
        assert page.locator('.flow-reference').first.get_by_role('link',name='Pinned source').count()==9
        assert page.evaluate('document.documentElement.scrollWidth')==400
        scenarios.append('without JavaScript all three static maps and expandable source notes remain usable')
        context.close();browser.close()
    assert not errors and not failures,(errors,failures)
    (output/'interaction-report.json').write_text(json.dumps({'source_revision':pin,'scenarios':scenarios,'observations':observations,'page_errors':errors,'failed_requests':failures},indent=2)+'\n')
    print(f'{len(scenarios)} interaction scenarios; {len(observations)} responsive observations; no browser errors. Report: {output}')


if __name__=='__main__': main()
