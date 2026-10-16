# 15 — Importação e Exportação

| Formato | Importar | Exportar | Base | Marco |
|---|---|---|---|---|
| `.gdr` | ✔ | ✔ | próprio (doc 13) | M0 |
| `.cdr`/`.cmx` | ✔ | — | libcdr (doc 14) | M0 |
| SVG 1.1/2 | ✔ | ✔ | parser próprio ou usvg (MIT/Apache) | M0 |
| PDF 1.7 / PDF/X-4 | ✔ (páginas como objetos) | ✔ | PDFium (leitura), Skia PDF ou escritor próprio | M0 exp. / M2 imp. |
| PNG / JPEG / WebP / TIFF | ✔ (bitmap) | ✔ (rasterização: DPI, área, anti-serrilhado, transparência, perfil ICC) | libpng, libjpeg-turbo, libwebp, libtiff | M0 |
| EPS / PS | ✔ via Ghostscript* | ✔ (escritor próprio, nível 3) | — | M2 |
| AI (PDF-compatível) | ✔ (camada PDF) | — | via PDF | M2 |
| DXF (R12/2000) | ✔ | ✔ | parser/escritor próprio ou libdxfrw (GPL — avaliar) | M2 |
| PLT / HPGL | — | ✔ | escritor próprio (doc 17) | M2 |
| WMF/EMF | ✔ | — | libwmf/libemf2svg (avaliar licenças) | pós-MVP |

\* Ghostscript é AGPL: usar como executável externo opcional, nunca linkado, ou dispensar.

## Requisitos gerais
- **GD-EXP-01** Exportar: documento inteiro, página atual, intervalo de páginas, seleção.
- **GD-EXP-02** Texto: manter como texto (fontes incorporadas em PDF/SVG) ou converter em curvas.
- **GD-EXP-03** PDF: perfis predefinidos (Web/RGB, Impressão/CMYK, PDF/X-4), sangria, marcas de corte/registro opcionais, compressão de imagens, reamostragem.
- **GD-EXP-04** Importar coloca objetos na página atual com opção de posição (centro / origem / clique).
- **GD-EXP-05** Toda importação gera relatório de itens não suportados (como GD-CDR-03).
- **GD-EXP-06** Linha de comando: `graphdraw convert <entrada> <saida> [--page N] [--dpi N] [--text-to-curves]`.
