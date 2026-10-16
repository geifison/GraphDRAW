# 04 — Preenchimento e Contorno

## Preenchimento
| ID | Tipo | Parâmetros |
|---|---|---|
| GD-FIL-01 | Nenhum | — |
| GD-FIL-02 | Uniforme | cor (doc 05), sobreimpressão (overprint) |
| GD-FIL-03 | Gradiente | tipo: linear, radial, elíptico, cônico, retangular; paradas de cor (posição 0–1, cor, opacidade, ponto médio); repetir/espelhar/estender; ângulo, centro, escala, inclinação; passos (contínuo ou N degraus); suavização |
| GD-FIL-04 | Padrão vetorial | tile vetorial, origem, escala, rotação, deslocamento linha/coluna, espelhar tiles, transformar com objeto (sim/não) |
| GD-FIL-05 | Padrão bitmap | idem com imagem; resolução |
| GD-FIL-06 | Padrão duas cores | bitmap 1-bit + cor frente/fundo |
| GD-FIL-07 | Textura procedural | gerador próprio (ruído, mármore, madeira…) com semente e parâmetros |
| GD-FIL-08 | Hachura | 1..n conjuntos de linhas: ângulo, espaçamento, deslocamento, espessura, cor; biblioteca de hachuras |
| GD-FIL-09 | Malha (mesh) | grade de nós com cor por nó e interpolação bicúbica; adicionar/remover linhas |

- **GD-FIL-10** Preenchimento "transformar com objeto": quando ligado, padrão/gradiente acompanham escala/rotação.
- **GD-FIL-11** Edição interativa na tela: arrastar paradas de gradiente, centro, ângulo; adicionar parada por duplo clique.
- **GD-FIL-12** Preenchimento de caminho aberto: opção global (preencher ou não).

## Contorno
- **GD-OUT-01** Espessura (inclusive "fio de cabelo" = menor espessura imprimível do dispositivo), cor, opacidade.
- **GD-OUT-02** Estilo de linha: sólido, tracejado (padrão de traços/espaços editável, biblioteca), pontilhado; ajuste de traço aos cantos.
- **GD-OUT-03** Junção: aguda (miter, com limite), arredondada, chanfrada. Terminação: reta, redonda, quadrada estendida.
- **GD-OUT-04** Posição: centrada, interna, externa.
- **GD-OUT-05** Pontas de seta no início/fim: biblioteca de setas (desenhos próprios), escala, deslocamento, rotação, espelho.
- **GD-OUT-06** Caneta caligráfica: largura do bico (%) e ângulo.
- **GD-OUT-07** Escalar com objeto (sim/não); contorno atrás do preenchimento (sim/não).
- **GD-OUT-08** Largura variável (doc 03 GD-VAR-01).
- **GD-OUT-09** Converter contorno em objeto (gera caminho fechado com preenchimento = cor do contorno).
- Aceite: linha de 2 mm com terminação redonda convertida em objeto → caminho fechado com semicírculos de r = 1 mm nas pontas.

## Transparência (aplica-se a preenchimento, contorno ou ambos)
- **GD-TRA-01** Uniforme, gradiente (mesmos tipos de GD-FIL-03, com opacidade nas paradas), padrão, textura.
- **GD-TRA-02** Modos de mesclagem: normal, multiplicar, tela, sobrepor, escurecer, clarear, subexposição/superexposição de cor, luz forte/suave, diferença, exclusão, matiz, saturação, cor, luminosidade, adicionar, subtrair, E, OU, XOU (os 16 modos PDF são obrigatórios para exportação fiel).
- **GD-TRA-03** Congelar transparência (rasteriza o que está por baixo) — opcional.

## Conta-gotas / copiar atributos
- **GD-ATR-01** Copiar preenchimento, contorno, transparência, efeitos, texto, transformações de um objeto para outros.
