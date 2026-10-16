# 02 — Modelo de Documento

## Hierarquia
```
Aplicação
└── Documento (1..n abertos)
    ├── Configurações (unidade, resolução, perfil de cor, origem, escala de desenho)
    ├── Página Mestra (camadas compartilhadas por todas as páginas: guias, grade, mestres)
    ├── Páginas (1..n, ordenadas)
    │   └── Camadas (1..n, ordenadas, z-order)
    │       └── Objetos (árvore: grupos contêm objetos)
    ├── Recursos (cores nomeadas, estilos, símbolos, padrões, fontes embutidas, imagens)
    └── Metadados (título, autor, palavras-chave, notas, datas)
```

## Unidades e coordenadas
- **GD-DOC-01** Unidade interna: micrômetro em ponto flutuante de 64 bits. Toda conversão (mm, cm, pol, pt, px, pica) ocorre só na UI e na importação/exportação.
- **GD-DOC-02** Eixo Y cresce para cima na API de documento; a UI pode exibir origem em qualquer canto (configurável). Origem padrão: canto inferior esquerdo da página.
- **GD-DOC-03** Escala de desenho opcional (ex.: 1:100) afeta só réguas/cotas, não a geometria armazenada.
- Aceite: criar retângulo de 10 mm → armazenado 10000 µm; exibido 10 mm, 0,3937 pol, 28,35 pt.

## Páginas
- **GD-PAG-01** Cada página tem tamanho e orientação próprios; predefinições ISO A/B/C, Carta, Ofício, Tabloide, e personalizados.
- **GD-PAG-02** Sangria (bleed) por documento, exibida como área ao redor da página.
- **GD-PAG-03** Inserir, duplicar (com ou sem conteúdo), excluir, reordenar, renomear páginas.
- **GD-PAG-04** Visualização em página única, múltiplas páginas lado a lado (pranchas/spreads) e visão geral.
- **GD-PAG-05** Cor de fundo da página (sólida ou transparente), não impressa por padrão.

## Camadas
- **GD-CAM-01** Propriedades por camada: nome, visível, imprimível, editável (bloqueada), cor de identificação, modo de exibição esboço (wireframe).
- **GD-CAM-02** Camadas da página mestra aparecem em todas as páginas; pode ocultar mestra por página.
- **GD-CAM-03** Mover/copiar objetos entre camadas e páginas.
- **GD-CAM-04** Camadas especiais: Guias, Grade, Área de Trabalho (fora da página, não imprime).
- Aceite: camada não imprimível não aparece em PDF de impressão, mas aparece na tela.

## Objetos (tipos)
| Tipo | Descrição | Doc |
|---|---|---|
| Caminho (curva) | 1..n subcaminhos de segmentos retos/Bézier | 03 |
| Retângulo | paramétrico: largura, altura, cantos (arredondado/chanfro/recorte) por canto | 03 |
| Elipse | paramétrica: elipse, arco, fatia; ângulos inicial/final | 03 |
| Polígono/estrela | nº de lados, profundidade da ponta | 03 |
| Espiral, grade de papel | paramétricos, convertem para grupo/caminho | 03 |
| Texto artístico | texto de linha livre | 06 |
| Texto de parágrafo | texto em quadro(s) com fluxo | 06 |
| Imagem (bitmap) | raster incorporado ou vinculado | 15 |
| Grupo | contém objetos, transformável como um | — |
| Clip (recipiente) | objeto recipiente que recorta o conteúdo | 10 |
| Objeto com efeito | objeto controlador + resultado gerado (mistura, contorno, sombra...) | 10 |
| Cota | linear, angular, raio, com valor calculado | 08 |
| Conector | linha que liga dois objetos e acompanha seu movimento | 08 |
| Instância de símbolo | referência a definição na biblioteca | 11 |

## Propriedades comuns a todos os objetos
- **GD-OBJ-01** ID único estável (persistido no `.gdr`), nome opcional editável.
- **GD-OBJ-02** Matriz de transformação afim 2×3 + geometria em coordenadas locais.
- **GD-OBJ-03** Preenchimento, contorno, transparência (doc 04/10), quando aplicável.
- **GD-OBJ-04** Estado: visível, bloqueado, imprimível (herda da camada, pode sobrepor).
- **GD-OBJ-05** Dados de usuário: pares chave/valor (útil para orçamento, código de produção).
- **GD-OBJ-06** Caixa delimitadora geométrica e visual (inclui contorno e efeitos), calculadas sob demanda e cacheadas.

## Seleção
- **GD-SEL-01** Seleção múltipla; ordem de seleção preservada (usada por alinhar "relativo ao último/primeiro").
- **GD-SEL-02** Selecionar dentro de grupo sem desagrupar (modifier + clique).
- **GD-SEL-03** Selecionar por: retângulo, laço, tudo, por tipo, por propriedade (mesma cor de preenchimento/contorno), inverter.
- **GD-SEL-04** Painel "Objetos" em árvore: páginas → camadas → objetos, com arrastar para reordenar.

## Ordenação (z-order)
- **GD-ORD-01** Trazer para frente / enviar para trás / um passo / à frente de objeto / atrás de objeto / inverter ordem.
