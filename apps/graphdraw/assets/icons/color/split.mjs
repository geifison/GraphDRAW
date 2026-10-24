// Splits the sheet of colour Align and Distribute pictures (drawn for
// GraphDraw by Geifison Queiroz Oliveira, MIT OR Apache-2.0) into one
// square SVG per picture, next to this script.
//
//   node apps/graphdraw/assets/icons/color/split.mjs <sheet.svg>
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const dir = dirname(fileURLToPath(import.meta.url));
const sheet = readFileSync(process.argv[2], "utf8");

// Where each picture sits on the sheet (millimetres): row by y, then by x.
const cells = [
  ["align-left", 0, 15, 55],
  ["align-center-v", 0, 60, 100],
  ["align-right", 0, 105, 145],
  ["align-bottom", 0, 150, 195],
  ["align-center-h", 0, 195, 240],
  ["align-top", 0, 240, 285],
  ["dist-right", 1, 15, 55],
  ["dist-center-h", 1, 60, 100],
  ["dist-left", 1, 105, 145],
  ["dist-top", 1, 150, 195],
  ["dist-center-v", 1, 195, 240],
  ["dist-bottom", 1, 240, 285],
];
const SIDE = 34;

const paths = [...sheet.matchAll(/<path\b[^>]*\/>/g)].map(([el]) => {
  const d = el.match(/ d="([^"]*)"/)[1];
  const n = d.match(/-?\d+(\.\d+)?/g).map(Number);
  const xs = n.filter((_, i) => i % 2 === 0), ys = n.filter((_, i) => i % 2 === 1);
  const box = [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
  return { el: el.replace(/ id="[^"]*"/, ""), box };
});

for (const [name, row, x0, x1] of cells) {
  const mine = paths.filter(({ box }) => {
    const cx = (box[0] + box[2]) / 2, cy = (box[1] + box[3]) / 2;
    return cx >= x0 && cx < x1 && (cy < 100) === (row === 0);
  });
  if (!mine.length) throw new Error(`${name}: no paths`);
  const b = mine.reduce((a, { box }) => [
    Math.min(a[0], box[0]), Math.min(a[1], box[1]), Math.max(a[2], box[2]), Math.max(a[3], box[3]),
  ], [Infinity, Infinity, -Infinity, -Infinity]);
  const vx = ((b[0] + b[2]) / 2 - SIDE / 2).toFixed(3);
  const vy = ((b[1] + b[3]) / 2 - SIDE / 2).toFixed(3);
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="${vx} ${vy} ${SIDE} ${SIDE}">\n${mine.map(p => p.el).join("\n")}\n</svg>\n`;
  writeFileSync(join(dir, `${name}.svg`), svg);
  console.log(name, mine.length);
}

// The sheet has no pictures for equal gaps; these two are drawn here in
// its style: blue boxes with a white slot, dark round-ended guides.
const INK = "#4c4968", BLUE = "#2c7cc7", R = 1.9, LINE = 2.81;
const f = (v) => +v.toFixed(3);
// A box from (x0, y0) to (x1, y1) with its slot.
const box = (x0, y0, x1, y1) => {
  const slot = [x0 + 2.6, y0 + 2.6, x1 - 2.6, y1 - 2.6];
  return `<rect x="${f(x0)}" y="${f(y0)}" width="${f(x1 - x0)}" height="${f(y1 - y0)}" rx="${R}" fill="${BLUE}"/>` +
    `<rect x="${f(slot[0])}" y="${f(slot[1])}" width="${f(slot[2] - slot[0])}" height="${f(slot[3] - slot[1])}" fill="#ffffff"/>`;
};
const guide = (x0, y0, x1, y1) =>
  `<rect x="${f(x0)}" y="${f(y0)}" width="${f(x1 - x0)}" height="${f(y1 - y0)}" rx="${f(Math.min(x1 - x0, y1 - y0) / 2)}" fill="${INK}"/>`;

// Along x, in a 34 x 34 box: three boxes with equal gaps, each gap marked
// under them by a guide as long as the gap.
const spaceH = (swap) => {
  const at = swap ? (x0, y0, x1, y1) => [y0, x0, y1, x1] : (...v) => v;
  const bw = 6.6, gap = 6.9;
  const xs = [0.2, 0.2 + bw + gap, 0.2 + 2 * (bw + gap)];
  const hs = [16.54, 23.0, 12.23];
  // Boxes a little narrower than the sheet's so the gaps read at 20 px.
  let out = "";
  xs.forEach((x, i) => { out += box(...at(x, 12.5 - hs[i] / 2, x + bw, 12.5 + hs[i] / 2)); });
  for (const i of [0, 1]) {
    const a = xs[i] + bw, b = xs[i + 1];
    out += guide(...at(a, 30.4 - LINE / 2, b, 30.4 + LINE / 2));
  }
  return `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 34 34">\n${out}\n</svg>\n`;
};
writeFileSync(join(dir, "dist-space-h.svg"), spaceH(false));
writeFileSync(join(dir, "dist-space-v.svg"), spaceH(true));
console.log("dist-space-h, dist-space-v drawn");
