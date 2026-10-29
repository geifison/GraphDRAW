// Splits the sheet of Import, Export, Publish to PDF, Undo and Redo
// pictures (drawn for GraphDraw by Geifison Queiroz Oliveira, MIT OR
// Apache-2.0) into one square SVG per picture, next to this script, in
// white like the rest of the set (the interface tints them with the
// theme's icon colour). The sheet's captions (black) are left out.
// make-icons.mjs does not draw these stems.
//
//   node apps/graphdraw/assets/icons/split-file.mjs <sheet.svg>
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const dir = dirname(fileURLToPath(import.meta.url));
const sheet = readFileSync(process.argv[2], "utf8");

// The pictures sit in one row, left to right in this order.
const names = ["file-import", "file-export", "file-pdf", "undo", "redo"];

const paths = [...sheet.matchAll(/<path\b[^>]*\/>/g)]
  .map(([el]) => el)
  .filter((el) => !el.includes('fill="#000000"'))
  .map((el) => {
    const d = el.match(/ d="([^"]*)"/)[1];
    const n = d.match(/-?\d+(\.\d+)?/g).map(Number);
    const xs = n.filter((_, i) => i % 2 === 0), ys = n.filter((_, i) => i % 2 === 1);
    const box = [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
    return { el: el.replace(/ id="[^"]*"/, "").replace(/ fill="#[0-9a-fA-F]{6}"/, ' fill="#ffffff"'), box };
  })
  .sort((a, b) => a.box[0] - b.box[0]);

// Paths whose x ranges overlap belong to the same picture.
const groups = [];
for (const p of paths) {
  const g = groups[groups.length - 1];
  if (g && p.box[0] <= g.b[2]) {
    g.mine.push(p);
    g.b = [Math.min(g.b[0], p.box[0]), Math.min(g.b[1], p.box[1]),
      Math.max(g.b[2], p.box[2]), Math.max(g.b[3], p.box[3])];
  } else {
    groups.push({ mine: [p], b: [...p.box] });
  }
}
if (groups.length !== names.length) {
  throw new Error(`${groups.length} pictures on the sheet, ${names.length} names`);
}

// One square for all, so their boxes come out the same size.
const SIDE = Math.ceil(Math.max(...groups.map(({ b }) => Math.max(b[2] - b[0], b[3] - b[1]))) + 1);
groups.forEach(({ mine, b }, i) => {
  const vx = ((b[0] + b[2]) / 2 - SIDE / 2).toFixed(3);
  const vy = ((b[1] + b[3]) / 2 - SIDE / 2).toFixed(3);
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="${vx} ${vy} ${SIDE} ${SIDE}">\n${mine.map(p => p.el).join("\n")}\n</svg>\n`;
  writeFileSync(join(dir, `${names[i]}.svg`), svg);
  console.log(names[i], mine.length, SIDE);
});
