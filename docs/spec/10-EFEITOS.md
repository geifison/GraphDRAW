# 10 — Efeitos

**Status: fora do MVP — a especificar na fase Beta.**

Princípio já decidido: efeito dinâmico = objeto(s) de origem + parâmetros + resultado gerado; editar origem ou parâmetro regenera o resultado; "converter em curvas" torna o resultado estático.

## Separar efeitos (GD-CMB-04)
Requisito registrado para quando cada efeito existir; Ctrl+K congela o resultado:
- **Mídia artística (pincel, espalhador, caligrafia):** o caminho de controle (esqueleto) se separa da forma gerada; a forma vira curva com nós editáveis e o esqueleto pode ser apagado.
- **Contorno (concêntrico, interno ou externo):** o objeto original se separa do grupo de linhas de contorno geradas, que podem ser pintadas e editadas uma a uma.
- **Sombra / brilho:** a sombra vira um bitmap independente do objeto original.
- **Sombra de bloco:** a sombra vira vetor independente.
- **Extrusão 3D:** o objeto da frente se separa do corpo tridimensional gerado atrás (grupo de faces vetoriais).
- **Mistura (blend):** objetos inicial e final se separam do grupo dos intermediários; Ctrl+K de novo nesse grupo o desfaz em objetos comuns.
- **Envelope:** a caixa de controle é desvinculada e os nós ficam fixos na forma deformada.

## Ferramentas e barra (2026-10-18)
- **GD-EFX-01** Caixa de ferramentas, grupo **Efeitos** (antes da Prancheta):
  Contorno, Mistura, Sombra projetada, Sombra de bloco e Envelope.
  - Contorno, sombra e bloco: arrastar a partir de um objeto aplica o
    efeito e o arrasto define o principal — no contorno, afastar do centro
    = para fora, aproximar = para dentro, e o comprimento dividido pelos
    passos vira a distância; na sombra, o fim do arrasto é a posição; no
    bloco, direção e comprimento dão ângulo e profundidade. Arrastar sobre
    um grupo com o mesmo efeito ajusta (soma ao que havia). Um passo do
    histórico por arrasto.
  - Mistura: arrastar de um objeto até outro mistura os dois.
  - Envelope: clicar no objeto põe o envelope; os 8 pontos ficam
    arrastáveis na própria ferramenta (e na Forma).
  - Clique seleciona (Shift soma); fora dos objetos, seleção por área.
  - Com o efeito do tipo da ferramenta selecionado, a tela mostra o vetor
    (sombra, bloco, contorno) com alças.
- **GD-EFX-02** Barra de propriedades (na ferramenta do efeito e na
  Seleção), com o grupo do efeito selecionado: tipo (trocar por outro
  efeito sobre os mesmos objetos), **Separar** (congela, GD-CMB-04),
  **Remover efeito** (os objetos de origem voltam soltos, como antes de
  aplicar) e os parâmetros:
  - Contorno: Para fora | Para dentro | Até o centro, passos, distância,
    cantos Redondos | Vivos | Chanfrados, cor final e aceleração da cor;
  - Mistura: passos, giro total, aceleração dos objetos e da cor, Inverter;
  - Sombra projetada: X, Y, desfoque, cor, opacidade e tamanho (%);
  - Sombra de bloco: direção, profundidade, cor (frente), degradê até a
    cor do fundo e perspectiva (tamanho do fundo, %);
  - Envelope: prontos Arco ↑, Arco ↓, Inflar, Apertar, Onda, Bandeira,
    Perspectiva com intensidade (%), e Restaurar.
  Sem efeito selecionado, a barra da ferramenta tem **Aplicar** (valores
  iniciais) e a dica do arrasto.
- Aceleração: de −100 a 100; 0 = linear; t' = t^(2^(a/50)).

Única dependência do MVP: o offset de caminho (doc 09, GD-OFS-01), necessário para linha de corte com sangria (doc 17).
