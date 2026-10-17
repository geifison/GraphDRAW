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
- **GD-EXP-07** Dois tipos de PDF na exportação (pedido do responsável pelo produto, 2026-10-16):
  - **PDF comum (tela):** cores no modelo original (RGB, CMYK, cinza, especiais), transparência nativa, sem caixas de produção.
  - **PDF/X-1a:2001 (gráfica, ISO 15930-1):**
    1. Só CMYK, cinza e cores especiais (Separation com alternativa CMYK). RGB (cores, gradientes e imagens) é convertido para CMYK na exportação.
    2. Nenhuma transparência: cada objeto transparente (opacidade, gradiente com opacidade, imagem com alfa) vira uma imagem CMYK de 300 ppp da composição dele com o que está embaixo, recortada pelo contorno vetorial dele — a aparência fica igual à da tela e o resto continua vetorial; nenhum SMask nem alfa < 1 no arquivo.
    3. Fontes: todo texto sai em curvas (nenhuma fonte a incorporar).
    4. Nenhum elemento interativo (links, anotações, formulários, JavaScript, multimídia).
    5. Caixas de produção em toda página: TrimBox = página; BleedBox = página + sangria; MediaBox = BleedBox.
    6. PDF 1.3, identificador do arquivo (/ID), Info com Title, CreationDate, ModDate, `Trapped /False`, `GTS_PDFXVersion (PDF/X-1:2001)` e `GTS_PDFXConformance (PDF/X-1a:2001)`.
    7. OutputIntent `GTS_PDFX` com condição de impressão registrada no ICC (`http://www.color.org`): FOGRA39 (couché, padrão), FOGRA47 (não couché) ou CGATS TR 001 (SWOP).
    8. Imagens comprimidas sem perda (Flate); a exportação avisa quando a resolução efetiva fica abaixo de 300 ppp (não aumenta a resolução).
    9. Relatório ao final listando o que foi convertido ou achatado.
  - *Limitações enquanto não houver gerenciamento de cor (GD-CMS, M2):* a conversão RGB → CMYK é por fórmula, sem perfil ICC — o relatório avisa.
  - Aceite: um documento com um retângulo RGB semitransparente, um gradiente RGB e uma imagem PNG com alfa exporta um PDF/X-1a em que: versão é 1.3; não há `DeviceRGB`, `SMask` nem alfa < 1; toda página tem TrimBox/BleedBox coerentes com a sangria; há OutputIntent GTS_PDFX; Info traz as chaves GTS e Trapped; o relatório cita as conversões.
- **GD-EXP-08** PNG exporta só a arte, nunca a página: com objetos selecionados (exportando a página atual), só a seleção; sem seleção, os objetos visíveis de camadas visíveis de cada página. A imagem tem o tamanho da caixa visual da arte, fundo transparente, sem página mestra; página sem arte fica de fora (sem nada para exportar, a barra de status avisa e o diálogo de salvar nem abre).
  - Aceite: dois retângulos visíveis (e um oculto) numa A4 exportam um PNG do tamanho da caixa dos dois, com transparência entre eles.
- **GD-EXP-04** Importar coloca objetos na página atual com opção de posição (centro / origem / clique).
- **GD-EXP-05** Toda importação gera relatório de itens não suportados (como GD-CDR-03).
- **GD-EXP-06** Linha de comando: `graphdraw convert <entrada> <saida> [--page N] [--dpi N] [--text-to-curves]`.
