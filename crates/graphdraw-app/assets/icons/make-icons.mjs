// Gera os ícones do GraphDraw (desenhos próprios do projeto, MPL-2.0).
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

const icons = {
  // ---- ferramentas ----
  "tool-select": `<path d="M6 3.5 L6 19 L10 15 L12.8 20.5 L15.2 19.3 L12.5 13.8 L18 13.5 Z"/>`,
  "tool-shape": `<path d="M4 18 C 7 6, 15 6, 20 12"/>${node(4, 18)}${node(20, 12)}<path d="M12 8 L17.5 4.5"/>${dot(17.5, 4.5)}<path d="M12 8 L6.5 11.5" />${dot(6.5, 11.5)}${node(12, 8, 2)}`,
  "tool-zoom": `<circle cx="10.5" cy="10.5" r="6"/><path d="M15 15 L20.5 20.5"/><path d="M8 10.5 H13 M10.5 8 V13"/>`,
  "tool-pan": `<path d="M8 12 V6.5 a1.5 1.5 0 0 1 3 0 V11 M11 10 V4.5 a1.5 1.5 0 0 1 3 0 V11 M14 10.5 V6 a1.5 1.5 0 0 1 3 0 V13 M8 11.5 a1.5 1.5 0 0 0 -3 0.5 L7.5 17 a5 5 0 0 0 4.5 3 H13 a4 4 0 0 0 4 -4 V12"/>`,
  "tool-bezier": `<path d="M3.5 19 C 6 9, 11 9, 13 13 S 19 17, 20.5 6"/>${node(3.5, 19, 2)}${node(13, 13, 2)}${node(20.5, 6, 2)}`,
  "tool-rect": `<rect x="4" y="6" width="16" height="12" rx="1"/>`,
  "tool-artboard": `<rect x="6.5" y="6.5" width="11" height="11" rx="1"/><path d="M3.5 7 V3.5 H7 M17 3.5 H20.5 V7 M20.5 17 V20.5 H17 M7 20.5 H3.5 V17"/>`,
  "tool-ellipse": `<ellipse cx="12" cy="12" rx="8.5" ry="6.5"/>`,
  "tool-polygon": `<path d="M12 3.5 L20.1 9.4 L17 18.9 H7 L3.9 9.4 Z"/>`,
  "poly-star": `<path d="M12 3.5 L14.2 9.3 L20.3 9.4 L15.5 13.2 L17.2 19.2 L12 15.7 L6.8 19.2 L8.5 13.2 L3.7 9.4 L9.8 9.3 Z"/>`,
  "poly-complex": `<path d="M12 3.5 L17.2 19.2 L3.7 9.4 H20.3 L6.8 19.2 Z"/>`,
  "tool-text": `<path d="M5 6.5 V4.5 H19 V6.5 M12 4.5 V19.5 M9 19.5 H15"/>`,
  // ---- texto (doc 06) ----
  "text-bold": `<path d="M7.5 4.5 H13 a3.75 3.75 0 0 1 0 7.5 H7.5 Z M7.5 12 H14 a3.75 3.75 0 0 1 0 7.5 H7.5 Z" stroke-width="2.2"/>`,
  "text-italic": `<path d="M10 4.5 H17 M7 19.5 H14 M13.5 4.5 L10.5 19.5"/>`,
  "text-left": `<path d="M4 6 H20 M4 10 H14 M4 14 H20 M4 18 H12"/>`,
  "text-center": `<path d="M4 6 H20 M7 10 H17 M4 14 H20 M8 18 H16"/>`,
  "text-right": `<path d="M4 6 H20 M10 10 H20 M4 14 H20 M12 18 H20"/>`,
  "text-justify": `<path d="M4 6 H20 M4 10 H20 M4 14 H20 M4 18 H13"/>`,
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
  "cur-shape": `<path d="M5 3.5 L5 17 L8.5 13.5 L11 18.5 L13 17.5 L10.6 12.6 L15.5 12.3 Z"/>${node(18.5, 18.5, 2.2)}`,
  "cur-move": `<path d="M12 3 V21 M3 12 H21 M9.5 5.5 L12 3 L14.5 5.5 M9.5 18.5 L12 21 L14.5 18.5 M5.5 9.5 L3 12 L5.5 14.5 M18.5 9.5 L21 12 L18.5 14.5"/>`,
  "cur-grab": `<path d="M7.5 11 V9.5 a1.5 1.5 0 0 1 3 0 V11 M10.5 9.5 a1.5 1.5 0 0 1 3 0 V11 M13.5 10 a1.5 1.5 0 0 1 3 0 V12.5 M7.5 11.5 a1.5 1.5 0 0 0 -2.5 1 L6.5 17 a5 5 0 0 0 4.5 3 H13 a4 4 0 0 0 4 -4 V12.5"/>`,
  "cur-rotate": `<path d="M18.5 12 a6.5 6.5 0 1 1 -2 -4.7"/><path d="M17 3.5 V7.5 H13"/>`,
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
  "bool-weld": `<circle cx="9" cy="12" r="5.5" fill="#fff"/><circle cx="15" cy="12" r="5.5" fill="#fff"/>`,
  "bool-trim": `<circle cx="9" cy="12" r="5.5" stroke-dasharray="2 2"/><path d="M12 7.39 A5.5 5.5 0 1 1 12 16.61 A5.5 5.5 0 0 0 12 7.39 Z" fill="#fff"/>`,
  "bool-intersect": `<circle cx="9" cy="12" r="5.5"/><circle cx="15" cy="12" r="5.5"/><path d="M12 7.39 A5.5 5.5 0 0 1 12 16.61 A5.5 5.5 0 0 1 12 7.39 Z" fill="#fff"/>`,
  combine: `<path d="M4 4 H20 V20 H4 Z M9 9 V15 H15 V9 Z" fill="#fff" fill-rule="evenodd"/>`,
  "break-apart": `<rect x="3.5" y="3.5" width="8" height="8" rx="1"/><rect x="12.5" y="12.5" width="8" height="8" rx="1"/><path d="M14 6 H18 V10 M10 18 H6 V14"/>`,
  offset: `<rect x="7.5" y="7.5" width="9" height="9" rx="1"/><rect x="3.5" y="3.5" width="17" height="17" rx="4" stroke-dasharray="2 2"/>`,
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
