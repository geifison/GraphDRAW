// Gera os ícones do GraphDraw (desenhos próprios do projeto, MPL-2.0).
// Estilo único: grade 24×24, traço 1,75 px, pontas e cantos arredondados,
// monocromático branco — a interface aplica a cor do tema (tint).
//
//   node crates/graphdraw-app/assets/icons/make-icons.mjs
import { writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const dir = dirname(fileURLToPath(import.meta.url));
const dot = (x, y, r = 1.6) => `<circle cx="${x}" cy="${y}" r="${r}" fill="#fff" stroke="none"/>`;
const node = (x, y, s = 2.4) => `<rect x="${x - s}" y="${y - s}" width="${2 * s}" height="${2 * s}" rx="0.6" fill="#fff" stroke="none"/>`;
const hnode = (x, y, s = 2.4) => `<rect x="${x - s}" y="${y - s}" width="${2 * s}" height="${2 * s}" rx="0.6"/>`;

const icons = {
  // ---- ferramentas ----
  "tool-select": `<path d="M6 3.5 L6 19 L10 15 L12.8 20.5 L15.2 19.3 L12.5 13.8 L18 13.5 Z"/>`,
  "tool-shape": `<path d="M4 18 C 7 6, 15 6, 20 12"/>${node(4, 18)}${node(20, 12)}<path d="M12 8 L17.5 4.5"/>${dot(17.5, 4.5)}<path d="M12 8 L6.5 11.5" />${dot(6.5, 11.5)}${node(12, 8, 2)}`,
  "tool-zoom": `<circle cx="10.5" cy="10.5" r="6"/><path d="M15 15 L20.5 20.5"/><path d="M8 10.5 H13 M10.5 8 V13"/>`,
  "tool-pan": `<path d="M8 12 V6.5 a1.5 1.5 0 0 1 3 0 V11 M11 10 V4.5 a1.5 1.5 0 0 1 3 0 V11 M14 10.5 V6 a1.5 1.5 0 0 1 3 0 V13 M8 11.5 a1.5 1.5 0 0 0 -3 0.5 L7.5 17 a5 5 0 0 0 4.5 3 H13 a4 4 0 0 0 4 -4 V12"/>`,
  "tool-bezier": `<path d="M3.5 19 C 6 9, 11 9, 13 13 S 19 17, 20.5 6"/>${node(3.5, 19, 2)}${node(13, 13, 2)}${node(20.5, 6, 2)}`,
  "tool-rect": `<rect x="4" y="6" width="16" height="12" rx="1"/>`,
  "tool-ellipse": `<ellipse cx="12" cy="12" rx="8.5" ry="6.5"/>`,
  // ---- arquivo e histórico ----
  "file-new": `<path d="M6 3 H14 L19 8 V21 H6 Z M14 3 V8 H19"/><path d="M12.5 12 V18 M9.5 15 H15.5"/>`,
  "file-open": `<path d="M3.5 18.5 V6 H9 L11 8 H19 V10.5"/><path d="M3.5 18.5 L6.5 11 H21 L18 18.5 Z"/>`,
  "file-save": `<path d="M4.5 4.5 H16.5 L19.5 7.5 V19.5 H4.5 Z"/><path d="M8 4.5 V9 H15 V4.5 M7.5 19.5 V14 H16.5 V19.5"/>`,
  "file-export": `<path d="M13 4.5 H6 V19.5 H18 V12"/><path d="M12 12 L20 4 M15 4 H20 V9"/>`,
  undo: `<path d="M9 8 L4.5 12 L9 16"/><path d="M4.5 12 H14 a5 5 0 0 1 0 10 H11" transform="translate(0 -3)"/>`,
  redo: `<path d="M15 8 L19.5 12 L15 16"/><path d="M19.5 12 H10 a5 5 0 0 0 0 10 H13" transform="translate(0 -3)"/>`,
  // ---- edição ----
  delete: `<path d="M4.5 7 H19.5 M9.5 7 V4.5 H14.5 V7 M6.5 7 L7.5 20 H16.5 L17.5 7 M10.5 10.5 V16.5 M13.5 10.5 V16.5"/>`,
  duplicate: `<rect x="8" y="8" width="12" height="12" rx="1.5"/><path d="M16 8 V5.5 a1.5 1.5 0 0 0 -1.5 -1.5 H5.5 A1.5 1.5 0 0 0 4 5.5 V14.5 A1.5 1.5 0 0 0 5.5 16 H8"/>`,
  "to-curves": `<rect x="4" y="4" width="16" height="16" rx="1" stroke-dasharray="2.5 2.5"/><path d="M4 16 C 8 6, 14 20, 20 8"/>${node(4, 16, 1.8)}${node(20, 8, 1.8)}`,
  "mirror-h": `<path d="M12 3.5 V20.5" stroke-dasharray="2 2.2"/><path d="M9.5 6.5 L3.5 17.5 H9.5 Z"/><path d="M14.5 6.5 L20.5 17.5 H14.5 Z" fill="#fff"/>`,
  "mirror-v": `<path d="M3.5 12 H20.5" stroke-dasharray="2 2.2"/><path d="M6.5 9.5 L17.5 3.5 V9.5 Z"/><path d="M6.5 14.5 L17.5 20.5 V14.5 Z" fill="#fff"/>`,
  "rotate-ccw": `<path d="M5 12 a7 7 0 1 0 2.2 -5.1"/><path d="M4.5 3.8 V7.5 H8.2"/>`,
  "rotate-cw": `<path d="M19 12 a7 7 0 1 1 -2.2 -5.1"/><path d="M19.5 3.8 V7.5 H15.8"/>`,
  // ---- ordem ----
  "order-front": `<rect x="3.5" y="9.5" width="9" height="9" rx="1" stroke-dasharray="2 2"/><rect x="9" y="4" width="11" height="11" rx="1" fill="#fff"/>`,
  "order-back": `<rect x="9" y="4" width="11" height="11" rx="1" fill="#fff"/><rect x="3.5" y="9.5" width="9" height="9" rx="1"/>`,
  "order-forward": `<rect x="4" y="9" width="10" height="10" rx="1"/><path d="M17 15 V5 M13.5 8.5 L17 5 L20.5 8.5"/>`,
  "order-backward": `<rect x="4" y="5" width="10" height="10" rx="1"/><path d="M17 9 V19 M13.5 15.5 L17 19 L20.5 15.5"/>`,
  // ---- alinhamento ----
  "align-left": `<path d="M4 3.5 V20.5"/><rect x="7" y="6" width="12" height="4" rx="1"/><rect x="7" y="14" width="7" height="4" rx="1"/>`,
  "align-center-h": `<path d="M12 3.5 V20.5"/><rect x="5" y="6" width="14" height="4" rx="1"/><rect x="8" y="14" width="8" height="4" rx="1"/>`,
  "align-right": `<path d="M20 3.5 V20.5"/><rect x="5" y="6" width="12" height="4" rx="1"/><rect x="10" y="14" width="7" height="4" rx="1"/>`,
  "align-top": `<path d="M3.5 4 H20.5"/><rect x="6" y="7" width="4" height="12" rx="1"/><rect x="14" y="7" width="4" height="7" rx="1"/>`,
  "align-center-v": `<path d="M3.5 12 H20.5"/><rect x="6" y="5" width="4" height="14" rx="1"/><rect x="14" y="8" width="4" height="8" rx="1"/>`,
  "align-bottom": `<path d="M3.5 20 H20.5"/><rect x="6" y="5" width="4" height="12" rx="1"/><rect x="14" y="10" width="4" height="7" rx="1"/>`,
  // ---- encaixe e grade ----
  snap: `<path d="M6 4 V12 a6 6 0 0 0 12 0 V4 H14 V12 a2 2 0 0 1 -4 0 V4 Z"/><path d="M6 7.5 H10 M14 7.5 H18"/>`,
  grid: `<rect x="4" y="4" width="16" height="16" rx="1"/><path d="M9.3 4 V20 M14.7 4 V20 M4 9.3 H20 M4 14.7 H20"/>`,
  // ---- nós ----
  "node-add": `<path d="M3.5 17 C 7 7, 13 7, 15 11"/>${node(3.5, 17, 2)}<path d="M18 12 V20 M14 16 H22"/>`,
  "node-delete": `<path d="M3.5 17 C 7 7, 13 7, 15 11"/>${node(3.5, 17, 2)}<path d="M14 16 H22"/>`,
  "node-join": `<path d="M3 17 C 5 9, 9 8, 11 12"/><path d="M21 17 C 19 9, 15 8, 13 12"/>${node(12, 12, 2.2)}`,
  "node-connect": `<path d="M3 17 C 4 10, 7 9, 8.5 12"/><path d="M21 17 C 20 10, 17 9, 15.5 12"/><path d="M8.5 12 H15.5" stroke-dasharray="1.6 2"/>${node(8.5, 12, 1.8)}${node(15.5, 12, 1.8)}`,
  "node-break": `<path d="M3 17 C 5 9, 8 8, 10 11"/><path d="M21 17 C 19 9, 16 8, 14 11"/>${node(10, 11, 1.8)}${node(14, 11, 1.8)}<path d="M12 15 V21"/>`,
  "path-close": `<path d="M5 18 L5 7 L12 4 L19 7 L19 18 Z"/>${node(5, 18, 1.8)}`,
  "path-open": `<path d="M5 18 L5 7 L12 4 L19 7 L19 18"/>${node(5, 18, 1.8)}${node(19, 18, 1.8)}`,
  "path-reverse": `<path d="M4 9 H18 M14.5 5.5 L18 9 L14.5 12.5"/><path d="M20 16 H6 M9.5 12.5 L6 16 L9.5 19.5"/>`,
  "seg-line": `<path d="M4 18 L20 6"/>${node(4, 18, 2)}${node(20, 6, 2)}`,
  "seg-curve": `<path d="M4 18 C 6 6, 14 4, 20 6"/>${node(4, 18, 2)}${node(20, 6, 2)}`,
  "node-corner": `<path d="M3 19 L12 6 L21 19"/>${node(12, 6, 2.2)}`,
  "node-smooth": `<path d="M3 18 C 6 8, 18 8, 21 18"/><path d="M5.5 10.5 H18.5"/>${dot(5.5, 10.5, 1.4)}${dot(18.5, 10.5, 1.4)}${hnode(12, 10.5, 2)}`,
  "node-symmetric": `<path d="M3 18 C 6 8, 18 8, 21 18"/><path d="M6.5 10.5 H17.5"/>${dot(6.5, 10.5, 1.4)}${dot(17.5, 10.5, 1.4)}${node(12, 10.5, 2)}`,
  // ---- atributos ----
  "fill-none": `<rect x="4" y="4" width="16" height="16" rx="1"/><path d="M5 19 L19 5"/>`,
  "stroke-none": `<rect x="5" y="5" width="14" height="14" rx="1" stroke-dasharray="2 2"/><path d="M5 19 L19 5"/>`,
  "fit-page": `<path d="M4 9 V4 H9 M15 4 H20 V9 M20 15 V20 H15 M9 20 H4 V15"/><rect x="8" y="8" width="8" height="8" rx="0.8"/>`,
  "fit-selection": `<path d="M4 9 V4 H9 M15 4 H20 V9 M20 15 V20 H15 M9 20 H4 V15"/>${node(9.5, 9.5, 1.6)}${node(14.5, 14.5, 1.6)}<rect x="9.5" y="9.5" width="5" height="5" stroke-dasharray="1.5 1.5"/>`,
  report: `<path d="M6 3.5 H18 V20.5 H6 Z M9 8 H15 M9 12 H15 M9 16 H12.5"/>`,
  history: `<circle cx="12" cy="12" r="8"/><path d="M12 7.5 V12 L15 14"/>`,
};

const header = `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">`;
for (const [name, body] of Object.entries(icons)) {
  writeFileSync(join(dir, `${name}.svg`), `${header}${body}</svg>\n`);
}
// Folha de prova para revisão visual de todos os ícones lado a lado.
const cols = 10;
const names = Object.keys(icons);
const rows = Math.ceil(names.length / cols);
let sheet = `<svg xmlns="http://www.w3.org/2000/svg" width="${cols * 64}" height="${rows * 64}" viewBox="0 0 ${cols * 64} ${rows * 64}"><rect width="100%" height="100%" fill="#2b2d31"/>`;
names.forEach((n, i) => {
  const x = (i % cols) * 64 + 8, y = Math.floor(i / cols) * 64 + 8;
  sheet += `<g transform="translate(${x} ${y}) scale(2)" fill="none" stroke="#fff" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">${icons[n]}</g>`;
});
writeFileSync(join(dir, "..", "icons-sheet.svg"), sheet + "</svg>\n");
console.log(`${names.length} ícones`);
