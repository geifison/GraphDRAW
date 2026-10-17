# GraphDraw — Recorte do MVP (v0.1)

Público: gráficas rápidas, comunicação visual, corte em plotter/laser.
Meta: **abrir `.cdr` legado, editar o básico, salvar `.gdr`, exportar PDF/SVG/PLT para produção.**

## Marcos
| Marco | Entrega | Requisitos |
|---|---|---|
| M0 — Visualizador | Abre `.cdr` (via libcdr) e `.gdr`, renderiza, zoom/pan, exporta SVG/PDF/PNG | 13, 14, 15 (exportação básica), 02 (leitura) |
| M1 — Editor básico | Seleção, mover/escalar/girar, retângulo, elipse, Bézier, edição de nós, preenchimento uniforme, contorno, desfazer, salvar `.gdr` | 02, 03 (GEO-01..05, NOD-01..05, PRI-01..02), 04 (FIL-01..02, OUT-01..04), 07 (FER-01, 04, 05, 10, 22, 30, 31), 08 (TRF-01, ALI-01, SNP-01), 12 |
| M2 — Produção | Texto artístico/parágrafo básico, booleanas, offset, linha de corte, exportação PLT/DXF, PDF CMYK | 05 (COR-01..03, CMS-01..02), 06 (TXT-01, 02, 04, 05), 09 (BOO-01..03, 10; CMB-01..03, 06; OFS-01; CNV-01..02), 17 |
| M3 — Polimento MVP | Paletas, guias, réguas, gradiente linear/radial, alinhar/distribuir, polígono/estrela, camadas completas | restante dos docs 02–09 marcados MVP |

## Fora do MVP
Doc 10 (efeitos), 11 (estilos/símbolos), 16 (imposição/separações avançadas), 18 (automação), 19 (comentários); preenchimentos malha/textura/hachura; mídia artística; vetorização de bitmap; ferramentas de pincel de distorção (GD-FER-11..15).

## Definição de pronto (cada requisito)
1. Critério de aceitação do requisito passa em teste automatizado.
2. Arquivo de teste `.gdr` correspondente em `tests/fixtures/`.
3. Sem regressão de desempenho (GD-NF-01/02) no benchmark do CI.
