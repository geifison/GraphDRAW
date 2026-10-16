# 08 — Transformações, Alinhamento, Encaixe, Réguas, Guias, Cotas

## Transformações
- **GD-TRF-01** Posição (x, y por ponto de referência: 9 âncoras), tamanho (L×A, proporcional), escala (%), rotação (graus, centro editável), reflexão H/V, inclinação (H/V graus).
- **GD-TRF-02** Aplicar a cada objeto individualmente ou ao conjunto; aplicar em cópia com N repetições (ex.: girar 30° × 11 cópias).
- **GD-TRF-03** Limpar transformações (volta à geometria original para primitivas/texto).
- **GD-TRF-04** Precisão: entrada numérica com expressões (`10+5`, `50%`, `2in`).

## Alinhamento e distribuição
- **GD-ALI-01** Alinhar esquerda/centro/direita/topo/meio/base, relativo a: último selecionado, primeiro selecionado, página, borda da página, grade, ponto específico, maior/menor objeto.
- **GD-ALI-02** Usar caixa geométrica ou visual (com contorno) — opção.
- **GD-ALI-03** Distribuir: espaçamento igual entre centros/bordas ou espaçamento fixo (valor), na extensão da seleção ou da página.
- **GD-ALI-04** Alinhamento por nós de texto (linha base).
- **GD-ALI-05** Guias dinâmicas/inteligentes durante arrasto: linhas temporárias mostrando alinhamento com bordas/centros de outros objetos e espaçamento igual.

## Encaixe (snap)
- **GD-SNP-01** Encaixar em: grade, guias, objetos (nós, interseções, pontos médios, quadrantes, tangentes, perpendiculares, centro, caixa delimitadora, borda), páginas (bordas, centro), pixels.
- **GD-SNP-02** Raio de encaixe em pixels de tela; indicação visual do tipo de ponto encontrado.
- **GD-SNP-03** Ligar/desligar global (atalho) e por tipo.

## Grades
- **GD-GRD-01** Grade de documento (espaçamento ou frequência), grade de linha base, grade de pixels; exibição como linhas ou pontos; imprimível opcional.

## Réguas e guias
- **GD-REG-01** Réguas H/V em unidade do documento, origem arrastável (duplo clique redefine).
- **GD-GUI-01** Guias horizontais, verticais e **inclinadas** (ângulo); arrastar da régua; posição numérica; bloquear; cor; guias de página mestra (todas as páginas) ou por página.
- **GD-GUI-02** Predefinições: margens, colunas, centro da página.
- **GD-GUI-03** Qualquer objeto pode virar guia (convertido para camada de guias).

## Cotas
- **GD-COT-01** Tipos: horizontal, vertical, paralela (alinhada), angular, segmento (mede segmento de curva), 3 pontos (com chamada), raio/diâmetro.
- **GD-COT-02** Dinâmica: ligada aos nós medidos, recalcula quando o objeto muda.
- **GD-COT-03** Formato: unidade, precisão, prefixo/sufixo, mostrar unidade, escala de desenho (doc 02 GD-DOC-03), posição do texto, estilo das setas e linhas de extensão.

## Conectores
- **GD-CON-01** Reto, ângulo reto (com cantos arredondados opcionais), curvo; ancorados a pontos de encaixe ou bordas; rotas recalculadas ao mover os objetos.

## Duplicação
- **GD-DUP-01** Duplicar (deslocamento configurável), clonar (cópia vinculada que herda alterações do mestre até ser sobreposta), repetir último comando, passo e repetição (N cópias com deslocamento X/Y).
