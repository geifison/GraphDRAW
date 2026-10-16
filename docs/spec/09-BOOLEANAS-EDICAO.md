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
- **GD-CMB-02** Separar (quebrar à parte): cada subcaminho vira objeto; opção de manter furos dentro do objeto pai ("separar em objetos com furos").

## Contorno de deslocamento (offset)
- **GD-OFS-01** Criar caminho paralelo interno/externo com distância d, cantos: agudo/arredondado/chanfrado; usado por efeito Contorno (doc 10) e por "corte com sangria" (doc 17).

## Converter
- **GD-CNV-01** Converter em curvas: primitivas, texto, efeitos (gera objetos estáticos).
- **GD-CNV-02** Converter contorno em objeto (doc 04 GD-OUT-09).
- **GD-CNV-03** Converter em bitmap: resolução, modo de cor, anti-serrilhado, fundo transparente.

## Vetorização de bitmap (trace)
- **GD-VET-01** Rastreio de contorno (linha central) e de silhueta/cores (nº de cores, suavização, remover fundo, detalhe); prévia lado a lado; resultado agrupado. Implementação própria ou biblioteca aberta (ex.: potrace é GPL — avaliar licença; alternativa: algoritmo próprio).
