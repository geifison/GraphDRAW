# Colocar objetos dentro de um clip frame

Um **clip frame** é um objeto fechado que contém outros objetos e mostra só
as partes deles que ficam dentro dele, como uma imagem vista por uma janela.
O clip frame mantém o próprio preenchimento e contorno; os objetos de dentro
ficam inteiros e podem ser retirados de novo. Os comandos ficam em
[[menu.object > menu.object.clip_frame]].

## Para colocar objetos dentro de um clip frame {#place-inside}

1. Selecione os objetos a colocar dentro.
2. Escolha [[menu.object > menu.object.clip_frame > menu.object.clip_frame_place_inside]].
3. Clique no objeto fechado que será o clip frame.

Os objetos mantêm a posição. Você também pode arrastar os objetos até o
clip frame com o botão direito do mouse e escolher colocá-los dentro; veja
[Arrastar com o botão direito](selecting.md#right-drag).

Um objeto que já é um clip frame recebe os novos objetos por cima do conteúdo
que já tem, mesmo quando está dentro de outro clip frame.

## Editar o que está dentro {#edit-contents}

- [[menu.object.clip_frame_edit]] entra no clip frame: só os objetos de dentro
  podem ser editados, o resto da página fica esmaecido e o clip frame aparece
  como contorno. O que você desenhar ou colar enquanto edita também entra.
  [[menu.object.clip_frame_finish]], o botão na barra de propriedades, Esc, ou
  clicar duas vezes ou Ctrl+clicar fora do conteúdo, sai dele.
- [[menu.object.clip_frame_extract]] torna os objetos de dentro objetos comuns
  de novo; o clip frame fica, vazio.
- [[menu.object.clip_frame_lock]] (ativado de início): mover, escalonar ou
  girar o clip frame leva o conteúdo junto. Desativado, o clip frame se move
  sozinho e o conteúdo fica onde está na página.

Clicar duas vezes ou Ctrl+clicar em um clip frame com a ferramenta
[[tool.pick]] também entra nele (num clip frame dentro do conteúdo, um nível
mais fundo); veja [Modo de foco](focus-mode.md). Copiar, colar e duplicar um clip
frame levam o conteúdo junto.

## Máscaras de opacidade {#opacity-mask}

[[menu.object > menu.object.clip_frame > menu.object.clip_frame_opacity_mask]]
transforma os clip frames selecionados em máscaras de opacidade: o clip frame
deixa de ser desenhado, e o brilho dele decide quanto do conteúdo aparece:
tudo onde ele é branco, nada onde é preto, e nada fora do clip frame.
Preencha o clip frame com um gradiente para um esmaecimento suave.
[[menu.object.clip_frame_invert_mask]] inverte isso, de modo que o preto
mostra. Escolha o comando de novo para voltar a um clip frame simples.

## Clip frames vazios {#empty-frames}

Clique com o botão direito em um objeto e use os comandos de
**Tipo de quadro**:

- **Criar clip frame vazio** transforma cada objeto selecionado em um
  clip frame vazio, pronto para receber objetos.
- **Sem quadro** devolve um clip frame vazio ao objeto que ele era.
- **Criar quadro de texto vazio** substitui o objeto por um quadro de texto de
  parágrafo vazio do mesmo tamanho, pronto para a ferramenta [[tool.text]].

Recortar um bitmap com a ferramenta [[tool.crop]] ou com a ferramenta
[[tool.shape]] também o coloca em um clip frame; veja
[Recortar, cortar e apagar](../lines-shapes/cut-and-erase.md#crop).
