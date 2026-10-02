import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { dirname, join, resolve } from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

// Vector sources and outlined custom lettering: no fonts or network needed.
const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const output = join(root, 'docs/design/assets/mochi/final');
const coral = '#FF7048';
const ink = '#171B24';
const paper = '#FAFAFA';
const helmet =
  'M68 40H176Q203 40 190 62L176 86Q172 94 179 99Q187 105 194 94L208 74Q212 68 217 78L238 121Q245 135 236 151L216 186Q207 203 188 207Q128 219 66 208Q47 205 35 187L17 155Q8 141 16 125L40 75Q50 49 68 40Z';

function mark(body = coral, eyes = ink) {
  return `<path d="${helmet}" fill="${body}"/><rect x="78" y="113" width="20" height="48" rx="10" fill="${eyes}"/><rect x="158" y="113" width="20" height="48" rx="10" fill="${eyes}"/>`;
}

function wordmark(color) {
  return `<g fill="none" stroke="${color}" stroke-width="16" stroke-linecap="round" stroke-linejoin="round"><path d="M20 116V29Q20 21 26 29L62 74L98 29Q104 21 104 29V116"/><ellipse cx="164" cy="84" rx="27" ry="33"/><path d="M274 60C254 42 224 54 224 84S254 126 274 108M308 24V116M308 81C308 48 364 44 364 82V116M402 59V116"/></g><circle cx="402" cy="29" r="9" fill="${color}"/>`;
}

function svg(width, height, body, title) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" role="img" aria-labelledby="title"><title id="title">${title}</title>${body}</svg>\n`;
}

await mkdir(output, { recursive: true });
const assets = new Map([
  ['mark.svg', svg(256, 256, mark(), 'Mochi Signal robot-face mark')],
  [
    'mark-monochrome-dark.svg',
    svg(256, 256, mark(ink, paper), 'Mochi monochrome dark mark'),
  ],
  [
    'mark-monochrome-light.svg',
    svg(256, 256, mark(paper, ink), 'Mochi monochrome light mark'),
  ],
  ['wordmark-dark.svg', svg(430, 140, wordmark(ink), 'Mochi wordmark')],
  [
    'wordmark-light.svg',
    svg(430, 140, wordmark(paper), 'Mochi light wordmark'),
  ],
  [
    'logo-light.svg',
    svg(
      700,
      200,
      `<g transform="translate(0 3) scale(.76)">${mark()}</g><g transform="translate(207 17) scale(1.1)">${wordmark(ink)}</g>`,
      'Mochi logo for light backgrounds',
    ),
  ],
  [
    'logo-dark.svg',
    svg(
      700,
      200,
      `<g transform="translate(0 3) scale(.76)">${mark()}</g><g transform="translate(207 17) scale(1.1)">${wordmark(paper)}</g>`,
      'Mochi logo for dark backgrounds',
    ),
  ],
  [
    'logo-monochrome-dark.svg',
    svg(
      700,
      200,
      `<g transform="translate(0 3) scale(.76)">${mark(ink, paper)}</g><g transform="translate(207 17) scale(1.1)">${wordmark(ink)}</g>`,
      'Mochi monochrome dark logo',
    ),
  ],
  [
    'logo-monochrome-light.svg',
    svg(
      700,
      200,
      `<g transform="translate(0 3) scale(.76)">${mark(paper, ink)}</g><g transform="translate(207 17) scale(1.1)">${wordmark(paper)}</g>`,
      'Mochi monochrome light logo',
    ),
  ],
  [
    'app-icon.svg',
    svg(
      1024,
      1024,
      `<rect x="64" y="72" width="896" height="888" rx="200" fill="#DCE0E6"/><rect x="64" y="64" width="896" height="880" rx="200" fill="${paper}"/><g transform="translate(128 120) scale(3)">${mark()}</g>`,
      'Mochi macOS app icon',
    ),
  ],
]);

// The template bitmap must have transparent eye holes, rather than white ink.
assets.set(
  'menu-bar.svg',
  svg(
    22,
    22,
    `<defs><mask id="face" maskUnits="userSpaceOnUse" x="0" y="0" width="256" height="256"><path d="${helmet}" fill="white"/><rect x="78" y="113" width="20" height="48" rx="10" fill="black"/><rect x="158" y="113" width="20" height="48" rx="10" fill="black"/></mask></defs><g transform="translate(0 -1) scale(.086)"><rect width="256" height="256" fill="black" mask="url(#face)"/></g>`,
    'Mochi monochrome template menu-bar icon',
  ),
);

for (const [name, content] of assets) {
  await writeFile(join(output, name), content);
}

const rasterizerIndex = process.argv.indexOf('--sharp-module');
if (rasterizerIndex !== -1) {
  const modulePath = process.argv[rasterizerIndex + 1];
  if (!modulePath)
    throw new Error('--sharp-module requires an installed sharp module path');
  const sharp = createRequire(import.meta.url)(resolve(modulePath));
  for (const name of [
    'mark',
    'logo-light',
    'logo-dark',
    'logo-monochrome-dark',
    'logo-monochrome-light',
  ]) {
    const source = await readFile(join(output, `${name}.svg`));
    const width = name === 'mark' ? 1024 : 2600;
    await sharp(source)
      .resize({ width })
      .png()
      .toFile(join(output, `${name}.png`));
  }
  const icon = await readFile(join(output, 'app-icon.svg'));
  for (const size of [16, 32, 64, 128, 256, 512, 1024]) {
    await sharp(icon)
      .resize(size, size)
      .png()
      .toFile(join(output, `app-icon-${size}.png`));
  }
  const menu = await readFile(join(output, 'menu-bar.svg'));
  for (const size of [22, 44]) {
    await sharp(menu)
      .resize(size, size)
      .png()
      .toFile(join(output, `menu-bar-${size}.png`));
  }
}

const proof = `<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Mochi Signal — identity proof</title>
<style>
*{box-sizing:border-box}body{margin:0;background:#f0f1f4;color:${ink};font:16px/1.5 -apple-system,BlinkMacSystemFont,sans-serif}main{max-width:1100px;margin:auto;padding:40px}h1{font-size:32px;margin:0 0 6px}p{margin:0 0 24px;color:#525a6a}.grid{display:grid;grid-template-columns:1fr 1fr;gap:20px}.card{padding:28px;border-radius:20px;background:${paper};overflow:hidden}.dark{background:${ink};color:${paper}}h2{font-size:14px;letter-spacing:.06em;text-transform:uppercase;margin:0 0 20px}.logo{width:100%;height:120px;object-fit:contain}.character{width:100%;height:340px;object-fit:contain}.sizes{display:flex;align-items:center;gap:22px;height:110px}.sizes img{object-fit:contain;image-rendering:auto}.swatches{display:flex;gap:12px;margin-top:24px}.swatch{width:90px;height:56px;border-radius:12px;display:flex;align-items:center;justify-content:center;font:11px ui-monospace,monospace}.note{margin-top:24px;font-size:13px}.menu{width:22px;height:22px}.dark .menu{filter:invert(1)}@media(max-width:700px){.grid{grid-template-columns:1fr}main{padding:20px}}
</style><main><h1>Mochi / Signal</h1><p>Selected orange–graphite identity · logo, icon and static character proof</p><div class="grid">
<section class="card"><h2>Logo / light</h2><img class="logo" src="logo-light.svg" alt="Mochi logo on a light background"></section>
<section class="card dark"><h2>Logo / dark</h2><img class="logo" src="logo-dark.svg" alt="Mochi logo on a dark background"></section>
<section class="card"><h2>Character / light</h2><img class="character" src="character-idle.png" alt="Signal robot on a light background"></section>
<section class="card dark"><h2>Character / dark</h2><img class="character" src="character-idle.png" alt="Signal robot on a dark background"></section>
<section class="card"><h2>Icon / 16, 32, 64 and 96 CSS pixels</h2><div class="sizes">${[16, 32, 64, 96].map((size) => `<img width="${size}" height="${size}" src="app-icon.svg" alt="Mochi icon at ${size} pixels">`).join('')}</div><h2>Menu bar / 22 pixels</h2><img class="menu" src="menu-bar.svg" alt="Mochi monochrome menu-bar mark"></section>
<section class="card dark"><h2>Icon / dark background</h2><div class="sizes">${[16, 32, 64, 96].map((size) => `<img width="${size}" height="${size}" src="app-icon.svg" alt="Mochi icon at ${size} pixels on dark">`).join('')}</div><h2>Menu bar / dark</h2><img class="menu" src="menu-bar.svg" alt="Mochi inverted template mark"></section>
</div><div class="swatches"><div class="swatch" style="background:${coral}">${coral}</div><div class="swatch" style="background:${ink};color:white">${ink}</div><div class="swatch" style="background:${paper}">${paper}</div></div><p class="note">Static identity proof. Desktop overlay and animation are separate native briefs.</p></main></html>\n`;
await writeFile(join(output, 'proof.html'), proof);
process.stdout.write(`Generated Mochi vector assets in ${output}\n`);
