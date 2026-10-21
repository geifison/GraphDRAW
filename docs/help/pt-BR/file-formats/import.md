# Importar arquivos

[[menu.file > menu.file.import]] (<kbd>Ctrl+I</kbd>) acrescenta o conteúdo de
um arquivo ao desenho aberto, na camada ativa, como um passo de desfazer.
[[menu.file.open]] abre o arquivo como um desenho próprio. Os dois leem os
formatos listados em [Formatos de arquivo suportados](supported-formats.md#open).

## Onde ficam os objetos importados {#placement}

De início, os objetos importados vão para onde o arquivo os tem. Para centrá-los
na vista, mude a configuração na página [[options.file_formats]] das opções
globais; veja [Opções globais](../customize/global-options.md#file-formats).

## Observações sobre alguns formatos {#formats}

- **PDF e AI**: um arquivo protegido pede a senha na caixa de diálogo
  [[pdf_password.title]]. O texto mantém as fontes quando elas estão
  instaladas.
- **EPS e PostScript**: cada página impressa vira uma página.
- **PSD**: cada camada vira um bitmap com opacidade, visibilidade e máscara.
- **DXF**: as unidades de desenho vêm do arquivo.
- **Arquivos de texto** (TXT, RTF, DOCX): viram texto de parágrafo, com
  negrito, itálico, tamanhos e fontes onde o arquivo os tem.
- **Bitmaps**: colocados na resolução deles; veja [Bitmaps](../bitmaps/index.md).

Quando alguns objetos de um arquivo não podem ser lidos, ou faltam fontes, uma
mensagem avisa; veja
[Observações após abrir](../get-started/open-and-save.md#open-notes). Você
também pode colar de outros programas; veja
[Colar especial](../objects/paste-special.md).

## Soltar arquivos na janela {#drop}

Arraste um ou mais arquivos do gerenciador de arquivos para a janela.
Enquanto estão sobre ela, o desenho mostra uma moldura azul tracejada e
"Solte para importar". Cada arquivo é importado e incorporado ao desenho:
o primeiro fica centralizado onde você o soltou, os seguintes um pouco
adiante cada um, e tudo o que foi importado fica selecionado. Um arquivo
que o GraphDraw não sabe ler abre a mensagem [[alert.unsupported_file]]
com o nome dele.
