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
Fonte, estilo, tamanho, entrelinha-base, espaçamento entre caracteres (tracking), kerning (métrico/óptico/manual por par), deslocamento de linha base, sobrescrito/subscrito, versalete, caixa alta/baixa, sublinhado/tachado/sobrelinha (estilo e espessura), preenchimento e contorno por caractere, recursos OpenType (ligaduras, alternativos, frações, ordinais, numerais estilísticos, conjuntos estilísticos ss01–ss20), escala horizontal/vertical, rotação individual de caractere.

## Atributos de parágrafo
Alinhamento (esquerda, centro, direita, justificado, forçado), recuos (primeira linha, esquerda, direita), espaço antes/depois, entrelinha (% do tamanho, pontos, múltiplo), tabulações (esquerda/centro/direita/decimal, preenchimento), marcadores e numeração com níveis, capitulares (linhas, distância), colunas no quadro (nº, largura, medianiz), alinhamento vertical no quadro, alinhamento à grade de linha base, controle de viúvas/órfãs, espaçamento palavras/letras para justificação.

## Texto em caminho
- **GD-TXT-09** Texto em caminho, por comprimento de arco (não pelo parâmetro da Bézier):
  - **Vincular:** com a ferramenta Texto, o cursor muda perto de uma curva (texto sobre linha) e o clique começa o texto nela, no ponto clicado; um texto existente arrastado com o botão direito sobre uma curva oferece "Ajustar ao caminho" (começa no ponto mais perto da soltura); ou Ajustar texto ao caminho com texto + curva selecionados. O vínculo é vivo: mexer na curva-guia refaz o texto.
  - **Barra:** orientação (girar letras; de pé; perspectiva pelas verticais ou pelas horizontais; deformar — a base segue o traço e o topo o arco externo), alinhamento vertical (linha de base, descendentes, altura-x, maiúsculas, centro), alinhamento horizontal (início, centro, fim, justificar no intervalo), X (deslocamento ao longo) e Y (distância do traço), espelhar na horizontal (percorre o caminho ao contrário) e na vertical (o texto passa para o outro lado sem inverter a leitura), intervalo útil De/até (%), regra dos cantos vivos (pular o canto — o glifo inteiro passa para a face seguinte; dobrar no canto; partes iguais por face), Anticolisão (nos côncavos o espaço abre na razão entre o topo e a base, para as letras não se tocarem), Selo superior/inferior (caminho fechado), Traço como construção, Desvincular (Ctrl+K: texto e caminho independentes, o texto mantém a curva) e Endireitar.
  - **Alças na tela** (texto em caminho selecionado), uma função cada, com cursor e nome da ação ao passar o mouse: losango vermelho no início (desliza o texto, com ímã no início, no meio e no fim do intervalo), bolinha verde (afasta do traço), seta dupla (troca de lado, no clique) e bolinha azul no fim (espaçamento entre letras). O intervalo útil fica na barra (De/até). No caminho fechado o arrasto não salta ao passar pelo começo.
  - **Caminho fechado:** o texto dá a volta. **Selo:** o texto vira o de cima (lendo por fora, centrado no ponto mais alto) e ganha o de baixo (lendo da esquerda para a direita por dentro, centrado no ponto mais baixo, com a altura das maiúsculas no traço — os dois na mesma faixa).
  - **Várias linhas:** cada linha corre paralela (concêntrica) à de cima, pela entrelinha do texto.
  - **Caminho fantasma:** Endireitar guarda o caminho; "Reativar caminho de origem" (menu de contexto) volta ao caminho e ao vínculo, recriando a guia se ela não existe mais.
  - **Espaçamento entre letras** (todo texto): campo na barra (% do corpo, ícone arrastável) e, no texto em caminho, a bolinha azul no fim do texto — puxar para a frente abre, para trás fecha (o quanto o fim andou se reparte entre as letras).
  - **Construção:** o traço-guia marcado como construção não sai na impressão, no PDF/SVG nem no corte; só as letras.

## Contorno de texto em objetos (text wrap)
- **GD-TXT-10** Objeto pode afastar texto de parágrafo: estilos contorno/quadrado, lados (esquerda/direita/ambos/acima-abaixo), distância.

## Fontes
- **GD-FNT-01** Lista de fontes do sistema + pasta de fontes do usuário; prévia; filtros (serifa, monoespaçada, variável, script).
- **GD-FNT-02** Fonte ausente ao abrir: substituição com mapeamento lembrado, aviso, texto preserva o nome original.
- **GD-FNT-03** Incorporar fontes no `.gdr` — inteiras (P-11 a), quando a licença permitir (fsType OS/2: instalável ou editável); opção do documento, desligada por padrão. Ao abrir com fonte ausente: trocar (sugestão da mais parecida), converter em curvas ou manter.

## Ferramentas de texto
- Inserir caractere especial (mapa Unicode), localizar/substituir (com atributos), estatísticas de texto, alterar caixa, colar texto sem formatação.
