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
| lopdf | 0.45.0 | MIT | Ler cores CMYK dos PDFs na medição de fidelidade (GD-FID-12) e conferir o PDF/X-1a nos testes |
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

## Perfis ICC embutidos (GD-CMS-01)

Arquivos sem alteração em `crates/graphdraw-core/assets/icc/`; textos completos das licenças e SHA-256 em `crates/graphdraw-core/assets/icc/LICENSES.md`.

| Nome | Origem | Licença | Uso |
|---|---|---|---|
| FOGRA39L_coated.icc | pacote `colorprofiles` do CTAN (20181105), OpenICC | zlib/libpng ("free of known copyright restrictions") | Perfil CMYK padrão: offset couché ISO 12647-2 (FOGRA39) |
| GRACoL2013_CRPC6.icc | registro de perfis do ICC (IDEAlliance, com permissão da X-Rite) | Uso, incorporação e compartilhamento sem restrição; proibido alterar ou vender sem permissão escrita da IDEAlliance | Perfil CMYK GRACoL 2013 (CGATS 21-2 CRPC6) |

Não incluídos por licença: perfis da ECI (ISO Coated v2, PSO Coated v3 — distribuição só com permissão escrita da ECI) e os perfis da Adobe. O usuário pode escolher esses arquivos na janela de gerenciamento de cor.

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

## Instalador Windows

| Nome | Versão | Licença | Uso |
|---|---|---|---|
| NSIS | 3.x (MSYS2 `mingw-w64-x86_64-nsis`) | zlib/libpng (stubs e plugins distribuídos no instalador) | Gera `GraphDraw-<versão>-setup.exe` (`installer/windows/graphdraw.nsi`) |

## Somente para desenvolvimento/testes (não distribuídas)

| Nome | Versão | Licença | Uso |
|---|---|---|---|
| jsonschema | 0.58.6 | MIT | Validação das fixtures contra o schema (GD-GDR-08) |
| roxmltree | 0.21.1 | MIT OR Apache-2.0 | Verificar SVG bem formado nos testes |
| tempfile | 3.27.0 | MIT OR Apache-2.0 | Diretórios temporários nos testes |
| resvg (com usvg e tiny-skia) | 0.45.1 | Apache-2.0 OR MIT | `scripts/brand-icons`: rasterizar o ícone vetorial do programa nos PNG/ICO (só os arquivos gerados vão com o programa) |
| MSYS2 (gcc, pkgconf, binutils, make) | — | várias (GPL, ferramentas de build) | Compilar a ponte; `dlltool` para o alvo GNU do Rust |
| poppler (pdftoppm/pdftotext/pdffonts/pdfinfo) | — | GPL-2.0 | Conferência dos PDFs e rasterização do `graphdraw fidelity` (programa externo chamado por processo, não vinculado nem distribuído) |

## Arquivos de teste de terceiros

| Arquivo | Origem | Licença |
|---|---|---|
| `tests/fixtures/cdr/*.cdr`, `*.cmx` | Corpus de testes do LibreOffice (`writerperfect/qa/unit/data/draw/`) | MPL-2.0 |

Ver [`tests/fixtures/cdr/README.md`](tests/fixtures/cdr/README.md).
