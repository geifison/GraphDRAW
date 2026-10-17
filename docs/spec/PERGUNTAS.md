# Perguntas abertas sobre a especificação

Dúvidas de comportamento registradas pelo time de implementação (regra 5
do `CLAUDE.md`). Cada item fica aberto até resposta do responsável pela
spec.

## P-01 — Cores CMYK/especiais em `.cdr` (GD-CDR-04)
**Contexto:** a interface pública da librevenge entrega cores como
`#rrggbb`; a libcdr converte CMYK, cores especiais e outros modelos para RGB
antes de chamá-la. Com a libcdr sem modificações, não é possível preservar o
modelo original exigido por GD-CDR-04 / GD-COR-02.
**Situação no M0:** cores importadas em RGB; o relatório de importação avisa
em todo `.cdr` (`cdr.color.rgb-only`).
**Pergunta:** aceitar a limitação até o M2, ou priorizar uma contribuição
upstream na libcdr/librevenge (propriedades extras com o modelo original)?

## P-02 — Referências de fidelidade da importação `.cdr` (doc 14, "Testes")
**Contexto:** a spec pede comparar a renderização com PDFs exportados pelo
autor de cada arquivo de teste. As amostras de licença livre disponíveis
(corpus do LibreOffice) não vêm com esses PDFs.
**Situação no M0:** referência PNG gerada pelo próprio GraphDraw e aprovada
por inspeção visual (teste de regressão).
**Pergunta:** há um corpus de `.cdr` próprios, com PDF do autor, que possa
ser incluído em `tests/fixtures/`?

## P-03 — Componentes de Lab (GD-COR-03 × GD-GDR-07)
**Contexto:** GD-COR-03 diz que componentes de cor são float 0–1
internamente; para Lab isso não é natural.
**Decisão provisória:** Lab guarda L* 0–100, a* e b* −128–127 (valores CIE);
demais modelos usam 0–1. Documentado em `docs/format/gdr-1.0.md`.
**Pergunta:** confirmar.

## P-04 — Geometria local das primitivas no `.gdr`
**Contexto:** o doc 13 define o envelope do objeto, mas não onde fica a
origem local de retângulos, elipses, textos e imagens.
**Decisão provisória:** retângulo e imagem ocupam `[0,w]×[0,h]`; elipse é
centrada na origem; texto tem origem no canto superior esquerdo do bloco,
linhas avançando para −Y; cantos do retângulo na ordem inferior-esquerdo,
inferior-direito, superior-direito, superior-esquerdo.
**Pergunta:** confirmar ou ajustar antes do M1 (edição).
**Resposta (2026-10-16):** confirmada pelo responsável pelo produto como
decisão final; vale para o formato 1.0 (`docs/format/gdr-1.0.md`).

## P-05 — Transparência só do preenchimento
**Contexto:** a libcdr informa a transparência do preenchimento
separadamente do contorno; o modelo do doc 04 (GD-TRA-01) prevê
transparência por preenchimento, contorno ou ambos.
**Decisão provisória:** `fill.opacity` (preenchimento sólido) e
`stroke.opacity`, além de `opacity` do objeto inteiro.
**Pergunta:** confirmar o nome e o lugar desses campos no formato.

## P-06 — Fio de cabelo na exportação
**Contexto:** GD-OUT-01 define fio de cabelo como "menor espessura
imprimível do dispositivo".
**Decisão provisória:** PDF grava largura 0 (semântica nativa do PDF); tela
e PNG usam 1 pixel; SVG usa 0,1 mm.
**Pergunta:** confirmar o valor para SVG.

## P-07 — Referência de "Alinhar nós" (GD-NOD-06)
**Contexto:** o requisito pede alinhar nós na horizontal/vertical, sem dizer
em relação a quê.
**Decisão provisória:** o meio da caixa dos nós escolhidos; as alças
acompanham cada nó.
**Pergunta:** confirmar ou escolher outra referência (primeiro/último nó
escolhido).

## P-08 — "Qualquer objeto pode virar guia" (GD-GUI-03) e camadas especiais (GD-CAM-04)
**Contexto:** as guias de hoje são retas (horizontais, verticais e
inclinadas). GD-GUI-03 fala em converter qualquer objeto para a camada de
guias, o que pede guias curvas e a camada especial de GD-CAM-04.
**Decisão provisória:** nada implementado ainda.
**Pergunta:** guias de objeto devem ser curvas de verdade (encaixe em
qualquer ponto do contorno) ou basta transformar retas e bordas de
retângulos em guias retas? A camada de guias é por página, do documento,
ou as duas?

## P-09 — Medida de geometria na fidelidade (GD-FID-11)
**Contexto:** o doc 25 pede desvio por "objeto identificável" em mm. Sem
abrir o software de origem (GD-FID-42), a ferramenta só vê os PDFs.
**Decisão provisória:** `graphdraw fidelity` mede o tamanho da página e a
caixa da tinta pela imagem (resolução de 1 pixel, 0,17 mm a 150 ppp) e
inclui o deslocamento; caixas por objeto ficam para depois.
**Pergunta:** essa medida serve para o MVP ou a referência deve trazer
algo a mais (por exemplo um PDF de corte por cor especial) para medir
objeto a objeto?

## P-10 — Pontas de seta e caneta caligráfica no formato (GD-OUT-05/06)
**Contexto:** o doc 13 não define esses campos.
**Decisão provisória:** `stroke.startArrow`/`endArrow` (`shape`, `scale`,
`offset`, `rotation`, `mirror`) e `stroke.nibStretch`/`nibAngle`, descritos
em `docs/format/gdr-1.0.md` e no schema; biblioteca de pontas própria
(triângulo, aberta, farpada, círculo, quadrado, barra).
**Pergunta:** confirmar os nomes e a biblioteca inicial.
