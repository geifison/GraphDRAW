# 26 — Interações do Botão Direito

Detalha GD-UI-12 (doc 20). Objetivo: menos deslocamento do cursor e menos passos — cor, cópia, combinação e cancelamento a partir do botão direito, sem trocar de ferramenta.

Fase por item: **MVP** (depende só de requisitos MVP) ou **Beta** (depende de doc 10/11 ou de requisito fora do MVP).

## 0. Regras gerais de gesto
- **GD-BDR-01** Distinção clique × arrasto: botão direito pressionado e solto com deslocamento < 4 px de tela (configurável) e antes de 500 ms = **clique direito**; deslocamento ≥ 4 px = **arrasto direito**; parado ≥ 500 ms (configurável) sem deslocar = **toque longo**.
  - Aceite: pressionar, mover 3 px, soltar → abre menu de contexto; mover 5 px → entra em arrasto direito, menu de contexto não abre.
- **GD-BDR-02** Precedência: se o botão esquerdo está pressionado (ação em curso), o botão direito **nunca** abre menu de contexto — vale só a regra do item 6.
- **GD-BDR-03** Exceções por ferramenta: Zoom (GD-FER-04) mantém botão direito = afastar. Demais ferramentas seguem este doc.
- **GD-BDR-04** Todo menu aberto por botão direito aparece no ponto do cursor, ajustado para caber na tela; Esc ou clique fora fecha sem alterar o documento. Itens indisponíveis aparecem desabilitados, com o motivo em dica (GD-UI-14).
- **GD-BDR-05** Cada comando acionado = uma transação nomeada (GD-UND-01). Abrir/fechar menu, cancelar e prévias não entram no histórico.
- **GD-BDR-06** Todo comando deste doc também existe em menu principal e é atribuível a atalho (GD-UI-10, GD-UI-30) — o botão direito é atalho, não o único caminho.

## 1. Paleta e amostras de cor — MVP
Complementa GD-PAL-05.
- **GD-BDR-10** Clique esquerdo em amostra → preenchimento da seleção; clique direito → contorno da seleção. Sem seleção: perguntar se a cor vira padrão para novos objetos (opção "não perguntar de novo").
- **GD-BDR-11** Amostra "nenhum" (X): clique esquerdo remove preenchimento; clique direito remove contorno.
  - Aceite: retângulo com contorno preto 1 pt; clique direito no X → contorno = nenhum; desfazer 1× → contorno preto 1 pt.
- **GD-BDR-12** Toque longo direito em amostra de gradiente/padrão/estilo composto → miniseletor no cursor com destinos: Preenchimento · Contorno · Sombra (Beta, doc 10). Destino inaplicável aparece desabilitado (ex.: Contorno para padrão, se GD-OUT não suportar). Soltar fora do miniseletor = cancelar.

## 2. Arrastar com botão direito — menu ao soltar
- **GD-BDR-20** Arrasto direito sobre objeto (ou seleção) mostra prévia fantasma do conjunto acompanhando o cursor; o original não se move até a escolha. Esc durante o arrasto cancela.
- **GD-BDR-21** Destino detectado sob o cursor no momento de soltar: **área vazia** ou **outro objeto** (o mais acima na ordem z, excluindo a própria seleção). Sobre objeto, o alvo ganha realce (mira) enquanto o cursor estiver nele.

### 2.1 Soltar em área vazia
| ID | Item | Efeito | Fase |
|---|---|---|---|
| GD-BDR-22 | Mover para cá | translada a seleção pelo deslocamento do arrasto | MVP |
| GD-BDR-23 | Copiar para cá | cópia no destino, original intacto (mesma semântica de GD-DUP) | MVP |
| GD-BDR-24 | Criar instância vinculada | converte original em símbolo se ainda não for e coloca instância no destino (GD-SIM-02/03) | Beta |
| GD-BDR-25 | Cancelar | nada muda, nada no histórico | MVP |

- Aceite (GD-BDR-25): arrastar com direito 50 mm, escolher Cancelar → posição original, pilha de desfazer inalterada.

### 2.2 Soltar sobre outro objeto
| ID | Item | Efeito | Fase |
|---|---|---|---|
| GD-BDR-26 | Colocar dentro (recorte) | seleção vira conteúdo do recipiente de recorte cujo limite é o alvo (doc 01, item 10) | Beta |
| GD-BDR-27 | Copiar todas as propriedades | preenchimento, contorno, transparência e efeitos da seleção → alvo; geometria do alvo intacta | MVP (efeitos: Beta) |
| GD-BDR-28 | Copiar só contorno | cor, espessura, tracejado, terminais e cantos (GD-OUT-*) | MVP |
| GD-BDR-29 | Copiar só preenchimento | uniforme, gradiente ou padrão (GD-FIL-*); contorno do alvo intacto | MVP |
| GD-BDR-30 | Soldar | GD-BOO-01 entre seleção e alvo | MVP (M2) |
| GD-BDR-31 | Aparar | GD-BOO-02: seleção recorta o alvo | MVP (M2) |
| GD-BDR-32 | Interseção | GD-BOO-03 | MVP (M2) |
| GD-BDR-33 | Centralizar no alvo | move a seleção para que o centro da caixa geométrica (GD-OBJ-06) coincida com o do alvo | MVP |
| GD-BDR-34 | Mover para cá / Copiar para cá / Cancelar | iguais a 2.1, disponíveis também aqui | MVP |

- **GD-BDR-35** Direção da cópia de propriedades: **sempre da seleção arrastada para o alvo**. Booleanas seguem as opções de GD-BOO-10 (padrão: resultado herda atributos do alvo; originais não mantidos).
- Aceite (GD-BDR-28): arrastar com direito objeto A (contorno vermelho 2 pt tracejado, preenchimento azul) sobre B (contorno preto 1 pt, preenchimento amarelo), escolher "Copiar só contorno" → B: contorno vermelho 2 pt tracejado, preenchimento amarelo; A inalterado.
- Aceite (GD-BDR-33): A 10×10 mm em (0,0), B 40×20 mm com centro (100,50) → centro de A = (100,50) ± 0,001 mm.

## 3. Menu de contexto sobre objeto (clique direito) — MVP
Clique direito sobre objeto não selecionado primeiro o seleciona (substitui seleção); sobre objeto já selecionado mantém a seleção múltipla.

- **GD-BDR-40** Geometria: Converter em curvas (GD-CNV-01); Separar (GD-CMB-02); Converter contorno em objeto (GD-OUT-09).
- **GD-BDR-41** Ordem (GD-ORD-01): Trazer para frente · Enviar para trás · Um passo à frente · Um passo para trás · À frente de… · Atrás de…. "À frente de…/Atrás de…" troca o cursor por mira; clique no objeto de referência conclui; Esc ou clique em área vazia cancela.
- **GD-BDR-42** Organização: Agrupar (≥ 2 objetos) · Desagrupar · Desagrupar tudo (todos os níveis) · Bloquear/Desbloquear · Ocultar (GD-OBJ-04). Objeto oculto continua no painel Objetos e volta por "Mostrar todos" no menu de contexto da área vazia.
- **GD-BDR-43** Itens mostrados dependem do tipo e da quantidade selecionada (ex.: Agrupar só com ≥ 2; Separar só com caminho de vários subcaminhos; Converter em curvas não aparece para caminho livre).
- **GD-BDR-44** Menu de contexto em área vazia: Colar · Selecionar tudo · Mostrar todos · Desbloquear todos · Sair do modo foco (se ativo).
- **GD-BDR-45** Objeto bloqueado: menu mostra só Desbloquear, Ordem e Propriedades.

## 4. Ferramenta Forma (nós) — MVP
Complementa GD-NOD-*.
- **GD-BDR-50** Clique direito em nó: Canto · Suave · Simétrico (tipo atual marcado; GD-NOD-05) · Quebrar no nó · Unir extremos (só com 2 extremos selecionados) · Excluir nó (GD-NOD-03, forma reajustada).
- **GD-BDR-51** Clique direito em segmento: Converter em curva · Converter em reta · Adicionar nó aqui (no ponto clicado) · Inverter direção (GD-NOD-04).
- **GD-BDR-52** Comando age sobre todos os nós selecionados se o nó clicado estiver na seleção; senão age só sobre ele e o seleciona.
- Aceite (GD-BDR-50 Simétrico): nó suave com alças de 5 e 12 mm → Simétrico → ambas com 8,5 mm (média), colineares; forma alterada só nos dois segmentos adjacentes.
- Aceite (GD-BDR-51 reta): segmento curvo entre (0,0) e (10,0) → reta; sem alças; nós extremos na mesma posição.

## 5. Texto e quadros
- **GD-BDR-60** Quadro de texto de parágrafo: Inserir texto de exemplo (preenche o quadro sem exceder; texto neutro próprio do projeto, traduzido por idioma GD-UI-20) — MVP (M2).
- **GD-BDR-61** Forma fechada: Usar como quadro de texto (GD-TXT-02) — MVP (M2).
- **GD-BDR-62** Forma fechada: Usar como quadro de imagem (marca o objeto como destino de recorte; soltar imagem/vetor sobre ele aplica GD-BDR-26 automaticamente) — Beta.
- **GD-BDR-63** Quadro de parágrafo: Converter em texto artístico (GD-TXT-03), conteúdo e formatação preservados — MVP (M2).
- **GD-BDR-64** Texto + caminho selecionados: Colocar texto no caminho (GD-TXT-09) — MVP (M2).

## 6. Botão direito durante ação em curso — MVP
- **GD-BDR-70** Durante **mover, escalar, girar, inclinar ou transformação livre** com botão esquerdo (ferramenta Seleção/Transformação livre): toque direito cria uma cópia e é **a cópia** que segue a manipulação até soltar o esquerdo; o **original volta na hora** ao estado e lugar de antes de pressionar o esquerdo. Só o primeiro toque conta. Ao soltar, tudo vira **uma** transação ("Mover cópia", "Girar cópia" etc.); a cópia fica selecionada.
  - Aceite: arrastar retângulo 30 mm, tocar direito, continuar até 60 mm, soltar → 2 retângulos (original no lugar de início, cópia a 60 mm); desfazer 1× → 1 retângulo na posição original. Escalar com toque direito → original com tamanho de antes, cópia com tamanho novo.
- **GD-BDR-71** Durante qualquer outra ação com botão esquerdo — desenhar com Bézier/caneta, arrastar guia, retângulo de seleção, mover nó — toque direito **cancela**: estado volta ao de antes de pressionar o esquerdo, nada entra no histórico. Soltar o esquerdo depois do cancelamento não faz nada.
  - Aceite: mover nó com esquerdo, tocar direito → nó no lugar de antes, histórico inalterado.
- **GD-BDR-72** Esc tem o mesmo efeito de cancelamento de GD-BDR-71, inclusive durante mover.

## 7. Produção
- **GD-BDR-80** Gerar contorno de sangria/corte: diálogo com distância e cantos (GD-OFS-01), resultado em objeto separado, opcionalmente na camada de corte (doc 17) — MVP (M2).
- **GD-BDR-81** Exportar seleção: abre exportação com escopo = seleção (GD-EXP-01), formato último usado (SVG/PDF vetorial ou PNG com fundo transparente).
- **GD-BDR-82** Modo foco: oculta da exibição tudo exceto a seleção (não altera visibilidade salva, não entra no histórico — GD-UND-06); edição permitida dentro do foco; sair por "Sair do modo foco" no menu de contexto, Esc, ou indicador na barra de status.
  - Aceite: entrar em foco com 1 de 10 objetos, salvar, reabrir → 10 objetos visíveis.

## Decisões tomadas (revisar)
1. Conflito entre "duplicar em voo" e "cancelar em voo": resolvido por tipo de ação — só **mover/escalar/girar/inclinar/transformação livre** duplicam (GD-BDR-70); todo o resto cancela (GD-BDR-71).
2. Toque longo × arrasto direito: limiares de 500 ms / 4 px (GD-BDR-01).
3. Ferramenta Zoom mantém botão direito = afastar (GD-BDR-03).
4. Menu de contexto não aparece com botão esquerdo pressionado (GD-BDR-02).
5. "Modo foco" sai por menu/Esc/barra de status, não por um novo clique direito qualquer (evita conflito com GD-BDR-40..45).

## Perguntas em aberto
- P1. Sem seleção, clique em amostra: perguntar (como acima) ou aplicar silenciosamente ao padrão?
- P2. Soltar sobre alvo que pertence a grupo: alvo = objeto interno ou o grupo inteiro? Proposta: grupo; modificador (GD-SEL-02) mira o interno.
- P3. ~~GD-BDR-70 também para rotação/escala?~~ Resolvido (2026-10-18): sim, vale para todas as transformações.
- P4. Mac/trackpad: equivalente ao botão direito (Ctrl+clique, dois dedos) e toque longo — precisa de mapeamento.
