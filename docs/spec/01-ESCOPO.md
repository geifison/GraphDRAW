# GraphDraw — Escopo do Produto

## Visão
Editor de ilustração vetorial para desktop, focado em **gráficas rápidas, comunicação visual e corte (plotter/laser)**. Abre arquivos `.cdr` legados, salva no formato nativo aberto `.gdr`, exporta para produção (PDF/X, SVG, EPS, DXF, PLT).

## Índice da especificação
| Doc | Área | Prioridade |
|---|---|---|
| 00 | Protocolo clean room | — |
| 01 | Escopo (este) | — |
| 02 | Modelo de documento (documento, páginas, camadas, objetos) | MVP |
| 03 | Geometria (caminhos, nós, segmentos, formas primitivas) | MVP |
| 04 | Preenchimento e contorno | MVP |
| 05 | Cor (modelos, paletas, gerenciamento ICC) | MVP |
| 06 | Texto e tipografia | MVP |
| 07 | Ferramentas (caixa de ferramentas, interação) | MVP |
| 08 | Transformações, alinhamento, encaixe (snap), réguas, guias | MVP |
| 09 | Operações booleanas e edição de forma | MVP |
| 10 | Efeitos (mistura, contorno, sombra, envelope, extrusão, lente, perspectiva, distorção, transparência, recipiente de recorte) | Beta |
| 11 | Estilos e símbolos | Beta |
| 12 | Desfazer/refazer e transações | MVP |
| 13 | Formato nativo `.gdr` | MVP |
| 14 | Importação `.cdr` | MVP |
| 15 | Importação/exportação (PDF, SVG, EPS, AI, DXF, PNG/JPG/TIFF) | MVP |
| 16 | Impressão e saída para produção (separações, marcas de corte, imposição, PDF/X) | Beta |
| 17 | Saída para corte (plotter/laser: contorno de corte, HPGL/PLT, DXF) | MVP (diferencial) |
| 18 | Automação/scripts (API de objetos própria) | Pós-beta |
| 19 | Comentários/colaboração | Pós-beta |
| 20 | Interface (janelas, painéis, atalhos) — identidade própria | MVP |

## Fora de escopo inicial
Pintura bitmap avançada, câmera RAW, tabelas, IA generativa, nuvem, OLE.

## Requisitos não funcionais
- **GD-NF-01** Abrir documento de 50 MB / 100 mil objetos em < 5 s em máquina de referência (i5, 16 GB).
- **GD-NF-02** Redesenho interativo ≥ 30 fps com 10 mil objetos visíveis.
- **GD-NF-03** Windows 10+ x64 primeiro; arquitetura portável (macOS/Linux depois).
- **GD-NF-04** Salvamento atômico e recuperação automática a cada N minutos (configurável).
- **GD-NF-05** Precisão de coordenadas: double, unidade interna fixa documentada no doc 02.

## Stack sugerida (somente bibliotecas abertas)
| Função | Biblioteca | Licença |
|---|---|---|
| Renderização 2D | Skia | BSD-3 |
| Texto/shaping | HarfBuzz + FreeType | MIT / FTL |
| Cor | LittleCMS 2 | MIT |
| Importação CDR | libcdr + librevenge | MPL-2.0 |
| PDF export | PDFium (leitura) / pdf-writer ou Skia PDF | BSD / MIT |
| Booleanas | Clipper2 ou algoritmo próprio sobre Bézier | BSL-1.0 |
| UI | Qt 6 (LGPL) ou Rust + egui/Slint | LGPL / MIT / GPL-comercial |
