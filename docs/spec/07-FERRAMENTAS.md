# 07 — Ferramentas (caixa de ferramentas)

Cada ferramenta: cursor próprio, barra de propriedades contextual, atalho de teclado configurável, comportamento com modificadores (Shift = restringir/proporcional, Ctrl = a partir do centro / restrição angular em passos de 15° configuráveis, Alt = alternativo). Nomes e ícones do GraphDraw são próprios.

## Seleção e navegação
| ID | Ferramenta | Comportamento |
|---|---|---|
| GD-FER-01 | Seleção | clique, retângulo, laço (modificador), Shift soma/subtrai; arrastar move; alças de escala; segundo clique alterna para alças de rotação/inclinação com centro móvel; seta do teclado = deslocamento (nudge) configurável, Shift = super-deslocamento |
| GD-FER-02 | Seleção livre | laço à mão livre |
| GD-FER-03 | Transformação livre | girar, refletir, escalar, inclinar livremente em torno de ponto |
| GD-FER-04 | Zoom | clique aproxima, botão direito afasta, retângulo; roda do mouse; zoom para seleção/página/tudo |
| GD-FER-05 | Mão (pan) | arrastar; barra de espaço ativa temporariamente |

## Edição de forma
| ID | Ferramenta | Comportamento |
|---|---|---|
| GD-FER-10 | Forma (nós) | ver doc 03 GD-NOD-* |
| GD-FER-11 | Suavizar | pincel que reduz nós/ruído localmente |
| GD-FER-12 | Borrar (smudge) | arrastar empurra a borda da forma para fora/dentro |
| GD-FER-13 | Espiralar (twirl) | gira a borda sob o pincel |
| GD-FER-14 | Atrair/repelir | puxa/empurra nós para/de o centro do pincel |
| GD-FER-15 | Áspero (roughen) | adiciona serrilhado à borda |
| GD-FER-16 | Recortar (crop) | retângulo remove tudo fora dele (vetores e bitmaps) |
| GD-FER-17 | Faca | corta objeto ao longo de linha reta/livre/Bézier, gerando objetos separados; opção de lacuna |
| GD-FER-18 | Borracha | apaga área de objetos sob pincel (forma/espessura) |
| GD-FER-19 | Excluir segmento virtual | apaga segmento entre interseções |

## Desenho
| ID | Ferramenta | Comportamento |
|---|---|---|
| GD-FER-20 | Mão livre | arrasta traço; suavização ajustável |
| GD-FER-21 | Linha 2 pontos | reta; modo tangente/perpendicular a objeto |
| GD-FER-22 | Bézier | clique = nó de canto, clique-arrasta = nó suave |
| GD-FER-23 | Caneta | idem com prévia do próximo segmento |
| GD-FER-24 | B-spline | doc 03 GD-BSP-01 |
| GD-FER-25 | Polilinha | cliques = retas, arrasta = trecho livre |
| GD-FER-26 | Curva 3 pontos | início, fim, ponto de curvatura |
| GD-FER-27 | Desenho inteligente | traço livre reconhecido como retângulo/elipse/triângulo/seta; nível de reconhecimento ajustável |
| GD-FER-28 | Mídia artística | pincel vetorial predefinido, pulverizador (spray de objetos ao longo do caminho), caligráfico, pressão (tablet) |
| GD-FER-29 | Esboço contínuo | traços sucessivos fundidos em curva única por tempo/distância |
| GD-FER-30 | Retângulo / Retângulo 3 pontos | doc 03 GD-PRI-01; 2 e 3 pontos são escolhas do mesmo grupo na caixa de ferramentas; duplo clique no botão do Retângulo cria um retângulo do tamanho exato da página (atributos padrão, selecionado, um passo de desfazer) |
| GD-FER-31 | Elipse / Elipse 3 pontos | doc 03 GD-PRI-02; 2 e 3 pontos são escolhas do mesmo grupo na caixa de ferramentas; o modo de cada ferramenta é independente |
| GD-FER-32 | Polígono, Estrela, Espiral, Papel gráfico, Formas básicas | doc 03 |
| GD-FER-33 | Simetria | desenhar com espelhamento ao vivo (N eixos, radial) |

## Cota e conexão
| ID | Ferramenta | Comportamento |
|---|---|---|
| GD-FER-40 | Cota paralela/horizontal/vertical/angular/segmento/3 pontos | ver doc 08 |
| GD-FER-41 | Conector (reto, em ângulo reto, arredondado) | ancora em pontos de encaixe dos objetos; segue ao mover |
| GD-FER-42 | Chamada (callout) | balão com seta para ponto |

## Interativas de efeito (doc 10)
Mistura, Contorno, Distorção, Sombra, Envelope, Extrusão, Bisel, Transparência, Sombra interna.

## Cor e atributos
| ID | Ferramenta | Comportamento |
|---|---|---|
| GD-FER-50 | Preenchimento interativo | doc 04 GD-FIL-11 |
| GD-FER-51 | Preenchimento de malha | doc 04 GD-FIL-09 |
| GD-FER-52 | Preenchimento inteligente | clica numa área delimitada por objetos sobrepostos → cria novo objeto daquela região |
| GD-FER-53 | Conta-gotas de cor / de atributos | doc 04 GD-ATR-01 |
| GD-FER-54 | Contorno | atalho para espessura/cor |

## Outras
| ID | Ferramenta |
|---|---|
| GD-FER-60 | Texto (doc 06) |
| GD-FER-61 | Tabela — fora de escopo inicial |
| GD-FER-62 | Limite (boundary): cria contorno ao redor da seleção |
| GD-FER-63 | Converter em (curva, bitmap, contorno) |
| GD-FER-64 | Comentário/anotação (doc 19) |

## Personalização
- **GD-FER-90** Caixa de ferramentas reorganizável (grupos expansíveis), atalhos configuráveis, perfis de área de trabalho salváveis.
