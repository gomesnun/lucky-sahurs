// Phase 4 (Monster) art: renders tools/phases/monster.html in headless Chromium once per verity colour
// (colors.json, from faces.py) into icons/pets/phases/<slug>_p4.png.
// usage (repo root, with a static server on :8124 serving the repo): node tools/phases/monster.mjs
import {chromium} from 'playwright';
import fs from 'fs';
const colors = JSON.parse(fs.readFileSync('tools/phases/colors.json'));
const b = await chromium.launch({args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader']});
const p = await b.newPage({viewport: {width: 512, height: 512}});
p.on('pageerror', e => console.log('ERR', e.message));
await p.goto(`http://127.0.0.1:8124/tools/phases/monster.html`);
await p.waitForFunction('window.done', null, {timeout: 180000});
for (const [slug, c] of Object.entries(colors)) {
  const url = await p.evaluate(rgb => renderColor(rgb), c);
  fs.writeFileSync(`icons/pets/phases/${slug}_p4.png`, Buffer.from(url.split(',')[1], 'base64'));
}
console.log('monsters', Object.keys(colors).length);
await b.close();
