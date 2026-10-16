# 21 — Barra de páginas em abas

Origem: requisito do responsável pelo produto (2026-10-16). Complementa
GD-PAG-01..05 (doc 02) e GD-UI-01 (doc 20).

## Layout
- **GD-PGT-01** Barra de páginas logo acima da barra de status, com, da
  esquerda para a direita: botão Grade (visão geral), botão Lista (busca),
  anterior, campo "Pág. [n] / total", próxima, abas `[n] Nome` e botão `+`.
- **GD-PGT-02** Roda do mouse sobre as abas rola a barra na horizontal.

## Índice e nome
- **GD-PGT-03** O número da aba é a posição da página (1..N) e muda ao
  reordenar; o nome nunca muda por reordenação.
- **GD-PGT-04** Renomear no próprio rótulo: duplo clique ou `F2`; `Enter`
  confirma, `Esc` cancela. Sem janela.
- **GD-PGT-05** Marcador na aba da página com alterações não salvas (o
  marcador some ao salvar e ao desfazer até o estado salvo).

## Navegação
- **GD-PGT-06** Campo numérico: clicar ou `Ctrl+J` foca; número + `Enter` salta.
- **GD-PGT-07** `PageUp`/`PageDown` = anterior/próxima; `Home`/`End` =
  primeira/última (fora de campos de texto).
- **GD-PGT-08** Lista: janela suspensa com campo de busca por nome e lista
  numerada; clicar salta.
- **GD-PGT-09** Grade: visão geral com miniaturas de todas as páginas;
  clicar salta.
- **GD-PGT-10** Miniatura ao repousar o cursor (≈ 0,5 s) sobre uma aba inativa.

## Gerência
- **GD-PGT-11** `+` e `Ctrl+Shift+N` criam página após a ativa, com o
  tamanho da ativa.
- **GD-PGT-12** Arrastar aba reordena (renumeração ao vivo). Com várias abas
  selecionadas, o bloco se move junto, mantendo a ordem relativa.
- **GD-PGT-13** `Alt` + arrastar aba duplica a página na posição solta, com
  nome `<nome> (Cópia)` e IDs novos.
- **GD-PGT-14** Seleção múltipla de abas: `Ctrl+clique` avulsas, `Shift+clique`
  intervalo; vale para excluir, mover e exportar.
- **GD-PGT-15** Excluir (menu, `Delete` na barra ou botão do meio do mouse):
  página vazia sai na hora; com objetos, confirmação
  "Excluir a página X com Y objetos? Pode ser desfeito com Ctrl+Z". Sempre
  desfazível. A última página não pode ser excluída.

## Parâmetros por página (menu de contexto da aba)
- **GD-PGT-16** Configurar página: largura, altura, orientação
  (retrato/paisagem troca as medidas), sangria própria (opcional; sem ela
  vale a do documento, GD-PAG-02). Perfil de cor por página fica para quando
  houver gerenciamento de cor (GD-CMS, M2).
- **GD-PGT-17** Duplicar página; renomear; excluir.
- **GD-PGT-18** Bloquear página: nada nela pode ser selecionado ou editado
  (nem objetos novos) até desbloquear. A aba mostra um cadeado.
- **GD-PGT-19** Exportar esta página (PDF, SVG, PNG); com várias abas
  selecionadas, exportar a seleção em um PDF.
- **GD-PGT-20** Etiqueta de cor da aba (ex.: verde = aprovado, laranja =
  rascunho), salva no documento.

## Página mestra
- **GD-PGT-21** Aba `[0] Mestra`: o conteúdo da página mestra (GD-CAM-02)
  aparece como fundo travado em todas as páginas que não a ocultam.

## Aceite
- Mover a página "Capa" da posição 2 para a 5 → aba `[5] Capa`; desfazer →
  `[2] Capa`.
- Alt+arrastar "Base" para depois da última → nova aba `[N+1] Base (Cópia)`
  com o mesmo número de objetos e IDs diferentes.
- Excluir página com 3 objetos pede confirmação; Ctrl+Z restaura os 3.
- Editar um objeto da página 2 → só a aba 2 ganha o marcador; salvar limpa.
- Página 1 em A4 retrato e página 2 com 1080×1080 px → exportação PDF com
  dois tamanhos de página diferentes.
