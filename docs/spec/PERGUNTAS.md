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

## P-11 — Fontes incorporadas no `.gdr` (GD-FNT-03)
**Contexto:** o doc 13 prevê `resources/fonts/<sha256>.otf` com
**subconjuntos**. O subconjunto que o projeto já gera (para o PDF) troca os
números dos glifos e não serve para diagramar o texto de novo ao abrir; e
um subconjunto só tem as letras usadas, então digitar letras novas cairia
na fonte substituta.
**Decisão provisória:** nada implementado.
**Pergunta:** (a) incorporar a fonte **inteira** quando o fsType permitir
(instalável ou editável), aceitando arquivos maiores; (b) subconjunto com
mapa de caracteres próprio (só para ver/imprimir, sem editar letras novas);
ou (c) outra regra?
**Resposta (2026-10-17):** (a), fonte inteira. Para não pesar todos os
arquivos, é uma opção do documento (Arquivo → Incorporar fontes ao salvar,
desligada por padrão); ao abrir um arquivo com fonte ausente, uma janela
oferece trocar pela mais parecida, converter os textos em curvas ou manter.

## P-12 — Conta-gotas: fora da janela e Alt no gradiente (doc 27)
**Contexto:**
- GD-CTG-04 pede para amostrar fora da janela do programa. Isso exige
  captura de tela do sistema operacional (uma API por plataforma) e
  permissão de gravação de tela no macOS.
- Alt + clique tem dois sentidos: "alterna pipeta/balde" (GD-CTG-05) e
  "clona a rampa" (GD-CTG-09).

**Decisão provisória:**
- GD-CTG-04 fica Beta. O botão "Fora da janela" aparece desabilitado.
- Alt + clique:
  - no balde, volta à pipeta;
  - na pipeta sobre um gradiente (modo Cor), clona a rampa;
  - na pipeta fora de gradiente com o balde cheio, volta ao balde.

**Pergunta:**
- Pode entrar uma dependência de captura de tela por plataforma?
- A regra do Alt serve?
**Resposta (2026-10-17):** não; o conta-gotas fica só dentro da janela do
programa (GD-CTG-04 fora da 1.0). A regra do Alt segue a decisão
provisória.

## P-13 — Espiral e grade sem bloco de transformação (doc 31, GD-BPC-08/09)
**Contexto:** todos os objetos da Seleção começam com B-TRF (posição,
tamanho, escala, ângulo, espelhar), menos Espiral e Grade, que também
trazem "fechar ☐" (a grade já é fechada) e B-SET (setas) na espiral.
**Decisão provisória (D-31-03):** os dois começam com B-TRF; "fechar" fica
só na espiral.
**Pergunta:** confirmar que foi omissão da transcrição.

## P-14 — Barra de propriedades nas laterais (doc 31, GD-BPR-06)
**Contexto:** a barra tem altura fixa de 72 px com controles empilhados em
duas linhas; numa lateral ela teria de virar uma coluna de ~200 px ou
quebrar os grupos, o que o GD-BPR-08 proíbe.
**Decisão provisória (D-31-06):** em cima (padrão) ou embaixo, e Bloquear
barras. Laterais e flutuante aguardam desenho.
**Pergunta:** as laterais são necessárias na 1.0? Se sim, com que desenho?

## P-15 — PostScript vetorial (doc 16, GD-IMP-04/18)
**Contexto:** o GraphDraw não tem gerador de PostScript; transparência e
mesclagem não existem no PS e exigiriam achatamento.
**Decisão provisória (D-16-06):** PS rasterizado por folha nesta fase.
**Pergunta:** algum cliente exige PS vetorial (RIP antigo sem PDF)? Se sim,
entra como item próprio pós-Beta.

## P-16 — Fase da impressão no M3 (doc 16 §8.3 C-01)
**Contexto:** o doc 16 põe GD-IMP-01..05 no M3; o `MVP.md` da origem,
GD-IMP-01..03; o pedido de implementação de 2026-10-19, GD-IMP-01..07. O
doc 29 (inteiro MVP) já inclui N-up, separações, marcas e posição, cujos
pares no doc 16 (GD-IMP-20, 08/12, 11, 07) estão na Beta.
**Pergunta:** o que entra no M3? Sugestão para decidir: doc 29 inteiro +
GD-IMP-01..07 (com estilos e posição por página); o restante do 16 na Beta.

## P-17 — Envio à impressora: sempre rasterizado ou vetorial? (C-02)
**Contexto:** GD-PRT-02 rasteriza toda folha (até 600 ppp) e envia em
faixas. GD-IMP-05 faz da rasterização uma opção de emergência, o que supõe
envio vetorial ao driver como padrão.
**Pergunta:** o padrão continua rasterizado (como hoje no app) e GD-IMP-05
só escolhe a resolução, ou o envio passa a ser vetorial?

## P-18 — PDF de "Imprimir em arquivo": padrão e negativo (C-03)
**Contexto:** GD-PRT-03 sai em PDF/X-1a e não aplica negativo (avisa).
GD-IMP-18b tem padrão *Acrobat DC* (PDF 1.7) e não fala de negativo.
**Pergunta:** qual compatibilidade padrão na impressão para arquivo? O
negativo continua fora do PDF?

## P-19 — Marcas de corte e barra de cores (C-04, C-05)
**Contexto:**
- Corte: GD-PRT-30 = traço de 5 mm a 2 mm além da sangria impressa;
  GD-IMP-11 = distância padrão 3 mm da página; GD-IMP-11b = 3,8 mm da
  linha de corte e 8,5 mm de comprimento (1,3/2,8 mm entre peças).
- Marcas que não cabem: GD-PRT-32 omite; o doc 16 só avisa.
- Barra de cores: GD-PRT-30 = C, M, Y, K, CM, CY, MY + K 10–100 %;
  GD-IMP-11 = CMYK + spots 0–100 % em passos de 10, escala de cinza de 7
  degraus e uma escala por tinta na composta (GD-IMP-11c).
**Pergunta:** vale a tabela do GD-IMP-11b (e o "3 mm" do GD-IMP-11 é outra
coisa, ou sai)? Marcas que não cabem são omitidas ou cortadas? A barra de
cores segue o doc 16 (com ou sem as sobreposições CM/CY/MY)?

## P-20 — Valores padrão divergentes (C-06)
**Contexto:** sobreposição do mosaico 10 mm (GD-PRT-23) × 0 (GD-IMP-70);
sangria de saída igual à do documento (GD-PRT-31, GD-IMP-10) × desligada
com 3 mm (GD-IMP-70); separações com as 4 chapas de processo sempre
(GD-PRT-42) × chapas vazias ocultas (GD-IMP-12/70).
**Pergunta:** valem os padrões do GD-IMP-70? E o valor inicial da sangria
de saída é o do documento ou 3 mm fixos?

## P-21 — Limites e rótulos (C-07)
**Contexto:** cópias até 999 (GD-PRT-12) × 9999 (GD-IMP-72); rótulo
*Agrupar* (GD-PRT-12, já no app) × *Intercalar* (GD-IMP-01); escala
1–1000 % (GD-PRT-20) × sem limite no GD-IMP-72. "Tamanho do ladrilho (%
da folha)" vai de 0 a 25 % (GD-IMP-72), o que seria um ladrilho minúsculo.
**Decisão provisória (D-16-02):** p % da largura e da altura da área
imprimível.
**Pergunta:** quais limites e qual rótulo? O campo de 0–25 % é o tamanho
do ladrilho ou a sobreposição em %?

## P-22 — Verificação: limiar de linha fina e onde ficam as regras (C-08)
**Contexto:** linha fina mínima 0,25 pt (GD-PRE-02) × 0,1 mm ≈ 0,28 pt
(GD-IMP-30b). Limites no documento (GD-PRE-02) × regras e limiares por
perfil independente do documento (GD-IMP-31).
**Decisão provisória (D-16-05):** um só motor de regras para o painel e
para a seção Problemas.
**Pergunta:** qual limiar? Limites ficam no documento, no perfil, ou o
perfil sobrepõe o documento?

## P-23 — Sub-modos da Imposição: 4 ou 6 (C-10)
**Contexto:** GD-IMP-32 lista 4 (grade, ordem, calhas e acabamento,
margens); o doc 28 da origem (§9, doc 31 aqui) fala em 6.
**Decisão provisória (D-16-10):** os 4 do GD-IMP-32.
**Pergunta:** faltam 2? Quais?

## P-24 — Onde ficam as opções de impressão (C-09)
**Contexto:** GD-PRT-05 guarda as opções por documento só na sessão e a
impressora entre sessões. GD-IMP-06 grava o último estilo usado no `.gdr`.
**Pergunta:** passa a valer o GD-IMP-06 (estilo no `.gdr`, o que muda o
formato, doc 13)? A impressora escolhida continua entre sessões?
