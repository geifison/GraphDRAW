# 29 — Impressão

Origem: pedido do responsável pelo produto (2026-10-18): processo de
impressão completo, do escritório à gráfica e ao grande formato. O desenho
segue o costume geral dos editores vetoriais profissionais (impressora,
layout, marcas, pré-impressão e prévia); nenhum programa comercial foi
aberto ou consultado (doc 00).

Fase: **MVP** (entra na 1.0). Só Windows (decisão de 2026-10-17).

Unidades internas em µm; a folha tem a origem no canto de baixo à esquerda
e Y para cima, como a página.

## 1. Envio
- **GD-PRT-01** **Imprimir** (Arquivo → Imprimir, Ctrl+P) abre a janela de
  impressão do documento atual. Sem documento, o comando fica desativado.
- **GD-PRT-02** Impressora do Windows: o GraphDraw rasteriza cada folha na
  resolução da impressora (até 600 ppp na impressão; a prévia usa menos) e
  envia pelo spooler, em faixas, para não estourar a memória em folhas
  grandes. O resultado é o mesmo da tela e da exportação PNG.
- **GD-PRT-03** **Imprimir para arquivo (PDF)**: as mesmas folhas saem como
  PDF vetorial, com as marcas, para o RIP ou a pasta quente da gráfica. O
  PDF segue a opção de PDF/X-1a da exportação (doc 15) e sai com uma
  cópia (as cópias ficam com o RIP). Negativo não se aplica ao PDF (fica a
  cargo do RIP) e é avisado.
- **GD-PRT-04** O envio roda em segundo plano com barra de progresso
  (folha n de N) e **Cancelar**, que aborta o trabalho no spooler.
- **GD-PRT-05** As opções da última impressão ficam guardadas por
  documento durante a sessão; a impressora escolhida fica guardada para as
  próximas sessões.

## 2. Geral
- **GD-PRT-10** Impressora: lista das impressoras instaladas, com a padrão
  marcada; **Propriedades** abre a janela do driver (configurações próprias
  do fabricante, como mídia e qualidade).
- **GD-PRT-11** Papel e orientação: lista de papéis da impressora e
  Retrato | Paisagem. A área imprimível vem do driver (margens físicas).
- **GD-PRT-12** Cópias (1–999) e **Agrupar** (cópia 1 inteira, depois a 2…;
  sem agrupar, cada folha se repete).
- **GD-PRT-13** Páginas: Todas | Página atual | Intervalo (`1,3-5`, base 1,
  como o GD-EXP-01).

## 3. Layout
- **GD-PRT-20** Escala: **Tamanho real** (100%), **Ajustar à folha** (maior
  escala que cabe na área imprimível, mantendo a proporção) ou
  **Porcentagem** (1–1000%).
- **GD-PRT-21** Posição: **Centralizar** na área imprimível ou **Canto**
  (canto de cima à esquerda da área imprimível), mais deslocamento X/Y.
- **GD-PRT-22** **Girar automaticamente**: a página gira 90° quando assim
  cabe na folha e do jeito original não.
- **GD-PRT-23** **Lado a lado (ladrilhar)**: página maior que a folha é
  dividida em ladrilhos do tamanho da área imprimível, com **sobreposição**
  (padrão 10 mm) para colar. Cada ladrilho leva, com as marcas ligadas,
  marcas de alinhamento nos cantos e a etiqueta "linha L, coluna C".
  Ladrilhar ignora a escala "Ajustar".
- **GD-PRT-24** **Imposição N-up**: várias cópias da página (ou páginas
  seguidas, com **Páginas diferentes**) em grade na folha. Colunas e linhas
  (0 = o máximo que cabe) e o espaço entre elas (padrão 3 mm; com marcas de
  corte, ao menos o dobro da sangria impressa). N-up e ladrilhar não se
  combinam.

## 4. Marcas
- **GD-PRT-30** As marcas usam a cor de registro (sai em todas as chapas)
  e ficam fora da área de corte, a 2 mm além da sangria impressa:
  - **Marcas de corte**: dois traços de 5 mm por canto, 0,25 pt;
  - **Marcas de registro**: alvo (círculo e cruz, 6 mm) no meio de cada lado;
  - **Barra de cores**: C, M, Y, K, CM, CY, MY e degraus de K (10–100%),
    quadrados de 5 mm acima da página;
  - **Informações**: nome do arquivo, página, chapa, data e hora, abaixo
    da página.
- **GD-PRT-31** **Imprimir sangria**: a página sai com a sangria do
  documento (doc 22) além do corte; sem a opção, sai recortada no corte.
- **GD-PRT-32** Marcas que não cabem na área imprimível são omitidas, e a
  janela avisa ("as marcas não cabem; reduza a escala ou use papel maior").

## 5. Pré-impressão e grande formato
- **GD-PRT-40** **Espelhar** (horizontal): para transfer, sublimação e
  impressão no verso de material transparente. As marcas e o texto de
  informações espelham junto.
- **GD-PRT-41** **Negativo**: inverte as cores da folha impressa (filme).
- **GD-PRT-42** **Separações**: uma folha por chapa (as quatro de processo e
  as cores especiais usadas, como na prévia de separações do doc 16), em
  tons de cinza; caixas para escolher as chapas. A informação de cada folha
  diz a chapa.
- **GD-PRT-43** Objetos e camadas marcados como não imprimíveis não saem
  (como na exportação).

## 6. Prévia
- **GD-PRT-50** A janela mostra a folha à direita: papel branco, a área
  imprimível tracejada, a página posicionada com as marcas, ladrilhos e
  cópias; navegação folha 1 de N. A prévia acompanha cada mudança das
  opções.
- **GD-PRT-51** Abaixo da prévia: tamanho da folha, escala efetiva e a
  quantidade de folhas (× cópias). Página que não cabe em tamanho real,
  sem ladrilhar, mostra o aviso "a página não cabe na folha; será cortada".
- **GD-PRT-52** Sem nenhuma impressora instalada, a janela abre só com
  **Imprimir para arquivo**, com papel A4 e área imprimível igual à folha.

## 7. Critérios de aceite
- Plano de folhas (escala, posição, giro, ladrilhos, N-up, separações,
  cópias) testado sem impressora (`print::tests`).
- A folha rasterizada é igual à exportação PNG da página quando não há
  marcas, escala 100% e a folha tem o tamanho da página.
- O PDF de impressão abre com uma página por folha, MediaBox = folha.
