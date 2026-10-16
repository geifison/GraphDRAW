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
| Interface | **egui/eframe** | MIT/Apache-2.0 | "Rust + egui/Slint" | Imediata, leve, sem licença copyleft. Mantida no M1: atende ferramentas, barras e painéis do editor. Os painéis ficam em abas numa coluna fixa à direita; painéis flutuantes e perfis de área de trabalho (GD-UI-03) ainda não existem. |
| Gerenciamento de cor | LittleCMS 2 — **adiado para o M2** | MIT | LittleCMS 2 | GD-CMS-01 é do marco M2. No M0, CMYK→RGB só para exibição, por fórmula simples, e o PDF mantém CMYK original. |

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

### 2.6 Núcleo de edição (`graphdraw-core::edit`)

Toda a lógica de edição fica na biblioteca e é testada sem interface. O app
só traduz gestos em chamadas destas funções.

| Módulo | Papel | Requisitos |
|---|---|---|
| `history` | `Editor`: documento + registro de alterações reversíveis por transação (só o "antes" e o "depois" de cada objeto, camada ou página tocados, nunca o documento inteiro). Arrastos viram um passo. | doc 12 (GD-UND-01..08), GD-CAM-01 |
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
| GD-NOD-01..05 | `nodes::tests::*` |
| GD-UND-01..07 + aceite do doc 12 | `history::tests::gd_und_*` |
| GD-OBJ-06, GD-SEL-01..03 | `query::tests::*` |
| GD-SEL-02 (dentro de grupo) | `ops::tests::gd_sel_02_edit_inside_group`, `query::tests::gd_sel_02_deep_hit_finds_child`, `tree::tests::*`, `tests/m1_editing.rs` |
| GD-TRF-01/02, GD-ALI-01/02, GD-ORD-01 | `ops::tests::gd_trf_*`, `gd_ali_*`, `gd_ord_01_ordering` |
| GD-TRF-04 / GD-UI-11 | `units::tests::gd_trf_04_expressions` |
| GD-SNP-01/02 | `snap::tests::gd_snp_01_objects_page_grid_priority` |
| GD-OUT-04 | `tests/stroke_position.rs`, `tests/m1_editing.rs` |
| GD-CAM-01 (esboço), GD-CAM-03 | `tests/m1_editing.rs`, `ops::tests::gd_cam_03_move_to_layer`, `history::tests::layers_undo` |
| Zoom/pan do editor | `graphdraw-app/src/view.rs` (testes de unidade) |

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

Antes das faixas paralelas com descarte, GD-NF-02 levava 230 ms/quadro.
No CI o passo roda com `--check` mas não bloqueia, porque o runner não é a
máquina de referência.

## 6. Limitações conhecidas

Do M0, ainda valem:

- Cores de `.cdr` chegam em RGB (limitação da interface librevenge; P-01).
- Texto no PDF sai em curvas; texto vivo com fontes incorporadas é M2 (GD-EXP-02/03).
- Sem gerenciamento ICC (M2); sem PDF/X, sangria e marcas (M2).
- Gradientes: só linear e radial; repetir/espelhar não existe no PDF (sai como "estender").
- Transparência de grupo é aproximada multiplicando a opacidade dos filhos.
- Diagramação de texto simples: sem bidi, hifenização ou justificação.
- Carregamento sob demanda de páginas (GD-GDR-11) não implementado: páginas
  separadas são lidas na abertura.

Do M1:

- Seleção por laço, por tipo/propriedade e inverter (GD-SEL-03) ainda não
  existem; só clique, Shift+clique e retângulo.
- Retângulo e elipse de 3 pontos (variantes de GD-FER-30/31) ainda não
  existem; distribuir (painel Alinhar) é do M3.
- Painéis em abas fixas (sem flutuar nem perfis, GD-UI-03); atalhos não são
  configuráveis (GD-UI-10). Falta o painel Objetos em árvore (GD-UI-02).
- Camadas da página mestra não aparecem no painel de Camadas.
- Agrupar exige objetos na mesma camada e no mesmo grupo.
- Contorno externo no PDF pode sair errado em formas com auto-sobreposição
  e regra não-zero (ver 2.4).
- A origem local de retângulos, elipses, textos e imagens segue a decisão
  provisória P-04 de [`docs/spec/PERGUNTAS.md`](spec/PERGUNTAS.md), ainda
  sem confirmação.
