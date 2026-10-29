// Gera os ícones do GraphDraw (desenhos próprios do projeto, MIT OR Apache-2.0).
// Estilo único (docs/design/DESIGN-SYSTEM.md, seção 3): traço de 1,5 px na
// grade em que o ícone é exibido (20 px na interface, 16 px nos
// microcontroles, 24 px nos cursores), pontas e cantos arredondados,
// monocromático branco — a interface aplica a cor do tema (tint).
// Os desenhos abaixo são feitos num rascunho 24×24, dentro da área 3..21.
//
//   node crates/graphdraw-app/assets/icons/make-icons.mjs
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const dir = dirname(fileURLToPath(import.meta.url));
const dot = (x, y, r = 1.6) => `<circle cx="${x}" cy="${y}" r="${r}" fill="#fff" stroke="none"/>`;
const node = (x, y, s = 2.4) => `<rect x="${x - s}" y="${y - s}" width="${2 * s}" height="${2 * s}" rx="0.6" fill="#fff" stroke="none"/>`;
const hnode = (x, y, s = 2.4) => `<rect x="${x - s}" y="${y - s}" width="${2 * s}" height="${2 * s}" rx="0.6"/>`;

// Seta de "escolher o alvo" no canto (À frente de… / Atrás de…).
const pick = (x, y) => `<path d="M${x} ${y} V${y + 6.5} L${x + 1.7} ${y + 5} L${x + 2.8} ${y + 7.3} L${x + 4} ${y + 6.7} L${x + 2.9} ${y + 4.4} L${x + 5.2} ${y + 4.2} Z" fill="#fff"/>`;
// "Todos": três pontos embaixo (Mostrar todos, Desbloquear todos).
const all = `${dot(7, 19.5, 1.3)}${dot(12, 19.5, 1.3)}${dot(17, 19.5, 1.3)}`;
// Seta da origem para o destino (copiar propriedades).
const hand = `<path d="M11.5 7.5 H16.5 V10.5 M14.8 8.8 L16.5 10.5 L18.2 8.8"/>`;

const icons = {
  // ---- ferramentas ----
  "tool-select": `<path d="M6 3.5 L6 19 L10 15 L12.8 20.5 L15.2 19.3 L12.5 13.8 L18 13.5 Z"/>`,
  // Forma: desenho do responsável pelo produto (2026-10-18), seta com nós e
  // alça; traços da curva e da alça engrossados para ler em 20 px.
  "tool-shape": `<path d="M7.089 20.193C7.089 20.193 9.876 13.183 18.672 14.352" stroke-width="1.6"/><path d="M11.829 15.73L13.138 15.423L12.832 14.115L11.523 14.421Z" fill="#fff" stroke="none"/><path d="M6.587 21L7.896 20.694L7.59 19.385L6.281 19.691Z" fill="#fff" stroke="none"/><path d="M8.558 16.586L16.046 13.018" stroke-width="0.9"/><path d="M18.212 15.333L19.52 15.027L19.214 13.719L17.906 14.025Z" fill="#fff" stroke="none"/><path d="M8.842 16.52C8.805 16.363 8.648 16.266 8.492 16.303C8.336 16.339 8.238 16.496 8.275 16.652C8.311 16.809 8.468 16.906 8.625 16.87C8.781 16.833 8.878 16.676 8.842 16.52Z" fill="#fff" stroke="none"/><path d="M16.33 12.952C16.293 12.795 16.137 12.698 15.98 12.735C15.824 12.771 15.726 12.928 15.763 13.084C15.8 13.241 15.956 13.338 16.113 13.301C16.269 13.265 16.366 13.108 16.33 12.952Z" fill="#fff" stroke="none"/><path d="M4.48 3L5.371 17.207L9.388 11.798L16.338 10.875L4.48 3Z" fill="#fff" stroke="none"/>`,
  "tool-zoom": `<circle cx="10.5" cy="10.5" r="6"/><path d="M15 15 L20.5 20.5"/><path d="M8 10.5 H13 M10.5 8 V13"/>`,
  "tool-pan": `<path d="M8 12 V6.5 a1.5 1.5 0 0 1 3 0 V11 M11 10 V4.5 a1.5 1.5 0 0 1 3 0 V11 M14 10.5 V6 a1.5 1.5 0 0 1 3 0 V13 M8 11.5 a1.5 1.5 0 0 0 -3 0.5 L7.5 17 a5 5 0 0 0 4.5 3 H13 a4 4 0 0 0 4 -4 V12"/>`,
  "tool-bezier": `<path d="M3.5 19 C 6 9, 11 9, 13 13 S 19 17, 20.5 6"/>${node(3.5, 19, 2)}${node(13, 13, 2)}${node(20.5, 6, 2)}`,
  "tool-rect": `<rect x="4" y="6" width="16" height="12" rx="1"/>`,
  "tool-rect-3pt": `<path d="M3.5 14 L13 5 L17.8 10.1 L8.3 19.1 Z"/>${dot(3.5, 14)}${dot(13, 5)}${dot(17.8, 10.1)}`,
  "tool-artboard": `<rect x="6.5" y="6.5" width="11" height="11" rx="1"/><path d="M3.5 7 V3.5 H7 M17 3.5 H20.5 V7 M20.5 17 V20.5 H17 M7 20.5 H3.5 V17"/>`,
  "tool-ellipse": `<ellipse cx="12" cy="12" rx="8.5" ry="6.5"/>`,
  "tool-ellipse-3pt": `<ellipse cx="12" cy="12" rx="8.5" ry="5" transform="rotate(-35 12 12)"/>${dot(5, 16.9)}${dot(19, 7.1)}${dot(14.9, 16.1)}`,
  "tool-polygon": `<path d="M12 3.5 L20.1 9.4 L17 18.9 H7 L3.9 9.4 Z"/>`,
  "tool-spiral": `<path d="M12 12 C 12 10.5, 14 10.5, 14 12 C 14 14.5, 10 14.5, 10 12 C 10 8.5, 16 8.5, 16 12 C 16 16.5, 8 16.5, 8 12 C 8 6.5, 18 6.5, 18 12 C 18 18.5, 5.5 18.5, 5.5 12"/>`,
  "tool-graph": `<rect x="4" y="5" width="16" height="14" rx="1"/><path d="M9.33 5 V19 M14.67 5 V19 M4 9.67 H20 M4 14.33 H20"/>`,
  "poly-star": `<path d="M12 3.5 L14.2 9.3 L20.3 9.4 L15.5 13.2 L17.2 19.2 L12 15.7 L6.8 19.2 L8.5 13.2 L3.7 9.4 L9.8 9.3 Z"/>`,
  "poly-complex": `<path d="M12 3.5 L17.2 19.2 L3.7 9.4 H20.3 L6.8 19.2 Z"/>`,
  "tool-gradient": `<rect x="3.5" y="3.5" width="17" height="17" rx="2" stroke-dasharray="2 2"/><path d="M7 17 L17 7"/>${hnode(7, 17, 2)}${node(17, 7, 2)}${dot(12, 12, 1.5)}`,
  "tool-fill-solid": `<path d="M5 11 L11 5 L19 13 L13 19 Z" fill="#fff"/><path d="M11 5 L9 3"/><path d="M19.5 15.5 C 19.5 15.5 21 17.5 21 18.5 a1.5 1.5 0 0 1 -3 0 C 18 17.5 19.5 15.5 19.5 15.5 Z" fill="#fff"/>`,
  "tool-eyedropper": `<path d="M14.5 4.5 a2.5 2.5 0 0 1 3.5 3.5 L16 10 L14 8 Z" fill="#fff"/><path d="M15 9 L7 17 L5 19.5 L4.5 19 L7 17"/><path d="M13 7 L17 11"/>`,
  "tool-freehand": `<path d="M3.5 17 C 6 10, 8 18, 11 12 S 15 5, 17 10 S 19.5 15, 20.5 9"/>`,
  "tool-bspline": `<path d="M3.5 18 L8 5.5 L16 5.5 L20.5 18" stroke-dasharray="1.6 1.6" stroke-width="1"/><path d="M3.5 18 C 6 10, 9 7.5, 12 7.5 C 15 7.5, 18 10, 20.5 18"/>${node(3.5, 18, 1.8)}${hnode(8, 5.5, 1.6)}${hnode(16, 5.5, 1.6)}${node(20.5, 18, 1.8)}`,
  "tool-smart-draw": `<path d="M3.5 13 C 3 7, 8 4, 11 6.5" stroke-dasharray="1.8 1.8"/><rect x="10.5" y="9.5" width="10" height="10" rx="1"/><path d="M6 19 L8 17 M5 16.5 L7.5 19"/>`,
  "tool-polyline": `<path d="M3.5 18 L8.5 8 L13 15 C 15 18, 18 9, 20.5 6"/>${node(3.5, 18, 1.8)}${node(8.5, 8, 1.8)}${node(13, 15, 1.8)}`,
  "tool-line2": `<path d="M5 19 L19 5"/>${node(5, 19, 2)}${node(19, 5, 2)}`,
  "tool-curve3": `<path d="M3.5 18 C 6 6, 18 6, 20.5 18"/>${node(3.5, 18, 1.8)}${node(20.5, 18, 1.8)}${dot(12, 9, 1.8)}`,
  "tool-free-transform": `<path d="M6 3.5 L6 15 L9 12 L11 16.5 L13 15.5 L11 11.2 L15 11 Z" fill="#fff"/><path d="M15 20.5 A 6 6 0 0 0 20.5 15"/><path d="M18.5 13.5 L20.5 15 L22 13"/>`,
  "tool-dimension": `<path d="M4 8 V18 M20 8 V18 M4 13 H20 M4 13 L7.5 11 M4 13 L7.5 15 M20 13 L16.5 11 M20 13 L16.5 15"/>`,
  "shape-basic": `<path d="M3.5 9 H13 V5 L20.5 12 L13 19 V15 H3.5 Z"/>`,
  "file-save-as": `<path d="M11 19.5 H4.5 V4.5 H15 L18.5 8 V10.5"/><rect x="7.5" y="4.5" width="6" height="4"/><path d="M13 20.5 L13.5 18 L19.5 12 L21.3 13.8 L15.3 19.8 Z"/>`,
  "file-print": `<path d="M7 8 V3.5 H17 V8"/><rect x="3.5" y="8" width="17" height="8" rx="1.5"/><path d="M7 13.5 H17 V20.5 H7 Z"/><path d="M9.5 16.5 H14.5 M9.5 18.5 H13"/>`,
  "doc-close": `<path d="M5.5 3.5 H14 L18.5 8 V20.5 H5.5 Z"/><path d="M9.5 11.5 L14.5 16.5 M14.5 11.5 L9.5 16.5"/>`,
  "copy": `<rect x="8.5" y="8.5" width="11" height="11" rx="1.5"/><path d="M15.5 8.5 V5.5 A1 1 0 0 0 14.5 4.5 H5.5 A1 1 0 0 0 4.5 5.5 V14.5 A1 1 0 0 0 5.5 15.5 H8.5"/>`,
  "cut": `<circle cx="6.5" cy="17.5" r="2.5"/><circle cx="17.5" cy="17.5" r="2.5"/><path d="M8.3 15.7 L17 4 M15.7 15.7 L7 4"/>`,
  "repeat": `<path d="M4.5 12 A7.5 7.5 0 0 1 17.5 7"/><path d="M18 3.5 V7.5 H14"/><path d="M19.5 12 A7.5 7.5 0 0 1 6.5 17"/><path d="M6 20.5 V16.5 H10"/>`,
  "select-invert": `<rect x="3.5" y="3.5" width="17" height="17" rx="1" stroke-dasharray="2 2"/><path d="M3.5 3.5 H12 V20.5 H3.5 Z" fill="#fff" stroke="none" opacity="0.5"/><path d="M12 3.5 V20.5"/>`,
  "select-same": `<rect x="3.5" y="4.5" width="7" height="7" rx="1"/><rect x="13.5" y="4.5" width="7" height="7" rx="1"/><path d="M7 16.5 L10 19.5 L17 14"/>`,
  "nest": `<rect x="3.5" y="3.5" width="17" height="17" rx="1"/><rect x="6" y="6" width="5" height="5" rx="0.5"/><rect x="13" y="6" width="5" height="5" rx="0.5"/><rect x="6" y="13" width="12" height="5" rx="0.5"/>`,
  "reg-marks": `<circle cx="12" cy="12" r="5"/><path d="M12 3.5 V20.5 M3.5 12 H20.5"/>`,
  "spell": `<path d="M3.5 14.5 L7 4.5 L10.5 14.5 M5 10.5 H9"/><path d="M12.5 4.5 V14.5 M12.5 4.5 H15.5 A2.5 2.5 0 0 1 15.5 9.5 H12.5 M12.5 9.5 H16 A2.5 2.5 0 0 1 16 14.5 H12.5"/><path d="M11 18 L13.5 20.5 L20.5 15"/>`,
  "outline-pen": `<path d="M12 3.5 L17 9 L13.5 20.5 H10.5 L7 9 Z"/><path d="M12 12 V3.5"/>`,
  "text-path-detach": `<path d="M3.5 18 C 7 10, 13 10, 20.5 16" stroke-dasharray="2 2"/><path d="M6 9 L8.5 3.5 L11 9 M7 7 H10"/><path d="M14 9.5 L18.5 5 M18.5 9.5 L14 5"/>`,
  "to-bitmap": `<rect x="3.5" y="3.5" width="17" height="17" rx="1"/><path d="M3.5 9.2 H20.5 M3.5 14.8 H20.5 M9.2 3.5 V20.5 M14.8 3.5 V20.5"/><rect x="9.2" y="9.2" width="5.6" height="5.6" fill="#fff" stroke="none"/>`,
  "zoom-in": `<circle cx="10.5" cy="10.5" r="6.5"/><path d="M15.5 15.5 L20.5 20.5 M7.5 10.5 H13.5 M10.5 7.5 V13.5"/>`,
  "zoom-out": `<circle cx="10.5" cy="10.5" r="6.5"/><path d="M15.5 15.5 L20.5 20.5 M7.5 10.5 H13.5"/>`,
  "zoom-actual": `<circle cx="10.5" cy="10.5" r="6.5"/><path d="M15.5 15.5 L20.5 20.5 M7.5 8.5 L8.5 8 V13 M12.5 8.5 L13.5 8 V13"/>`,
  "page-first": `<path d="M6.5 5 V19 M17.5 5 L10.5 12 L17.5 19"/>`,
  "page-last": `<path d="M17.5 5 V19 M6.5 5 L13.5 12 L6.5 19"/>`,
  "page-jump": `<path d="M5 3.5 H12.5 L16.5 7.5 V10 M16.5 18 V20.5 H5 Z"/><path d="M9 14 H20.5 M17.5 11 L20.5 14 L17.5 17"/>`,
  "page-rename": `<path d="M5.5 3.5 H13 L17 7.5 V20.5 H5.5 Z"/><path d="M8.5 11 H14 M11.2 11 V17"/>`,
  "text-props": `<path d="M3.5 5 H13 M8.2 5 V18"/><path d="M15 9 H20.5 M15 13.5 H20.5 M15 18 H20.5"/>`,
  "glyphs": `<path d="M5 19.5 H9.5 V17 A6.5 6.5 0 1 1 14.5 17 V19.5 H19"/>`,
  "text-edit": `<path d="M3.5 5.5 H14.5 M3.5 9.5 H11.5 M3.5 13.5 H9"/><path d="M11 20.5 L11.5 18 L18.5 11 L20.5 13 L13.5 20 Z"/>`,
  "text-bigger": `<path d="M3.5 18 L8 6 L12.5 18 M5.2 14 H10.8"/><path d="M17.5 7 V13 M14.5 10 H20.5"/>`,
  "text-smaller": `<path d="M3.5 18 L8 6 L12.5 18 M5.2 14 H10.8"/><path d="M14.5 10 H20.5"/>`,
  "tool-artistic": `<path d="M4 19.5 C 8 19.5, 8 14, 11 13 L14 16 C 13 19, 8 20.5, 4 19.5 Z"/><path d="M12.5 14.5 L19.5 4.5 L20.5 5.5 L15 15.5"/>`,
  "tool-mesh": `<rect x="3.5" y="3.5" width="17" height="17" rx="1"/><path d="M3.5 10 C 9 8, 15 12, 20.5 9.5 M3.5 15 C 9 13, 15 17, 20.5 14.5 M10 3.5 C 8 9, 12 15, 9.5 20.5 M15 3.5 C 13 9, 17 15, 14.5 20.5"/>`,
  "tp-tracking": `<path d="M3 15 L5.5 7 L8 15 M3.8 12.5 H7.2 M16 15 L18.5 7 L21 15 M16.8 12.5 H20.2"/><path d="M9.5 18.5 H14.5 M9.5 18.5 L11 17 M9.5 18.5 L11 20 M14.5 18.5 L13 17 M14.5 18.5 L13 20"/>`,
  "tp-rotate": `<path d="M3 19 Q12 5 21 19" stroke-dasharray="1.6 1.6"/><rect x="5.4" y="9.1" width="3.2" height="4.8" rx="0.4" transform="rotate(-40 7 11.5)"/><rect x="10.4" y="5.799999999999999" width="3.2" height="4.8" rx="0.4" transform="rotate(0 12 8.2)"/><rect x="15.4" y="9.1" width="3.2" height="4.8" rx="0.4" transform="rotate(40 17 11.5)"/>`,
  "tp-upright": `<path d="M3 19 Q12 5 21 19" stroke-dasharray="1.6 1.6"/><rect x="5.4" y="9.1" width="3.2" height="4.8" rx="0.4" transform="rotate(0 7 11.5)"/><rect x="10.4" y="5.799999999999999" width="3.2" height="4.8" rx="0.4" transform="rotate(0 12 8.2)"/><rect x="15.4" y="9.1" width="3.2" height="4.8" rx="0.4" transform="rotate(0 17 11.5)"/>`,
  "tp-skew-v": `<path d="M3 19 Q12 5 21 19" stroke-dasharray="1.6 1.6"/><path d="M5.4 14.2 L8.6 11.6 V7 L5.4 9.6 Z M10.4 10.6 H13.6 V5.8 H10.4 Z M15.4 11.6 L18.6 14.2 V9.6 L15.4 7 Z"/>`,
  "tp-skew-h": `<path d="M3 19 Q12 5 21 19" stroke-dasharray="1.6 1.6"/><path d="M4.8 13 H8 L10 8.2 H6.8 Z M10.4 10.6 H13.6 V5.8 H10.4 Z M14 8.2 H17.2 L19.2 13 H16 Z"/>`,
  "tp-warp": `<path d="M3 19 Q12 5 21 19" stroke-dasharray="1.6 1.6"/><path d="M4.6 14.6 Q6 12 8.2 10.8 L6.6 7.6 Q3.6 9.4 2.4 13 Z M9.6 10.2 Q12 9.3 14.4 10.2 L15.2 6.4 Q12 5.3 8.8 6.4 Z M15.8 10.8 Q18 12 19.4 14.6 L21.6 13 Q20.4 9.4 17.4 7.6 Z"/>`,
  "tpv-baseline": `<path d="M3.5 16.5 L7.5 5 L11.5 16.5 M4.9 12.5 H10.1"/><circle cx="16.5" cy="12.5" r="3"/><path d="M19.5 9.5 V18.5 Q19.5 21 16.5 21 Q14.8 21 14 20"/><path d="M2 16.5 H22" stroke-dasharray="1.8 1.4"/>`,
  "tpv-descender": `<path d="M3.5 16.5 L7.5 5 L11.5 16.5 M4.9 12.5 H10.1"/><circle cx="16.5" cy="12.5" r="3"/><path d="M19.5 9.5 V18.5 Q19.5 21 16.5 21 Q14.8 21 14 20"/><path d="M2 21 H22" stroke-dasharray="1.8 1.4"/>`,
  "tpv-xheight": `<path d="M3.5 16.5 L7.5 5 L11.5 16.5 M4.9 12.5 H10.1"/><circle cx="16.5" cy="12.5" r="3"/><path d="M19.5 9.5 V18.5 Q19.5 21 16.5 21 Q14.8 21 14 20"/><path d="M2 9.5 H22" stroke-dasharray="1.8 1.4"/>`,
  "tpv-cap": `<path d="M3.5 16.5 L7.5 5 L11.5 16.5 M4.9 12.5 H10.1"/><circle cx="16.5" cy="12.5" r="3"/><path d="M19.5 9.5 V18.5 Q19.5 21 16.5 21 Q14.8 21 14 20"/><path d="M2 5 H22" stroke-dasharray="1.8 1.4"/>`,
  "tpv-center": `<path d="M3.5 16.5 L7.5 5 L11.5 16.5 M4.9 12.5 H10.1"/><circle cx="16.5" cy="12.5" r="3"/><path d="M19.5 9.5 V18.5 Q19.5 21 16.5 21 Q14.8 21 14 20"/><path d="M2 11.5 H22" stroke-dasharray="1.8 1.4"/>`,
  "tp-offset-x": `<path d="M3 19 Q12 5 21 19" stroke-dasharray="1.6 1.6"/><path d="M6 16.5 Q12 8 18 16.5"/><path d="M15.6 13.4 L18 16.5 L18.4 12.6"/>`,
  "tp-offset-y": `<path d="M3 20 Q12 10 21 20" stroke-dasharray="1.6 1.6"/><path d="M12 14.5 V3.5 M9.5 6 L12 3.5 L14.5 6 M9.5 12 L12 14.5 L14.5 12"/>`,
  "tp-reverse": `<path d="M3 19 Q12 5 21 19" stroke-dasharray="1.6 1.6"/><path d="M17 9.5 Q12 6.5 7 9.5"/><path d="M9.6 7.3 L7 9.5 L10.2 10.6"/>`,
  "tp-flip-side": `<path d="M3 12 Q12 4 21 12" stroke-dasharray="1.6 1.6"/><rect x="6.4" y="3.9" width="3.2" height="4.8" rx="0.4" transform="rotate(-30 8 6.3)"/><rect x="14.4" y="3.9" width="3.2" height="4.8" rx="0.4" transform="rotate(30 16 6.3)"/><path d="M12 12.5 V20.5 M10 18.5 L12 20.5 L14 18.5" />`,
  "tp-corner-jump": `<path d="M3.5 20.5 H19.5 V3.5" stroke-dasharray="1.6 1.6"/><rect x="11" y="15" width="3.5" height="4" rx="0.4"/><rect x="14.8" y="10" width="4" height="3.5" rx="0.4"/><path d="M15 18.5 Q18 18 18 15" />`,
  "tp-corner-wrap": `<path d="M3.5 20.5 H19.5 V3.5" stroke-dasharray="1.6 1.6"/><path d="M13.5 19.5 V16 H19 V10.5 H15.5 V16"/>`,
  "tp-corner-split": `<path d="M3.5 20.5 H19.5 V3.5" stroke-dasharray="1.6 1.6"/><rect x="5" y="16" width="3" height="3.5" rx="0.4"/><rect x="11" y="16" width="3" height="3.5" rx="0.4"/><rect x="15" y="12" width="3.5" height="3" rx="0.4"/><rect x="15" y="5.5" width="3.5" height="3" rx="0.4"/>`,
  "tp-relax": `<path d="M3 5 Q12 19 21 5" stroke-dasharray="1.6 1.6"/><rect x="5.9" y="7.799999999999999" width="3.2" height="4.8" rx="0.4" transform="rotate(35 7.5 10.2)"/><rect x="14.9" y="7.799999999999999" width="3.2" height="4.8" rx="0.4" transform="rotate(-35 16.5 10.2)"/><path d="M10.5 6 H13.5 M10.5 6 L11.8 4.8 M13.5 6 L12.2 4.8"/>`,
  "tp-badge": `<circle cx="12" cy="12" r="6" stroke-dasharray="1.6 1.6"/><path d="M5.2 6.8 A8.6 8.6 0 0 1 18.8 6.8 M5.2 17.2 A8.6 8.6 0 0 0 18.8 17.2"/>`,
  "tp-construction": `<path d="M3 19 Q12 5 21 19" stroke-dasharray="1 2.2"/><path d="M4 4 L20 20"/><rect x="10.4" y="5.1" width="3.2" height="4.8" rx="0.4" transform="rotate(0 12 7.5)"/>`,
  "tp-unlink": `<path d="M3 19 Q7 12 10 10.5 M14 10.5 Q17 12 21 19"/><path d="M10.5 5 L9 3.5 M13.5 5 L15 3.5 M12 5.5 V3"/><rect x="10.4" y="10.6" width="3.2" height="4.8" rx="0.4" transform="rotate(0 12 13)"/>`,
  "tp-straighten": `<path d="M3 11 Q12 2 21 11" stroke-dasharray="1.6 1.6"/><path d="M3 19.5 H21"/><path d="M12 9.5 V16 M10 14 L12 16 L14 14"/>`,
  "tool-contour": `<rect x="9" y="9" width="6" height="6" rx="1"/><rect x="6" y="6" width="12" height="12" rx="2.5"/><rect x="3" y="3" width="18" height="18" rx="4.5" stroke-dasharray="2 1.6"/>`,
  "tool-blend": `<circle cx="5.5" cy="15.5" r="3"/><rect x="14.5" y="3.5" width="6" height="6" rx="1"/><circle cx="9.5" cy="12" r="2.3" stroke-dasharray="1.4 1.2"/><rect x="11.2" y="6.8" width="4" height="4" rx="1" stroke-dasharray="1.4 1.2"/>`,
  "tool-shadow": `<rect x="8.5" y="8.5" width="12" height="12" rx="1.5" fill="#fff" stroke="none" opacity="0.35"/><rect x="3.5" y="3.5" width="12" height="12" rx="1.5"/><path d="M16 12 L18.5 14.5"/>`,
  "tool-extrude": `<path d="M3.5 9.5 H13.5 V19.5 H3.5 Z"/><path d="M3.5 9.5 L9.5 3.5 H19.5 V13.5 L13.5 19.5 M13.5 9.5 L19.5 3.5" /><path d="M13.5 9.5 L19.5 3.5 V13.5 L13.5 19.5 Z" fill="#fff" stroke="none" opacity="0.35"/>`,
  "tool-envelope": `<path d="M4 6 Q12 1.5 20 6 Q22.5 12 20 18 Q12 22.5 4 18 Q1.5 12 4 6 Z"/>${dot(4, 6, 1.4)}${dot(20, 6, 1.4)}${dot(20, 18, 1.4)}${dot(4, 18, 1.4)}${dot(12, 3.8, 1.4)}${dot(12, 20.2, 1.4)}`,
  "tool-smart-fill": `<rect x="3.5" y="3.5" width="11" height="11" rx="1"/><rect x="9.5" y="9.5" width="11" height="11" rx="1"/><path d="M9.5 9.5 H14.5 V14.5 H9.5 Z" fill="#fff"/>`,
  "symmetry": `<path d="M12 3 V21" stroke-dasharray="2 2"/><path d="M9.5 6 L4 12 L9.5 18 Z"/><path d="M14.5 6 L20 12 L14.5 18 Z"/>`,
  "tool-width": `<path d="M3.5 12 C 8 4.5, 16 4.5, 20.5 12 C 16 15, 8 15, 3.5 12 Z"/><path d="M12 6.4 V13.8"/>${dot(12, 6.4)}${dot(12, 13.8)}`,
  "dash-standard": `<path d="M4 20 V13 M4 9 V4 H8 M12 4 H17 M20 4 H20.5"/>`,
  "dash-align": `<path d="M4 20 V16 M4 12 V4 H12 M16 4 H20.5"/>`,
  "dash-fixed": `<path d="M4 20 V18 M4 14 V8 M4 4 H10 M14 4 H20"/>${dot(4, 4, 1.6)}`,
  "fill-gradient": `<defs><linearGradient id="g-fg" x1="0" x2="1"><stop offset="0" stop-color="#fff"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></linearGradient></defs><rect x="3.5" y="5.5" width="17" height="13" rx="1"/><rect x="4.5" y="6.5" width="15" height="11" fill="url(#g-fg)" stroke="none"/>`,
  "fill-vector": `<rect x="3.5" y="3.5" width="17" height="17" rx="1"/><path d="M8 6 L10.5 10 H5.5 Z M16 6 L18.5 10 H13.5 Z M8 14 L10.5 18 H5.5 Z M16 14 L18.5 18 H13.5 Z"/>`,
  "fill-bitmap": `<rect x="3.5" y="4.5" width="17" height="15" rx="1"/><circle cx="8.5" cy="9" r="1.6"/><path d="M4 17 L9.5 12 L13 15 L16 12.5 L20 16"/>`,
  "fill-two-color": `<rect x="3.5" y="3.5" width="17" height="17" rx="1"/><path d="M3.5 3.5 H8.75 V8.75 H3.5 Z M14 3.5 H19.25 V8.75 H14 Z M8.75 8.75 H14 V14 H8.75 Z M3.5 14 H8.75 V19.25 H3.5 Z M14 14 H19.25 V19.25 H14 Z" fill="#fff"/>`,
  "fill-texture": `<rect x="3.5" y="3.5" width="17" height="17" rx="1"/><path d="M5 9 C 7 7, 9 10, 11 8 S 15 6, 19 9 M5 14 C 8 12, 10 16, 13 13 S 17 12, 19 15"/>`,
  "fill-postscript": `<rect x="3.5" y="3.5" width="17" height="17" rx="1"/>${dot(7, 7, 1.2)}${dot(12, 7, 1.5)}${dot(17, 7, 1.8)}${dot(7, 12, 1.5)}${dot(12, 12, 1.8)}${dot(17, 12, 1.2)}${dot(7, 17, 1.8)}${dot(12, 17, 1.2)}${dot(17, 17, 1.5)}`,
  "grad-reflect": `<path d="M3 16 L12 7 L21 16"/><path d="M3 19 H21" stroke-dasharray="2 2"/>`,
  "grad-repeat": `<path d="M3 16 L10 8 V16 L17 8 V16"/><path d="M3 19 H21" stroke-dasharray="2 2"/>`,
  "grad-reverse": `<path d="M4 9 H18 L15 6 M20 15 H6 L9 18"/>`,
  "grad-smooth": `<path d="M3 16 C 8 16, 9 6, 14 6 S 19 6, 21 6"/>${dot(5, 20, 1)}${dot(9, 20, 1)}${dot(13, 20, 1)}${dot(17, 20, 1)}${dot(7, 18, 0.8)}${dot(11, 18, 0.8)}${dot(15, 18, 0.8)}`,
  "blend-direct": `<circle cx="12" cy="12" r="8"/><path d="M6.5 6.5 L17.5 17.5"/>${dot(6.5, 6.5)}${dot(17.5, 17.5)}`,
  "blend-cw": `<circle cx="12" cy="12" r="8" stroke-dasharray="2 2"/><path d="M12 4 A 8 8 0 0 1 20 12"/><path d="M20 12 L17.5 9.5 M20 12 L22 9.5"/>`,
  "blend-ccw": `<circle cx="12" cy="12" r="8" stroke-dasharray="2 2"/><path d="M12 4 A 8 8 0 0 0 4 12"/><path d="M4 12 L6.5 9.5 M4 12 L2 9.5"/>`,
  "tool-connector": `<rect x="3.5" y="3.5" width="6" height="6" rx="1"/><rect x="14.5" y="14.5" width="6" height="6" rx="1"/><path d="M9.5 6.5 H12 V17.5 H14.5"/>`,
  "tool-knife": `<path d="M3.5 20.5 L8.5 15.5"/><path d="M8.5 15.5 L19.5 4.5 C 20.5 9.5 16 14 11 18 Z"/>`,
  "tool-crop": `<path d="M7 3.5 V17 H20.5 M3.5 7 H17 V20.5"/>`,
  "tool-eraser": `<path d="M9 20 L3.8 14.8 a1.5 1.5 0 0 1 0 -2.1 L12.7 3.8 a1.5 1.5 0 0 1 2.1 0 L20.2 9.2 a1.5 1.5 0 0 1 0 2.1 L11.3 20 Z M7.5 9 L15 16.5 M11.3 20 H20.5"/>`,
  "tool-virtual-delete": `<path d="M3.5 12 H12 M9 3.5 V20.5"/><path d="M12 12 H20.5" stroke-dasharray="1.5 1.8"/><path d="M15 15.5 L19 19.5 M19 15.5 L15 19.5"/>`,
  "tool-text": `<path d="M5 6.5 V4.5 H19 V6.5 M12 4.5 V19.5 M9 19.5 H15"/>`,
  // ---- texto (doc 06) ----
  "text-bold": `<path d="M7.5 4.5 H13 a3.75 3.75 0 0 1 0 7.5 H7.5 Z M7.5 12 H14 a3.75 3.75 0 0 1 0 7.5 H7.5 Z" stroke-width="2.2"/>`,
  "text-italic": `<path d="M10 4.5 H17 M7 19.5 H14 M13.5 4.5 L10.5 19.5"/>`,
  "text-left": `<path d="M4 6 H20 M4 10 H14 M4 14 H20 M4 18 H12"/>`,
  "text-center": `<path d="M4 6 H20 M7 10 H17 M4 14 H20 M8 18 H16"/>`,
  "text-right": `<path d="M4 6 H20 M10 10 H20 M4 14 H20 M12 18 H20"/>`,
  "text-justify": `<path d="M4 6 H20 M4 10 H20 M4 14 H20 M4 18 H13"/>`,
  // Caixa alta e baixa (GD-TXT-11..13): Mudar caixa = A grande, a pequeno e a troca.
  "text-case": `<path d="M3.5 18 L7.5 7 L11.5 18 M5 14 H10"/><circle cx="16.2" cy="15.3" r="2.7"/><path d="M18.9 12 V18 M13.5 5.5 H19.5 M17.7 3.7 L19.5 5.5 L17.7 7.3"/>`,
  "text-small-caps": `<path d="M3.5 18 L7.5 6 L11.5 18 M5 14.2 H10 M13.5 18 L16.5 11 L19.5 18 M14.6 15.6 H18.4"/>`,
  "text-all-small-caps": `<path d="M3.5 18 L7 10 L10.5 18 M4.7 15.3 H9.3 M13.5 18 L17 10 L20.5 18 M14.7 15.3 H19.3"/><path d="M3.5 6 H20.5" stroke-dasharray="1.5 1.8"/>`,
  "text-all-caps": `<path d="M3.5 5.5 H11 M7.25 5.5 V19 M13 5.5 H20.5 M16.75 5.5 V19"/>`,
  "text-cap-spacing": `<path d="M3.5 6 H9.5 M6.5 6 V18 M14.5 6 H20.5 M17.5 6 V18"/><path d="M9.8 13 H14.2 M9.8 13 L11.2 11.6 M9.8 13 L11.2 14.4 M14.2 13 L12.8 11.6 M14.2 13 L12.8 14.4"/>`,
  // ---- arquivo e histórico ----
  "file-new": `<path d="M6 3 H14 L19 8 V21 H6 Z M14 3 V8 H19"/><path d="M12.5 12 V18 M9.5 15 H15.5"/>`,
  "file-open": `<path d="M3.5 18.5 V6 H9 L11 8 H19 V10.5"/><path d="M3.5 18.5 L6.5 11 H21 L18 18.5 Z"/>`,
  "file-save": `<path d="M4.5 4.5 H16.5 L19.5 7.5 V19.5 H4.5 Z"/><path d="M8 4.5 V9 H15 V4.5 M7.5 19.5 V14 H16.5 V19.5"/>`,
  // file-import, file-export, file-pdf, undo and redo: drawn by the product
  // owner (2026-10-29), made by split-file.mjs from that drawing sheet.
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
  "node-add": `<path d="M3.5 17 C 7 7, 13 7, 15 11"/>${node(3.5, 17, 2)}<path d="M17.5 12.5 V19.5 M14 16 H21"/>`,
  "node-delete": `<path d="M3.5 17 C 7 7, 13 7, 15 11"/>${node(3.5, 17, 2)}<path d="M14 16 H21"/>`,
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
  // ---- grupos e camadas ----
  group: `<rect x="3.5" y="3.5" width="17" height="17" rx="1.5" stroke-dasharray="2.5 2.2"/><rect x="7" y="7" width="5" height="5" rx="0.8"/><circle cx="15" cy="15" r="2.6"/>`,
  ungroup: `<rect x="3.5" y="3.5" width="8" height="8" rx="1" stroke-dasharray="2.2 2"/><rect x="5.5" y="5.5" width="4" height="4" rx="0.6"/><rect x="12.5" y="12.5" width="8" height="8" rx="1" stroke-dasharray="2.2 2"/><circle cx="16.5" cy="16.5" r="2"/>`,
  layers: `<path d="M12 4 L21 8.5 L12 13 L3 8.5 Z"/><path d="M3 12.5 L12 17 L21 12.5"/><path d="M3 16.5 L12 21 L21 16.5"/>`,
  "layer-add": `<path d="M11 4 L19 8 L11 12 L3 8 Z"/><path d="M3 12 L11 16 L13 15"/><path d="M18 14 V21 M14.5 17.5 H21.5"/>`,
  "layer-move-to": `<path d="M3 15 L12 19.5 L21 15 L12 10.5 Z"/><path d="M12 3 V8.5 M9.5 6 L12 8.5 L14.5 6"/>`,
  "eye-on": `<path d="M2.5 12 C 5 7, 9 5.5, 12 5.5 S 19 7, 21.5 12 C 19 17, 15 18.5, 12 18.5 S 5 17, 2.5 12 Z"/><circle cx="12" cy="12" r="3"/>`,
  "eye-off": `<path d="M2.5 12 C 5 7, 9 5.5, 12 5.5 S 19 7, 21.5 12 C 19 17, 15 18.5, 12 18.5 S 5 17, 2.5 12 Z"/><path d="M4 20 L20 4"/>`,
  "print-on": `<path d="M7 9 V3.5 H17 V9"/><rect x="3.5" y="9" width="17" height="7.5" rx="1.5"/><path d="M7 14 H17 V20.5 H7 Z"/>`,
  "print-off": `<path d="M7 9 V3.5 H17 V9"/><rect x="3.5" y="9" width="17" height="7.5" rx="1.5"/><path d="M4 20 L20 4"/>`,
  "lock-on": `<rect x="5" y="10.5" width="14" height="10" rx="1.5"/><path d="M8 10.5 V7.5 a4 4 0 0 1 8 0 V10.5"/>`,
  // ---- cantos do retângulo e modo da elipse (GD-BPC-04/05) ----
  // Cantos: o canto de cima à direita de um quadrado, em cada tipo.
  "corner-round": `<path d="M4 20 V4 H11 a9 9 0 0 1 9 9 V20"/>`,
  "corner-scallop": `<path d="M4 20 V4 H11 a9 9 0 0 0 9 9 V20"/>`,
  "corner-chamfer": `<path d="M4 20 V4 H11 L20 13 V20"/>`,
  // Elipse inteira, fatia (pizza) e arco.
  "ellipse-whole": `<ellipse cx="12" cy="12" rx="8.5" ry="7"/>`,
  "ellipse-pie": `<path d="M12 12 L20.5 12 A8.5 7 0 1 1 18 7 Z"/>`,
  "ellipse-arc": `<path d="M20.5 12 A8.5 7 0 1 1 18 7"/>`,
  // Passo das setas: tecla com seta; com Shift, seta dupla.
  "nudge": `<rect x="4" y="4" width="16" height="16" rx="2.5"/><path d="M8 12 H16 M13 9 L16 12 L13 15"/>`,
  "nudge-big": `<rect x="4" y="4" width="16" height="16" rx="2.5"/><path d="M7 12 H17 M11 9 L14 12 L11 15 M14 9 L17 12 L14 15"/>`,
  // Escala X/Y em %: quadrado menor dentro do maior com seta diagonal.
  "scale-xy": `<rect x="3.5" y="10.5" width="10" height="10" rx="1"/><path d="M3.5 7 V3.5 H20.5 V20.5 H17 M11 13 L19 5 M14 5 H19 V10"/>`,
  // ---- barra de propriedades (GD-BPR-06/07/08) ----
  // Alça: duas colunas de três pontos.
  "bar-grip": `${dot(9.5, 6.5, 1.3)}${dot(14.5, 6.5, 1.3)}${dot(9.5, 12, 1.3)}${dot(14.5, 12, 1.3)}${dot(9.5, 17.5, 1.3)}${dot(14.5, 17.5, 1.3)}`,
  // Mais controles (transbordo): duas setas para a direita.
  "bar-overflow": `<path d="M6 7 L11 12 L6 17 M13 7 L18 12 L13 17"/>`,
  // Personalizar: três pontos na horizontal.
  "bar-more": `${dot(6, 12, 1.6)}${dot(12, 12, 1.6)}${dot(18, 12, 1.6)}`,
  "lock-off": `<rect x="5" y="10.5" width="14" height="10" rx="1.5"/><path d="M8 10.5 V7.5 a4 4 0 0 1 7.7 -1.5"/>`,
  wireframe: `<path d="M12 4 L20.5 18.5 H3.5 Z" stroke-dasharray="2 2"/><circle cx="12" cy="13.5" r="2.2"/>`,
  // ---- contorno: junção, terminação, posição, opções ----
  // Junção: contorno de uma faixa grossa dobrada em ângulo, com a linha-guia fina no meio.
  "join-miter": `<path d="M5.2 19.2 L12 8.6 L18.8 19.2" stroke-width="1" stroke-dasharray="1.4 1.4"/><path d="M2.5 17.1 L12 3.2 L21.5 17.1 M8.3 19.7 L12 13.7 L15.7 19.7"/>`,
  "join-round": `<path d="M5.2 19.2 L12 8.6 L18.8 19.2" stroke-width="1" stroke-dasharray="1.4 1.4"/><path d="M2.5 17.1 L9.7 6.5 a2.8 2.8 0 0 1 4.6 0 L21.5 17.1 M8.3 19.7 L12 13.7 L15.7 19.7"/>`,
  "join-bevel": `<path d="M5.2 19.2 L12 8.6 L18.8 19.2" stroke-width="1" stroke-dasharray="1.4 1.4"/><path d="M2.5 17.1 L9.7 6.5 H14.3 L21.5 17.1 M8.3 19.7 L12 13.7 L15.7 19.7"/>`,
  // Terminação: faixa grossa até a linha-guia vertical (o fim geométrico).
  "cap-butt": `<path d="M15 4 V20" stroke-width="1" stroke-dasharray="1.4 1.4"/><path d="M3 8.5 H15 V15.5 H3"/><path d="M3 12 H15" stroke-width="1"/>`,
  "cap-round": `<path d="M15 4 V20" stroke-width="1" stroke-dasharray="1.4 1.4"/><path d="M3 8.5 H15 a3.5 3.5 0 0 1 0 7 H3"/><path d="M3 12 H15" stroke-width="1"/>`,
  "cap-square": `<path d="M15 4 V20" stroke-width="1" stroke-dasharray="1.4 1.4"/><path d="M3 8.5 H18.5 V15.5 H3"/><path d="M3 12 H15" stroke-width="1"/>`,
  // Posição: interior do objeto (claro, à esquerda), borda (linha) e faixa do contorno.
  "pos-center": `<path d="M3 4 H13 V20 H3 Z" fill="#fff" stroke="none" opacity="0.3"/><path d="M13 3 V21"/><rect x="10" y="6" width="6" height="12" rx="0.8" stroke-dasharray="1.8 1.4"/>`,
  "pos-inside": `<path d="M3 4 H13 V20 H3 Z" fill="#fff" stroke="none" opacity="0.3"/><path d="M13 3 V21"/><rect x="7" y="6" width="6" height="12" rx="0.8" stroke-dasharray="1.8 1.4"/>`,
  "pos-outside": `<path d="M3 4 H13 V20 H3 Z" fill="#fff" stroke="none" opacity="0.3"/><path d="M13 3 V21"/><rect x="13" y="6" width="6" height="12" rx="0.8" stroke-dasharray="1.8 1.4"/>`,
  // Opções: contorno atrás do preenchimento e contorno que escala com o objeto.
  "behind-fill": `<rect x="4" y="4" width="11" height="11" rx="1" stroke-width="3"/><rect x="9" y="9" width="11" height="11" rx="1" fill="#fff"/>`,
  "scale-stroke": `<rect x="3.5" y="12" width="7" height="7" rx="0.8"/><rect x="11" y="4.5" width="9.5" height="9.5" rx="1" stroke-width="3"/><path d="M8 10 L12.5 5.5 M10 5.5 H12.5 V8"/>`,
  // ---- barra de páginas ----
  "page-grid": `<rect x="4" y="4" width="7" height="7" rx="1"/><rect x="13" y="4" width="7" height="7" rx="1"/><rect x="4" y="13" width="7" height="7" rx="1"/><rect x="13" y="13" width="7" height="7" rx="1"/>`,
  "page-list": `<path d="M4 6 H13 M4 11 H10 M4 16 H9"/><circle cx="16" cy="15" r="3.5"/><path d="M18.5 17.5 L21 20"/>`,
  "page-prev": `<path d="M14.5 6 L8.5 12 L14.5 18"/>`,
  "page-next": `<path d="M9.5 6 L15.5 12 L9.5 18"/>`,
  "page-add": `<rect x="5" y="3.5" width="12" height="16" rx="1"/><path d="M18 14.5 V21.5 M14.5 18 H21.5"/>`,
  // ---- cursores (desenhados sobre o canvas no lugar do cursor do sistema) ----
  // Cursor Forma: o mesmo desenho; a ponta da seta (1,973; 0) é o ponto do clique.
  "cur-shape": `<path d="M5.451 22.923C5.451 22.923 9.168 13.577 20.896 15.136" stroke-width="1.5"/><path d="M11.772 16.973L13.517 16.564L13.109 14.82L11.364 15.228Z" fill="#fff" stroke="none"/><path d="M4.783 24L6.528 23.592L6.12 21.847L4.375 22.255Z" fill="#fff" stroke="none"/><path d="M7.411 18.115L17.395 13.357" stroke-width="0.9"/><path d="M20.282 16.444L22.027 16.036L21.619 14.291L19.874 14.7Z" fill="#fff" stroke="none"/><path d="M7.789 18.026C7.74 17.818 7.531 17.688 7.323 17.737C7.114 17.786 6.984 17.995 7.033 18.203C7.082 18.412 7.291 18.542 7.5 18.493C7.708 18.444 7.838 18.235 7.789 18.026Z" fill="#fff" stroke="none"/><path d="M17.773 13.269C17.724 13.06 17.515 12.931 17.307 12.979C17.098 13.028 16.968 13.237 17.017 13.446C17.066 13.654 17.275 13.784 17.484 13.735C17.692 13.686 17.822 13.477 17.773 13.269Z" fill="#fff" stroke="none"/><path d="M1.973 0L3.161 18.942L8.518 11.731L17.783 10.5L1.973 0Z" fill="#fff" stroke="none"/>`,
  // Escolher o caminho do texto (Ajustar texto ao caminho): seta para a direita.
  "cur-pick-path": `<path d="M2.5 9 H13.5 V4.5 L22 12 L13.5 19.5 V15 H2.5 Z" fill="#fff"/>`,
  "cur-move": `<path d="M12 3 V21 M3 12 H21 M9.5 5.5 L12 3 L14.5 5.5 M9.5 18.5 L12 21 L14.5 18.5 M5.5 9.5 L3 12 L5.5 14.5 M18.5 9.5 L21 12 L18.5 14.5"/>`,
  "cur-grab": `<path d="M7.5 11 V9.5 a1.5 1.5 0 0 1 3 0 V11 M10.5 9.5 a1.5 1.5 0 0 1 3 0 V11 M13.5 10 a1.5 1.5 0 0 1 3 0 V12.5 M7.5 11.5 a1.5 1.5 0 0 0 -2.5 1 L6.5 17 a5 5 0 0 0 4.5 3 H13 a4 4 0 0 0 4 -4 V12.5"/>`,
  "cur-rotate": `<path d="M18.5 12 a6.5 6.5 0 1 1 -2 -4.7"/><path d="M17 3.5 V7.5 H13"/>`,
  // Pré-seleção: a seta da Seleção com uma caixa de alças embaixo, à direita
  // (o objeto sob o cursor é clicável). Ponta da seta em (4; 2,5).
  "cur-select-pick": `<path d="M4 2.5 L4 15 L7.2 11.8 L9.5 16.3 L11.4 15.3 L9.2 10.9 L13.5 10.6 Z"/><rect x="14.5" y="14.5" width="6" height="6" stroke-dasharray="1.5 1.5"/>${node(14.5, 14.5, 1.3)}${node(20.5, 20.5, 1.3)}`,
  // ---- página (barra de propriedades sem seleção) ----
  "page-portrait": `<rect x="7" y="3.5" width="10" height="17" rx="1"/><path d="M9.5 7.5 H14.5"/>`,
  "page-landscape": `<rect x="3.5" y="7" width="17" height="10" rx="1"/><path d="M6.5 10 H11.5"/>`,
  "fit-art": `<path d="M3.5 8 V3.5 H8 M16 3.5 H20.5 V8 M20.5 16 V20.5 H16 M8 20.5 H3.5 V16"/><path d="M8.5 15.5 L12 8.5 L15.5 15.5 Z"/>`,
  bleed: `<rect x="7" y="7" width="10" height="10" rx="0.5"/><path d="M3.5 7 V3.5 H7 M17 3.5 H20.5 V7 M20.5 17 V20.5 H17 M7 20.5 H3.5 V17" stroke-dasharray="1.6 1.6"/>`,
  // Documento (abas de documento).
  doc: `<path d="M6.5 3 H14 L18.5 7.5 V21 H6.5 Z M14 3 V7.5 H18.5"/>`,
  report: `<path d="M6 3.5 H18 V20.5 H6 Z M9 8 H15 M9 12 H15 M9 16 H12.5"/>`,
  history: `<circle cx="12" cy="12" r="8"/><path d="M12 7.5 V12 L15 14"/>`,
  // ---- booleanas, combinar e deslocamento (doc 09) ----
  // bool-weld, bool-trim, bool-intersect, bool-simplify, bool-front-minus,
  // bool-back-minus and bool-boundary: drawn by the product owner
  // (2026-10-24), made by split-shaping.mjs from that drawing sheet.
  "bool-exclude": `<path d="M12 7.39 A5.5 5.5 0 1 0 12 16.61 A5.5 5.5 0 0 1 12 7.39 Z" fill="#fff"/><path d="M12 7.39 A5.5 5.5 0 1 1 12 16.61 A5.5 5.5 0 0 0 12 7.39 Z" fill="#fff"/>`,
  "bool-divide": `<path d="M12 7.39 A5.5 5.5 0 1 0 12 16.61 A5.5 5.5 0 0 1 12 7.39 Z" transform="translate(-1.5 0)"/><path d="M12 7.39 A5.5 5.5 0 0 1 12 16.61 A5.5 5.5 0 0 1 12 7.39 Z"/><path d="M12 7.39 A5.5 5.5 0 1 1 12 16.61 A5.5 5.5 0 0 0 12 7.39 Z" transform="translate(1.5 0)"/>`,
  "node-reduce": `<path d="M3.5 17 C 7 5, 17 5, 20.5 17"/>${node(3.5, 17, 2)}${node(20.5, 17, 2)}${hnode(8, 9.4, 1.6)}${hnode(16, 9.4, 1.6)}<path d="M10.5 19.5 H13.5"/>`,
  "node-align-h": `<path d="M3.5 12 H20.5" stroke-dasharray="2 2"/>${node(6, 12, 2)}${node(12, 12, 2)}${node(18, 12, 2)}<path d="M12 4 V8 M10.5 6.5 L12 8 L13.5 6.5 M12 20 V16 M10.5 17.5 L12 16 L13.5 17.5"/>`,
  "node-align-v": `<path d="M12 3.5 V20.5" stroke-dasharray="2 2"/>${node(12, 6, 2)}${node(12, 12, 2)}${node(12, 18, 2)}<path d="M4 12 H8 M6.5 10.5 L8 12 L6.5 13.5 M20 12 H16 M17.5 10.5 L16 12 L17.5 13.5"/>`,
  "node-extract": `<rect x="3.5" y="8.5" width="11" height="11" rx="1"/><rect x="10" y="4" width="7" height="7" rx="1" fill="#fff"/><path d="M17 7.5 L20.5 4"/>`,
  "clear-transform": `<rect x="6" y="6" width="12" height="12" rx="1"/><path d="M4 9 L9 3.5 L20 9 L15 20.5 Z" stroke-dasharray="2 2"/>`,
  "align-baseline": `<path d="M3.5 16.5 H20.5"/><path d="M5 16.5 L8 7 L11 16.5 M6 13.5 H10"/><path d="M14 16.5 V11 a2.5 2.5 0 0 1 5 0 V16.5"/>`,
  combine: `<path d="M4 4 H20 V20 H4 Z M9 9 V15 H15 V9 Z" fill="#fff" fill-rule="evenodd"/>`,
  "break-apart": `<rect x="3.5" y="3.5" width="8" height="8" rx="1"/><rect x="12.5" y="12.5" width="8" height="8" rx="1"/><path d="M14 6 H18 V10 M10 18 H6 V14"/>`,
  offset: `<rect x="7.5" y="7.5" width="9" height="9" rx="1"/><rect x="3.5" y="3.5" width="17" height="17" rx="4" stroke-dasharray="2 2"/>`,
  palette: `<path d="M12 3.5 A8.5 8.5 0 1 0 12 20.5 C13.6 20.5 14 19.4 13.4 18.4 C12.8 17.4 13.4 15.9 14.8 15.9 H16.8 A3.7 3.7 0 0 0 20.5 12.2 C20.5 7.4 16.7 3.5 12 3.5 Z"/>${dot(8, 10)}${dot(11.5, 7.5)}${dot(15.5, 8.5)}${dot(7.8, 14)}`,
  "fill-linear": `<rect x="4" y="4" width="16" height="16" rx="1.5"/><path d="M8 4V20 M12 4V20 M16 4V20" stroke-dasharray="1.5 1.5"/>`,
  "fill-elliptic": `<rect x="4" y="4" width="16" height="16" rx="1.5"/><ellipse cx="12" cy="12" rx="2.5" ry="1.5"/><ellipse cx="12" cy="12" rx="6" ry="3.5" stroke-dasharray="1.5 1.5"/>`,
  "fill-conic": `<rect x="4" y="4" width="16" height="16" rx="1.5"/><path d="M12 12 L19 12 M12 12 L7 7 M12 12 L9 18.5"/>`,
  "fill-rectangular": `<rect x="4" y="4" width="16" height="16" rx="1.5"/><rect x="10" y="10" width="4" height="4"/><rect x="7" y="7" width="10" height="10" stroke-dasharray="1.5 1.5"/>`,
  "fill-pattern": `<rect x="4" y="4" width="16" height="16" rx="1.5"/>${dot(8, 8, 1.4)}${dot(16, 8, 1.4)}${dot(12, 12, 1.4)}${dot(8, 16, 1.4)}${dot(16, 16, 1.4)}`,
  "fill-radial": `<rect x="4" y="4" width="16" height="16" rx="1.5"/><circle cx="12" cy="12" r="2.5"/><circle cx="12" cy="12" r="5.5" stroke-dasharray="1.5 1.5"/>`,
  // ---- distribuir (GD-ALI-03) ----
  "dist-left": `<path d="M4 3.5V20.5 M11 3.5V20.5 M18 3.5V20.5" stroke-dasharray="1.6 1.8"/><rect x="4" y="6" width="4" height="5" rx="0.8" fill="#fff"/><rect x="11" y="10" width="5" height="5" rx="0.8" fill="#fff"/><rect x="18" y="14" width="3" height="5" rx="0.8" fill="#fff"/>`,
  "dist-bottom": `<g transform="rotate(-90 12 12)"><path d="M4 3.5V20.5 M11 3.5V20.5 M18 3.5V20.5" stroke-dasharray="1.6 1.8"/><rect x="4" y="6" width="4" height="5" rx="0.8" fill="#fff"/><rect x="11" y="10" width="5" height="5" rx="0.8" fill="#fff"/><rect x="18" y="14" width="3" height="5" rx="0.8" fill="#fff"/></g>`,
  "dist-center-h": `<path d="M5.5 3.5V20.5 M12 3.5V20.5 M18.5 3.5V20.5" stroke-dasharray="1.6 1.8"/><rect x="3.5" y="6" width="4" height="5" rx="0.8" fill="#fff"/><rect x="9.5" y="10" width="5" height="5" rx="0.8" fill="#fff"/><rect x="17" y="14" width="3" height="5" rx="0.8" fill="#fff"/>`,
  "dist-center-v": `<g transform="rotate(-90 12 12)"><path d="M5.5 3.5V20.5 M12 3.5V20.5 M18.5 3.5V20.5" stroke-dasharray="1.6 1.8"/><rect x="3.5" y="6" width="4" height="5" rx="0.8" fill="#fff"/><rect x="9.5" y="10" width="5" height="5" rx="0.8" fill="#fff"/><rect x="17" y="14" width="3" height="5" rx="0.8" fill="#fff"/></g>`,
  "dist-right": `<path d="M7 3.5V20.5 M14 3.5V20.5 M20.5 3.5V20.5" stroke-dasharray="1.6 1.8"/><rect x="3" y="6" width="4" height="5" rx="0.8" fill="#fff"/><rect x="9" y="10" width="5" height="5" rx="0.8" fill="#fff"/><rect x="17.5" y="14" width="3" height="5" rx="0.8" fill="#fff"/>`,
  "dist-top": `<g transform="rotate(-90 12 12)"><path d="M7 3.5V20.5 M14 3.5V20.5 M20.5 3.5V20.5" stroke-dasharray="1.6 1.8"/><rect x="3" y="6" width="4" height="5" rx="0.8" fill="#fff"/><rect x="9" y="10" width="5" height="5" rx="0.8" fill="#fff"/><rect x="17.5" y="14" width="3" height="5" rx="0.8" fill="#fff"/></g>`,
  "dist-space-h": `<rect x="3" y="7" width="4" height="10" rx="0.8" fill="#fff"/><rect x="10" y="7" width="4" height="10" rx="0.8" fill="#fff"/><rect x="17" y="7" width="4" height="10" rx="0.8" fill="#fff"/><path d="M7.5 20 H9.5 M14.5 20 H16.5"/>`,
  "dist-space-v": `<g transform="rotate(-90 12 12)"><rect x="3" y="7" width="4" height="10" rx="0.8" fill="#fff"/><rect x="10" y="7" width="4" height="10" rx="0.8" fill="#fff"/><rect x="17" y="7" width="4" height="10" rx="0.8" fill="#fff"/><path d="M7.5 20 H9.5 M14.5 20 H16.5"/></g>`,
  // ---- menu do botão direito (doc 26) ----
  "outline-to-object": `<path d="M4 18 C 6 8, 18 8, 20 18 H16.5 C 15 12.5, 9 12.5, 7.5 18 Z"/>${node(4, 18, 1.6)}${node(20, 18, 1.6)}`,
  "order-in-front-of": `<rect x="3.5" y="9" width="10" height="10" rx="1"/><rect x="8.5" y="12.5" width="8" height="8" rx="1" fill="#fff"/>${pick(14.5, 3.5)}`,
  "order-behind-of": `<rect x="8.5" y="12.5" width="8" height="8" rx="1" stroke-dasharray="2 2"/><rect x="3.5" y="9" width="10" height="10" rx="1" fill="#fff"/>${pick(14.5, 3.5)}`,
  "ungroup-all": `<rect x="3.5" y="3.5" width="6.5" height="6.5" rx="1" stroke-dasharray="2 1.8"/><rect x="14" y="3.5" width="6.5" height="6.5" rx="1" stroke-dasharray="2 1.8"/><rect x="3.5" y="14" width="6.5" height="6.5" rx="1" stroke-dasharray="2 1.8"/><rect x="14" y="14" width="6.5" height="6.5" rx="1" stroke-dasharray="2 1.8"/>${dot(6.75, 6.75, 1.4)}${dot(17.25, 6.75, 1.4)}${dot(6.75, 17.25, 1.4)}${dot(17.25, 17.25, 1.4)}`,
  "show-all": `<path d="M3 10 C 5.5 6, 9 4.5, 12 4.5 S 18.5 6, 21 10 C 18.5 14, 15 15.5, 12 15.5 S 5.5 14, 3 10 Z"/><circle cx="12" cy="10" r="2.5"/>${all}`,
  "unlock-all": `<rect x="6" y="9" width="12" height="7.5" rx="1.3"/><path d="M8.5 9 V6.5 a3.5 3.5 0 0 1 6.8 -1.2"/>${all}`,
  "text-sample": `<rect x="3.5" y="3.5" width="17" height="17" rx="1"/><path d="M7 8 H17 M7 12 H17 M7 16 H13"/>`,
  "text-convert": `<path d="M3.5 17 L7.5 7 L11.5 17 M5 13.5 H10"/><rect x="14" y="4.5" width="6.5" height="15" rx="0.8" stroke-dasharray="1.6 1.6"/><path d="M15.8 8.5 H18.7 M15.8 12 H18.7 M15.8 15.5 H17.5"/>`,
  "text-frame": `<ellipse cx="12" cy="12" rx="8.5" ry="7.5" stroke-dasharray="2 2"/><path d="M7.5 9.5 H16.5 M6 12.5 H18 M7.5 15.5 H14"/>`,
  "image-frame": `<ellipse cx="12" cy="12" rx="8.5" ry="7.5" stroke-dasharray="2 2"/><path d="M6.5 16 L10 11.5 L12.5 14 L14.5 12 L17.5 16 Z"/>${dot(14.5, 8.5, 1.3)}`,
  "text-path": `<path d="M3.5 19 C 7 10, 17 10, 20.5 19" stroke-dasharray="2 2"/><path d="M7 12 L9 6.5 L11 12 M7.7 10.3 H10.3 M13.5 6.5 V12 H15.7 a1.4 1.4 0 0 0 0 -2.8 H13.5 M13.5 6.5 H15.4 a1.35 1.35 0 0 1 0 2.7"/>`,
  "cut-line": `<rect x="7.5" y="7.5" width="9" height="9" rx="1" fill="#fff"/><rect x="4" y="4" width="16" height="16" rx="3" stroke-dasharray="2 2"/><path d="M17.5 21 L21 17.5"/>`,
  "export-selection": `<rect x="3.5" y="9" width="10" height="11" rx="1" stroke-dasharray="2 2"/>${node(3.5, 9, 1.5)}${node(13.5, 20, 1.5)}<path d="M10.5 12.5 L20 4 M15 4 H20 V9"/>`,
  focus: `<path d="M3.5 8 V3.5 H8 M16 3.5 H20.5 V8 M20.5 16 V20.5 H16 M8 20.5 H3.5 V16"/><circle cx="12" cy="12" r="3.5" fill="#fff"/>`,
  paste: `<rect x="5" y="5" width="14" height="16" rx="1.5"/><rect x="8.5" y="3" width="7" height="4" rx="1" fill="#fff"/><path d="M8.5 12 H15.5 M8.5 16 H13"/>`,
  "select-all": `<rect x="4" y="4" width="16" height="16" rx="1" stroke-dasharray="2 2"/><rect x="7.5" y="7.5" width="4" height="4" rx="0.6" fill="#fff"/><rect x="12.5" y="12.5" width="4" height="4" rx="0.6" fill="#fff"/>`,
  "clip-inside": `<rect x="3.5" y="3.5" width="13" height="13" rx="1"/><circle cx="14" cy="14" r="6.5" stroke-dasharray="1.8 1.8"/><path d="M16.5 8 A6.5 6.5 0 0 0 8 16.5 H16.5 Z" fill="#fff"/>`,
  // Moldura de recorte: extrair (seta saindo por cima), editar (lápis sobre a
  // moldura tracejada), concluir (seta saindo pela direita), centralizar,
  // preencher (setas para fora) e ajustar (setas para dentro).
  "clip-extract": `<rect x="3.5" y="8.5" width="11" height="12" rx="1"/><circle cx="9" cy="14.5" r="2.8" fill="#fff"/><path d="M18 14 V3.5 M15 6.5 L18 3.5 L21 6.5"/>`,
  "clip-edit": `<rect x="3.5" y="3.5" width="13" height="13" rx="1" stroke-dasharray="2 2"/><circle cx="10" cy="10" r="2.8" fill="#fff"/><path d="M11.5 20.5 L12.1 17.6 L18.6 11.1 L21 13.5 L14.5 20 Z" fill="#fff"/>`,
  "clip-finish": `<rect x="3.5" y="3.5" width="12" height="17" rx="1"/><circle cx="9.5" cy="12" r="2.6" fill="#fff"/><path d="M12.5 12 H20.5 M17.5 9 L20.5 12 L17.5 15"/>`,
  "clip-center": `<rect x="3.5" y="3.5" width="17" height="17" rx="1.5"/><rect x="9.5" y="9.5" width="5" height="5" rx="0.6" fill="#fff"/><path d="M5.5 12 H8 M6.8 10.8 L8 12 L6.8 13.2 M18.5 12 H16 M17.2 10.8 L16 12 L17.2 13.2"/>`,
  "clip-fill": `<rect x="5.5" y="5.5" width="13" height="13" rx="1"/><rect x="8.5" y="8.5" width="7" height="7" rx="0.6" fill="#fff"/><path d="M3.5 7.5 V3.5 H7.5 M3.5 3.5 L7 7 M20.5 16.5 V20.5 H16.5 M20.5 20.5 L17 17"/>`,
  "clip-fit": `<rect x="3.5" y="3.5" width="17" height="17" rx="1.5"/><rect x="5.5" y="9" width="13" height="6" rx="0.6" fill="#fff"/><path d="M12 3.5 V6.5 M10.5 5 L12 6.5 L13.5 5 M12 20.5 V17.5 M10.5 19 L12 17.5 L13.5 19"/>`,
  "copy-props": `<rect x="3.5" y="3.5" width="8" height="8" rx="1" fill="#fff" stroke-width="2.6"/><rect x="12.5" y="12.5" width="8" height="8" rx="1" stroke-dasharray="2 2"/>${hand}`,
  "copy-stroke": `<rect x="3.5" y="3.5" width="8" height="8" rx="1" stroke-width="2.6"/><rect x="12.5" y="12.5" width="8" height="8" rx="1" stroke-dasharray="2 2"/>${hand}`,
  "copy-fill": `<rect x="3.5" y="3.5" width="8" height="8" rx="1" fill="#fff" stroke="none"/><rect x="12.5" y="12.5" width="8" height="8" rx="1" stroke-dasharray="2 2"/>${hand}`,
  "center-on": `<circle cx="12" cy="12" r="8.5"/><rect x="9" y="9" width="6" height="6" rx="0.8" fill="#fff"/><path d="M12 3.5 V6 M12 18 V20.5 M3.5 12 H6 M18 12 H20.5"/>`,
  "move-here": `<rect x="3.5" y="3.5" width="7" height="7" rx="1" stroke-dasharray="2 2"/><rect x="13.5" y="13.5" width="7" height="7" rx="1" fill="#fff"/><path d="M8.5 8.5 L12.5 12.5 M12.5 9 V12.5 H9"/>`,
  instance: `<rect x="3.5" y="3.5" width="7" height="7" rx="1"/><rect x="13.5" y="13.5" width="7" height="7" rx="1" stroke-dasharray="2 2"/><path d="M10.5 10.5 L13.5 13.5"/>${dot(12, 12, 1.3)}`,
  cancel: `<circle cx="12" cy="12" r="8.5"/><path d="M8.5 8.5 L15.5 15.5 M15.5 8.5 L8.5 15.5"/>`,
  "guide-object": `<circle cx="12" cy="12" r="7" stroke-dasharray="2 2"/><path d="M3 12 H21 M12 3 V21" stroke-dasharray="1.5 2"/>`,
  styles: `<rect x="3.5" y="3.5" width="7" height="7" rx="1"/><rect x="13.5" y="3.5" width="7" height="7" rx="3.5"/><path d="M3.5 15 H20.5 M3.5 19 H14"/>`,
  preflight: `<path d="M5.5 3.5 H14 L18.5 8 V20.5 H5.5 Z M14 3.5 V8 H18.5"/><path d="M8.5 14 L11 16.5 L15.5 11.5"/>`,
  // ---- painéis (área de trabalho, GD-UI-03) ----
  "panel-collapse": `<rect x="3.5" y="4.5" width="17" height="15" rx="1.5"/><path d="M14.5 4.5 V19.5"/><path d="M7.5 9 L10.5 12 L7.5 15"/>`,
  "panel-expand": `<rect x="3.5" y="4.5" width="17" height="15" rx="1.5"/><path d="M14.5 4.5 V19.5"/><path d="M10.5 9 L7.5 12 L10.5 15"/>`,
  "panel-undock": `<path d="M10 4.5 H4.5 V19.5 H19.5 V14"/><rect x="12" y="3.5" width="8.5" height="7" rx="1"/><path d="M8 16 L13 11"/>`,
};

// Ferramentas que também são cursores: o mesmo desenho, na grade de cursor.
for (const t of ["select", "zoom", "pan"]) icons[`cur-${t}`] = icons[`tool-${t}`];

// Grades de exibição (DESIGN-SYSTEM.md, 3.1). Os desenhos acima ocupam a
// área 3..21 do rascunho 24×24; cada grade leva essa área para dentro da
// margem óptica de 1,5 px, e o traço é corrigido para dar 1,5 px na grade
// em que o ícone é exibido.
const DRAFT_MIN = 3, DRAFT_SPAN = 18, MARGIN = 1.5, STROKE = 1.5;
const grid = (size) => {
  const s = (size - 2 * MARGIN) / DRAFT_SPAN;
  const t = MARGIN - DRAFT_MIN * s;
  return { size, transform: `translate(${t.toFixed(4)} ${t.toFixed(4)}) scale(${s.toFixed(5)})`, stroke: (STROKE / s).toFixed(4) };
};
const UI = grid(20);
const MICRO = grid(16);
// Cursores: grade própria de 24 px, desenhados direto nela.
const CURSOR = { size: 24, transform: "", stroke: STROKE };
// Ícones de microcontroles (linhas do painel de camadas, abas).
const micro = ["eye-on", "eye-off", "print-on", "print-off", "lock-on", "lock-off", "doc", "bleed"];

const gridOf = (name) => (name.startsWith("cur-") ? CURSOR : UI);
const svg = (g, body) =>
  `<svg xmlns="http://www.w3.org/2000/svg" width="${g.size}" height="${g.size}" viewBox="0 0 ${g.size} ${g.size}">` +
  `<g${g.transform ? ` transform="${g.transform}"` : ""} fill="none" stroke="#fff" stroke-width="${g.stroke}" stroke-linecap="round" stroke-linejoin="round">${body}</g></svg>\n`;

mkdirSync(join(dir, "16"), { recursive: true });
for (const [name, body] of Object.entries(icons)) {
  writeFileSync(join(dir, `${name}.svg`), svg(gridOf(name), body));
}
for (const name of micro) {
  writeFileSync(join(dir, "16", `${name}.svg`), svg(MICRO, icons[name]));
}

// Folha de prova: cada ícone na grade em que é exibido, ampliado 3×, com o
// limite do container e o limite óptico marcados.
const cols = 10, cell = 84, zoom = 3;
const entries = [
  ...Object.keys(icons).map((n) => [n, gridOf(n)]),
  ...micro.map((n) => [`${n} (16)`, MICRO, n]),
];
const rows = Math.ceil(entries.length / cols);
let sheet = `<svg xmlns="http://www.w3.org/2000/svg" width="${cols * cell}" height="${rows * cell}" viewBox="0 0 ${cols * cell} ${rows * cell}"><rect width="100%" height="100%" fill="#1E1E23"/>`;
entries.forEach(([label, g, src], i) => {
  const x = (i % cols) * cell + (cell - g.size * zoom) / 2, y = Math.floor(i / cols) * cell + 4;
  const inner = g.size - 2 * MARGIN;
  sheet += `<g transform="translate(${x} ${y}) scale(${zoom})">` +
    `<rect width="${g.size}" height="${g.size}" fill="none" stroke="#363642" stroke-width="${1 / zoom}"/>` +
    `<rect x="${MARGIN}" y="${MARGIN}" width="${inner}" height="${inner}" fill="none" stroke="#27272E" stroke-width="${1 / zoom}" stroke-dasharray="${2 / zoom}"/>` +
    `<g${g.transform ? ` transform="${g.transform}"` : ""} fill="none" stroke="#fff" stroke-width="${g.stroke}" stroke-linecap="round" stroke-linejoin="round">${icons[src ?? label]}</g></g>` +
    `<text x="${(i % cols) * cell + cell / 2}" y="${Math.floor(i / cols) * cell + cell - 6}" fill="#A0A0AB" font-family="sans-serif" font-size="8" text-anchor="middle">${label}</text>`;
});
writeFileSync(join(dir, "..", "icons-sheet.svg"), sheet + "</svg>\n");
console.log(`${Object.keys(icons).length} ícones + ${micro.length} na grade de 16`);
