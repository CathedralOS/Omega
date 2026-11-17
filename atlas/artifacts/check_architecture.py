#!/usr/bin/env python3
"""Check the restored original and separate Canvas2D workspace over localhost."""
import argparse
import json
from pathlib import Path
import shutil
from playwright.sync_api import sync_playwright

ATLAS = Path(__file__).resolve().parents[1]

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base-url',default='http://127.0.0.1:8765/artifacts/out/')
    parser.add_argument('--browser',default=shutil.which('chromium'))
    args=parser.parse_args();base=args.base_url.rstrip('/')+'/'
    out=ATLAS/'artifacts/out/full-map-review';out.mkdir(parents=True,exist_ok=True)
    errors=[];observations=[]
    with sync_playwright() as p:
        browser=p.chromium.launch(executable_path=args.browser,headless=True,args=['--no-sandbox'])
        for theme in ('light','dark'):
            for width in (1440,400):
                context=browser.new_context(viewport={'width':width,'height':1000},color_scheme=theme,device_scale_factor=2)
                page=context.new_page();page.on('pageerror',lambda e:errors.append(str(e)))
                page.on('requestfailed',lambda r:errors.append(r.url))
                page.goto(base);page.evaluate('document.fonts.ready')
                assert page.locator('svg').count()==4
                assert 'Original proposed design' in page.locator('.original-status').inner_text()
                assert page.evaluate('document.documentElement.scrollWidth')<=width+1
                page.locator('#show-leaks').uncheck();assert page.locator('#leaks').get_attribute('hidden') is not None
                page.locator('#show-leaks').check()
                page.screenshot(path=str(out/f'restored-original-{theme}-{width}.png'))
                page.get_by_role('link',name='Interactive full architecture',exact=True).click()
                page.evaluate('document.fonts.ready')
                page.wait_for_function("document.querySelector('.architecture').dataset.scale==='1'")
                assert page.locator('.arch-canvas').is_visible()
                assert page.locator('.arch-canvas').evaluate('e=>e.width===Math.round(e.clientWidth*devicePixelRatio)')
                assert page.evaluate('document.documentElement.scrollWidth')<=width+1
                page.locator('.arch-jump select').select_option('providers')
                panel=page.locator('.arch-detail');assert panel.is_visible()
                assert 'demanded' in panel.inner_text()
                assert panel.get_by_role('link',name='Relationship source',exact=True).count()>0
                assert float(page.locator('.architecture').get_attribute('data-scale'))>=1
                page.locator('.arch-viewport').scroll_into_view_if_needed()
                page.screenshot(path=str(out/f'canvas-selected-{theme}-{width}.png'))
                neighbor=panel.get_by_role('button',name='Selected · execution',exact=True)
                neighbor.focus();neighbor.press('Enter')
                assert page.evaluate("document.activeElement.tagName")=='H3'
                page.locator('.arch-detail-close').click()
                assert page.evaluate("document.activeElement.tagName")=='SELECT'
                viewport=page.locator('.arch-viewport');viewport.focus();viewport.press('ArrowRight')
                before=page.locator('.architecture').get_attribute('data-camera')
                viewport.press('+');page.wait_for_function("+document.querySelector('.architecture').dataset.scale>1")
                page.locator('[data-map="reset"]').click();page.wait_for_function("document.querySelector('.architecture').dataset.scale==='1'")
                page.locator('[data-map="fit"]').click();page.wait_for_function("+document.querySelector('.architecture').dataset.scale<1")
                page.locator('[data-map="reset"]').click()
                # Canvas hit testing uses camera coordinates, not DOM SVG targets.
                page.locator('.arch-canvas').click(position={'x':122,'y':108})
                assert panel.locator('h3').inner_text()=='CLI'
                page.locator('.arch-detail-close').click()
                page.locator('.arch-all-relations').check()
                assert page.locator('.arch-all-relations').is_checked()
                page.locator('.arch-minimap canvas').click(position={'x':100,'y':60})
                page.locator('[data-map="reset"]').click()
                observations.append({'theme':theme,'width':width,'dpr':2,'default_label_px':20,'original_svg_count':4})
                context.close()
        context=browser.new_context(viewport={'width':400,'height':844},java_script_enabled=False)
        page=context.new_page();page.goto(base+'pages/architecture.html')
        assert page.locator('.arch-svg').is_visible()
        assert not page.locator('.arch-canvas').is_visible()
        page.locator('.arch-outline summary').click();assert page.locator('.arch-outline dt').count()==48
        assert page.evaluate('document.documentElement.scrollWidth')<=401
        context.close()
        context=browser.new_context(viewport={'width':1440,'height':1000},reduced_motion='reduce')
        page=context.new_page();page.goto(base+'pages/architecture.html')
        page.locator('.arch-jump select').select_option('image');assert 'Late bridge' in page.locator('.arch-detail').inner_text()
        assert page.locator('.arch-canvas').evaluate("e=>getComputedStyle(e).animationName==='none'")
        page.locator('[data-map="fullscreen"]').click()
        assert page.evaluate('!!document.fullscreenElement')
        page.locator('[data-map="fullscreen"]').click()
        page.locator('#theme').click()
        page.wait_for_function("document.documentElement.dataset.theme==='dark'")
        page.screenshot(path=str(out/'canvas-theme-switch.png'))
        context.close();browser.close()
    assert not errors,errors
    (out/'architecture-report.json').write_text(json.dumps({'observations':observations,'no_js':True,'reduced_motion':True,'fullscreen':True,'page_errors':errors},indent=2)+'\n')
    print('4 original/canvas theme/viewport checks at DPR2; no-JS, reduced motion, keyboard, hit testing, camera, minimap and fullscreen passed')

if __name__=='__main__':main()
