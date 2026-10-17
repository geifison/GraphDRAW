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
