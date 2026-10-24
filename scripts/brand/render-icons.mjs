// Renders the application and .gdr icons from their SVG sources in
// assets/brand (app.svg, gdr.svg) into the PNG sizes and the .ico files
// the build and the packaging use:
//   assets/brand/{app,gdr}-{16,20,24,32,40,48,64,128,256,512,1024}.png
//   assets/brand/{app,gdr}.ico (16 to 256 px, PNG entries)
// Then run scripts/brand/make-assets.ps1 for the Store tiles.
//
// Needs @resvg/resvg-js, resolved from the current directory:
//   npm i --no-save @resvg/resvg-js
//   node <repo>/scripts/brand/render-icons.mjs
import { createRequire } from "node:module";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const { Resvg } = createRequire(join(process.cwd(), "/"))("@resvg/resvg-js");
const brand = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "assets", "brand");
const sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256, 512, 1024];
const icoSizes = [16, 24, 32, 48, 64, 128, 256];

// An .ico whose entries are PNG images (Windows Vista and later).
function ico(images) {
  const head = Buffer.alloc(6 + 16 * images.length);
  head.writeUInt16LE(0, 0);
  head.writeUInt16LE(1, 2);
  head.writeUInt16LE(images.length, 4);
  let offset = head.length;
  images.forEach(([size, png], i) => {
    const e = 6 + 16 * i;
    head.writeUInt8(size >= 256 ? 0 : size, e);
    head.writeUInt8(size >= 256 ? 0 : size, e + 1);
    head.writeUInt16LE(1, e + 4);
    head.writeUInt16LE(32, e + 6);
    head.writeUInt32LE(png.length, e + 8);
    head.writeUInt32LE(offset, e + 12);
    offset += png.length;
  });
  return Buffer.concat([head, ...images.map(([, png]) => png)]);
}

for (const kind of ["app", "gdr"]) {
  const svg = readFileSync(join(brand, `${kind}.svg`));
  const pngs = new Map();
  for (const size of sizes) {
    const png = new Resvg(svg, { fitTo: { mode: "width", value: size } }).render().asPng();
    writeFileSync(join(brand, `${kind}-${size}.png`), png);
    pngs.set(size, png);
  }
  writeFileSync(join(brand, `${kind}.ico`), ico(icoSizes.map((s) => [s, pngs.get(s)])));
  console.log(`${kind}: ${sizes.length} PNG sizes and ${kind}.ico`);
}
