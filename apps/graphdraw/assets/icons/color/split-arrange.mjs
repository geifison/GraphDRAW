// Splits the sheet of colour To Front, To Back, Combine and Break Apart
// pictures (drawn for GraphDraw by Geifison Queiroz Oliveira, MIT OR
// Apache-2.0) into one square SVG per picture, next to this script. The
// sheet's captions (black) are left out.
//
//   node apps/graphdraw/assets/icons/color/split-arrange.mjs <sheet.svg>
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const dir = dirname(fileURLToPath(import.meta.url));
const sheet = readFileSync(process.argv[2], "utf8");

// Where each picture sits on the sheet (millimetres): a 2 x 2 grid split
// at x 110 and y 140, [name, right column, bottom row].
const cells = [
  ["order-back", false, false],
  ["order-front", true, false],
  ["combine", false, true],
  ["break-apart", true, true],
];

const paths = [...sheet.matchAll(/<path\b[^>]*\/>/g)]
  .map(([el]) => el)
  .filter((el) => !el.includes('fill="#000000"'))
  .map((el) => {
    const d = el.match(/ d="([^"]*)"/)[1];
    const n = d.match(/-?\d+(\.\d+)?/g).map(Number);
    const xs = n.filter((_, i) => i % 2 === 0), ys = n.filter((_, i) => i % 2 === 1);
    const box = [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
    return { el: el.replace(/ id="[^"]*"/, ""), box };
  });

const groups = cells.map(([name, right, bottom]) => {
  const mine = paths.filter(({ box }) => {
    const cx = (box[0] + box[2]) / 2, cy = (box[1] + box[3]) / 2;
    return (cx >= 110) === right && (cy >= 140) === bottom;
  });
  if (!mine.length) throw new Error(`${name}: no paths`);
  const b = mine.reduce((a, { box }) => [
    Math.min(a[0], box[0]), Math.min(a[1], box[1]), Math.max(a[2], box[2]), Math.max(a[3], box[3]),
  ], [Infinity, Infinity, -Infinity, -Infinity]);
  return { name, mine, b };
});

// One square for all four, so their boxes come out the same size.
const SIDE = Math.ceil(Math.max(...groups.map(({ b }) => Math.max(b[2] - b[0], b[3] - b[1]))) + 1);
for (const { name, mine, b } of groups) {
  const vx = ((b[0] + b[2]) / 2 - SIDE / 2).toFixed(3);
  const vy = ((b[1] + b[3]) / 2 - SIDE / 2).toFixed(3);
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="${vx} ${vy} ${SIDE} ${SIDE}">\n${mine.map(p => p.el).join("\n")}\n</svg>\n`;
  writeFileSync(join(dir, `${name}.svg`), svg);
  console.log(name, mine.length, SIDE);
}
