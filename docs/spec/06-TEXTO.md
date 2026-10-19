# 06 — Texto e Tipografia

## Tipos de texto
- **GD-TXT-01** **Texto artístico**: linha(s) livres criadas com clique; escala do objeto altera o tamanho visual das letras; pode ser colocado em caminho; pode ter envelope/efeitos.
- **GD-TXT-02** **Texto de parágrafo**: dentro de quadro retangular ou de qualquer forma fechada; quebra automática; quadros encadeados (fluxo entre quadros, inclusive entre páginas); indicador de texto excedente.
- **GD-TXT-03** Conversão artístico ↔ parágrafo preservando formatação.
- **GD-TXT-04** Converter em curvas (glifos viram caminhos).

## Motor
- **GD-TXT-05** Shaping com HarfBuzz; fontes OpenType (TTF/OTF/TTC/OTC), Type 1 opcional, fontes variáveis (eixos editáveis), cores (COLRv1/SVG-in-OT) quando suportado.
- **GD-TXT-06** Unicode completo, bidirecional (UAX#9), quebra de linha (UAX#14), segmentação de grafemas; scripts complexos (árabe, devanágari) via HarfBuzz.
- **GD-TXT-07** Hifenização por idioma (dicionários Hunspell/hyphen, licença livre). Idioma por parágrafo. Sem dicionário instalado (`%APPDATA%\GraphDraw\hifenizacao\hyph_<idioma>.dic`), o português usa regras de divisão silábica próprias; outros idiomas não hifenizam.
- **GD-TXT-08** Corretor ortográfico por idioma, opcional. No Windows usa o corretor do sistema (os dicionários de idioma do Windows, que também aceita dicionários Hunspell); em outros sistemas fica desligado.

## Atributos de caractere
Fonte, estilo, tamanho, entrelinha-base, espaçamento entre caracteres (tracking), kerning (métrico/óptico/manual por par), deslocamento de linha base, sobrescrito/subscrito, versalete e caixa alta tipográfica (GD-TXT-13), sublinhado/tachado/sobrelinha (estilo e espessura), preenchimento e contorno por caractere, recursos OpenType (ligaduras, alternativos, frações, ordinais, numerais estilísticos, conjuntos estilísticos ss01–ss20), escala horizontal/vertical, rotação individual de caractere.

## Atributos de parágrafo
Alinhamento (esquerda, centro, direita, justificado, forçado), recuos (primeira linha, esquerda, direita), espaço antes/depois, entrelinha (% do tamanho, pontos, múltiplo), tabulações (esquerda/centro/direita/decimal, preenchimento), marcadores e numeração com níveis, capitulares (linhas, distância), colunas no quadro (nº, largura, medianiz), alinhamento vertical no quadro, alinhamento à grade de linha base, controle de viúvas/órfãs, espaçamento palavras/letras para justificação.

## Texto em caminho
- **GD-TXT-09** Texto em caminho, por comprimento de arco (não pelo parâmetro da Bézier):
  - **Vincular:** com a ferramenta Texto, o cursor muda perto de uma curva (texto sobre linha) e o clique começa o texto nela, no ponto clicado; um texto existente arrastado com o botão direito sobre uma curva oferece "Ajustar ao caminho" (começa no ponto mais perto da soltura); ou Ajustar texto ao caminho com texto + curva selecionados. Com só o texto selecionado (ou em edição), o comando troca o cursor por uma seta para a direita e o próximo clique escolhe o objeto-guia (o objeto sob a seta fica com o contorno verde se serve, vermelho se não); o texto começa no ponto clicado. Se o clique não cair numa curva, linha ou forma, uma janela explica e pergunta se tenta de novo (Tentar de novo | Cancelar); Esc cancela. Sem texto selecionado (nem em edição), o item fica desativado no menu e no atalho; no menu de contexto, "Colocar texto no caminho" aparece com o texto sozinho (e então espera o clique) ou com o texto e a forma. O vínculo é vivo: mexer na curva-guia refaz o texto.
  - **Barra:** orientação (girar letras; de pé; perspectiva pelas verticais ou pelas horizontais; deformar — a base segue o traço e o topo o arco externo), alinhamento vertical (linha de base, descendentes, altura-x, maiúsculas, centro), alinhamento horizontal (início, centro, fim, justificar no caminho), X (deslocamento ao longo) e Y (distância do traço), espelhar na horizontal (inverte o sentido de leitura sem tirar o texto do lugar nem da faixa junto ao traço — põe de pé o texto da base de um círculo que saiu de cabeça para baixo, lendo da direita para a esquerda) e na vertical (o texto passa para o outro lado do traço sem inverter a leitura), regra dos cantos vivos (pular o canto — o glifo inteiro passa para a face seguinte; dobrar no canto; partes iguais por face), Anticolisão (nos côncavos o espaço abre na razão entre o topo e a base, para as letras não se tocarem), Selo superior/inferior (caminho fechado), Traço como construção, Desvincular (Ctrl+K: texto e caminho independentes, o texto mantém a curva) e Endireitar.
  - **Alças na tela** (texto em caminho selecionado), uma função cada, com cursor e nome da ação ao passar o mouse: losango vermelho no início (desliza o texto, com ímã no início, no meio e no fim do caminho), bolinha verde (afasta do traço), seta dupla (troca de lado, no clique) e bolinha azul no fim (espaçamento entre letras). No caminho fechado o arrasto não salta ao passar pelo começo.
  - **Caminho fechado:** o texto dá a volta. **Selo:** o texto vira o de cima (lendo por fora, centrado no ponto mais alto) e ganha o de baixo (lendo da esquerda para a direita por dentro, centrado no ponto mais baixo, com a altura das maiúsculas no traço — os dois na mesma faixa).
  - **Várias linhas:** cada linha corre paralela (concêntrica) à de cima, pela entrelinha do texto.
  - **Caminho fantasma:** Endireitar guarda o caminho; "Reativar caminho de origem" (menu de contexto) volta ao caminho e ao vínculo, recriando a guia se ela não existe mais.
  - **Espaçamento entre letras** (todo texto): campo na barra (% do corpo, ícone arrastável) e, no texto em caminho, a bolinha azul no fim do texto — puxar para a frente abre, para trás fecha (o quanto o fim andou se reparte entre as letras).
  - **Construção:** o traço-guia marcado como construção não sai na impressão, no PDF/SVG nem no corte; só as letras.

## Caixa alta e baixa
Duas camadas: **Mudar caixa** reescreve os caracteres do texto (GD-TXT-11/12); a **caixa tipográfica** só muda o desenho na tela e na saída, e o texto digitado fica intacto (GD-TXT-13).

- **GD-TXT-11** **Mudar caixa** (reescreve o texto), cinco modos:
  - **Primeira letra da frase maiúscula:** a primeira letra (alfabética) do bloco e a primeira depois de `.`, `!` ou `?` seguido de espaço (aspas e parênteses de fechamento entre os dois não contam) vão para maiúscula; as demais letras, para minúscula. No texto de parágrafo a quebra de parágrafo também começa frase; no texto artístico o objeto é uma linha contínua única: só a primeira letra do texto vira maiúscula.
  - **minúsculas** e **MAIÚSCULAS:** cada letra pelo mapeamento de caixa Unicode; números, pontuação e símbolos ficam iguais; acentos acompanham a letra.
  - **Todas as Iniciais Maiúsculas:** a primeira letra de cada palavra em maiúscula e o resto da palavra em minúscula. Palavra = sequência de letras, algarismos e apóstrofos (`'`, `’`); espaço, quebra de linha, hífen, travessão, barra e as demais pontuações separam palavras (`sub-base multi-eixo` → `Sub-Base Multi-Eixo`; `3mm` fica `3mm`).
  - **iNVERTER mAIÚSCULAS/mINÚSCULAS:** cada letra troca de caixa (`bOTÃO dIREITO` → `Botão Direito`).
  - **Unicode:** mapeamento completo, inclusive os que mudam o número de letras (`ß` → `SS`) e os dígrafos de caixa de título (`ǆ` → `ǅ`); com o idioma do parágrafo turco ou azeri (`tr`, `az`), `i` ↔ `İ` e `ı` ↔ `I`.
  - **Alcance:** o trecho selecionado na edição de texto; sem trecho, o texto inteiro de cada objeto de texto selecionado. O que vem antes do trecho decide o começo de frase e de palavra.
  - **Estilos:** só os caracteres mudam; fonte, cor, contorno e os demais atributos de cada trecho ficam onde estavam.
  - **Desfazer:** cada aplicação é um passo só do histórico (Ctrl+Z volta tudo).
- **GD-TXT-12** **Janela Mudar caixa:** no menu Texto, no menu de contexto do texto e no atalho **Shift+F3**. Cinco opções exclusivas, uma prévia do resultado e Aplicar | Cancelar; abre com o último modo usado. Com a janela aberta, Shift+F3 de novo percorre minúsculas → MAIÚSCULAS → Todas as Iniciais Maiúsculas → minúsculas (a prévia acompanha); Enter aplica, Esc cancela. Sem texto selecionado nem em edição, o comando fica desativado.
- **GD-TXT-13** **Caixa tipográfica** (atributo de caractere, não destrutivo), na barra de propriedades do texto e no menu Texto; vale para o trecho selecionado ou para o texto inteiro:
  - **Versalete:** as minúsculas aparecem com desenho de maiúscula na altura-x. Com o recurso OpenType `smcp` na fonte, usa os glifos dela; sem ele, versalete sintético: maiúsculas a 72% do corpo (largura e altura) com +4% do corpo de espaço depois de cada uma.
  - **Todas em versalete:** maiúsculas e minúsculas viram versalete (`c2sc` + `smcp`; sintético se a fonte não tiver os dois).
  - **Todas em maiúsculas:** só o desenho vai para maiúscula (com o recurso `case`, formas próprias para caixa alta, quando a fonte tem); desligar volta o texto digitado como estava.
  - Os três são exclusivos (ligar um desliga o outro; clicar no ligado volta ao normal).
  - **Espaçamento de capitais:** liga/desliga; +5% do corpo entre duas maiúsculas seguidas (como aparecem, inclusive versalete e Todas em maiúsculas).
  - Cursor, seleção, busca, cópia e o texto vivo do PDF usam o texto digitado; o PDF põe os glifos na escala do versalete sintético; o SVG grava `text-transform`/`font-variant`.

## Contorno de texto em objetos (text wrap)
- **GD-TXT-10** Objeto pode afastar texto de parágrafo: estilos contorno/quadrado, lados (esquerda/direita/ambos/acima-abaixo), distância.

## Fontes
- **GD-FNT-01** Lista de fontes do sistema + pasta de fontes do usuário; prévia; filtros (serifa, monoespaçada, variável, script).
- **GD-FNT-02** Fonte ausente ao abrir: substituição com mapeamento lembrado, aviso, texto preserva o nome original.
- **GD-FNT-03** Incorporar fontes no `.gdr` — inteiras (P-11 a), quando a licença permitir (fsType OS/2: instalável ou editável); opção do documento, desligada por padrão. Ao abrir com fonte ausente: trocar (sugestão da mais parecida), converter em curvas ou manter.

## Ferramentas de texto
- Inserir caractere especial (mapa Unicode), localizar/substituir (com atributos), estatísticas de texto, alterar caixa (GD-TXT-11), colar texto sem formatação.
