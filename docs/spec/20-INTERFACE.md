# 20 — Interface

Identidade **própria**: nome, ícones, cores, layout e textos criados pelo projeto. Não reproduzir aparência de outros produtos. O objetivo é que usuários migrando de outros editores encontrem conceitos familiares (ferramenta à esquerda, paleta de cores, painéis à direita) — convenções genéricas do setor —, não telas copiadas.

## Estrutura
- **GD-UI-01** Janela principal: barra de menus, barra de ferramentas padrão, barra de propriedades contextual (muda conforme ferramenta/seleção), caixa de ferramentas vertical, área de desenho com abas por documento, painéis encaixáveis (direita), barra de paleta, barra de status (coordenadas do cursor, info da seleção, preenchimento/contorno atual).
- **GD-UI-02** Painéis MVP: Objetos (árvore páginas/camadas/objetos), Propriedades (preenchimento/contorno/transparência do selecionado), Transformação, Alinhar e distribuir, Paletas, Histórico, Relatório de importação.
- **GD-UI-03** Painéis flutuantes ou encaixados, agrupáveis em abas; layout salvo por perfil de área de trabalho.
- **GD-UI-04** Tema claro e escuro; escala de UI para telas de alta densidade (100–300%).

## Interação
- **GD-UI-10** Atalhos de teclado configuráveis; esquema padrão próprio documentado; importação de mapa de atalhos genérico (JSON); botão "Restaurar padrão" volta ao perfil de fábrica. A tecla gravada na janela não executa o comando nem reaciona a janela; Ctrl+C/X/V funcionam e podem ser remapeados como qualquer outro atalho.
  - Perfis prontos: **GraphDraw** (padrão de fábrica, próprio) e **Alternativo** (lista de teclas de função e letras de alinhamento definida pelo responsável pelo produto em 2026-10-16). O perfil escolhido fica salvo no perfil do usuário e vale sempre. Editar qualquer atalho cria o perfil Personalizado; importar/exportar em JSON.
  - Um mesmo atalho pode ter um comando durante a edição de texto e outro fora dela (ex.: Ctrl+R repetir × alinhar texto à direita).
  - Atalhos de ferramentas ainda inexistentes ficam reservados e avisam na barra de status.
- **GD-UI-11** Entrada numérica em qualquer campo aceita expressões e unidades (doc 08 GD-TRF-04).
- **GD-UI-12** Menu de contexto (botão direito) com ações relevantes ao objeto sob o cursor.
- **GD-UI-13** Prévia ao vivo ao alterar parâmetros (antes de confirmar).
- **GD-UI-14** Mensagens de erro e avisos em linguagem simples, com ação sugerida.

## Idiomas
- **GD-UI-20** Interface internacionalizada desde o início (arquivos de tradução); idiomas iniciais: pt-BR e en.

## Acessibilidade
- **GD-UI-30** Navegação completa por teclado nos painéis; contraste WCAG AA; leitores de tela nos diálogos.
