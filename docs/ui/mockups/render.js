// 把界面稿渲染成 PNG：node docs/ui/mockups/render.js <输出目录>
const { chromium } = require('playwright');
const path = require('path');
const fs = require('fs');

const dir = __dirname;
const out = process.argv[2] || path.join(dir, '..');
const pages = fs.readdirSync(dir).filter(f => /^\d\d-.*\.html$/.test(f)).sort();

(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
  for (const f of pages) {
    await page.goto('file://' + path.join(dir, f));
    await page.evaluate(() => document.fonts.ready);
    const png = path.join(out, f.replace('.html', '.png'));
    await page.screenshot({ path: png, fullPage: true });
    console.log('wrote', png);
  }
  await browser.close();
})();
