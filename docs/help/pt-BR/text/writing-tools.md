# Ferramentas de escrita

O GraphDraw verifica ortografia e gramática, encontra sinônimos, corrige
enquanto você digita e conserta textos que chegaram com caracteres errados. Os
comandos ficam em [[menu.text > menu.text.writing_tools]] e no menu
[[menu.text]].

## A caixa de diálogo Ferramentas de escrita {#writing-tools}

[[menu.text > menu.text.writing_tools > menu.text.spell_check]],
[[menu.text.grammatik]] e [[menu.text.thesaurus]] abrem a caixa de diálogo
[[writing.title]] nessa guia. [[writing.check]] escolhe se verifica todo texto
da página ([[writing.scope_document]]) ou só os textos selecionados
([[writing.scope_selection]]).

![A caixa de diálogo Ferramentas de escrita](../../img/text-writing-tools.png)

### Verificador ortográfico {#spell-checker}

1. Clique em [[writing.start]]. A primeira palavra não encontrada aparece, com
   sugestões em [[writing.replacements]].
2. Clique em uma sugestão para colocá-la em [[writing.replace_with]] (clique
   duplo para substituir de uma vez), ou digite a palavra certa.
3. Clique em [[writing.replace]] para este lugar, [[writing.auto_replace]] para
   todos os lugares, [[writing.skip_once]] ou [[writing.skip_all]] para deixá-la,
   ou [[writing.add]] para acrescentá-la à sua lista de palavras.
4. [[writing.undo]] desfaz a última substituição.

[[writing.options]] tem [[writing.ignore_digits]] e
[[writing.ignore_capitals]]. O GraphDraw usa os dicionários instalados no seu
sistema; sem nenhum, só uma lista de palavras incluída é usada.

### Gramática {#grammar}

O verificador gramatical encontra palavras repetidas, espaços duplos, espaços
antes da pontuação, falta de espaço depois dela, frases que começam com
minúscula, parênteses e aspas sem par, pontuação repetida, frases com mais de
40 palavras, falta de ponto final e, em inglês, "a" e "an" antes do som errado.
Para cada ocorrência, mostra a substituição, a [[writing.new_sentence]] e o
[[writing.rule_name]]. Clique em [[writing.start]] (depois [[writing.replace]]),
[[writing.skip_once]] ou [[writing.skip_rule]].

### Dicionário de sinônimos {#thesaurus}

Digite uma palavra (de início, a primeira palavra do texto selecionado) e clique
em [[thesaurus.look_up]]. Os significados aparecem com os sinônimos: clique em
um para escolhê-lo, clique duas vezes para procurá-lo. [[writing.insert]] o
coloca no texto, mantendo as maiúsculas. Em [[writing.options]],
[[writing.auto_close]] fecha a caixa de diálogo depois de inserir, e
[[thesaurus.pick_file]] usa um arquivo de sinônimos que você tenha.

## Correção automática {#autocorrect}

[[menu.text > menu.text.writing_tools > menu.text.autocorrect]] define o que é
corrigido enquanto você digita:

- [[autocorrect.enabled]];
- [[autocorrect.capitalize]];
- [[autocorrect.two_capitals]] ("THe" vira "The");
- [[autocorrect.quotes]], no estilo do idioma da interface;
- a [[autocorrect.replacements]]: `(c)` vira ©, `(r)` ®, `(tm)` ™, `1/2` ½ e
  assim por diante. [[autocorrect.add]] e [[autocorrect.remove]] mudam as
  linhas.

[[autocorrect.apply_selection]] aplica as regras ao texto selecionado. Texto
colado ou importado nunca é alterado.

![A caixa de diálogo Correção automática](../../img/text-writing-tools-autocorrect.png)

## Hífens personalizados {#custom-hyphens}

[[menu.text > menu.text.custom_hyphens]] (também a engrenagem da seção
Parágrafo do painel [[docker.text]]) define onde suas próprias palavras podem
quebrar, como nomes de produtos. Essas palavras quebram só ali quando a
hifenização está ativada.

1. Escolha o [[hyphens.language]] ([[hyphens.all_languages]] lista todos).
2. Digite a [[hyphens.word]] e a [[hyphens.hyphenated]] com hífens onde ela pode
   quebrar, como `Graph-Draw`.
3. Clique em [[hyphens.add]]. [[hyphens.remove]] remove a definição
   selecionada.
4. Em [[hyphens.auto_insert]], [[hyphens.when_typing]] e
   [[hyphens.when_pasting]] ativam a hifenização nos textos que contêm uma
   palavra definida.
5. Clique em [[hyphens.apply_selection]] para aplicá-las ao texto selecionado
   agora, ou em [[dialog.ok]] para guardá-las.

![A caixa de diálogo Hífens opcionais personalizados](../../img/text-writing-tools-custom-hyphens.png)

## Mudar maiúsculas e minúsculas {#change-case}

[[menu.text > menu.text.change_case]] muda o texto selecionado para
[[menu.text.case_sentence]], [[menu.text.case_lower]],
[[menu.text.case_upper]], [[menu.text.case_title]] ou
[[menu.text.case_toggle]].

![A caixa de diálogo Mudar maiúsculas e minúsculas](../../img/text-writing-tools-change-case.png)

## Estatísticas do texto {#statistics}

[[menu.text > menu.text.text_statistics]] conta os objetos de texto, as linhas,
as palavras, os caracteres e as fontes do texto selecionado, ou da página.

![A caixa de diálogo Estatísticas do texto](../../img/text-writing-tools-statistics.png)

## Codificar {#encode}

Texto vindo de outro programa às vezes mostra os caracteres errados, como
"cafÃ©" em vez de "café". [[menu.text > menu.text.encode]] o conserta:

1. Selecione o texto.
2. Escolha como o texto [[dialog.encode_from]] e o que ele [[dialog.encode_to]]
   (UTF-8, Windows-1252, ISO-8859-1, Mac OS Roman ou Windows-1251).
3. Confira a [[dialog.preview]] e clique em [[dialog.ok]].

![A caixa de diálogo Codificar texto](../../img/text-writing-tools-encode.png)

## Texto compatível com a web {#web-compatible}

[[menu.text > menu.text.make_web_compatible]] simplifica o texto selecionado
para páginas web: preenchimentos em gradiente, padrão, textura e malha viram uma
cor lisa, e contornos, sombras e efeitos são removidos.
