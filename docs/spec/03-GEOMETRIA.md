# 03 — Geometria

## Caminho (curva)
- **GD-GEO-01** Um caminho tem 1..n **subcaminhos**; cada subcaminho é aberto ou fechado.
- **GD-GEO-02** Subcaminho = sequência de **nós** ligados por **segmentos**. Segmento é reto ou Bézier cúbica (2 pontos de controle).
- **GD-GEO-03** Tipo de nó: **canto** (alças independentes), **suave** (alças colineares, comprimentos livres), **simétrico** (colineares e de mesmo comprimento). Nó de segmento reto adjacente a curva pode ser suave (alça alinhada à reta).
- **GD-GEO-04** Regra de preenchimento por caminho: par-ímpar (padrão) ou não-zero. Subcaminhos internos viram furos conforme a regra.
- **GD-GEO-05** Cálculos exigidos: comprimento (por segmento, subcaminho, total), área com sinal, ponto/tangente/normal em parâmetro t e em distância s, ponto mais próximo, interseções (segmento×segmento, caminho×caminho), teste ponto-dentro.
- Aceite: círculo de raio 10 mm convertido a curvas → comprimento 62,83 mm ± 0,01; área 314,16 mm² ± 0,05.

## Edição de nós (ferramenta Forma)
- **GD-NOD-01** Selecionar nós (clique, retângulo, laço, todos do subcaminho).
- **GD-NOD-02** Mover nós e alças; arrastar segmento diretamente o deforma (mantendo extremos).
- **GD-NOD-03** Adicionar nó (duplo clique no segmento ou comando, divide preservando forma), excluir nó (forma reajustada para mínimo desvio).
- **GD-NOD-04** Unir dois nós extremos; unir com segmento; quebrar caminho em nó; fechar/abrir subcaminho; inverter direção.
- **GD-NOD-05** Converter segmento reta↔curva; alterar tipo de nó (canto/suave/simétrico).
- **GD-NOD-06** Alinhar nós (horizontal/vertical), esticar/girar/escalar conjunto de nós.
- **GD-NOD-07** Reduzir nós com tolerância (simplificação), mantendo desvio ≤ tolerância.
- **GD-NOD-08** Extrair subcaminho como objeto separado; combinar caminhos em um; separar (quebrar à parte).

## Primitivas paramétricas
Mantêm parâmetros editáveis até "Converter em curvas".

| ID | Primitiva | Parâmetros | Comportamento |
|---|---|---|---|
| GD-PRI-01 | Retângulo | largura, altura; por canto: tipo (redondo/chanfro/recorte côncavo) e raio; cantos relativos à escala (opção) | Escala não uniforme não distorce raio se "cantos relativos" desligado |
| GD-PRI-02 | Elipse | raios rx, ry; modo elipse/arco/fatia; ângulos; sentido | Arco = caminho aberto |
| GD-PRI-03 | Polígono | nº lados (3–500), modo polígono/estrela, nitidez da ponta | Editar um vértice replica simetricamente nos demais |
| GD-PRI-04 | Estrela complexa | pontas, nitidez | Gera auto-interseção controlada |
| GD-PRI-05 | Espiral | voltas, simétrica/logarítmica, fator de expansão | Gera caminho aberto |
| GD-PRI-06 | Papel gráfico | colunas × linhas | Gera grupo de retângulos |
| GD-PRI-07 | Formas básicas (setas, fluxograma, banners, balões) | glifo paramétrico com alça(s) de ajuste | Biblioteca própria, desenhos originais |

## Ferramentas de desenho de caminho (detalhe no doc 07)
- Mão livre, Bézier (clique-arrasta), Caneta (com prévia), B-spline (pontos de controle fora da curva), Polilinha, 3 pontos (curva por 3 cliques), Mídia artística (pincel vetorial), Desenho inteligente (reconhece formas).

## B-spline
- **GD-BSP-01** Caminho definido por pontos de controle; cada ponto pode ser "fixado" (curva passa por ele) ou "flutuante". Avaliação: B-spline cúbica uniforme; conversível para Bézier.

## Largura variável de contorno
- **GD-VAR-01** Nós de largura ao longo do caminho (posição s, largura esquerda/direita), interpolação suave. Conversível em caminho fechado ("contorno em objeto").
