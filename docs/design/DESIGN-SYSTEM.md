# GraphDraw — Design System

Versão 1.0 · 2026-10-16 · dono: responsável pelo produto

Este documento é a **fonte única** de como o GraphDraw parece e se comporta
na tela. Toda tela, painel, ícone, cursor, janela ou controle novo segue o
que está aqui. Se algo novo não couber nestas regras, o documento é
atualizado **antes** do código, no mesmo commit ou PR.

Relação com a especificação funcional (`docs/spec/`): a spec diz **o que**
o sistema faz; este guia diz **como aparece e responde**. Em caso de
conflito visual, vale este guia; em caso de conflito de comportamento, vale
a spec. Os IDs de requisito citados (ex.: GD-UI-01) apontam para a spec.

---

## 1. Princípios

1. **A arte é a protagonista.** A interface sustenta a criação e não
   compete com ela: controles sutis, precisos e visualmente silenciosos;
   o foco fica na página.
2. **Densidade profissional sem poluição.** Muitos controles à mão, mas
   compactos, com números de largura fixa e revelação progressiva (o
   avançado aparece quando é pedido).
3. **Resposta imediata.** Toda ação no canvas ou nos painéis tem retorno
   visual em menos de 16 ms (60 quadros por segundo ou mais).
4. **Mouse e teclado juntos.** Nenhuma ação essencial exige vários cliques
   em profundidade; comandos críticos têm atalho direto e, quando forem
   numéricos, ajuste contínuo por arrasto.
5. **Estrutura flexível.** Grade e espaçamento claros garantem alinhamento,
   com tolerância para módulos novos sem redesenhar a base.

---

## 2. Fundamentos (tokens)

Todo valor visual vem de um token. **Cor, espaçamento, raio, tamanho de
controle, espessura de traço da interface e tipografia escritos direto no
código são proibidos** fora do módulo de tema (seção 7).

Fica fora da regra a geometria própria de um componente que não se repete
em outro lugar (largura de um campo específico, largura de uma janela
modal, tamanho de uma prévia), desde que use os tokens para tudo o mais.
Testes também podem usar valores diretos (ex.: tela de 1366 × 768).

### 2.1 Espaçamento (base 4 px)

| Token | Valor | Uso |
|---|---|---|
| `space-050` | 2 px | Micro-ajustes, recuo interno de botões compactos, espessura de borda de seleção |
| `space-100` | 4 px | Mínimo entre ícone e rótulo; raio micro |
| `space-150` | 6 px | Padding vertical de campos numéricos densos |
| `space-200` | 8 px | Entre componentes irmãos de um grupo (botões de uma barra) |
| `space-300` | 12 px | Padding interno de cartões, menus e cabeçalhos de seção |
| `space-400` | 16 px | Entre blocos funcionais; padding de painéis laterais |
| `space-600` | 24 px | Margens de janelas modais e divisores principais |
| `space-800` | 32 px | Respiro em telas de boas-vindas e assistentes |

### 2.2 Cores — tema escuro (padrão)

Tons frios e neutros, com acentos fortes só para foco, seleção e alertas de
produção.

**Superfícies**

| Token | Valor | Uso |
|---|---|---|
| `bg-canvas` | `#101012` | Fundo infinito onde ficam as páginas |
| `bg-surface-0` | `#161619` | Barra de status e painéis de fundo |
| `bg-surface-1` | `#1E1E23` | Painéis laterais (propriedades, camadas, ferramentas) |
| `bg-surface-2` | `#27272E` | Campos de texto, botões secundários em repouso, cartões |
| `bg-surface-hover` | `#32323B` | Controle sob o mouse |
| `bg-surface-active` | `#3D3D48` | Botão pressionado, aba ativa |
| `bg-surface-toggled` | `#2A2D4A` | Ferramenta ou opção ligada |
| `page-paper` | `#FFFFFF` | A folha (página) no canvas |

**Bordas**

| Token | Valor | Uso |
|---|---|---|
| `border-subtle` | `#26262E`, 1 px | Divisão entre painéis |
| `border-default` | `#363642`, 1 px | Contorno de campos e botões secundários |
| `border-focus` | `#6366F1`, 1,5 px | Foco de teclado; nós vetoriais selecionados |

**Texto e ícones**

| Token | Valor | Uso |
|---|---|---|
| `fg-primary` | `#E2E2E8` | Texto padrão, rótulo de botão secundário |
| `fg-strong` | `#F4F4F6` | Ícone/texto sob o mouse |
| `fg-on-accent` | `#FFFFFF` | Texto sobre `accent-primary`; ícone pressionado |
| `fg-muted` | `#A0A0AB` | Ícone em repouso, texto secundário |

**Acentos e estados**

| Token | Valor | Uso |
|---|---|---|
| `accent-primary` | `#6366F1` (índigo) | Seleção de objetos e nós, aba ativa, ação principal |
| `accent-primary-hover` | `#7A7DF5` | Sobrevoo de elementos com acento |
| `accent-success` | `#10B981` (esmeralda) | Sangria correta, exportação concluída, medidas calculadas |
| `accent-warning` | `#F59E0B` (âmbar) | Linha de corte perigosa, objeto sem sangria, perfil de cor em conflito |
| `accent-danger` | `#EF4444` (coral) | Nó quebrado, exclusão definitiva, dado inválido |
| `guide-object` | `#EC4899` (magenta) | Guia inteligente: alinhamento a borda/centro de outro objeto |
| `guide-page` | `#06B6D4` (ciano) | Guia inteligente: alinhamento à página, margem ou sangria |

**Sombras**

| Token | Valor | Uso |
|---|---|---|
| `shadow-page` | deslocamento 0/8 px, desfoque 30 px, preto 35% | Folha sobre o canvas |
| `shadow-popover` | deslocamento 0/10 px, desfoque 25 px, recuo −5 px, preto 50% + contorno 1 px `border-default` | Menus, popovers, tooltips |

**Tema claro.** O tema escuro é a referência. O claro continua existindo
(GD-UI-04) com os **mesmos nomes de token**, os mesmos acentos, as mesmas
cores de guia e o mesmo contraste relativo. Muda só o que está abaixo:

| Token | Valor (claro) |
|---|---|
| `bg-canvas` | `#D9D9DF` |
| `bg-surface-0` | `#EDEDF1` |
| `bg-surface-1` | `#F6F6F8` |
| `bg-surface-2` | `#FFFFFF` |
| `bg-surface-hover` | `#E6E6EC` |
| `bg-surface-active` | `#D8D8E0` |
| `bg-surface-toggled` | `#E2E3FC` |
| `border-subtle` | `#DEDEE5` |
| `border-default` | `#C6C6D0` |
| `fg-primary` | `#26262E` |
| `fg-strong` | `#101012` |
| `fg-muted` | `#62626D` |
| `shadow-page` | mesma forma, preto 18% |
| `shadow-popover` | mesma forma, preto 20% |

`page-paper`, `border-focus`, `fg-on-accent`, acentos e guias são iguais
nos dois temas.

### 2.3 Tipografia

Fonte neo-grotesca de alta legibilidade (padrão: **Inter**, licença OFL,
embutida). Números em campos e réguas usam **algarismos de largura fixa**.

| Estilo | Tamanho / entrelinha | Peso | Uso |
|---|---|---|---|
| `type-display` | 18 / 24 px | Semibold | Títulos de janelas modais e de exportação |
| `type-section` | 13 / 18 px | Medium | Títulos de painel ("Camadas", "Cor") |
| `type-body` | 12 / 16 px | Regular | Rótulos de propriedades, descrições |
| `type-numeric` | 11 / 14 px | Mono | X/Y, largura/altura, graus, medidas |
| `type-micro` | 10 / 12 px | Regular | Atalhos de teclado, dicas |

**Números de largura fixa:** todo campo numérico, régua ou leitura de
medida usa `type-numeric`, para os valores não "tremerem" durante arrastos
e animações.

### 2.4 Raios de canto

| Token | Valor | Uso |
|---|---|---|
| `radius-none` | 0 px | Divisores de janela, réguas, barras de rolagem |
| `radius-xs` | 2 px | Alças de vetor (nós e âncoras) |
| `radius-sm` | 4 px | Botões compactos, campos, abas da barra de páginas |
| `radius-md` | 6 px | Menus flutuantes, popovers, tooltips, abas de documento |
| `radius-lg` | 10 px | Janelas modais e assistentes |
| `radius-full` | 9999 px | Pílulas de status, botões circulares flutuantes |

### 2.5 Movimento

| Token | Valor | Uso |
|---|---|---|
| `motion-fast` | 100 ms, curva cúbica | Entrada de menus e popovers (desloca 4 px + opacidade) |
| `motion-base` | 120–150 ms | Reposicionar abas ao arrastar; girar ícone de orientação |
| `tooltip-delay` | 400 ms | Atraso das dicas e das miniaturas ao repousar o cursor |

Nada de animação decorativa. Movimento só para explicar mudança de lugar ou
de estado.

---

## 3. Iconografia (desenhos próprios)

### 3.1 Grade e traço

| Contexto | Grade | Exibição |
|---|---|---|
| Ferramentas e barras da interface | 20 × 20 px | 20 px |
| Status interno e microcontroles | 16 × 16 px | 16 px |
| Hub de criação (Novo documento) | 24 × 24 px | 24 px |
| Cursores sobre o canvas | 24 × 24 px | 24 px |

- **Traço:** 1,5 px constante em todo o conjunto (medido na grade em que o
  ícone é exibido).
- **Terminações:** pontas e cantos arredondados (`round-cap`, `round-join`).
- **Margem óptica:** 1,5 px de respiro em volta da grade; o desenho ocupa a
  área interna (17 × 17 px na grade de 20), para ícones redondos, quadrados
  e triangulares terem o mesmo peso.

```
+--------------------+  <- limite do container (20 × 20 px)
|  . . . . . . . .   |
|  .  +--------+  .  |  <- limite óptico de desenho (17 × 17 px)
|  .  | ÍCONE  |  .  |
|  .  +--------+  .  |
|  . . . . . . . .   |
+--------------------+
```

- **Cor:** monocromáticos, desenhados em branco e coloridos pela interface
  (tint) com os tokens `fg-*` ou `accent-*`. Nunca cor fixa no arquivo,
  exceto preenchimentos parciais com opacidade para sugerir área.
- **Origem:** só desenhos próprios do projeto (regra 4 do `CLAUDE.md`).
  Imagens de referência servem para entender a ideia, nunca para copiar
  traços. Todo ícone sai do gerador único
  `crates/graphdraw-app/assets/icons/make-icons.mjs`, que também produz a
  folha de prova `icons-sheet.svg`; conferir a folha a cada ícone novo.

### 3.1.1 Marca e ícone do programa

A marca é a logo do projeto (`assets/brand/logo-original.jpg`): o G e o D
em faixa colorida que termina numa pena.
`scripts/brand-icons` gera os ícones a partir dela:
- `mark.png`: a marca sem fundo. Ela é separada do fundo pela saturação,
  sem emenda.
- `app-<n>.png` e `app.ico`: a marca ocupando 82% de largura num quadrado
  com o escuro do fundo da logo e cantos de 20%.
- `gdr-<n>.png` e `gdr.ico`: folha clara com dobra e o quadrado da marca
  (ícone do arquivo `.gdr`).
- Tamanhos: 16, 20, 24, 32, 40, 48, 64, 128, 256 e 1024 px.

No Windows, o `.exe` leva os dois ícones (`build.rs` com `windres`):
`app` no índice 0 e `gdr` no índice 1, para o instalador associar o
`.gdr`. A janela usa `app-256.png`. Os ícones de interface (3.1) seguem
monocromáticos; a marca colorida só aparece como ícone do programa e do
arquivo, e na janela Sobre.

### 3.2 Cursores

Sobre o canvas, o cursor mostra a ferramenta ativa (desenho próprio, grade
de 24 px, preto com halo branco de 1 px para aparecer no papel e no fundo
escuro). Ferramentas de desenho usam mira precisa com o ícone da ferramenta
ao lado. Alças de redimensionar usam as setas do sistema.

---

## 4. Componentes

### 4.1 Botões

**Tamanhos**

| Tamanho | Altura | Padding horizontal | Onde |
|---|---|---|---|
| Compact | 24 px | 6 px | Barras de ferramentas, réguas, abas densas |
| Default | 28 px | 10 px | Painéis laterais e inspetor |
| Wide | 32 px | 12 px | Painéis laterais na densidade Ampla (seção 6) |
| Prominent | 36 px | 16 px | Só a ação principal de uma janela ("Criar documento", "Exportar") |

**Variantes**

- **Primary:** fundo `accent-primary`, texto `fg-on-accent`. **Uma por
  grupo de comandos**, para a ação mais importante.
- **Secondary:** fundo `bg-surface-2`, contorno `border-default`, texto
  `fg-primary`. Ações neutras ou complementares.
- **Ghost / ferramenta:** transparente em repouso (estados abaixo).
- **Split:** ação direta no ícone + seta pequena (▾) à direita que abre as
  variantes da ferramenta (ex.: Bézier e mão livre).

**Estados do botão de ferramenta (ghost)**

| Estado | Fundo | Ícone | Extra |
|---|---|---|---|
| Repouso | transparente | `fg-muted` | — |
| Hover | `bg-surface-hover` | `fg-strong` | Dica após `tooltip-delay` |
| Pressionado | `bg-surface-active` | `fg-on-accent` | Desce 0,5 px |
| Ligado | `bg-surface-toggled` + borda `border-focus` | `accent-primary` | Ferramenta selecionada |

### 4.2 Campos numéricos com arrasto

Largura, altura, ângulo e posição (W, H, X, Y…) são controles, não só caixas
de texto.

```
┌───────────────┐
│ W  148.00 mm  │   repouso
└───────────────┘
      ◄────►        arrastar o rótulo
┌───────────────┐
│ W ↔152.40 mm  │   arrastando (cursor de seta dupla)
└───────────────┘
```

- **Arrasto no rótulo** (W, H, X…) **ou na unidade** (`mm`, `°`):
  esquerda/direita altera o valor aos poucos (0,1 mm ou 0,5 por pixel);
  **Shift** = passo 10×; **Alt** = passo 0,1× (micro-precisão). O arrasto
  inteiro é um passo só de desfazer.
- **Contas no valor:** clicar no número abre a edição e aceita expressões
  (`100 * 2`, `297 / 3`, `50 + 12.5mm`) — GD-TRF-04 / GD-UI-11.
- **Unidades:** digitar `10 in` num documento em mm converte sem erro.
- Valor em `type-numeric`; raio `radius-sm`; fundo `bg-surface-2`; borda
  `border-default`, `border-focus` com foco.

### 4.3 Menus suspensos e popovers

- Sombra `shadow-popover`; raio `radius-md`; fundo `bg-surface-1`.
- Entrada com `motion-fast`.
- Itens com 26 px de altura: ícone à esquerda, rótulo alinhado à esquerda,
  atalho em `type-micro` alinhado à direita.

**Menu Texto** (barra de menus, entre Objeto e Exibir): Ferramenta Texto;
Negrito, Itálico, Aumentar/Diminuir a fonte; os quatro alinhamentos;
Converter artístico ↔ parágrafo, Ajustar texto ao caminho, Separar do
caminho, Converter texto em curvas. Comandos de texto não ficam no menu
Objeto.

**Menu do botão direito** (doc 26): mesmo visual dos menus suspensos,
largura mínima `context-menu-width` = 200 px, aberto no ponto do clique e
mantido dentro da janela. Conteúdo pelo alvo: objeto, área vazia, nó,
segmento ou soltura de arrasto direito. Item que ainda não existe (Beta)
aparece desabilitado com o motivo na dica — nunca some. Clique fora ou Esc
fecha sem efeito. Clique é soltar com menos de 4 px de movimento e menos de
0,5 s; toque longo no canvas não abre nada.
Cada item tem ícone de 16 px (`icon-micro`) na cor do texto, à esquerda do
rótulo. A função que já tem ícone em outro lugar usa o mesmo, por exemplo:
- `to-curves`, `group`/`ungroup`, `order-*`, `lock-on`/`lock-off`
  e `eye-off`;
- os de nó e de segmento;
- `bool-weld`/`trim`/`intersect` e `duplicate` (Copiar para cá).

Os ícones próprios do menu ficam no grupo “menu do botão direito” do
gerador. O teste `every_menu_item_has_an_icon` garante que nenhum item fica
sem ícone.

**Objeto bloqueado** (GD-BDR-45): o clique esquerdo passa reto por ele. Só
o botão direito chega nele e abre Desbloquear e Ordem. Abrir o menu não muda
a seleção; o comando escolhido vale para o objeto bloqueado.

### 4.4 Janelas (modais)

- Raio `radius-lg`; margens `space-600`; título em `type-display`.
- **Nunca maiores que a tela:** altura limitada (até 75% da área) com
  rolagem interna; nenhum elemento do layout pode pedir "toda a altura
  disponível" dentro de um modal. Todo modal novo tem teste de layout que
  confirma que cabe numa tela de 1366 × 768.
- Botões no rodapé, à direita: Cancelar (secondary) e a ação (primary).
  `Esc` cancela.
- **Converter em bitmap** (GD-CNV-03): resolução em ppp (10–2400, padrão
  300), Suavizar bordas e Fundo transparente (senão branco); Converter /
  Cancelar.
- **Janela de contorno, seção Caneta caligráfica** (GD-OUT-06): largura do
  bico (%, 100% = redondo) e ângulo (habilitado abaixo de 100%).
- **Janela de contorno, seção Pontas de seta** (GD-OUT-05): uma linha para
  o início e outra para o fim, com a lista de formas (`arrow-combo-width` =
  96 px; Nenhuma, Triângulo, Aberta, Farpada, Círculo, Quadrado, Barra) e,
  com forma escolhida, tamanho (%), giro (°), Espelhar e afastamento.

### 4.5 Abas

- **Documento** (topo da área de trabalho), no padrão de abas de
  navegador (desenho próprio, sem copiar nenhum produto):

  ```
   ╭──────────────╮
   │ ▢ Cartaz • × │  ▢ Folheto   × │ ▢ Logo    ×    +
  ─╯              ╰───────────────────────────────────────
  ```

  - Faixa das abas em `bg-surface-0`, altura `tab-doc-height` = 32 px
    (28 px de aba + `space-100` em cima).
  - **Aba ativa:** fundo `bg-surface-1`, cantos de cima com `radius-md` e
    cantos de baixo curvados **para fora** (raio `radius-md`), para a aba
    se fundir com a linha de base; texto `fg-strong`.
  - **Abas inativas:** sem fundo; no hover, a mesma forma em
    `bg-surface-hover`; texto `fg-muted`.
  - **Divisor:** linha vertical de 1 px `border-default`, com metade da
    altura da aba, entre abas inativas; some ao lado da ativa e da que
    está sob o mouse.
  - **Conteúdo:** ícone de documento de 16 px, título cortado com
    reticências, `•` quando há alterações não salvas, `×` de 16 px à
    direita (círculo `bg-surface-active` no hover). O `×` aparece sempre na
    ativa e nas outras só quando a aba tem largura para isso.
  - **Largura:** 220 px; com muitas abas, todas encolhem por igual até
    72 px; abaixo disso, a faixa rola na horizontal.
  - **"+"** logo depois da última aba: botão circular ghost de 24 px.
  - Clique ativa; clique do meio ou `×` fecha; dica com o caminho do
    arquivo.
- **Página** (abaixo do desenho): raio `radius-sm`, faixa de 3 px em cima
  com a etiqueta de cor, `*` para alterações não salvas, cadeado se
  bloqueada.

### 4.6 Barra de propriedades (barra superior contextual)

- **Altura fixa:** `bar-property-height` = 72 px (duas linhas de controles
  Compact de 24 px, `space-150` em cima e embaixo e `bar-row-gap` =
  `space-300` entre elas). A altura **nunca muda**, seja qual for a
  ferramenta, a seleção ou a quantidade de controles mostrados; trocar de
  contexto não pode empurrar o canvas para cima ou para baixo.
- **Empilhar:** pares relacionados ficam um sobre o outro (X/Y, L/A,
  espelhar H/V, ordem 2 × 2, agrupar/desagrupar, fonte/estilo e corpo,
  negrito/itálico, alinhamentos 2 × 2). Controles soltos ficam centrados
  na altura.
- **Sem quebra de linha:** nada quebra linha. Se os controles não cabem na
  largura, a barra rola na horizontal (roda do mouse ou arrasto; barra de
  rolagem oculta) e um degradê na borda indica que há mais conteúdo.
- Textos de ajuda longos (dicas da ferramenta) são cortados com reticências
  em uma linha; o texto completo aparece na dica (`tooltip-delay`).
- Controles da barra em tamanho Compact; grupos separados por divisor
  vertical, com `space-200` entre irmãos. Controle composto (ex.: seletor
  de âncora 3 × 3) também cabe em 24 px de altura: não usar grade de
  linhas do toolkit, que dá a cada linha a altura mínima de controle.
- Todo contexto novo da barra tem teste de layout que confirma a altura de
  72 px em 1366 × 768.

**Ordem fixa (regra).** Com seleção, a barra sempre começa pelo bloco de
posição e nada entra antes dele nem no meio dele:

1. **Bloco fixo:** ponto de referência (3 × 3) · X/Y · L/A · travar
   proporção · ângulo de rotação.
2. Depois do ângulo, os grupos do contexto, nesta ordem, cada um separado
   por divisor e só quando se aplica:
   1. espelhar H/V;
   2. **parâmetros do objeto** (converter em curvas; cantos do retângulo;
      polígono/estrela; elipse/arco/fatia; texto: fonte, estilo, corpo,
      negrito/itálico, alinhamento e texto em caminho);
   3. **contorno**: espessura e estilo de linha (ou "Sem contorno"); pontas
      de seta início/fim quando há curva aberta; caneta caligráfica (bico
      % e ângulo) quando há contorno;
   4. **combinar** (2 ou mais objetos): as 9 booleanas;
   5. **ordem** e **agrupar/desagrupar**.

Recurso novo na barra entra em um desses grupos (ou num grupo novo depois
deles), nunca antes do ângulo.

**Barra × janela (regra).** A barra serve para edição rápida, sem abrir
janela: só valores que se mudam com um clique ou um arrasto e que valem
para a maior parte dos objetos. A janela fica para o ajuste fino e para as
opções raras. Todo recurso novo decide, na especificação, o que vai à
barra (no máximo os 2–4 controles mais usados) e o que fica só na janela.

| Recurso | Na barra | Só na janela |
|---|---|---|
| Posição e tamanho | ponto de referência, X/Y, L/A, travar, ângulo | Transformação (posição relativa, cópias, escala %) |
| Contorno | espessura, estilo de linha, "Sem contorno" | cor, junção, terminação, limite de canto, posição, atrás do preenchimento, escalar com objeto, traço personalizado |
| Pontas de seta | forma no início e no fim | tamanho, afastamento, giro, espelho |
| Caneta caligráfica | bico % e ângulo | — |
| Preenchimento | — (cor pela paleta; gradiente pela ferramenta G) | tipo, paradas, opacidade, sobreimpressão, ângulo, repetir |
| Retângulo | cantos escolhidos, tipo e raio | — |
| Polígono/estrela | modo, lados/pontas, nitidez | — |
| Elipse | elipse/arco/fatia, ângulos | — |
| Texto | fonte, estilo, corpo, negrito, itálico, alinhamento; texto em caminho: início, distância, lado, orientação, espelho | (menu Texto) |
| Booleanas | as 9 (2 ou mais objetos) | — |
| Aparência | — | painel Camadas: opacidade, mesclagem, dados do objeto |
| Gradiente (ferramenta G) | linear/radial na barra da ferramenta | paradas: cor e opacidade no balão da parada |

### 4.7 Painel de camadas e objetos

- Cada camada é um recolhível (aberto por padrão) com olho, impressão,
  cadeado e nome; os objetos ficam **dentro** da camada, do topo (frente)
  para baixo. Grupos são recolhíveis (fechados por padrão) com os filhos.
- **Linha de objeto:** olho e cadeado (microcontroles de 16 px), miniatura
  1:1 de `object-thumb` = 24 px (fundo `page-paper`, borda 1 px
  `border-subtle`, raio `radius-xs`, o objeto inteiro centrado com 1 px
  de margem) e o nome — o dado pelo usuário ou o tipo.
- Clique seleciona, Shift soma/tira; duplo clique no nome abre o campo de
  renomear (Enter ou sair confirma, Esc cancela; vazio volta ao tipo).
- **Arrastar e soltar:** a linha de destino mostra uma linha de 2 px
  `accent-primary` em cima (fica na frente) ou embaixo (fica atrás);
  soltar na linha da camada mostra a moldura de 2 px e põe o objeto no topo
  dela. Entrar ou sair de grupo e trocar de camada mantêm a aparência.
- **Botão direito** no objeto (no painel ou no canvas): menu de edição
  rápida com o campo Nome (`menu-field-width` = 160 px, Enter confirma),
  Travar/Destravar e Ocultar/Mostrar.
- **Texto em caminho** (GD-TXT-09), na barra de texto quando o texto corre
  num caminho: Início e Dist. (comprimentos), Acima/Abaixo, orientação
  (lista de 150 px: Girar letras, Inclinar vertical, Inclinar horizontal,
  De pé) e Espelhar.
- **Aparência** (GD-TRA-02), abaixo da árvore com seleção: opacidade do
  objeto (%) e modo de mesclagem (lista de `blend-combo-width` = 150 px).
- **Dados do objeto** (GD-OBJ-05), abaixo da árvore com um objeto
  selecionado: recolhível com uma linha por par (chave, campo de valor de
  `user-data-field-width` = 90 px, botão de tirar) e uma linha para o par
  novo (chave e valor com dica, botão de adicionar ou Enter). O valor é
  gravado ao sair do campo; cada alteração é um passo de desfazer.

### 4.8 Caixa de ferramentas e gavetas

- Ferramentas parecidas formam um **grupo**: o botão mostra a escolhida e
  tem um triângulo de `tool-drawer-mark` = 5 px em `fg-muted` no canto
  inferior direito. Clique no botão usa a ferramenta mostrada; clique no
  canto (`tool-drawer-hit` = 9 px) ou botão direito abre a **gaveta**: as
  ferramentas do grupo em linha, ao lado do botão, em `shadow-popover`.
  Escolher na gaveta usa e passa a mostrar aquela; clique fora ou Esc fecha.
- Grupos: **Retângulo** (2 pontos, 3 pontos; duplo clique no botão cria um
  retângulo do tamanho da página), **Elipse** (2 pontos, 3 pontos; cada
  ferramenta guarda o seu modo, o mesmo da barra de propriedades), **Formas** (polígono, estrela, estrela complexa, espiral, papel
  gráfico — a família sai da barra de propriedades) e **Preenchimento**
  (interativo/gradiente, uniforme, conta-gotas).
- Conta-gotas: com seleção, copia o preenchimento e a cor do contorno do
  objeto clicado para ela (a espessura do destino fica); sem seleção, vira
  o estilo dos novos objetos.

---

## 5. Canvas e controles sobre a arte

### 5.1 Alças e nós

Tamanho constante em pixels de tela, qualquer que seja o zoom.

| Elemento | Forma | Medidas e cores |
|---|---|---|
| Âncora (canto) | quadrado | 6 × 6 px, fundo `#FFFFFF`, borda 1,5 px `accent-primary`, raio `radius-xs` |
| Nó de canto | quadrado | 6 × 6 px, borda 1,5 px `accent-primary`, raio `radius-xs` |
| Nó de curva suave | círculo | 6 px de diâmetro, borda 1,5 px `accent-primary`; linha da alça 1 px `accent-primary` |
| Alça de ângulo | ○—○ | comprimento livre; Shift prende a cada 15° |
| Caixa de seleção | retângulo | linha sólida 1 px `accent-primary`, alças nos cantos e meios |
| Balão da parada | clique numa parada ou na ponta do eixo | `shadow-popover`, largura `stop-popup-width` = 240 px: “Parada N · posição”, editor de cor no modelo da cor, opacidade %, OK; clique fora ou Esc fecha; o que muda enquanto aberto é um passo de desfazer |
| Eixo do gradiente | ■—●—■ | linha 1 px `accent-primary` sobre 2,5 px branco; início/fim em quadrado de 8 px (`handle-size` + 2); paradas em círculo de `gradient-mark` (10 px) com a cor da parada; parada escolhida com anel `focus-width` `accent-primary` |

Nós não selecionados têm fundo `#FFFFFF`; selecionados, fundo
`accent-primary`. Sobreposições de seleção usam o acento com transparência:

| Token | Valor | Uso |
|---|---|---|
| `selection-object` | `accent-primary` 43% | Contorno de cada objeto da seleção |
| `selection-fill` | `accent-primary` 10% | Fundo do retângulo elástico |
| `grid-line` | `accent-primary` 16% | Linhas da grade |

**Arrasto com o botão direito** (GD-BDR-20): a seleção não se move; um
fantasma em `selection-object` segue o cursor e o objeto sob ele ganha
contorno `accent-primary` de `drop-target-width` = 2 px. Ao soltar abre
o menu de soltura (Mover para cá, Copiar para cá, Copiar tudo/contorno/
preenchimento, booleanas, Centralizar no alvo, Cancelar).

**Modo foco** (GD-BDR-82): só a seleção aparece no canvas; a barra de
status mostra o botão “◉ Modo foco — sair”; Esc também sai.

**Conta-gotas** (doc 27):
- Cursor: pipeta (`tool-eyedropper`) para coletar; balde
  (`tool-fill-solid`) para despejar. Com Shift, volta à pipeta.
- **Lupa** em `shadow-popover`, a `eyedrop-offset` = 24 px do cursor, do
  outro lado quando chega na borda. Conteúdo:
  - grade de 11 × 11 pixels físicos em células de `eyedrop-cell` = 10 px,
    com linhas de 1 px em preto 11%;
  - retículo do tamanho do raio: traço branco de `focus-width` e borda
    preta de 1 px;
  - HEX em negrito; CMYK, RGB e o nome da paleta em `type-micro`;
  - o balde e os últimos `eyedrop-recent` = 5 tons em círculos de
    `eyedrop-dot` = 14 px.
- A lupa para depois de `eyedrop-dwell` = 0,4 s parado e volta a seguir
  quando o cursor sai de `eyedrop-release` = 40 px em volta dela.
- **Realce de aplicação**: contorno `accent-primary` de
  `eyedrop-ripple-width` = 2 px na caixa do destino. Ele cresce
  `eyedrop-ripple-grow` = 6 px e some em `eyedrop-ripple` = 150 ms.
- **Paradas para o balde**: círculos de `gradient-mark` com a cor da
  parada e borda branca de `focus-width`.
- **Barra**:
  - Cor | Atributos;
  - raio (Vetor nativo, 1×1, 3×3, 5×5) ou os filtros
    (Preenchimento, Contorno, Efeitos — desabilitado, Tipografia,
    Transformações);
  - o balde, Esvaziar e a dica dos gestos.

### 5.2 Guias inteligentes e encaixe

- Alinhamento a borda/centro de outro objeto: linha contínua de 1 px
  `guide-object`.
- Alinhamento à página, margem ou sangria: linha contínua de 1 px
  `guide-page`.
- Espaçamento igual: marcadores em barra com a distância ao vivo (ex.:
  `12 mm`).
- Valem ao mover, ao redimensionar (a alça encaixa; no canto proporcional
  manda o eixo que encaixou) e ao desenhar (o canto final encaixa).
- **Raio de atração: 6 px de tela.**

### 5.3 Página

- **Lado a lado** (GD-PAG-04, menu Exibir): a página anterior à esquerda e
  a seguinte à direita, a `spread-gap` = 24 px da ativa, alinhadas pelo
  topo, com `shadow-page`, véu `spread-dim` (preto 16%) e o nome acima em
  `type-micro` `fg-muted`. Só para ver; clicar numa delas vai para ela.

- Folha `page-paper` com `shadow-page` sobre `bg-canvas`.
- Sangria: contorno tracejado na distância da sangria com marcas de corte
  nos cantos; `accent-success` quando correta, `accent-warning` quando há
  objeto perto da borda sem chegar à sangria (doc 22).

---

## 6. Responsividade e adaptação

- **Painéis recolhíveis:** toda barra lateral recolhe por duplo clique na
  aba ou no botão de recolher, virando uma faixa de ícones de 36 px
  (`panel-rail`); clicar num ícone da faixa abre a coluna naquela aba.
- **Painéis soltos:** botão de soltar (ou botão direito na aba) leva o
  painel para uma janela própria, de `panel-float-size` = 260 × 360 px;
  fechar a janela devolve o painel à coluna.
- **Largura da coluna:** `panel-width` = 260 px inicial, arrastável entre
  `panel-min-width` = 200 px e `panel-max-width` = 520 px.
- **Densidade** (*Exibir → Densidade dos painéis*, salva na área de
  trabalho; vale para os painéis laterais e janelas de painel solto — a
  barra de propriedades, as barras de ferramentas e os menus não mudam):
  - *Compacta:* controles de 24 px (notebooks de 13").
  - *Padrão:* controles de 28 px (Default).
  - *Ampla:* controles de 32 px e espaçamento entre itens de
    `space-300` × `space-200` (tablets, toque).
- **Área de trabalho lembrada:** posição, largura e estado de painéis e
  ferramentas são salvos no perfil do usuário
  (`area-de-trabalho.json`) e restaurados ao abrir; *Exibir → Restaurar
  área de trabalho padrão* volta ao início.
- Funciona de notebook até 4K ultrawide sem layout quebrado; textos e
  ícones respeitam a escala do sistema (GD-UI-04: 100–300%).

---

## 7. Implementação (egui)

- **Um módulo de tema** (`crates/graphdraw-app/src/theme.rs`) concentra os
  tokens deste documento como constantes com os mesmos nomes (ex.:
  `theme::ACCENT_PRIMARY`, `theme::SPACE_200`) e aplica o `Style`/`Visuals`
  do egui. Nenhum outro arquivo escreve cor, espaçamento ou raio direto.
- **Números de largura fixa:** o egui não aplica `tabular-nums`; os campos
  numéricos usam a fonte monoespaçada no estilo `type-numeric`.
- **Fonte:** Inter embutida (OFL; registrar em `THIRD_PARTY.md`).
- **Sombras:** `egui::epaint::Shadow` com os valores dos tokens. O egui
  não aceita recuo negativo (`spread` é sem sinal): `shadow-popover` usa
  recuo 0.
- **Fonte monoespaçada** de `type-numeric`: a que vem embutida no egui
  (Hack, licença MIT), já registrada junto do egui.
- **Limitação conhecida:** o egui não faz desfoque translúcido de fundo
  (blur). Os painéis usam a cor sólida do token.
- **Cursores próprios:** o sistema operacional não recebe imagens próprias
  pelo egui; sobre o canvas o cursor do sistema é escondido e o da
  ferramenta é desenhado numa camada por cima (seção 3.2).

---

## 8. Conformidade atual (2026-10-16)

**Já segue o documento:** módulo `theme.rs` com todos os tokens e os
temas escuro (padrão) e claro (menu *Exibir → Tema*); Inter embutida e
fonte monoespaçada nos campos numéricos; acento `accent-primary` na
seleção; alças e nós de 6 px com borda de 1,5 px; marca de encaixe em
`guide-object`/`guide-page` com raio de 6 px; ícones na grade de 20 (16
nos microcontroles, 24 nos cursores) com traço de 1,5 px; botões de
ferramenta ghost Compact de 24 px com os quatro estados; botões primário
e secundário nos rodapés das janelas; modais com raio `radius-lg` e margem
`space-600`; barra de propriedades com altura fixa de 72 px (teste de
layout para página, seleção e nós); arrasto no rótulo dos campos
numéricos; dicas com 400 ms; itens de menu com 26 px; `shadow-page` na
folha.

Itens abertos, em ordem de impacto:

| Item | Hoje | Alvo |
|---|---|---|
| Entrada animada de menus | padrão do egui (só opacidade) | deslocamento de 4 px + opacidade em `motion-fast` |
| Botão Split (▾ com variantes) | não existe | quando houver ferramenta com variantes (4.1) |

Ao corrigir um item, atualizar esta tabela no mesmo commit.

---

## 9. Checklist para qualquer coisa nova

- [ ] Usa só tokens deste documento (nenhum valor visual direto no código).
- [ ] Ícones e cursores novos saíram do gerador, na grade certa, traço
      1,5 px, e foram conferidos na folha de prova.
- [ ] Botão no tamanho e na variante certos; no máximo uma ação primária
      por grupo.
- [ ] Campos numéricos em `type-numeric`, com contas, unidades e arrasto.
- [ ] Janela nova cabe em 1366 × 768 (teste de layout).
- [ ] Contexto novo na barra de propriedades mantém a altura de 72 px.
- [ ] Toda ação tem retorno visual imediato; as essenciais têm atalho.
- [ ] Comando novo com atalho entra como `Command` em
      `crates/graphdraw-app/src/keymap.rs`, com atalho nos dois perfis
      (GraphDraw e Alternativo) quando couber, e aparece sozinho em *Ajuda →
      Atalhos de teclado*. Nunca testar tecla direto na interface; rótulos de
      menu e dicas usam `keymap::menu`/`keymap::hint` para mostrar o atalho
      do perfil ativo.
- [ ] Textos próprios, em português e inglês (`tr`).
- [ ] Nada copiado de produto de terceiros (desenho, texto ou layout).
- [ ] Se nada aqui cobre o caso: atualizar este documento primeiro.
