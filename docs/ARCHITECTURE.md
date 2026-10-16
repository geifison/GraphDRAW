# Arquitetura do GraphDraw

Versão 0.1 · marco M0 (visualizador) · 2026-10-16

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
| Interface | **egui/eframe** | MIT/Apache-2.0 | "Rust + egui/Slint" | Imediata, leve, sem licença copyleft; suficiente para o visualizador do M0. Reavaliar no M1 (painéis encaixáveis, GD-UI-03). |
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
  graphdraw-viewer/    binário `graphdraw-viewer` (GUI egui)
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
                    raster (tiny-skia)  export::svg   export::pdf    viewer (egui)
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
  Fio de cabelo (`width: 0`) = 1 pixel do dispositivo. PNG com fragmentos
  sRGB e pHYs (resolução).
- **SVG** (`export::svg`): conteúdo num grupo `matrix(0.001 0 0 -0.001 0 H)`
  — coordenadas em µm com Y para cima, `viewBox` em mm. Texto vivo por
  padrão; `--text-to-curves` converte em curvas (GD-EXP-02).
- **PDF** (`export::pdf`): uma matriz `cm` µm→pt por página; cores no modelo
  original (DeviceRGB / DeviceCMYK / DeviceGray / Separation); opacidade por
  ExtGState; gradientes por shading axial/radial; imagens com SMask. Texto
  sai em curvas no M0.
- **Impressão × tela** (GD-CAM-01, GD-OBJ-04): `SceneOptions { print }`.
  Exportações usam `print: true` (omitem camadas/objetos não imprimíveis);
  o visualizador usa `print: false`.

### 2.5 Linha de comando

`graphdraw convert <entrada> <saida> [--page N] [--dpi N] [--text-to-curves]`
(GD-EXP-06), mais `--format`, `--background`, `--no-antialias`, `--report`.
Entrada pasta = conversão em lote (GD-CDR-07), um relatório JSON por
arquivo com `--report`. `graphdraw info <arquivo>` mostra páginas e o
relatório de importação.

### 2.6 Visualizador

`graphdraw-viewer [arquivo]`: janela egui com menu, lista de páginas,
área de desenho com zoom (roda do mouse, centrado no cursor; atalhos) e
pan (arrastar), barra de status com coordenadas em mm na convenção do
documento (Y para cima), painel "Relatório de importação" (GD-UI-02),
abrir por diálogo ou arrastar-soltar (GD-CDR-01) e exportação SVG/PDF/PNG.
A página é rasterizada com o mesmo `render::raster` da exportação, apenas
na região visível, e a cena de cada página é mantida em cache.

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
# núcleo, CLI, visualizador e testes
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

Fixtures `.gdr` são geradas por
`cargo run -p graphdraw-core --example make_fixtures` (IDs fixos). Amostras
`.cdr`/`.cmx` vêm do corpus de testes do LibreOffice (MPL-2.0) — ver
[`tests/fixtures/cdr/README.md`](../tests/fixtures/cdr/README.md).

Testes de `.cdr` exigem a ponte compilada; sem ela são pulados com aviso,
exceto se `GRAPHDRAW_REQUIRE_BRIDGE=1` (usado no CI).

## 5. Limitações conhecidas do M0

- Cores de `.cdr` chegam em RGB (limitação da interface librevenge; P-01).
- Texto no PDF sai em curvas; texto vivo com fontes incorporadas é M2 (GD-EXP-02/03).
- Sem gerenciamento ICC (M2); sem PDF/X, sangria e marcas (M2).
- Gradientes: só linear e radial; repetir/espelhar não existe no PDF (sai como "estender").
- Transparência de grupo é aproximada multiplicando a opacidade dos filhos.
- Posição do contorno (interna/externa, GD-OUT-04) é desenhada como centrada.
- Diagramação de texto simples: sem bidi, hifenização ou justificação.
- Carregamento sob demanda de páginas (GD-GDR-11) não implementado: páginas
  separadas são lidas na abertura.
- Benchmarks GD-NF-01/02 ainda não automatizados.
