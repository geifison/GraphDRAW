# Arquitetura do GraphDraw

Versão 0.2 · marco M1 (editor básico) · 2026-10-16

Este documento registra a escolha de tecnologia e a organização do código.
Requisitos vêm exclusivamente de [`docs/spec/`](spec/); cada decisão abaixo
cita os IDs que a motivam.

## 1. Decisão de stack

| Função | Escolha | Licença | Candidata da spec (doc 01) | Por quê |
|---|---|---|---|---|
| Linguagem | **Rust** (estável, alvo `x86_64-pc-windows-gnu`) | MIT/Apache-2.0 | "Rust + egui/Slint" | Segurança de memória num programa que abre arquivos de terceiros; `f64` nativo (GD-NF-05); compila para Windows/macOS/Linux (GD-NF-03); um único gerenciador de dependências com verificação de licença automática. |
| Renderização 2D | **tiny-skia** | BSD-3-Clause | Skia (BSD-3) | Porte em Rust de um subconjunto do Skia: mesma semântica de caminhos, traços, gradientes e anti-serrilhado, sem compilar o Skia em C++. Atende tela, PNG e miniaturas. |
| Geometria | **kurbo** | MIT/Apache-2.0 | — | Bézier, matrizes afins, arcos, área/caixa (base para GD-GEO-05 no M1). |
| Texto/shaping | **rustybuzz** + **fontdb** (ttf-parser) | MIT | HarfBuzz + FreeType | rustybuzz é porte do HarfBuzz; ttf-parser lê contornos OpenType sem FreeType. |
| PDF | **pdf-writer** | MIT/Apache-2.0 | "pdf-writer ou Skia PDF" | Escrita de baixo nível: controle total de espaços de cor (DeviceCMYK, Separation), necessário para GD-COR-02 e produção. |
| SVG | escritor próprio | — | "parser próprio ou usvg" | Exportação é simples e precisa preservar estrutura; importação SVG não está no M0. |
| Raster/imagens | **png**, **image** (png, jpeg, bmp, tiff, webp, gif) | MIT/Apache-2.0 | libpng, libjpeg-turbo… | Puro Rust, sem DLLs nativas. |
| Contêiner `.gdr` | **zip** (deflate), **serde_json**, **sha2**, **uuid** (v7) | MIT/Apache-2.0 | — | GD-GDR-01..04. |
| Importação CDR | **libcdr 0.1.9 + librevenge 0.0.6** (via ponte C++) | MPL-2.0 | libcdr + librevenge | Exigência do doc 14. |
| Interface | **egui/eframe** | MIT/Apache-2.0 | "Rust + egui/Slint" | Imediata, leve, sem licença copyleft. Mantida no M1: atende ferramentas, barras e painéis do editor. Os painéis ficam em abas numa coluna fixa à direita; a coluna recolhe e os painéis podem flutuar (GD-UI-03). |
| Gerenciamento de cor | **LittleCMS 2** (crate `lcms2`, fonte C compilada no build) | MIT | LittleCMS 2 | GD-CMS-01/02 (M2): perfis RGB/CMYK por documento, intenção e ponto preto; CMYK na tela e RGB→CMYK no PDF/X pelo perfil, que vai como DestOutputProfile. Sem perfil CMYK, fórmula simples. Escolhido pelo responsável pelo produto em vez de um motor em Rust puro, aceitando o compilador C no build. |

Alternativa descartada: **C++ com Qt 6 + Skia**. Exigiria compilar Skia
(cadeia de build pesada no Windows), cuidar da vinculação dinâmica do Qt
(LGPL) em todas as plataformas e não traria ganho para o M0.

Todas as dependências são compatíveis com MPL-2.0 e estão em
[`THIRD_PARTY.md`](../THIRD_PARTY.md). O script
[`scripts/check-licenses.mjs`](../scripts/check-licenses.mjs) falha se
alguma dependência transitiva tiver licença fora da lista permitida.

## 2. Componentes

```
crates/
  graphdraw-core/      biblioteca: modelo, .gdr, importação .cdr, cena, raster, SVG, PDF
  graphdraw-cli/       binário `graphdraw` (convert, info)
  graphdraw-app/       binário `graphdraw-app` (editor egui)
native/
  cdr-bridge/          `gd-cdr-bridge` (C++17) sobre libcdr + librevenge
docs/
  spec/                requisitos (única fonte)
  format/              formato .gdr publicado (schema + descrição)
tests/fixtures/        arquivos de teste (.gdr gerados; .cdr de licença livre)
```

### 2.1 Fluxo de dados

```
arquivo ──► open::detect (bytes iniciais)
             ├─ .gdr ─► gdr::read_bytes ──────────────┐
             └─ .cdr/.cmx ─► cdr::bridge (processo) ─► cdr::mapper ─┤
                                                     ▼
                                    model::Document (µm, Y↑) + ImportReport
                                                     │
                                  render::build_scene (por página)
                                                     │  itens achatados em
                                                     │  coordenadas da página
                          ┌──────────────┬───────────┴──┬──────────────┐
                    raster (tiny-skia)  export::svg   export::pdf    app (egui)
                    PNG, miniaturas
```

A **cena** (`render::scene`) é o ponto de convergência: grupos, matrizes,
primitivas paramétricas, texto e imagens são resolvidos uma vez, e todos os
destinos desenham a mesma lista. Isso garante que tela, PNG, SVG e PDF
mostrem o mesmo resultado.

### 2.2 Modelo de documento (doc 02)

- Unidade interna: **micrômetro em `f64`** (GD-DOC-01). Conversões só em
  `units.rs`, na importação e na exportação.
- **Y para cima**, origem no canto inferior esquerdo da página (GD-DOC-02).
- Cada objeto tem matriz afim `[a,b,c,d,e,f]` + geometria local (GD-OBJ-02).
- Retângulo ocupa `[0,w]×[0,h]` local; elipse é centrada na origem local;
  texto tem origem no canto superior esquerdo do bloco; imagem ocupa
  `[0,w]×[0,h]` (descrito em [`docs/format/gdr-1.0.md`](format/gdr-1.0.md)).
- Cor guarda modelo e valores originais (GD-COR-02); conversão para sRGB é
  feita só na hora de exibir.
- Campos e tipos desconhecidos são preservados como JSON bruto
  (GD-GDR-03); objeto inválido vira `ObjectKind::Unknown` com o erro, sem
  impedir a abertura (GD-GDR-10).

### 2.3 Importação `.cdr` — ponte em processo separado

A libcdr é C++ e emite chamadas da interface `RVNGDrawingInterface` da
librevenge. O GraphDraw **não** vincula essas bibliotecas ao executável
principal; em vez disso:

1. `gd-cdr-bridge` (≈250 linhas, [`native/cdr-bridge/`](../native/cdr-bridge/))
   lê o arquivo da entrada padrão e escreve **cada chamada da librevenge como
   uma linha JSON** (`{"op":"drawPath","p":{...}}`). Valores de propriedade
   são `[texto, unidade, número]`.
2. `cdr::bridge` inicia o processo, envia os bytes e lê a saída com limite
   de tempo (300 s).
3. `cdr::mapper` traduz as chamadas para o modelo (polegadas/Y↓ → µm/Y↑),
   registrando aproximações no `ImportReport` (GD-CDR-03).

Motivos: (a) um arquivo malformado que derrube a libcdr derruba só a ponte
— o GraphDraw mostra a mensagem e continua (GD-CDR-06); (b) não há mistura
de ABIs C++ (MinGW × Rust); (c) o protocolo JSON é testável e documentado.

Códigos de saída da ponte: `0` ok, `1` uso/E-S, `2` não suportado ou
protegido, `3` falha de leitura (pode haver saída parcial → aviso
`cdr.partial`).

Localização da ponte, em ordem: variável `GRAPHDRAW_CDR_BRIDGE`; mesma
pasta do executável (ou subpasta `cdr-bridge/`); `../cdr-bridge/` subindo
até 3 níveis (layout `target/` de desenvolvimento); `PATH`.

**Limitação conhecida (GD-CDR-04):** a interface da librevenge entrega
cores como `#rrggbb`; a libcdr converte CMYK/cores especiais para RGB antes.
O relatório de importação informa isso em todo `.cdr` importado. Ver
[`docs/spec/PERGUNTAS.md`](spec/PERGUNTAS.md) (P-01).

A versão do arquivo (GD-CDR-06) é detectada por `cdr::sniff` a partir da
assinatura RIFF (ou do RIFF dentro do contêiner ZIP), seguindo a regra
documentada no código aberto da libcdr.

### 2.4 Renderização e exportação

- **Tela/PNG** (`render::raster`): tiny-skia, com matriz página→pixels.
  A imagem é dividida em faixas horizontais renderizadas em paralelo (uma
  por núcleo); cada faixa só desenha os itens cuja caixa a intercepta. As
  caixas são calculadas uma vez por cena (GD-NF-02).
  Fio de cabelo (`width: 0`) = 1 pixel do dispositivo. PNG com fragmentos
  sRGB e pHYs (resolução).
- **SVG** (`export::svg`): conteúdo num grupo `matrix(0.001 0 0 -0.001 0 H)`
  — coordenadas em µm com Y para cima, `viewBox` em mm. Texto vivo por
  padrão; `--text-to-curves` converte em curvas (GD-EXP-02).
- **PDF** (`export::pdf`): uma matriz `cm` µm→pt por página; cores no modelo
  original (DeviceRGB / DeviceCMYK / DeviceGray / Separation); opacidade por
  ExtGState; gradientes por shading axial/radial; imagens com SMask. Texto
  sai em curvas no M0.
- **PDF/X-1a:2001** (GD-EXP-07, `--pdfx` / *Arquivo → Exportar PDF/X-1a*): PDF 1.3; cores convertidas para CMYK (RGB por fórmula, sem ICC até o GD-CMS), transparência achatada (imagem CMYK de 300 ppp da composição, recortada pelo contorno vetorial do objeto), imagens em DeviceCMYK sem SMask e sem Interpolate, TrimBox/BleedBox/MediaBox com a sangria, OutputIntent GTS_PDFX (FOGRA39, FOGRA47 ou CGATS TR 001), Info com as chaves GTS e Trapped, /ID. `PdfReport` lista o que foi convertido. Verificação estrutural em `tests/pdfx.rs` (sem validador externo).
- **Posição do contorno** (GD-OUT-04): interno e externo são um traço de
  espessura dupla recortado pela forma (tela/PNG: máscara; SVG:
  `clipPath`/`mask`; PDF: recorte `W`). No PDF o externo usa par-ímpar com
  uma moldura em volta, exato para formas sem auto-sobreposição. Curvas
  abertas e fio de cabelo usam sempre centrado.
- **Impressão × tela** (GD-CAM-01, GD-OBJ-04): `SceneOptions { print }`.
  Exportações usam `print: true` (omitem camadas/objetos não imprimíveis);
  o editor usa `print: false`.

### 2.5 Linha de comando

`graphdraw convert <entrada> <saida> [--page N] [--dpi N] [--text-to-curves]`
(GD-EXP-06), mais `--format`, `--background`, `--no-antialias`, `--report`.
Entrada pasta = conversão em lote (GD-CDR-07), um relatório JSON por
arquivo com `--report`. `graphdraw info <arquivo>` mostra páginas e o
relatório de importação.

`graphdraw fidelity <corpus> [--dpi 150] [--run NOME]` (GD-FID-30, doc 25)
abre cada `casos/NNNN/original.cdr` (ou `.cmx`/`.gdr`), exporta o PDF em
curvas e compara com `ref-curvas.pdf`: os dois passam pelo mesmo
rasterizador (`pdftoppm` do Poppler, achado no PATH, em `GRAPHDRAW_POPPLER`
ou no MSYS2). O candidato é alinhado à referência (até 3 px, numa moldura
ampliada para que empurrar tinta para fora não conte como acerto) e cada
página recebe % de pixels com ΔE2000 > 5, SSIM e desvio de geometria em mm
(tamanho da página e bordas da caixa da tinta, deslocamento incluído;
resolução de 1 pixel, 0,17 mm a 150 ppp) → Fiel / Aceitável / Falha pelos
limiares do doc 25. Também mede páginas, travamentos e tempo de abertura
(GD-FID-14/15), e com `ref-texto.pdf` o texto (`pdftotext`) e as fontes
da referência que o candidato não usa (`pdffonts`, GD-FID-13). Grava em
`<corpus>/resultados/<execução>/` o relatório privado (HTML com resumo por
categoria e versão e referência | candidato | diferença, e JSON),
`resumo-publico.json` (só números, GD-FID-31) e a tendência contra a
execução anterior. Sai com código 1 se houver falha ou travamento. Com
`ref-cmyk.pdf`, compara as cores CMYK sólidas (`k`/`sc` de quatro valores,
lidas com `lopdf`) com o PDF/X-1a do candidato: maior desvio em pp pela cor
mais próxima, até 1 pp Fiel e 3 pp Aceitável (GD-FID-12). Caixas por
objeto (GD-FID-11) e contagem de objetos (GD-FID-14) ainda não são medidas. Código em `graphdraw-core::fidelity`.

### 2.6 Núcleo de edição (`graphdraw-core::edit`)

Toda a lógica de edição fica na biblioteca e é testada sem interface. O app
só traduz gestos em chamadas destas funções.

| Módulo | Papel | Requisitos |
|---|---|---|
| `history` | `Editor`: documento + registro de alterações reversíveis por transação (só o "antes" e o "depois" de cada objeto, camada ou página tocados, nunca o documento inteiro). Arrastos viram um passo. | doc 12 (GD-UND-01..08), GD-CAM-01 |
| `pages` | Operações da barra de abas: nova página após a atual, duplicar com IDs novos, mover em bloco, excluir mantendo ao menos uma. | GD-PGT-03, 11..15 |
| `tree` | `ObjRef` = posição do objeto de primeiro nível + caminho de índices até o filho. Alterar um filho substitui o objeto de primeiro nível no histórico, então grupos não precisam de tipo de alteração próprio. | GD-SEL-02 |
| `ops` | Operações sobre a seleção (lista ordenada de IDs, em qualquer nível): transformar em coordenadas da página (convertidas para o espaço do pai), alinhar, ordem, agrupar/desagrupar, duplicar, excluir, mover para camada, preenchimento/contorno (num grupo valem para o conteúdo). | GD-TRF-01/02, GD-ALI-01/02, GD-ORD-01, GD-CAM-03, GD-FIL-01/02, GD-OUT-01..04 |
| `nodes`, `convert` | Edição de nós e conversão de primitivas em curvas. | GD-NOD-01..05, GD-CNV-01 (parcial) |
| `query` | Caixas geométrica e visual (a visual considera a posição do contorno), acerto de clique (inclusive dentro de grupos) e seleção por área. | GD-OBJ-06, GD-SEL-01..03 |
| `snap` | Encaixe em objetos, página e grade, com prioridade. | GD-SNP-01/02 |
| `geometry` | Comprimento, área, ponto mais próximo, interseções, ponto dentro. | GD-GEO-05 |

`units` interpreta os campos numéricos com contas e unidades (GD-TRF-04 / GD-UI-11).

### 2.7 Editor (`graphdraw-app`)

`graphdraw-app [arquivo]`: janela egui com barra de menus, barra padrão,
barra de propriedades contextual, caixa de ferramentas, área de desenho,
painéis à direita, paleta e barra de status (GD-UI-01). Os painéis em abas
são Transformar, Alinhar, Camadas, Histórico, Páginas e Relatório (parte de
GD-UI-02). Abre por diálogo ou arrastar-soltar (GD-CDR-01), salva `.gdr` com
aviso de alterações não salvas (GD-UND-07) e exporta SVG/PDF/PNG.

- `canvas.rs`: ferramentas Seleção, Forma, Zoom, Mão, Bézier, Retângulo e
  Elipse (GD-FER-01, 04, 05, 10, 22, 30, 31). Clique pega o objeto de
  primeiro nível e `Ctrl+clique` o objeto dentro do grupo; a ferramenta
  Forma sempre desce no grupo para editar o caminho.
- `doc.rs`: documento aberto, seleção (fora do histórico, GD-UND-06),
  camada ativa (por ID) e cache da cena.
- `panels.rs`: barra de propriedades e painéis.
- `pagebar.rs`: barra de páginas em abas (doc 21, GD-PGT-*). A seleção de
  abas guarda IDs de página, para sobreviver a reordenar e desfazer. O
  marcador `*` vem de `Editor::dirty_pages`: cada transação registra as
  páginas que tocou, e o conjunto é a diferença entre a posição atual e a do
  último salvamento no histórico.
- `newdoc.rs`: janela Novo documento (doc 22, GD-DIM-10..14).
- `canvas.rs` também desenha a sangria da página (GD-DIM-21) e a ferramenta Prancheta (GD-DIM-23..25), sobre `edit::pagesize`.
- `dialogs.rs`: janelas de contorno e preenchimento, abertas pelos
  indicadores da barra de status.
- `icons.rs` + `assets/icons/make-icons.mjs`: ícones próprios do projeto,
  gerados de uma tabela única (grade 24×24, traço 1,75 px, monocromáticos).

A página é rasterizada com o mesmo `render::raster` da exportação, só na
região visível, e a cena é refeita apenas quando o documento muda.

Camadas em modo esboço (GD-CAM-01) mostram só o traço fino na tela; a
exportação, que usa `print: true`, sai normal.

## 3. Compilação

Pré-requisitos no Windows:

1. Rust estável com alvo GNU: `rustup-init -y --default-host x86_64-pc-windows-gnu`.
2. MSYS2 com: `mingw-w64-ucrt-x86_64-gcc mingw-w64-ucrt-x86_64-pkgconf
   mingw-w64-ucrt-x86_64-libcdr make` (ponte) e `mingw-w64-x86_64-binutils`
   (o alvo GNU do Rust usa o `dlltool` dele — basta colocar
   `msys64/mingw64/bin` **no fim** do `PATH`).

```bash
# ponte libcdr (gera target/cdr-bridge/ com as DLLs necessárias)
MSYSTEM=UCRT64 C:/msys64/usr/bin/bash.exe -lc "cd /d/caminho/graphdraw && bash native/cdr-bridge/build.sh"
# núcleo, CLI, editor e testes
cargo build --release
cargo test --workspace
```

Em Linux: `apt install libcdr-dev librevenge-dev pkg-config g++` e o mesmo
`build.sh`.

## 4. Testes e rastreabilidade

| Requisito | Teste |
|---|---|
| GD-DOC-01 | `units::tests::gd_doc_01_conversions` |
| GD-DOC-02 | `gdr_format::gd_doc_02_y_axis_up_origin_bottom_left` |
| GD-COR-02 | `color::tests::gd_cor_02_cmyk_preserved`, `export::pdf_keeps_cmyk_and_spot_colors` |
| GD-CAM-01, GD-OBJ-04 | `export::gd_cam_01_non_printable_layer_only_on_screen` |
| GD-GDR-01..11 + aceite | `tests/gdr_format.rs` (um teste por ID) |
| GD-GDR-08 | `gd_gdr_08_fixtures_validate_against_schema` (+ documentos importados de `.cdr`) |
| GD-CDR-01, 02, 03, 06, 07 | `tests/cdr_import.rs`, `graphdraw-cli/tests/cli.rs` |
| GD-FNT-02 / GD-CDR-05 | `export::gd_fnt_02_missing_font_reported_and_name_kept` |
| GD-EXP-01, 02, 06 | `tests/export.rs`, `graphdraw-cli/tests/cli.rs` |
| GD-NF-01, GD-NF-02 | `examples/bench.rs` (medição), `export::gd_nf_02_parallel_bands_match_sequential` |
| GD-GEO-05, aceite do doc 03 | `geometry::tests::*`, `convert::tests::gd_geo_acceptance_circle_to_curves` |
| GD-NOD-01..07 | `nodes::tests::*` (`gd_nod_06_align_nodes`, `gd_nod_07_reduce_nodes`) |
| GD-PRI-05, GD-PRI-06, GD-FER-32 | `generate::tests::*`, `canvas::tests::polygon_tool_draws_spiral_and_graph_paper` (app) |
| GD-FIL-03 (elíptico, cônico, retangular) | `gradient::tests::*`, `tests/gradients.rs` (PNG e PDF pelo Poppler), `mapper::tests::cdr_gradient_styles_map_to_kinds` |
| Robustez (GD-GDR-10, GD-CDR-06) | `tests/robustness.rs`: `.gdr` com bytes trocados/cortado, `document.json` hostil, `.cdr` cortado/alterado (sem pânico) e ida e volta com os campos novos |
| GD-PAG-04 (lado a lado) | `canvas::tests::spread_shows_neighbors_and_click_switches` (app); visão geral em `pagebar` |
| GD-TXT-09 | `ops::tests::gd_txt_09_text_on_path` |
| GD-TRA-02 | `tests/blend.rs` |
| GD-DOC-03 | `rulers::tests::gd_doc_03_drawing_scale_on_rulers` (app) |
| GD-TXT-03 | `text::tests::gd_txt_03_convert_artistic_paragraph` |
| GD-ALI-04 | `ops::tests::gd_ali_04_align_baseline` |
| GD-CNV-03 | `ops::tests::gd_cnv_03_convert_to_bitmap`, `app::tests::bitmap_window_fits_small_screen` |
| GD-PRE-01..10 (verificação prévia, sangria automática) | `preflight::tests::gd_pre_*` (core), `app::tests::gd_pre_preflight_panel_and_extend_command` |
| GD-FER-20/21/25/26 (mão livre, linha 2 pontos, polilinha, curva 3 pontos) | `drawing::tests::gd_fer_*` (core), `canvas::tests::line_tools_on_canvas` (app) |
| GD-FER-52 (preenchimento inteligente) | `smartfill::tests::*` (core), `canvas::tests::smart_fill_click` (app) |
| GD-PRI-07, GD-FER-42 (formas básicas, chamada) | `basic::tests::*` (core), `canvas::tests::basic_shape_draw_and_adjust` (app) |
| GD-GUI-03 (objeto vira guia) | `tests/guide_layer.rs` |
| GD-CMS-04 (política de perfis ao abrir) | `palette::tests::gd_cms_04_convert_to_working_profile` (core), `app::tests::gd_cms_04_profile_policy_on_open` |
| GD-SEP-01 (prévia de separações) | `separation::tests::*` (core), `canvas::tests::separation_preview_on_canvas` (app) |
| GD-FIL-04..06/10/12 (padrões, caminho aberto) | `pattern::tests::*` (core), `tests/patterns.rs`, `dialogs::tests::pattern_fill_dialog` (app) |
| GD-TXT-10 (texto contornando objeto) | `textwrap::tests::*`, `render::text::tests::wrap_flows_around_zone` (core), fixture `text-wrap.gdr` |
| GD-GRD-01 (grades) | `ops::tests::gd_grd_01_printable_grid` (core), `canvas::tests::grids_paint` (app) |
| GD-FER-29 (esboço contínuo) | `drawing::tests::gd_fer_29_sketch_join` (core), `canvas::tests::continuous_sketch_joins` (app) |
| GD-VAR-01, GD-OUT-08 (largura variável) | `varwidth::tests::*` (core), fixture `variable-width.gdr`, `canvas::tests::width_tool_on_canvas` (app) |
| GD-FER-33 (simetria) | `symmetry::tests::*` (core), fixture `symmetry.gdr`, `canvas::tests::symmetry_draw_inside` (app) |
| GD-FER-24, GD-BSP-01 (B-spline) | `bspline::tests::*` (core), fixture `bspline.gdr`, `canvas::tests::bspline_tool_on_canvas` (app) |
| GD-FER-41, GD-CON-01 (conectores) | `connector::tests::*` (core), fixture `connectors.gdr`, `canvas::tests::connector_drag_links` (app) |
| GD-FER-40, GD-COT-01..03, GD-DOC-03 (cotas) | `dimension::tests::*` (core), `tests/dimensions.rs`, fixture `prepress.gdr`, `canvas::tests::dimension_tool_on_canvas` (app) |
| GD-FER-03 (transformação livre) | `freexf::tests::gd_fer_03_free_transform_modes` (core), `canvas::tests::free_transform_on_canvas` (app) |
| GD-FER-23 (caneta) | coberta pela Bézier: a prévia do próximo segmento segue o cursor (`canvas.rs`, desenho do rascunho) |
| GD-FER-16..19 (recortar, faca, borracha, segmento virtual) | `cutting::tests::gd_fer_*` (core), `canvas::tests::cut_tools_on_canvas` (app) |
| GD-PRE-20 (kerf) | `cut::tests::gd_pre_20_kerf_grows_parts_and_shrinks_holes` |
| GD-BDR-01/02/20/22/25/70/71 (botão direito) | `canvas::tests::gd_bdr_*` (app) |
| GD-BDR-10/11 | `app::tests::gd_bdr_10_11_palette_without_and_with_selection` |
| GD-BDR-28/33/41/42/81 | `ops::tests::gd_bdr_*` |
| GD-CTG-03/06/07/08/09 (conta-gotas, núcleo) | `eyedrop::tests::gd_ctg_*` (core) |
| GD-CTG-03/11, lupa | `eyedrop::tests::*` (app) |
| GD-CTG-05/06/12 (ciclo amostrar e despejar) | `canvas::tests::gd_ctg_05_pick_and_apply_cycle` (app) |
| GD-BDR-43/45 (menus pelo alvo) | `ctxmenu::tests::*` (app) |
| GD-BDR-50/51 | `nodes::tests::gd_bdr_50_symmetric_averages_handles`, `gd_bdr_51_segment_to_line` |
| GD-OUT-06 | `scene::tests::gd_out_06_calligraphic_nib` |
| GD-OUT-05 | `arrows::tests::gd_out_05_arrow_at_end`, `tests/arrows.rs` |
| GD-OBJ-05 | `ops::tests::gd_obj_05_user_data` |
| GD-NOD-08, GD-TRF-03 | `ops::tests::gd_nod_08_extract_subpath`, `ops::tests::gd_trf_03_clear_transformations` |
| GD-UND-01..07 + aceite do doc 12 | `history::tests::gd_und_*` |
| GD-OBJ-06, GD-SEL-01..03 | `query::tests::*` |
| GD-SEL-02 (dentro de grupo) | `ops::tests::gd_sel_02_edit_inside_group`, `query::tests::gd_sel_02_deep_hit_finds_child`, `tree::tests::*`, `tests/m1_editing.rs` |
| GD-TRF-01/02, GD-ALI-01/02, GD-ORD-01 | `ops::tests::gd_trf_*`, `gd_ali_*`, `gd_ord_01_ordering` |
| GD-TRF-04 / GD-UI-11 | `units::tests::gd_trf_04_expressions` |
| GD-SNP-01/02 | `snap::tests::gd_snp_01_objects_page_grid_priority` |
| GD-PGT-03, 05, 11..15 | `pages::tests::*`, `history::tests::page_props_keep_objects`, `pagebar::tests::*` (app) |
| GD-OUT-04 | `tests/stroke_position.rs`, `tests/m1_editing.rs` |
| GD-CAM-01 (esboço), GD-CAM-03 | `tests/m1_editing.rs`, `ops::tests::gd_cam_03_move_to_layer`, `history::tests::layers_undo` |
| Zoom/pan do editor | `graphdraw-app/src/view.rs` (testes de unidade) |
| GD-BOO-01..03 | `shapeops::tests::gd_boo_01_*`, `gd_boo_02_03_*` |
| GD-BOO-04..08 | `shapeops::tests::gd_boo_04_simplify` … `gd_boo_08_boundary` |
| GD-ALI-05 | `smart::tests::gd_ali_05_*`, `canvas::tests::smart_guides_*` (app) |
| GD-FIL-11 / GD-FER-50 | `gradtool::tests::*`, `canvas::tests::interactive_gradient_tool` (app) |
| GD-PRI-03 (edição) | `canvas::tests::polygon_vertex_edit_is_symmetric` (app) |
| Doc 06, formatação por trecho | `text::tests::style_and_fill_by_range`, `app::tests::text_range_style_and_palette_color` |
| GD-FID-10..15/30/31 | `fidelity::tests::*`, `tests/fidelity.rs` (corpus sintético; pulado sem Poppler) |

Fixtures `.gdr` são geradas por
`cargo run -p graphdraw-core --example make_fixtures` (IDs fixos). Amostras
`.cdr`/`.cmx` vêm do corpus de testes do LibreOffice (MPL-2.0) — ver
[`tests/fixtures/cdr/README.md`](../tests/fixtures/cdr/README.md).

CI: [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) (Windows e Linux; formatação, clippy, testes e licenças).

Testes de `.cdr` exigem a ponte compilada; sem ela são pulados com aviso,
exceto se `GRAPHDRAW_REQUIRE_BRIDGE=1` (usado no CI).

## 5. Desempenho

`cargo run --release -p graphdraw-core --example bench` gera documentos
sintéticos determinísticos e mede os requisitos não funcionais. Resultado
em i5-13420H (12 threads), 2026-10-16:

| Requisito | Cenário | Medido | Limite |
|---|---|---|---|
| GD-NF-01 | abrir `.gdr` com 100 mil objetos (48,7 MB de JSON, 10,7 MB no disco) | 1,3–2,3 s (varia entre execuções) | 5 s |
| GD-NF-02 | redesenhar 10 mil objetos visíveis em 1920×1080 | 15–17 ms/quadro (60–65 fps) | 33 ms (30 fps) |
| GD-NF-02 (recursos novos, 2026-10-17) | os mesmos 10 mil objetos com 1 em 10 de gradiente calculado (elíptico/cônico/retangular), 1 em 15 com mesclagem, 1 em 20 com seta e um fundo cônico de página inteira | 26 ms/quadro (39 fps) | 33 ms (30 fps) |

Antes das faixas paralelas com descarte, GD-NF-02 levava 230 ms/quadro.
O cenário com recursos novos levava 66 ms até a tabela de cores dos
gradientes calculados ganhar cache (era refeita por objeto e por faixa).
No CI o passo roda com `--check` mas não bloqueia, porque o runner não é a
máquina de referência.

## 5.1 Salvamento automático e recuperação

`recovery.rs` (app):
- A cada 2 min, copia os documentos alterados para
  `%APPDATA%GraphDrawecuperacao` (`<execução>-<documento>.gdr` + `.json`
  com título, origem e hora), numa linha de execução à parte.
- Salvar ou fechar o documento apaga a cópia; a saída normal apaga todas
  as cópias da execução.
- Cada execução segura `<execução>.alive` sem permitir apagar: no início, as
  cópias cuja execução não segura mais o arquivo são de uma queda e o
  programa oferece recuperar. O documento recuperado conta como alterado.
- No Linux, o arquivo aberto pode ser apagado; duas instâncias ao mesmo
  tempo veriam as cópias uma da outra.
- Pânico grava versão, sistema e pilha em `%APPDATA%GraphDrawalhas`, e
  o próximo início mostra onde ficou.
- Salvar continua atômico (GD-GDR-09) e deixa o arquivo anterior em
  `.gdr.bak`.

## 6. Limitações conhecidas

Do M0, ainda valem:

- Cores de `.cdr` chegam em RGB (limitação da interface librevenge; P-01).
- Texto no PDF sai vivo (Type0/CID, subconjunto da fonte com `subsetter`,
  ToUnicode); fontes OpenType/CFF ficam em curvas no PDF/X-1a (PDF 1.3).
  Contorno, gradiente ou transparência no texto também saem em curvas.
- Gerenciamento de cor: perfis RGB e CMYK por documento (arquivo ICC por
  caminho, não embutido no `.gdr`); perfil de cinza, prova de cor na tela
  e política ao abrir (GD-CMS-03/04) ainda não existem. Sem perfil CMYK, a
  conversão do PDF/X-1a é a fórmula simples.
- Gradientes: só linear e radial; repetir/espelhar não existe no PDF (sai como "estender").
- Transparência de grupo é aproximada multiplicando a opacidade dos filhos.
- Diagramação de texto: quebra no quadro, alinhamentos e justificado,
  aviso de texto excedente. Formatação por trecho: com texto selecionado
  na edição, fonte, estilo, corpo e a cor da paleta valem só para ele (o
  alinhamento para os parágrafos tocados); sem seleção, para o objeto
  inteiro. Texto em caminho (GD-TXT-09): o caminho é copiado para o
  texto (o objeto original fica), cada glifo vai para o ponto do caminho no
  meio do seu avanço; no PDF e no SVG sai em curvas; na edição o cursor não
  acompanha o caminho. Sem bidi, hifenização nem quadros encadeados.
- Carregamento sob demanda de páginas (GD-GDR-11) não implementado: páginas
  separadas são lidas na abertura.

Do M3:

- Padrões (GD-FIL-04..06):
  - o ladrilho é rasterizado num superladrilho 2 × 2 na resolução do padrão
    (padrão 300 ppp, até 1024 px por lado), já com deslocamentos e
    espelhamento;
  - na tela vai por shader de repetição; no PDF, por padrão de ladrilho
    com o superladrilho como imagem RGB; no SVG, por `<pattern>` com PNG;
  - padrão vetorial, portanto, sai rasterizado na saída;
  - no PDF/X-1a, o objeto com padrão é achatado em CMYK como as
    transparências;
  - cores especiais do padrão de duas cores não viram chapa separada.
- Política de perfis (GD-CMS-04):
  - os perfis de trabalho e a política (Perguntar, Manter, Converter, Usar
    sem converter) ficam no perfil do usuário e se escolhem na janela
    Gerenciamento de cor;
  - ao abrir um `.gdr` com perfis diferentes, o editor aplica a política;
  - Converter passa CMYK → CMYK e RGB → RGB pelo LittleCMS, num passo de
    desfazer. Sem perfil CMYK de um dos lados, o CMYK fica como está;
  - imagens com perfil ICC próprio e a importação `.cdr` ainda não seguem a
    política (Beta).
- Prévia de separações (GD-SEP-01): a cena é refeita por chapa, com as
  tintas em cinza. Ela segue o vazamento e a sobreimpressão do objeto. Na
  composta, as chapas se multiplicam e cada especial usa a cor alternativa.
  Imagens e padrões (RGB) entram por fórmula de CMYK, sem perfil.
- GD-FIL-10 vale para padrões; gradientes sempre acompanham o objeto.
- GD-FIL-12: um caminho misto (aberto e fechado) mantém o preenchimento.

- A ferramenta Polígono tem três famílias (GD-FER-32): polígono/estrelas,
  espiral (simétrica ou logarítmica, vira caminho aberto) e papel gráfico
  (grupo de retângulos). Espiral e papel gráfico não ficam paramétricos.
- Polígono/estrela/estrela complexa são primitivas paramétricas; na
  ferramenta Forma, o vértice externo muda tamanho e giro de todas as
  pontas e o interno a nitidez (GD-PRI-03). Para editar nó a nó, converter
  em curvas.
- Guias: horizontais, verticais e inclinadas, da página ou do documento;
  guias de página mestra e camada especial de guias (GD-CAM-04) não
  existem; a origem das réguas não é salva.
- Gradientes: linear, radial, elíptico, cônico e retangular pela janela
  (geometria pela caixa do objeto; os três últimos têm `radiusY` e
  `angle`, calculados ponto a ponto: na tela por pixel, no PDF por
  sombreamento tipo 1 com função PostScript, no SVG o elíptico vira radial
  esticado e cônico/retangular um ladrilho de imagem) e pela ferramenta de preenchimento interativo (G; GD-FIL-11:
  arrastar cria linear, alças de início/fim ou centro/raio, paradas
  deslizam na linha, duplo clique cria parada com a cor do ponto, Delete
  tira, clique na paleta pinta a parada escolhida). Ponto médio das paradas
  não é editável; a ferramenta interativa edita elíptico/cônico/retangular
  pelo centro e raio. Na importação `.cdr`, elipsoide, quadrado,
  retangular e axial chegam como o tipo próprio.
- Paletas: arquivos ASE/GPL/CSV/JSON; .xml/.cpl de terceiros não são lidos.

Do M2:

- Booleanas (soldar, aparar, interseção), combinar/separar e contorno de
  deslocamento operam sobre polígonos com tolerância de 1 µm
  (`i_overlay`) e o resultado volta a Bézier por ajuste próprio com erro
  de 2 µm (`edit/fit.rs`). Simplificar, frente/trás menos, excluir (XOR),
  dividir e limite (GD-BOO-04..08, `shapeops::shape_op`) seguem a ordem de
  empilhamento, não a de seleção; dividir dá às peças os atributos do
  objeto mais à frente que as cobre; limite cria um contorno sem
  preenchimento (sem furos) e mantém os originais.
- Corte (doc 17): marcação por cor especial ou camada "CutContour",
  gerar linha de corte (união + deslocamento), marcas de registro, ordem
  (furos primeiro, vizinho mais próximo, sentido), sobreposição,
  compensação de lâmina, caixa de descasque, prévia animada e saída
  PLT/HPGL e DXF R12 da página atual (`export/cut.rs`). No PDF a linha
  de corte sai como Separation "CutContour"; o SVG ainda não separa a
  camada de corte (GD-CRT-12) e o envio direto ao plotter é pós-MVP.

Do M1:

- Seleção (GD-SEL-03): clique, Shift+clique, retângulo, laço (Alt +
  arrastar), tudo, inverter e semelhantes (mesmo tipo, preenchimento ou cor
  de contorno). Os semelhantes comparam só objetos de primeiro nível.
- Retângulo e elipse têm o modo 3 pontos (GD-FER-30/31) na barra de
  propriedades; o painel Alinhar também distribui (GD-ALI-03).
- Painéis (GD-UI-03): coluna à direita com abas, recolhível numa faixa de
  ícones, painéis soltos em janelas e área de trabalho salva no perfil do
  usuário; ainda não há vários perfis de área nem encaixe em outros lados. Atalhos têm
  perfis e importação/exportação (GD-UI-10). O painel Camadas mostra os
  objetos dentro de cada camada (GD-UI-02), com miniatura, renomear,
  olho/cadeado, menu de botão direito e arrastar-soltar entre posições,
  camadas e grupos (arrasta um objeto por vez).
- A página mestra existe no núcleo, mas a interface não a mostra.
- Agrupar exige objetos na mesma camada e no mesmo grupo.
- Contorno externo no PDF pode sair errado em formas com auto-sobreposição
  e regra não-zero (ver 2.4).
- A origem local de retângulos, elipses, textos e imagens segue P-04 de
  [`docs/spec/PERGUNTAS.md`](spec/PERGUNTAS.md), confirmada em 2026-10-16.
