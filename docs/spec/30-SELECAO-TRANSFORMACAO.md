# 30 — Ferramenta Seleção: estados, modificadores e retorno visual

Fonte: especificação do responsável pelo produto (2026-10-18). Detalha
GD-FER-01 (doc 07), GD-SEL-01..03 (doc 02) e GD-ALI-01/03/05 (doc 08).
Onde este documento e os anteriores divergem, vale este para a ferramenta
Seleção. A aparência (cores, medidas) segue `docs/design/DESIGN-SYSTEM.md`
(5.1 e 5.2); os valores citados aqui como "ex." são indicativos.

## 1. Visão geral
A Seleção é a ferramenta principal de interação. Opera em estados
conforme o evento (clique único, segundo clique, clicar e arrastar) e as
teclas modificadoras seguradas.

## 2. Estados de interação e retorno visual

- **GD-SLT-01 Pré-seleção (sobrevoo).** Com a Seleção ativa e nenhum
  arrasto em andamento, o objeto não selecionado sob o cursor ganha um
  contorno sutil (ex.: linha de 1 px no acento a 30%) que indica que é
  clicável, e o cursor troca da seta padrão para o cursor de seleção
  ativa. Sobre um objeto já selecionado, o cursor é o de mover (4 setas).
  *Aceite:* cursor sobre objeto livre → contorno de sobrevoo daquele
  objeto e cursor de pré-seleção; fora de objetos → nada e seta padrão.
- **GD-SLT-02 Estado 1 — seleção e dimensionamento (primeiro clique).**
  Caixa delimitadora tangente às extremidades do objeto, com 8 alças
  quadradas simétricas (4 cantos e meio das 4 arestas) e um marcador "X"
  no centro geométrico. Arrastar o interior do objeto ou o "X" move; o
  cursor é a cruz de 4 setas.
  *Aceite:* arrastar a partir do "X" de um anel sem preenchimento (ponto
  vazio) move a seleção.
- **GD-SLT-03 Estado 2 — rotação e inclinação (segundo clique).** Um
  segundo clique no objeto já selecionado alterna as alças:
  - cantos: setas bidirecionais curvas — arrastar gira livremente;
  - meios das arestas: setas bidirecionais retas, paralelas à aresta —
    arrastar inclina no eixo X (arestas de cima/baixo) ou Y (laterais);
  - centro: alvo circular (círculo com ponto), o pivô. Arrastar o pivô
    leva o ponto de ancoragem da rotação e da inclinação para onde se
    quiser, inclusive fora da caixa.
  *Aceite:* segundo clique → estado 2; terceiro → estado 1; girar depois
  de mover o pivô gira em torno do novo pivô.

## 3. Seleção em massa (retângulo elástico)
- **GD-SLT-04** Clicar em área vazia e arrastar desenha o retângulo de
  seleção com linha tracejada animada ("formigas marchando") e fundo
  translúcido (ex.: acento a 10%).
  - **Envolvimento (padrão):** entram só os objetos cuja caixa fica 100%
    dentro do retângulo.
  - **Toque (Alt + arrastar):** a linha muda de cor (acento → verde) e
    entra todo objeto que cruza o retângulo ou fica dentro dele, sem
    precisar englobá-lo. Vale a tecla no momento de soltar.
  - **Laço (Ctrl + Alt + arrastar)** (GD-SEL-03, decisão de 2026-10-18):
    laço à mão livre, também por toque e com a mesma linha animada verde;
    soltando o Alt antes do fim, volta a exigir o objeto inteiro dentro.
  *Aceite:* retângulo que pega metade de um objeto não o seleciona; o
  mesmo com Alt seleciona.

## 4. Matriz de modificadores (GD-SLT-05..07)

| Tecla | Dimensionamento (estado 1) | Rotação/inclinação (estado 2) | Seleção |
|---|---|---|---|
| **Shift** | Redimensiona a partir do centro (pivô) em vez do canto oposto | Ignorado | Soma/retira o objeto da seleção |
| **Ctrl** | Espelha o objeto se a alça cruzar o eixo oposto (sem Ctrl, o tamanho para no mínimo e não espelha) | Prende o ângulo em passos de 15°; arrastando o pivô, liga o ímã nos nós e cantos (GD-SLT-14) | Seleciona o objeto de dentro de um grupo (GD-SEL-02) |
| **Alt** | Ignora as proporções (transformação livre); sem Alt, o canto é proporcional | Cria uma cópia ao soltar (o original fica onde estava) | Seleciona o objeto de baixo (z menor) sob o cursor; cliques seguidos descem a pilha e voltam ao topo |
| **Shift + Ctrl** | Centralizado e proporcional garantido (vale também para as alças laterais e com Alt) | — | — |

- **GD-SLT-05** Dimensionamento conforme a coluna 1. *Aceite:* canto sem
  modificador mantém a proporção; com Alt, não; com Shift, o centro fica
  parado; arrastar além do lado oposto sem Ctrl não espelha, com Ctrl
  espelha.
- **GD-SLT-06** Rotação conforme a coluna 2. *Aceite:* Ctrl → ângulo
  múltiplo de 15°; Alt ao soltar → dois objetos, original intacto.
- **GD-SLT-07** Seleção conforme a coluna 3. *Aceite:* Alt+clique sobre
  dois objetos sobrepostos seleciona o de baixo; outro Alt+clique volta
  ao de cima.

- **GD-SLT-14** (2026-10-18) No estado 2, arrastar o pivô com Ctrl liga um
  ímã nos pontos do próprio objeto selecionado: nós do contorno (também
  os cantos de retângulos, vértices de polígonos e quadrantes de elipses,
  inclusive dentro de grupos) e os cantos, meios e centro da caixa. Dentro
  do raio de encaixe o pivô gruda no ponto mais próximo e aparece a marca
  de encaixe com o nome do ponto. *Aceite:* soltar o pivô a 2 px de um
  canto com Ctrl → pivô exatamente no canto; sem Ctrl → onde soltou.

## 5. Alinhamento e distribuição
- **GD-SLT-08 Âncora (objeto-chave).** A seleção guarda a ordem
  cronológica (GD-SEL-01). O último selecionado é o objeto-chave: com dois
  ou mais objetos selecionados, a caixa dele aparece com linha contínua
  mais grossa, indicando que **não muda de lugar** no alinhamento
  "relativo ao último selecionado"; os outros vão até ele.
- **GD-SLT-09 Alinhamento por eixo.** Y: topo (máx. Y na página, Y para
  cima), centro, base; X: esquerda (mín. X), centro, direita (GD-ALI-01).
- **GD-SLT-10 Distribuição por vãos iguais.** Os objetos de menor e de
  maior X ficam parados; a soma das larguras dos objetos é descontada da
  distância entre as extremidades; o vão livre é dividido pelo número de
  intervalos (objetos − 1) e vira a margem entre todos (GD-ALI-03). Idem
  em Y.

## 6. Recursos visuais
- **GD-SLT-11 Guias inteligentes** (GD-ALI-05): linhas temporárias
  magenta (outro objeto) ou ciano (página) quando bordas ou centros se
  alinham durante o arrasto.
- **GD-SLT-12 Aderência magnética:** perto do alinhamento perfeito (raio
  de encaixe, 6 px de tela) o objeto gruda na posição e o cursor muda de
  cor enquanto estiver grudado.
- **GD-SLT-13 Fantasma:** ao mover, redimensionar, girar ou inclinar, um
  fantasma do resultado com 50% de opacidade acompanha o cursor; o
  original fica sólido no lugar até soltar o botão.
