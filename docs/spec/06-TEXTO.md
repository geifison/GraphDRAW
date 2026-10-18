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
- **GD-TXT-08** Corretor ortográfico por idioma (Hunspell), opcional.

## Atributos de caractere
Fonte, estilo, tamanho, entrelinha-base, espaçamento entre caracteres (tracking), kerning (métrico/óptico/manual por par), deslocamento de linha base, sobrescrito/subscrito, versalete, caixa alta/baixa, sublinhado/tachado/sobrelinha (estilo e espessura), preenchimento e contorno por caractere, recursos OpenType (ligaduras, alternativos, frações, ordinais, numerais estilísticos, conjuntos estilísticos ss01–ss20), escala horizontal/vertical, rotação individual de caractere.

## Atributos de parágrafo
Alinhamento (esquerda, centro, direita, justificado, forçado), recuos (primeira linha, esquerda, direita), espaço antes/depois, entrelinha (% do tamanho, pontos, múltiplo), tabulações (esquerda/centro/direita/decimal, preenchimento), marcadores e numeração com níveis, capitulares (linhas, distância), colunas no quadro (nº, largura, medianiz), alinhamento vertical no quadro, alinhamento à grade de linha base, controle de viúvas/órfãs, espaçamento palavras/letras para justificação.

## Texto em caminho
- **GD-TXT-09** Posição (distância inicial), lado (acima/abaixo do caminho), orientação dos glifos (girar, inclinar vertical/horizontal), distância do caminho, espelhar.

## Contorno de texto em objetos (text wrap)
- **GD-TXT-10** Objeto pode afastar texto de parágrafo: estilos contorno/quadrado, lados (esquerda/direita/ambos/acima-abaixo), distância.

## Fontes
- **GD-FNT-01** Lista de fontes do sistema + pasta de fontes do usuário; prévia; filtros (serifa, monoespaçada, variável, script).
- **GD-FNT-02** Fonte ausente ao abrir: substituição com mapeamento lembrado, aviso, texto preserva o nome original.
- **GD-FNT-03** Incorporar fontes no `.gdr` — inteiras (P-11 a), quando a licença permitir (fsType OS/2: instalável ou editável); opção do documento, desligada por padrão. Ao abrir com fonte ausente: trocar (sugestão da mais parecida), converter em curvas ou manter.

## Ferramentas de texto
- Inserir caractere especial (mapa Unicode), localizar/substituir (com atributos), estatísticas de texto, alterar caixa, colar texto sem formatação.
