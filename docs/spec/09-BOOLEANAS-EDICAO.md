# 09 — Operações Booleanas e Edição de Forma

## Booleanas
| ID | Operação | Resultado |
|---|---|---|
| GD-BOO-01 | Soldar (união) | um caminho com a área unida |
| GD-BOO-02 | Aparar (subtração) | objetos-alvo perdem a área dos objetos-fonte |
| GD-BOO-03 | Interseção | novo objeto com a área comum |
| GD-BOO-04 | Simplificar | remove partes ocultas sobrepostas (cada objeto mantém só a área visível) |
| GD-BOO-05 | Frente menos trás / Trás menos frente | subtrai pela ordem z |
| GD-BOO-06 | Excluir (XOR) | área de não sobreposição |
| GD-BOO-07 | Dividir | todas as regiões resultantes de sobreposição viram objetos separados |
| GD-BOO-08 | Limite (boundary) | contorno externo da seleção |

- **GD-BOO-10** Opções: manter originais (fonte e/ou alvo); resultado herda atributos do último/primeiro selecionado ou do alvo — configurável.
- **GD-BOO-11** Robustez: operar sobre Bézier exatas (não poligonizar) quando possível; tolerância numérica documentada; nós coincidentes e tangências tratados; resultado sem nós redundantes.
- Aceite: soldar dois círculos de r=10 mm com centros a 10 mm → 1 caminho, área = 2πr² − área da lente ± 0,1%.

## Combinar / separar
- **GD-CMB-01** Combinar: vários caminhos em um objeto com subcaminhos (furos conforme regra de preenchimento).
- **GD-CMB-02** Separar (quebrar à parte, Ctrl+K): cada subcaminho que não está ligado a outro por um nó vira um objeto de curva próprio, com os atributos do original. Os furos deixam de existir: as curvas internas viram objetos preenchidos independentes, empilhados por cima. Opção à parte de manter furos dentro do objeto pai ("separar em objetos com furos").
- **GD-CMB-03** Separar texto (Ctrl+K), um nível por vez, na ordem:
  1. Texto em caminho: o texto se desvincula do caminho guia; o caminho fica com a forma original e o texto mantém a curvatura; viram dois objetos independentes (se o caminho de origem não existe mais, ele é recriado com contorno fino).
  2. Texto de parágrafo com vários parágrafos: um bloco de texto de parágrafo independente por parágrafo (mesma largura de quadro, mesmo alinhamento).
  3. Texto com várias linhas (artístico ou parágrafo, inclusive linhas da quebra automática): uma linha de texto artístico por objeto.
  4. Uma linha com várias palavras: uma palavra por objeto.
  5. Uma palavra: um caractere (grafema) por objeto.
  Cada pedaço fica exatamente onde estava na página e conserva fonte, corpo, cor e demais atributos; espaços não viram objetos. Num texto em caminho já desvinculado, os pedaços seguem na mesma curva.
- **GD-CMB-04** Separar efeitos dinâmicos (Ctrl+K): congela o resultado visual e separa o objeto de controle do objeto gerado, que passa a ser vetor (ou bitmap, no caso de sombra) comum e editável. Comportamento por efeito em doc 10 (§ Separar efeitos).
- **GD-CMB-05** Separar cópia vinculada (Ctrl+K): quebra o vínculo; a cópia vira objeto comum e deixa de acompanhar as alterações do original.
- **GD-CMB-06** Ctrl+K vale para cada objeto da seleção, em uma só transação de desfazer; a seleção passa a ser os pedaços (e os objetos que não tinham o que separar). Sem nada a separar, nenhuma entrada vai para o histórico e a barra de status avisa. Princípio geral: tudo o que foi gerado a partir de outra coisa perde a dependência e vira pedaços independentes.

## Contorno de deslocamento (offset)
- **GD-OFS-01** Criar caminho paralelo interno/externo com distância d, cantos: agudo/arredondado/chanfrado; usado por efeito Contorno (doc 10) e por "corte com sangria" (doc 17).

## Converter
- **GD-CNV-01** Converter em curvas: primitivas, texto, efeitos (gera objetos estáticos).
- **GD-CNV-02** Converter contorno em objeto (doc 04 GD-OUT-09).
- **GD-CNV-03** Converter em bitmap: resolução, modo de cor, anti-serrilhado, fundo transparente.

## Vetorização de bitmap (trace)
- **GD-VET-01** Rastreio de contorno (linha central) e de silhueta/cores (nº de cores, suavização, remover fundo, detalhe); prévia lado a lado; resultado agrupado. Implementação própria ou biblioteca aberta (ex.: potrace é GPL — avaliar licença; alternativa: algoritmo próprio).
