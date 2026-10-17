# Dependências de terceiros

Todas as licenças abaixo são compatíveis com a MPL-2.0 do GraphDraw. As
dependências Rust transitivas são verificadas automaticamente por
`node scripts/check-licenses.mjs` (falha se aparecer licença fora da lista
permitida). A lista completa de crates transitivos fica em `Cargo.lock`.

## Distribuídas com o GraphDraw — Rust (vinculadas estaticamente)

| Nome | Versão | Licença | Uso |
|---|---|---|---|
| serde | 1.0.229 | MIT OR Apache-2.0 | Serialização do modelo |
| serde_json | 1.0.154 | MIT OR Apache-2.0 | JSON do `.gdr` e da ponte libcdr |
| zip | 9.0.4 | MIT | Contêiner `.gdr` (GD-GDR-01) |
| sha2 | 0.11.1 | MIT OR Apache-2.0 | Hashes do manifesto e nomes de imagens |
| uuid | 1.28.0 | Apache-2.0 OR MIT | IDs UUID v7 (GD-GDR-04) |
| kurbo | 0.13.1 | Apache-2.0 OR MIT | Geometria Bézier e matrizes |
| lcms2 / lcms2-sys | 6.2.0 / 4.0.7 | MIT | Gerenciamento de cor (GD-CMS-01); o lcms2-sys compila o LittleCMS 2 (MIT) que vem junto |
| subsetter | 0.2.6 | MIT OR Apache-2.0 | Subconjunto de fontes para incorporar no PDF (GD-EXP-02); traz write-fonts, read-fonts, skrifa e font-types (MIT OR Apache-2.0) |
| i_overlay | 9.1.0 | MIT OR Apache-2.0 | Booleanas e deslocamento de polígonos (doc 09); traz i_float, i_shape, i_tree e i_key_sort, mesma licença |
| tiny-skia | 0.12.0 | BSD-3-Clause | Rasterização (tela, PNG, miniaturas) |
| png | 0.18.1 | MIT OR Apache-2.0 | Codificação PNG |
| image | 0.25.10 | MIT OR Apache-2.0 | Decodificação de imagens (PNG, JPEG, BMP, TIFF, WebP, GIF) |
| pdf-writer | 0.15.0 | MIT OR Apache-2.0 | Exportação PDF |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 | Compressão Flate no PDF |
| base64 | 0.23.1 | MIT OR Apache-2.0 | Imagens embutidas (SVG, ponte libcdr) |
| fontdb | 0.24.0 | MIT | Localização de fontes do sistema |
| rustybuzz | 0.20.1 | MIT | Shaping de texto (porte do HarfBuzz) |
| ttf-parser | 0.25.1 | MIT OR Apache-2.0 | Contornos de glifos (via rustybuzz) |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | Tipos de erro |
| clap | 4.6.7 | MIT OR Apache-2.0 | Linha de comando `graphdraw` |
| eframe / egui | 0.37.0 | MIT OR Apache-2.0 | Interface do visualizador |
| rfd | 0.17.2 | MIT | Diálogos nativos de abrir/salvar |

## Fontes embutidas no aplicativo

| Nome | Versão | Licença | Uso |
|---|---|---|---|
| Inter (Regular, Medium, SemiBold) | 4.1 | SIL Open Font License 1.1 (texto em `crates/graphdraw-app/assets/fonts/Inter-LICENSE.txt`) | Fonte da interface (DESIGN-SYSTEM 2.3) |
| Hack | a que vem no egui 0.37.0 | MIT (fonte embutida pelo egui) | Fonte monoespaçada dos campos numéricos (`type-numeric`) |

## Distribuídas com o GraphDraw — ponte `gd-cdr-bridge` (DLLs ao lado do executável no Windows)

| Nome | Versão | Licença | Uso |
|---|---|---|---|
| libcdr | 0.1.9 | MPL-2.0 | Leitura de `.cdr`/`.cmx` (doc 14) |
| librevenge (+ librevenge-stream) | 0.0.6 | MPL-2.0 OR LGPL-2.1+ (usada sob MPL-2.0) | Interface de desenho emitida pela libcdr |
| ICU (libicuuc, libicuin, libicudt) | 78 | Unicode-3.0 | Dependência da libcdr (texto) |
| Little CMS 2 (liblcms2) | 2.x | MIT | Dependência da libcdr (conversão de cor interna dela) |
| zlib | 1.3.x | Zlib | Dependência da libcdr |
| libstdc++, libgcc_s | GCC 16 | GPL-3.0 com GCC Runtime Library Exception | Runtime C++ da ponte (a exceção permite distribuição com software não-GPL) |
| winpthreads | — | MIT e BSD-3-Clause | Runtime MinGW |
| Boost (somente cabeçalhos, em tempo de build da libcdr) | — | BSL-1.0 | — |

Binários da libcdr/librevenge e dependências vêm dos pacotes do MSYS2
(`mingw-w64-ucrt-x86_64-libcdr` e dependências).

## Somente para desenvolvimento/testes (não distribuídas)

| Nome | Versão | Licença | Uso |
|---|---|---|---|
| jsonschema | 0.58.6 | MIT | Validação das fixtures contra o schema (GD-GDR-08) |
| roxmltree | 0.21.1 | MIT OR Apache-2.0 | Verificar SVG bem formado nos testes |
| tempfile | 3.27.0 | MIT OR Apache-2.0 | Diretórios temporários nos testes |
| lopdf | 0.45.0 | MIT | Ler o PDF/X-1a exportado e conferir os requisitos (GD-EXP-07) nos testes |
| MSYS2 (gcc, pkgconf, binutils, make) | — | várias (GPL, ferramentas de build) | Compilar a ponte; `dlltool` para o alvo GNU do Rust |
| poppler (pdftoppm/pdfinfo) | — | GPL-2.0 | Conferência manual dos PDFs (ferramenta externa, não vinculada) |

## Arquivos de teste de terceiros

| Arquivo | Origem | Licença |
|---|---|---|
| `tests/fixtures/cdr/*.cdr`, `*.cmx` | Corpus de testes do LibreOffice (`writerperfect/qa/unit/data/draw/`) | MPL-2.0 |

Ver [`tests/fixtures/cdr/README.md`](tests/fixtures/cdr/README.md).
