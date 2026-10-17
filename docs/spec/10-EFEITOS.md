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

Única dependência do MVP: o offset de caminho (doc 09, GD-OFS-01), necessário para linha de corte com sangria (doc 17).
