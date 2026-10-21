# 31 — Barra de Propriedades: regras, contextos e controles

Sincronizado de `../spec/28` v1 (2026-10-19) a pedido do responsável pelo
produto; renumerado para 31 porque o 28 deste repositório é a verificação
prévia. As regras gerais GD-BPR-01..10 vêm de `../spec/27` §14.0, das quais
este doc depende. Havendo conflito com o DESIGN-SYSTEM, vale este doc e o
DESIGN-SYSTEM é atualizado (seção 4.6).

## 0. Regras gerais (todas as ferramentas)


> Lista exata de controles por contexto: seções 1–9 abaixo (GD-BPC-*).

- **GD-BPR-01 Contexto.** O conteúdo é escolhido pela chave `(janela, ferramenta ativa, tipo da seleção, tarefa)`. Tarefa = modo especial que sobrepõe a ferramenta: edição de texto, prévia de impressão (doc 16 GD-IMP-32), dados variáveis (GD-IMP-50). Troca de contexto atualiza a barra no mesmo quadro, sem piscar nem mudar a altura.
  - Aceite: ferramenta Seleção com retângulo selecionado → X/Y/L/A; clicar no vazio → controles de página; ativar Texto → controles tipográficos.
- **GD-BPR-02 Seleção mista.** Vários objetos de tipos diferentes → só controles comuns a todos. Campo cujo valor difere entre objetos fica vazio com dica "vários"; digitar aplica a todos.
- **GD-BPR-03 Aplicação.** Campo numérico aplica com `Enter`, `Tab` ou ao perder foco; `Esc` restaura o valor. Setas ↑/↓ ajustam pelo passo do campo (`Shift` ×10). Cada aplicação = uma entrada de desfazer (doc 12; nada agrupado).
- **GD-BPR-04 Unidade no campo.** Valor e unidade no mesmo campo (`12,5 mm`), aceitando expressões e outra unidade (`1in` → convertido). Ajuste contínuo por arraste só sobre o ícone do campo.
- **GD-BPR-05 Sem seleção vs. padrão.** Alterar atributo com ferramenta de criação ativa e nada selecionado altera o **padrão** daquela ferramenta para novos objetos (confirmação na primeira vez, com "não perguntar de novo").
- **GD-BPR-06 Posição.** Encaixa em qualquer um dos 4 lados da janela ou flutua; arrastar pela alça. Opção *Bloquear barras* impede mover. Posição salva no perfil de área de trabalho (doc 20 GD-UI-03).
- **GD-BPR-07 Personalização por contexto.** Botão **⋯** no fim da barra: lista com caixas de marcação dos controles daquele contexto (mostrar/ocultar) + *Adicionar comando…* (qualquer comando do catálogo, com seu ícone) + *Restaurar padrão deste contexto*. Item adicionado aparece **só** no contexto em que foi adicionado. Reordenar arrastando com `Alt`.
  - Aceite: ocultar "Rotação" na barra da Seleção → some só ali; ativar Retângulo → barra do Retângulo inalterada.
- **GD-BPR-08 Largura insuficiente.** Grupos que não cabem vão para o botão **»** (ordem de corte: da direita para a esquerda, grupos marcados como essenciais por último). Nunca quebra em duas linhas.
- **GD-BPR-09 Mostrar/ocultar.** Menu *Janela › Barras › Barra de propriedades*. Oculta → atalhos e painéis continuam funcionando.
- **GD-BPR-10 Ícones.** Todo controle com ícone igual ao do mesmo comando no menu, menu de contexto e atalho; dica (tooltip) com nome + atalho.


Notação: `#` campo numérico com unidade · `▾` lista · `☐` alternância (liga/desliga ou grupo exclusivo) · `|` separador de grupo. Ordem = esquerda → direita.

---

## 1. Blocos reutilizáveis

Contextos montam a barra a partir destes blocos + controles próprios. Bloco = unidade que a personalização (GD-BPR-07) mostra/oculta de uma vez ou item a item.

| ID | Bloco | Controles |
|---|---|---|
| **B-TRF** | Transformação | âncora de referência (9 pontos) · X/Y `#` | L/A `#` · escala X/Y % `#` · travar proporção ☐ | ângulo `#` | espelhar horizontal · espelhar vertical |
| **B-CTN** | Contorno | espessura ▾ (lista + digitação) · estilo de traço ▾ |
| **B-CTV** | Contorno variável | ☐ abre ferramenta de largura variável no objeto |
| **B-SET** | Setas | ponta inicial ▾ · ponta final ▾ |
| **B-CUR** | Opções de curva | suavização à mão livre (deslizante 0–100) · fechar curva ☐ · separar · caixa de limites ☐ · desenho paralelo ☐ |
| **B-ORG** | Organização | contornar texto de parágrafo ▾ · trazer para frente da camada · enviar para trás da camada |
| **B-FOC** | Foco | entrar no grupo (isola para edição) · sair do foco |
| **B-PRE** | Predefinições de efeito | lista ▾ · adicionar · remover |
| **B-EFX** | Fim de efeito | copiar efeito de outro objeto · remover efeito · converter em curvas (quando aplicável) |
| **B-CNV** | Converter | converter em curvas |
| **B-FON** | Fonte | família ▾ · tamanho ▾ · eixos de fonte variável | negrito ☐ · itálico ☐ · sublinhado ☐ |
| **B-PEN** | Caneta (tablet) | usar pressão ☐ · usar inclinação ☐ + ângulo `#` · usar rotação ☐ + ângulo `#` |

---

## 2. Contextos da ferramenta Seleção

- **GD-BPC-01 Nada selecionado (página).** Tamanho de papel ▾ | largura/altura `#` | ajustar página ao conteúdo (botão com opções) | retrato ☐ · paisagem ☐ | aplicar a: todas as páginas ☐ · só esta ☐ | unidade de desenho ▾ | passo das setas (deslocamento) `#` | distância de duplicação X/Y `#` | tratar vazados como preenchidos ☐ | B-FOC.
- **GD-BPC-02 Vários objetos.** B-TRF | combinar | booleanas: unir · aparar · interseção · simplificar · frente menos trás · trás menos frente · contorno envolvente | B-CTN | agrupar · desagrupar · desagrupar tudo | B-FOC | alinhar e distribuir ☐ (abre painel).
- **GD-BPC-03 Grupo.** B-TRF | B-CTN | desagrupar · desagrupar tudo | B-FOC | B-ORG.
- **GD-BPC-04 Retângulo.** B-TRF | tipo de canto: arredondado ☐ · côncavo ☐ · chanfrado ☐ | raio por canto (4 × `#`, 2 colunas) · travar cantos juntos ☐ · raio relativo ao tamanho ☐ | B-CTN · B-CTV | B-FOC | B-ORG | B-CNV.
  - Aceite: travar cantos ligado + digitar 5 mm num canto → 4 cantos = 5 mm (1 desfazer).
- **GD-BPC-05 Elipse.** B-TRF | elipse ☐ · fatia ☐ · arco ☐ | ângulo inicial/final `#` | inverter sentido ☐ | B-CTN · B-CTV | B-FOC | B-ORG | B-CNV.
- **GD-BPC-06 Polígono.** B-TRF | nº de lados `#` (3–500) | B-CTN · B-CTV | B-FOC | B-ORG | B-CNV.
- **GD-BPC-07 Estrela.** B-TRF | estrela simples ☐ · estrela entrelaçada ☐ | nº de pontas `#` · profundidade das pontas `#` (1–99) | B-CTN | B-FOC | B-ORG | B-CNV.
- **GD-BPC-08 Espiral.** voltas `#` | simétrica ☐ · logarítmica ☐ | expansão (deslizante) | B-CTN | B-SET | fechar ☐ | B-ORG | B-FOC.
- **GD-BPC-09 Grade (papel milimetrado).** colunas/linhas `#` | B-CTN | fechar ☐ | contornar texto.
- **GD-BPC-10 Curva.** B-TRF | B-CTN · B-CTV | B-SET | B-CUR | B-FOC | B-ORG.
- **GD-BPC-11 Linha de 2 pontos.** B-TRF | modo: livre ☐ · perpendicular ☐ · tangente ☐ | B-CTN | B-SET | B-CUR.
- **GD-BPC-12 Polilinha.** B-TRF | B-CTN | B-SET | suavização | separar | fechar automaticamente ☐ | contornar texto | caixa de limites ☐ | desenho paralelo ☐.
- **GD-BPC-13 Curva de 3 pontos / Caneta / Bézier.** = GD-BPC-10; Caneta acrescenta prévia do próximo segmento ☐ · adicionar/remover nó automático ☐.
- **GD-BPC-14 Texto artístico/parágrafo.** B-TRF | B-FON | alinhamento ▾ | marcadores ☐ · numeração ☐ · capitular ☐ · aumentar recuo · diminuir recuo | recursos OpenType no texto ☐ | editar texto (diálogo) · painel de texto ☐ | horizontal ☐ · vertical ☐ | B-FOC.
- **GD-BPC-15 Texto em caminho.** orientação ▾ | distância do caminho `#` · deslocamento ao longo `#` | espelhar texto H · V | ajuste de passo (encaixe) | B-FON (sem sublinhado) | painel de texto ☐ | B-FOC.
- **GD-BPC-16 Edição de caracteres (kerning manual).** B-FON | deslocamento H/V `#` · ângulo do caractere `#` | sobrescrito ☐ · subscrito ☐ | versalete ☐ · maiúsculas ☐ | painel de texto ☐.
- **GD-BPC-17 Imagem bitmap.** B-TRF | vetorizar ▾ | editar bitmap | recortar bitmap · reamostrar | máscara de cor ☐ | B-FOC | B-ORG.
- **GD-BPC-18 Objeto incorporado (OLE).** B-TRF (sem espelhar) | camada frente/trás.
- **GD-BPC-19 Símbolo.** B-TRF | B-FOC | B-ORG.
- **GD-BPC-20 Forma predefinida.** B-TRF | galeria de formas ▾ | B-CTN | contornar texto | camada frente/trás.
- **GD-BPC-21 Linha-guia.** posição `#` | ângulo `#` · centro de rotação `#` | travar guia ☐ | mostrar guias ☐ · aderir às guias ☐ | guias predefinidas (abre opções).
- **GD-BPC-22 Tabela.** B-TRF (sem escala) | linhas × colunas `#` | preenchimento: cor ▾ · editar | bordas: seleção de lados ▾ · cor ▾ · espessura ▾ | opções ▾ | contornar texto | camada frente/trás.
- **GD-BPC-23 Células de tabela.** largura/altura de célula `#` | preenchimento | bordas | margens ▾ | mesclar · dividir em linhas · dividir em colunas · desfazer mesclagem | painel de texto ☐.
- **GD-BPC-24 Texto dentro de tabela.** B-FON | alinhamento H · V | marcadores · numeração · capitular · recuos | margens ▾ | editar texto · painel ☐ | horizontal/vertical ☐.
- **GD-BPC-25 Código de barras / QR.** B-TRF | conteúdo (texto) + campos | editar (diálogo) · validar. QR: margem `#` | forma do módulo ▾ · fator de preenchimento `#` | soldar módulos ☐ | arredondamento `#` | validar.
- **GD-BPC-26 Objeto de origem desconhecida.** aviso "dados de outro programa" + desbloquear.

## 3. Edição de forma e nós

- **GD-BPC-30 Ferramenta Forma (curva).** modo de seleção de nós ▾ (retângulo / à mão livre) | adicionar nó · remover nó | unir nós · quebrar | segmento em linha · em curva | nó cúspide · suave · simétrico | inverter sentido | extrair subcaminho | fechar curva (unir extremos) · fechada ☐ | esticar/escalar nós · girar/inclinar nós | alinhar nós | refletir nós H ☐ · V ☐ | elástico ☐ | selecionar todos os nós | reduzir nós | suavidade (deslizante) | caixa de limites ☐ | desenho paralelo ☐ | B-FOC.
- **GD-BPC-31 Forma em B-spline.** modo de seleção ▾ | adicionar/remover ponto de controle | ponto preso ☐ · flutuante ☐ | B-FOC.
- **GD-BPC-32 Suavizar / Mancha / Redemoinho / Atrair-repelir / Borrar / Áspero.** raio da ponta `#` | intensidade (deslizante) | opções próprias: Mancha → suavizar ☐ · modo forte ☐; Redemoinho → anti-horário ☐ · horário ☐; Atrair-repelir → atrair ☐ · repelir ☐; Borrar/Áspero → frequência `#` · secura `#` · direção ▾ | B-PEN.
- **GD-BPC-33 Borracha / Apagar segmento virtual.** forma da ponta: redonda ☐ · quadrada ☐ | espessura `#` | B-PEN (só Borracha) | reduzir nós automaticamente ☐.
- **GD-BPC-34 Faca.** modo: 2 pontos ☐ · mão livre ☐ · Bézier ☐ | fechar automaticamente ☐ | suavização | tipo de corte ▾ (sem folga / com folga / sobreposição) + largura `#` | tratamento do contorno ▾ | caixa de limites ☐.
- **GD-BPC-35 Recortar (crop).** posição `#` | tamanho `#` | ângulo `#`. Enter aplica; Esc cancela.

## 4. Desenho artístico

- **GD-BPC-40 Mídia artística — cabeçalho comum.** modo: predefinido ☐ · pincel ☐ · pulverizador ☐ · caligráfico ☐ · expressivo (tablet) ☐ | … | escalar traço com objeto ☐ | caixa de limites ☐ | B-FOC.
  - Predefinido: lista de traços ▾ | suavização | largura `#`.
  - Pincel: categoria ▾ · traço ▾ · procurar · salvar · excluir | suavização | largura `#`.
  - Pulverizador: categoria ▾ · padrão ▾ · procurar · salvar · excluir | ordem dos itens (abre lista) | tamanho `#` · escala progressiva ☐ | ordem ▾ (aleatória/sequencial/por direção) | adicionar | quantidade/espaçamento `#` | rotação ▾ · deslocamento ▾.
  - Caligráfico: suavização | largura `#` | ângulo da pena `#`.
  - Expressivo: B-PEN | suavização.
- **GD-BPC-41 Pincel de pintura (bitmap).** pincel ▾ + 3 modos ☐ | tamanho `#` · opacidade | brilho ☐ | simular pressão ☐ | densidade · maciez · inclinação · direção · textura (deslizantes) | redefinir pincel | textura ▾ | caixa de limites ☐ | B-FOC.
- **GD-BPC-42 Desenho inteligente.** nível de reconhecimento ▾ | nível de suavização ▾ | B-CTN.
- **GD-BPC-43 Esboço ao vivo.** temporizador | incluir curvas existentes ☐ · distância de captura | gerar curva única ☐ | suavização | prévia ☐ | caixa de limites ☐.

## 5. Preenchimento e transparência

- **GD-BPC-50 Preenchimento interativo — cabeçalho.** tipo: nenhum ☐ · uniforme ☐ · degradê ☐ · padrão vetorial ☐ · padrão bitmap ☐ · hachura ☐ · padrão 2 cores ☐ · textura ☐ · procedural ☐ | … | copiar preenchimento | editar (painel).
  - Uniforme: cor ▾.
  - Degradê: galeria ▾ | linear ☐ · elíptico ☐ · cônico ☐ · retangular ☐ | cor do nó ▾ · opacidade do nó · posição do nó | inverter ☐ · repetição ▾ · suavizar ☐ | aceleração | transformar livre ☐.
  - Padrões/textura: galeria ▾ | espelhar ladrilhos H ☐ · V ☐ | transformar com objeto ☐ | (2 cores: frente ▾ · fundo ▾) | (textura: biblioteca ▾ · opções · regenerar).
- **GD-BPC-51 Malha de preenchimento.** linhas × colunas `#` | modo seleção ▾ | adicionar interseção · remover | linha · curva | cúspide · suave · simétrico | amostrar cor · cor do nó ▾ · opacidade do nó | suavidade | suavizar cor ☐ | copiar malha · remover malha | B-FOC.
- **GD-BPC-52 Transparência — cabeçalho.** tipo: nenhuma ☐ · uniforme ☐ · degradê ☐ · padrão vetorial ☐ · bitmap ☐ · 2 cores ☐ · textura ☐ | modo de mesclagem ▾ | … | aplicar a: tudo ☐ · preenchimento ☐ · contorno ☐ | congelar ☐ | copiar · editar.
  - Uniforme: opacidade · galeria ▾.
  - Degradê: galeria ▾ | linear · elíptico · cônico · retangular ☐ | opacidade do nó · posição | ângulo `#` | transformar livre ☐.
  - Padrões/textura: galeria ▾ | opacidade frente · inverter · opacidade fundo | espelhar H/V ☐.
- **GD-BPC-53 Preenchimento inteligente.** origem do preenchimento ▾ (padrão/especificar/nenhum) · cor ▾ | origem do contorno ▾ · espessura ▾ · cor ▾.
- **GD-BPC-54 Conta-gotas.** modo: amostrar cor ☐ · amostrar atributos ☐.
  - Cor: área 1×1 ☐ · 3×3 ☐ · 5×5 ☐ · amostrar fora do app ☐ | cor amostrada | aplicar ☐ · adicionar à paleta.
  - Atributos: propriedades ▾ · transformações ▾ · efeitos ▾ (listas de marcação) | aplicar ☐.

## 6. Efeitos interativos

Todos começam com **B-PRE** e terminam com **B-EFX**.
- **GD-BPC-60 Mistura.** posição/tamanho do objeto | por passos ☐ · por distância ☐ | nº de passos `#` | rotação `#` | laço ☐ | caminho ▾ | direto ☐ · horário ☐ · anti-horário ☐ | aceleração cor/objeto · acelerar tamanho ☐ | opções ▾ · início/fim ▾.
- **GD-BPC-61 Contorno (offset).** posição/tamanho | para o centro ☐ · interno ☐ · externo ☐ | passos `#` | distância `#` | cantos ▾ | sequência de cor ▾ | cor do contorno ▾ · cor de preenchimento ▾ · última cor ▾ | aceleração.
- **GD-BPC-62 Distorção.** empurrar-puxar ☐ · zíper ☐ · torcer ☐ | centralizar | parâmetros: amplitude `#` (empurrar-puxar) / amplitude + frequência `#` + aleatório ☐ · suave ☐ · local ☐ (zíper) / horário ☐ · anti-horário ☐ + voltas `#` + graus `#` (torcer) | nova distorção.
- **GD-BPC-63 Envelope.** modo seleção ▾ | adicionar/remover nó | linha/curva | cúspide/suave/simétrico | modo: livre ☐ · reto ☐ · arco simples ☐ · arco duplo ☐ | mapeamento ▾ | manter linhas retas ☐ | novo envelope · criar a partir de objeto.
- **GD-BPC-64 Extrusão.** posição | ponto de fuga X/Y `#` | tipo ▾ | profundidade `#` | rotação ▾ | cor ▾ · chanfro ▾ · iluminação ▾ | ponto de fuga ▾ · relativo a objeto/página ☐.
- **GD-BPC-65 Sombra / Sombra interna.** sombra ☐ · sombra interna ☐ | cor ▾ · mesclagem ▾ · opacidade | desfoque · direção do desfoque ▾ · borda ▾ | deslocamento X/Y `#` | (sombra) ângulo · alongamento · esmaecimento / (interna) largura.
- **GD-BPC-66 Sombra em bloco.** posição/tamanho | profundidade `#` · direção `#` | cor ▾ · sobreimprimir ☐ | simplificar ☐ · remover furos ☐ | a partir do contorno ☐ · expandir `#` | remover sombra.
- **GD-BPC-67 Efeito de linhas (padrão paralelo).** estilo ▾ · limite interno ☐ · externo ☐ | ângulo `#` · pontos inicial/final | largura de linha `#` + passos `#` + aleatório ☐ | espaçamento `#` + passos `#` + aleatório ☐ | estilo de linha ▾ · ponto mais largo.
- **GD-BPC-68 Contorno variável.** adicionar nó · remover nó | largura lado 1/lado 2 `#` · travar ☐ · total `#` | escalar com traço ☐ | copiar · limpar nós.

## 7. Cotas, conectores, legendas

- **GD-BPC-70 Cota.** cota contínua automática ☐ (só linear) | cota dinâmica ☐ · estilo ▾ · precisão ▾ · unidade ▾ · mostrar unidade ☐ · zero à esquerda ☐ | prefixo · sufixo | B-CTN | seta ▾ | posição do texto ▾ | linhas de extensão ▾.
- **GD-BPC-71 Conector.** reto ☐ · ortogonal ☐ · ortogonal arredondado ☐ | raio `#` (ortogonal arredondado) | B-CTN | B-SET.
- **GD-BPC-72 Âncora de conector.** posição `#` · relativa ao objeto ☐ | direção ajustável ☐ · direção `#` | âncora automática ☐ | remover âncora.
- **GD-BPC-73 Legenda (callout).** forma ▾ | folga `#` (ou tamanho final `#`) | halo ▾ | B-CTN | seta inicial ▾. Objeto de legenda selecionado: escalar proporcional ☐ | halo | caneta · cor · espessura · setas · estilo.

## 8. Navegação e página

- **GD-BPC-80 Zoom.** níveis ▾ | aproximar · afastar | à seleção · a tudo | a todas as páginas · à página · à largura · à altura.
- **GD-BPC-81 Classificador de páginas.** papel ▾ | tamanho `#` | retrato/paisagem ☐ | todas/esta ☐ | miniatura pequena ☐ · média ☐ · grande ☐ | sair do classificador ☐.
- **GD-BPC-82 Transformação livre.** girar ☐ · refletir ☐ · escalar ☐ · inclinar ☐ | B-TRF | centro de rotação `#` | inclinação H/V `#` | aplicar a duplicata ☐ | relativo ao objeto ☐ | B-FOC.

## 9. Barra da Visualização de impressão

Ver doc 16 GD-IMP-32 (Seleção, Imposição com 4 sub-modos — D-16-10, provisória, P-23 —, Marcas, Zoom).

---

## Aceites gerais
- Cada contexto listado tem teste de interface: ativar a ferramenta / selecionar o tipo → barra contém exatamente os controles do contexto, na ordem, com separadores.
- Contextos de efeito: aplicar efeito, trocar de objeto e voltar → barra reflete os valores do efeito do objeto atual.

---

## 10. Revisão do Time B (2026-10-19)

- **D-31-01 Transbordo.** GD-BPR-08 (botão **»**) substitui a rolagem horizontal com degradê do DESIGN-SYSTEM 4.6. A altura fixa de 72 px em duas linhas empilhadas continua.
- **D-31-02 Ordem.** Vale a ordem de cada contexto deste doc (ex.: Retângulo termina em B-CNV; Vários objetos põe combinar/booleanas antes de B-CTN). O DESIGN-SYSTEM 4.6 passa a remeter para cá.
- **D-31-03 B-TRF em todo objeto.** Espiral (BPC-08) e Grade (BPC-09) aparecem sem B-TRF, ao contrário de todos os outros objetos e do aceite do GD-BPR-01 ("Seleção com objeto → X/Y/L/A"). Tratado como omissão: os dois começam com B-TRF. Pergunta P-13.
- **D-31-04 Recursos a mais do GraphDraw.** Controles que o GraphDraw já tem e o contexto não lista (simetria, texto em caminho, booleanas ao vivo, contornar texto, parâmetros de efeito, Excluir e Dividir das booleanas) entram como grupos **depois** dos grupos do contexto, como o DESIGN-SYSTEM já pede para recurso novo. O aceite "exatamente os controles" vale para o prefixo listado aqui.
- **D-31-05 Contextos sem a ferramenta.** Contextos de recursos que o GraphDraw ainda não tem (tabela, código de barras/QR, OLE, malha, extrusão, distorção, pulverizador, pincel bitmap, legenda, âncora de conector, classificador de páginas, esboço ao vivo, recorte de bitmap) ficam registrados no catálogo de contextos como **indisponíveis** e entram junto com a ferramenta.
- **D-31-06 Posição (GD-BPR-06).** A barra tem duas linhas horizontais de 72 px; nas laterais ela não cabe sem outro desenho. Nesta fase: **em cima** (padrão) ou **embaixo**, mais *Bloquear barras*. Laterais e flutuante esperam desenho próprio (P-14).
- **D-31-07 Personalização (GD-BPR-07).** Guardada na área de trabalho (`workspace.json`), por chave de contexto: itens ocultos, comandos adicionados e ordem. Reordenar: arrastar com `Alt` dentro do menu **⋯**.
- **D-31-08 Padrão sem seleção (GD-BPR-05).** Usa a mesma pergunta do GD-BDR-10 (cor), com "não perguntar de novo" valendo para todos os atributos.
- **D-31-09 Chave de contexto.** Texto estável `janela/ferramenta/seleção[/tarefa]`, ex.: `main/select/rect`, `main/select/none`, `main/zoom`, `preview/imposition/grid`. É a chave da personalização e dos testes.
- **D-31-10 Ordem (GD-BPR-07).** Alt + arrastar no menu **⋯** reordena **grupos** (blocos); os itens dentro de um grupo mantêm a ordem, porque vários são pares empilhados (X/Y, L/A). Grupo que aparece depois (ex.: setas com curva aberta) entra logo após o que o precede na ordem padrão.
- **D-31-11 Passo das setas (GD-BPR-03).** ↑/↓ no campo somam 1 mm nos comprimentos e 1 nos números (°, %, pt); Shift = 10 passos. Cada toque aplica e é uma entrada de desfazer. `Esc` desiste do que foi digitado.
- **D-31-12 Grupos vazios e dica (GD-BPR-08).** Grupo que não se aplica à seleção não ocupa lugar nem vai para o **»**. A dica da ferramenta ocupa o espaço que sobra, cortada com reticências, e nunca vai para o **»**. Alça à esquerda (GD-BPR-06), **»** e **⋯** à direita. Desmarcar no **⋯** um comando acrescentado o retira.
- **D-31-13 Tarefa na chave.** Edição de texto acrescenta `/texto` à chave (`main/text/text/texto`).
- **D-31-14 Controles sem recurso.** Nos contextos da Seleção (GD-BPC-01..10, 14), controles da spec que o GraphDraw ainda não tem ficam fora da barra e listados no catálogo do código (`barcatalog::MISSING`): lista de espessuras (B-CTN); suavização, caixa de limites e desenho paralelo (B-CUR); aplicar a todas/só esta, unidade de desenho, distância de duplicação e vazados como preenchidos (página); raio relativo (cantos); fechar (espiral); eixos de fonte variável e sublinhado (B-FON); marcadores, numeração, capitular, recuos, OpenType, painel de texto e vertical (GD-BPC-14). Entram com o recurso, sem mudar a ordem.
- **D-31-15 Ajustes de agrupamento.** Polígono e estrela usam um grupo só (modo, lados/pontas, nitidez, cantos), porque o controle já existente troca entre os dois; a espiral idem (voltas, modelo, expansão, fita). B-FOC usa o *Modo foco* (isola a seleção) no lugar de "entrar no grupo". B-ORG traz para frente/envia para trás **na camada**; os passos (um à frente/atrás) e a caneta caligráfica vêm depois do contexto (D-31-04). Escala X/Y % é relativa ao tamanho atual (digitar 50 % reduz à metade). O tipo de canto vale para os 4 cantos.
