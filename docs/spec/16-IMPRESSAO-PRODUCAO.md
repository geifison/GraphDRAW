# 16 — Impressão e Saída para Produção

Sincronizado de `../spec/16` v6 (2026-10-19) a pedido do responsável pelo
produto; substitui a cópia v3. Seções 1–7 sem alteração de texto. A
seção 8 é a revisão do Time B: correspondência com os docs 29 (GD-PRT) e
28 (GD-PRE), conflitos em aberto e decisões.

**Fase: Beta** (exceto GD-IMP-01..05, que entram no M3). Versão 6 · 2026-10-19 (ACHADOS-002…006: ajuda pública, configuração de UI, análise REA dos módulos de impressão, padrões, regras e PDF).

Princípio: impressão **nunca altera o documento**. Tudo que se configura aqui (escala, imposição, conversão de spots, marcas) vale só para a saída e é guardado no estilo de impressão (GD-IMP-06), não nos objetos.

---

## 1. Diálogo Imprimir

Comando **Arquivo › Imprimir** (`Ctrl+P`). Diálogo único com seções laterais (lista à esquerda, conteúdo à direita) e prévia em miniatura sempre visível à direita (GD-IMP-02). Seções: **Destino**, **Posição**, **Cor**, **Chapas**, **Marcas**, **Avançado**, **Problemas**.

### Destino
- **GD-IMP-01** Impressora (lista do SO) + botão *Preferências do driver* (abre o diálogo nativo); abaixo, só leitura: status, local e comentário da impressora. Papel e orientação: *do driver*, *igual ao documento* (troca orientação automática por página) ou escolha explícita; destinos PostScript aceitam *tamanho personalizado* (largura, altura, direção de alimentação, folha solta ou bobina contínua + deslocamentos).
  - Cópias (1–9999) e *Intercalar* (agrupar) quando cópias > 1.
  - Intervalo: **Documento inteiro**, **Página atual**, **Páginas** (texto `1-3, 5, 8-`; filtro *todas / pares / ímpares*), **Seleção** (desabilitado sem seleção), **Documentos abertos** (lista com caixa de marcação).
  - Aceite: documento de 6 páginas, intervalo `2-5` + *pares* → imprime 2 e 4.
- **GD-IMP-04** *Imprimir em arquivo*: PDF (padrão), PostScript (.ps) ou PRN; modo *arquivo único*, *um arquivo por página* ou *um arquivo por chapa*. Nome gerado `<doc>_p<n>` / `<doc>_<chapa>`.
- **GD-IMP-05** *Imprimir como imagem*: rasteriza cada página em N dpi (72–2400) antes de enviar. Saída de emergência para drivers que falham com vetores complexos.

### Estilos de impressão
- **GD-IMP-06** Conjunto de todas as opções deste documento salvo como **estilo** (arquivo `.gdprint`, JSON legível, copiável entre máquinas). Lista *Estilo* no topo do diálogo: *Padrão GraphDraw*, estilos salvos, *Procurar…*. Botões *Salvar como…* e *Excluir*. Alterou opção sem salvar → nome do estilo ganha `*`; fechar com *Cancelar* descarta.
  - Último estilo usado fica gravado no documento .gdr (doc 13), não as opções soltas.
  - Aceite: salvar estilo "Gráfica X", copiar arquivo para outra máquina, escolher via *Procurar…* → todas as opções iguais.

### Posição
- **GD-IMP-07** Tamanho e posição da arte na folha — *Configurar para:* ▾ (todas as páginas ou uma página específica; cada página pode ter posição própria):
  - **Tamanho real** (padrão) — 1:1, centralizado.
  - **Ajustar à folha** — escala uniforme máxima que cabe na área imprimível do driver.
  - **Manual** — X, Y, largura, altura, escala %, âncora (9 pontos); valores com unidade dentro do campo.
- **GD-IMP-03** **Mosaico** (ladrilhos) para arte maior que o papel: sobreposição (mm ou % da folha), opção *tamanho do ladrilho = % da largura da folha*, marcas de alinhamento nos cantos e etiqueta `linha/coluna` em cada pedaço. Prévia mostra a grade numerada.
  - Aceite: área imprimível 200 × 287 mm (retrato), sobreposição 10 mm, arte 390 × 564 mm → 2 × 2 = 4 folhas; com *orientação automática* ligada escolhe o arranjo de menos folhas.
- **GD-IMP-10** **Sangria de saída**: liga/desliga + valor (padrão = sangria do documento, doc 02 GD-PAG-02). Objetos são recortados nesse limite; o que passar disso não vai para a saída. Aviso se valor > 10 mm.
- **GD-IMP-20** **Imposição**: lista de modelos (`1 por folha`, `2 × 1`, `2 × 2`, `2 × 3`, `3 × 3`, `Livreto`, `Grade automática`…) + *Editar* (abre a prévia no modo Imposição, §2). Ver §3.

### Cor
- **GD-IMP-08** Modo: **Composta** ou **Separações** (abre seção Chapas).
- Saída de cor: *nativa* (cada objeto no seu espaço), *RGB*, *CMYK*, *Tons de cinza*. Drivers sem PostScript/PDF nativo aceitam só RGB/cinza → opções incompatíveis ficam desabilitadas com dica.
- Conversão feita por: *GraphDraw* (padrão) ou *dispositivo* (só destinos PostScript/PDF).
- Perfil de correção (ICC) — padrão: perfil do documento; lista inclui perfis instalados.
- Intenção de renderização: *Perceptual*, *Colorimétrica relativa* (padrão), *Saturação*, *Colorimétrica absoluta* (doc 05).
- *Preservar preto puro* (K 100% não vira preto composto na conversão).
- Rodapé da seção: resumo em texto do que acontecerá com as cores (ex.: "RGB → CMYK com perfil X; 2 spots convertidos").
- *Preservar números CMYK* (não converte valores CMYK já definidos).
- *Usar prova de cor* — imprime simulando o perfil de prova configurado no documento.
- *Converter spots em processo* — global ou por spot (tabela em Chapas).

### Chapas (só com Separações)
- **GD-IMP-12** Tabela de chapas: marcação imprimir sim/não, nome, ordem (editável), lineatura (lpi), ângulo (°), sobreimpressão de gráficos / texto por chapa. Valores de tela vêm vazios = "delegar ao RIP" (GD-IMP-15).
  - Prévia mostra uma aba por chapa + aba *Composta*.
  - *Mostrar chapas vazias* ☐ (padrão desligado: chapa sem tinta não sai).
  - *Texto sempre em preto* ☐ (todo texto vai só na chapa K).
  - *Avançado…*: tecnologia de tela, resolução do dispositivo, lineatura base, formato do ponto (redondo, elíptico, linha…) — tudo opcional, vazio = RIP decide.
  - Aceite: documento com C, M, Y, K e spot "Pantone 186" → 5 linhas; desmarcar Y → saída sem chapa Y.
- **GD-IMP-13** Sobreimpressão do documento: **Preservar** (padrão), **Ignorar** (só a cor de cima, as de baixo são vazadas), **Simular** (achata para imagem composta — útil para prova em impressora comum).
  - *Preto sempre sobreimpresso*: objetos com K ≥ limiar (padrão 95%, configurável) sobreimprimem.
- **GD-IMP-14** Trapping: **fora de escopo** (delegado ao RIP). Não há dispersão automática nem trapping in-RIP. Registrado aqui por ser recurso esperado por usuários de pré-impressão; reavaliar pós-1.0.
- **GD-IMP-15** Tela de meio-tom: delegada ao RIP; só emitida se o usuário preencher lpi/ângulo na tabela.

### Marcas
- **GD-IMP-11b** Tamanho das marcas de corte:
  | Situação | Distância da linha de corte | Comprimento |
  |---|---|---|
  | Peça sem vizinha | 3,8 mm | 8,5 mm |
  | Peça encostada em outra (grade) | 1,3 mm | 2,8 mm |
  | Sangria limitada e maior que a distância | = sangria | igual à linha acima |
  | Estilo japonês (tombo) | 3 mm fixo | marca de canto dupla + **marca de centro** em cada lado livre |
  - Aceite: cartão isolado com sangria 5 mm limitada → marcas começam a 5 mm do corte.
- **GD-IMP-11c** Escala de cinza/densidade: com separações, sai só a escala da chapa em impressão; em composta, uma escala por tinta de processo (C, M, Y, K).
- **GD-IMP-11** Marcas de impressora, cada uma com caixa própria:
  - **Corte** — nos cantos da página final; opção *só borda externa* (na imposição, marca só o contorno do bloco) ou *por peça* (GD-IMP-22). Opção *marcas na caixa da seleção* em vez da página.
  - **Dobra** — onde a imposição define dobra.
  - **Registro** — alvo nos 4 lados, em todas as chapas; 3 estilos próprios.
  - **Barra de cores** (CMYK + spots, 0–100% em passos de 10) e **escala de cinza** (7 degraus, valores editáveis).
  - **Número da página** e **informações do trabalho** (nome do arquivo, data/hora, chapa, perfil, nome do trabalho editável); opção *dentro da página*.
  - Distância das marcas até a página (padrão 3 mm) e espessura (padrão 0,25 pt).
  - Aviso quando papel não comporta as marcas (precisa ≥ ~13 mm de folga por lado) → sugere *ajustar à folha* ou papel maior.
  - Marcas saem em cor de registro (100% em todas as chapas).

### Avançado
- **GD-IMP-16** Filme: *Negativo* (inverte) e *Espelhar* (emulsão para baixo). Aviso se ligado com impressora de escritório.
- **GD-IMP-17** Imagens: *reduzir resolução* acima de N dpi (cor/cinza padrão 300, monocromático padrão 1200); compressão para PostScript/PDF (sem perda / JPEG + qualidade).
- **GD-IMP-18** PostScript: nível 2 ou 3; conforme DSC ☐; arquivo PPD opcional; *mais passos em degradês* (evita faixas); *otimizar degradês* (menos passos onde não aparece); *simplificar curvas* (tolerância); fontes: incorporar / converter em curvas; compressão de imagens (nenhuma / sem perda / JPEG + qualidade); marcas PDF (abrir em, links ☐, marcadores ☐) quando o PS for destilado.
  - Caminhos muito longos: acima de **10 000 nós** o caminho é dividido em pedaços (preenchimento e contorno tratados separadamente) para não estourar o limite do intérprete. Valor configurável em *Opções › Impressão*.
  - Planicidade de curva enviada ao dispositivo: **1,0** por padrão (maior = menos segmentos); *simplificar curvas* aumenta esse valor automaticamente quando o dispositivo recusa o trabalho.
- **GD-IMP-18b** Destino PDF (quando *Imprimir em arquivo* = PDF) — mesmo componente do doc 15 (exportar PDF):
  | Opção | Valores | Padrão |
  |---|---|---|
  | Compatibilidade (nesta ordem) | Acrobat DC · Acrobat 9.0 · PDF/X-1a:2001 · PDF/X-3:2002 · PDF/A-1b · PDF/X-4:2010 | Acrobat DC |
  | Compressão de imagens | nenhuma · LZW · ZIP · JPEG · JPEG 2000 | ZIP |
  | Qualidade (só JPEG/JPEG 2000) | 1–100 % | 95 % |
  | Comprimir texto e vetores | ☐ | ligado |
  | Procedurais complexos como imagem | ☐ | desligado |
  | Texto como curvas | ☐ | desligado |
  | Subconjunto de fontes quando uso < N % dos glifos | ☐ + 0–99 % | ligado, 80 % |
  | Arquivos EPS embutidos | PostScript original · só pré-visualização | pré-visualização |
  | Senha de abertura / de permissão | texto + confirmação | vazias |
  | Impressão permitida | nenhuma · baixa resolução · alta resolução | nenhuma (só vale com senha de permissão) |
  | Edição permitida | nenhuma · montar páginas (inserir/excluir/girar) · tudo exceto extrair páginas | nenhuma |
  | Copiar conteúdo | ☐ | desligado |
  - "Acrobat DC" e "Acrobat 9.0" designam níveis de compatibilidade de leitor (uso nominativo da marca de terceiros, só como rótulo de compatibilidade).
  - Arquivo gerado por opção:
    | Opção | Cabeçalho | Extras obrigatórios |
    |---|---|---|
    | Acrobat DC | PDF 1.7 | declaração de extensão nível 3 sobre a base 1.7 |
    | Acrobat 9.0 | PDF 1.7 | idem — **saída idêntica a Acrobat DC** (as duas opções existem só como rótulo) |
    | PDF/X-1a:2001 | PDF 1.3 | intenção de saída (perfil de destino) + identificação PDF/X |
    | PDF/X-3:2002 | PDF 1.3 | intenção de saída + identificação PDF/X |
    | PDF/A-1b | PDF 1.4 | intenção de saída + metadados XMP com identificação PDF/A |
    | PDF/X-4:2010 | PDF 1.6 | intenção de saída + metadados XMP |
    - Aceite: exportar com "Acrobat 9.0" e com "Acrobat DC" o mesmo documento → arquivos iguais byte a byte, exceto data/ID.
  - Regras de dependência: padrões PDF/X e PDF/A **escondem LZW** (e JPEG 2000 em X-1a/X-3/A); se a compressão escolhida deixar de existir, volta para ZIP. Campo de qualidade fica desabilitado fora de JPEG/JPEG 2000.
  - Aceite: escolher JPEG → qualidade habilitada em 95 %; trocar para PDF/X-1a com LZW escolhido → compressão vira ZIP e LZW some da lista.
- **GD-IMP-19** Driver sem PostScript: imagens grandes enviadas em faixas (limite em MB + sobreposição em px, ocultos em *Opções › Impressão*).

### Problemas
- **GD-IMP-30** Verificação antes de imprimir: o rótulo da seção mostra a contagem (`Problemas (3)` / `Sem problemas`). Regras: imagem abaixo de X dpi na saída, fonte ausente, RGB em saída de separações, spots com nome duplicado, **branco com sobreimpressão** (some na impressão), objetos fora da página/sangria, contorno abaixo da espessura mínima, cobertura total de tinta > N%, degradê com risco de faixas, mais de N spots, mais de N fontes, papel menor que página + marcas.
- **GD-IMP-30b** Regras de fábrica (todas ligadas; limiar editável onde indicado; severidade: ℹ informação, ⚠ aviso, ⛔ grave):
  | Regra | Dispara quando | Limiar padrão | Sev. |
  |---|---|---|---|
  | Texto pequeno | algum texto menor que N pt | 7 pt | ⚠ |
  | Imagem com pouca resolução | resolução efetiva na saída < N dpi | 96 dpi | ⚠ |
  | Muitas fontes | fontes a incorporar/baixar > N | 10 | ⚠ |
  | Muitas chapas spot | chapas spot que sairão > N | 2 | ⚠ |
  | Spots parecidos | dois spots cujo nome normalizado (sem espaços, maiúsc./minúsc., sufixos de acabamento C/U/M) coincide | — | ⚠ |
  | Ângulos repetidos | duas chapas com o mesmo ângulo de tela | — | ⚠ |
  | Degradê com faixas | níveis de cinza disponíveis ((dpi ÷ lpi)² + 1) < passos necessários para o comprimento do degradê | — | ⚠ |
  | Branco sobreimpresso | preenchimento ou contorno 100 % branco com sobreimpressão (some na impressão) | — | ⛔ |
  | Vínculo ausente | imagem/arquivo vinculado não encontrado | — | ⛔ |
  | Fora da área imprimível | conteúdo ou marcas além da área imprimível do driver | — | ⚠ |
  | Fora do papel | conteúdo ou marcas além do papel escolhido | — | ⚠ |
  | Conversão de spot não alcança arquivo embutido | conversão ligada + EPS/PDF embutido com spot | — | ⚠ |
  | Correção de cor dupla | gestão de cor ligada + driver sem PostScript/PDF | — | ℹ |
  | Cor de registro em objeto comum | objeto (não marca) usa cor de registro | — | ℹ |
  | Páginas reduzidas na imposição | imposição reduziu páginas para caber | — | ℹ |
  | Imagem girada | rotação diferente de 0/90/180/270° | — | ℹ |
  | Página vazia | página do intervalo sem objetos | — | ℹ |
  | (GraphDraw) RGB em separações, cobertura de tinta > N %, contorno fino, objeto fora da sangria | ver GD-IMP-30 | 300 % / 0,1 mm | ⚠ |
  - Cada item mostra: título, frase curta, detalhes (lista de páginas/objetos/fontes) e **sugestões** de correção específicas da regra.
- **GD-IMP-31** Cada problema: severidade, objeto (clicar → fecha o diálogo e seleciona), correção sugerida. Regras liga/desliga e salvas por perfil (ex.: "Gráfica X"), independente do estilo de impressão.
  - Lista no topo: *verificar para* ▾ (impressão / PDF / exportação) + *Regras…*.
  - Abaixo da lista: explicação do item selecionado; botão **Corrigir** quando há correção automática (ex.: converter RGB → CMYK, aumentar contorno ao mínimo); caixa *Ignorar este tipo de problema daqui em diante* ☐.
  - Durante a análise: mensagem de progresso no lugar da lista; o diálogo segue utilizável.
- Imprimir com problemas de severidade *erro* pede confirmação; *aviso* não.

---

## 2. Visualização de impressão

- **GD-IMP-02** Janela (ou modo de tela cheia da área de desenho) que mostra a **folha** como sairá: papel, área imprimível do driver em tracejado, arte, marcas, mosaico/imposição. Mesmo renderizador da saída — o que se vê é o que imprime.
  - Abre por **Arquivo › Visualizar impressão** ou botão no diálogo; fecha com `Esc` voltando ao diálogo com as mudanças.
  - Navegação: primeira · anterior · lista de folhas · próxima · última; *Ir para…*; zoom (ajustar, 100%, %, retângulo, roda do mouse), mão (espaço); *Tela cheia*.
  - Com separações: abas na base, uma por chapa + *Composta*.
  - Menu **Exibir** da prévia: mostrar arte ☐ (desligado = só molduras, rápido) · cor da prévia: *automática (simula saída)* / *cor* / *cinza* · modo: *automático* / *composta* / *chapas* · réguas ☐ · área imprimível ☐ · ladrilho atual ☐ · renderizar preenchimentos procedurais ☐ · barra de status ☐ · travar barras ☐.
  - **Barra padrão da prévia**: estilo de impressão ▾ · salvar estilo como · excluir estilo · abrir diálogo Imprimir · imprimir · zoom ▾ · tela cheia · separações ☐ · negativo ☐ · espelhar ☐ · fechar prévia.
  - Arquivo da prévia: salvar estilo · salvar como · excluir · imprimir · **imprimir só esta folha** · fechar.
  - Menu **Configurações** abre direto cada seção do diálogo (Destino, Posição, Chapas, Marcas, PostScript, PDF, Problemas) + *Folha de informações do trabalho* (GD-IMP-42) + *Assistente de frente e verso manual*.
  - Menu de contexto (botão direito) na folha: seções do diálogo · zoom ± · ir para · tela cheia · cor/separações · imprimir · imprimir esta folha · fechar.
- **GD-IMP-32** Ferramentas próprias da prévia (caixa vertical: **Seleção**, **Imposição**, **Marcas**, **Zoom**) e **barra de propriedades contextual própria** (segue as regras de §14.0 da spec 27). Enquanto Imposição edita, a caixa troca por um único botão *Concluir edição de imposição*.

| Ferramenta | Barra de propriedades mostra |
|---|---|
| Seleção | posição da arte na folha ▾ (centro, cantos, personalizada) \| canto superior esquerdo X/Y `#` \| L/A `#` · escala % `#` · manter proporção ☐ \| mosaico ☐ \| unidade ▾ |
| Marcas | caixa das marcas automática ☐ \| caixa das marcas X/Y e L/A `#` \| info do trabalho ☐ · nº de página ☐ · corte ☐ · registro ☐ · barra de cores ☐ · escala de cinza ☐ \| abrir seção Marcas |
| Zoom | aproximar · afastar \| 1:1 · à seleção · a tudo \| à folha · à largura · à altura |
| Imposição | sempre: modelo de imposição ▾ · salvar modelo · excluir modelo \| **O que editar** ▾ \| … |

  Sub-modos de **O que editar** (Imposição):
  | Sub-modo | Controles após *O que editar* |
  |---|---|
  | Grade básica | prévia modelo/documento ☐ \| peças H × V `#` · frente e verso ☐ \| manter tamanho de página do documento ☐ \| encadernação ▾ (*colada*, *grampo*, *empilhar e cortar*, *personalizada*) + (grampo) páginas por caderno `#` / (personalizada) nº de cadernos `#` |
  | Ordem das páginas | ordenar: inteligente ☐ · sequencial ☐ · repetida ☐ \| nº da página na posição selecionada `#` · rotação da posição ▾ (0/90/180/270) |
  | Calhas e acabamento | calha automática ☐ · calhas iguais ☐ · tamanho da calha `#` \| marcar corte entre peças ☐ · marcar dobra ☐ |
  | Margens | margens iguais ☐ · automáticas ☐ \| sup/esq `#` · inf/dir `#` |

  - Aceite: ativar ferramenta Imposição → barra troca para os controles da linha correspondente sem reabrir janela; trocar *O que editar* troca só o trecho após a lista.
- **GD-IMP-33** Alternativa compacta (painel lateral em vez de janela): mesma prévia com três abas — **Posição** (predefinição, unidade, posição, tamanho, proporção, mosaico), **Imposição** (predefinição, tipo, peças H/V, tamanho do doc, frente e verso, prévia, encadernação, ordem, rotação, calhas, acabamento) e **Marcas** (posição + 4 distâncias, nº de página, info, corte, registro, cores, escala, marcas no objeto). Botão *Imprimir…*. Mesmo modelo de dados de GD-IMP-32; só muda a apresentação.

---

## 3. Imposição

- **GD-IMP-20** **Grade**: N × M peças por folha, calha (espaço entre peças), margens, sangria por peça; *grade automática* calcula N × M e orientação que mais aproveitam a folha. Útil para cartões, etiquetas, adesivos.
  - Ordem das peças: **sequencial** (esq→dir, cima→baixo), **repetida** (mesma página em todas as posições — cartão de visita) ou **livre** (usuário define nº por posição). Rotação por peça: 0/90/180/270°.
  - Aceite: cartão 90 × 50 mm, A4, calha 4 mm, margem 10 mm, repetida → 10 peças (2 × 5) na orientação retrato.
- **GD-IMP-21** **Livreto** (pós-Beta): frente/verso, encadernação *grampo* (cadernos encaixados), *colada* (folhas soltas empilhadas), *empilhar e cortar*; ordem de páginas calculada; nº de folhas por caderno. Impressora sem duplex → assistente indica como recolocar o papel.
- **GD-IMP-22** Marcas de corte por peça na grade; linhas de dobra no livreto.
- Modelos de imposição salvos com nome (`.gdimpo`); editar modelo de fábrica obriga *Salvar como*.
- **GD-IMP-23** Modelos de fábrica:
  - **Grade N-up**: 1×2, 2×1, 2×2, 2×3, 3×2, 3×3, 3×4, 4×3, 4×4 (colunas × linhas).
  - **Caderno de 8 páginas**, encadernação *colada* ou *grampo*, cada uma em 3 esquemas de frente e verso: *chapas separadas* (frente e verso com chapas próprias), *virar de lado* (mesma chapa, folha gira no eixo longo) e *virar de ponta-cabeça* (mesma chapa, folha gira no eixo curto).
  - **Dobras**: cartão tenda, livreto, dobra no topo, dobra lateral, dobra em três.
  - Aceite: escolher "caderno 8 grampo, virar de lado" num documento de 8 páginas → 1 folha, frente e verso usando a mesma chapa, ordem de páginas que resulta em 1–8 após dobra e corte.

---

## 4. Saída para produção

- **GD-IMP-40** **Relatório de produção**: tamanho final, chapas usadas, fontes, imagens (resolução efetiva), área (m²) e comprimento de corte (m) — para orçamento.
  - Aceite: arte 1 × 0,5 m com linha de corte retangular → área 0,5 m², corte 3,0 m.
- **GD-IMP-42** **Folha de informações do trabalho**: relatório de texto com seções marcáveis (aplicativo, driver, trabalho, chapas, fontes, vínculos); destino: arquivo .txt e/ou impressora escolhida.
- **GD-IMP-41** **Empacotar para gráfica**: copia para uma pasta o .gdr, imagens vinculadas, fontes (se licença permitir incorporar; senão lista), PDF/X de produção, relatório GD-IMP-40 e estilo de impressão usado. Assistente em passos:
  1. Modo: *coletar tudo automaticamente* (recomendado) ou *seguir um perfil de gráfica* (GD-IMP-43).
  2. (perfil) requisitos do perfil: driver/impressora exigida, papel e orientação exigidos, perfil ICC exigido × escolhido, arquivos exigidos × atuais — cada um com *Procurar…*; comentários da gráfica (só leitura).
  3. Formatos: incluir PDF ☐ + predefinição ▾ · incluir .gdr ☐ + versão ▾.
  4. Fontes: incluir ☐ + lista (nome, arquivo, permissão de incorporação).
  5. Perfis de cor: incluir ☐ + lista.
  6. Dados do remetente: nome, empresa, endereço, telefone, e-mail, nome do trabalho, comentários (vão para um `LEIA-ME.txt` na pasta).
  7. Destino: pasta + *compactar em .zip* ☐.
  8. Progresso → resumo com caminho e lista de arquivos; aviso se o perfil pede prova impressa.
  - Conflito de nome na pasta: *Substituir* / *Substituir todos* / *Cancelar*.
- **GD-IMP-43** **Perfil de gráfica** (`.gdperfil`, JSON): dados da empresa (nome, endereço, telefone, e-mail, site), comentários, o que deve ser entregue (PDF, .gdr, fontes, imagens, PS, prova impressa), driver/PPD exigido, perfil ICC exigido, papel exigido (largura, altura, orientação, margem mínima). Criado num editor próprio; usado pelo GD-IMP-41 e pelas regras de verificação (GD-IMP-31).
- **GD-IMP-44** Verificação em andamento mostra janela com página atual "n de N", itens analisados, barra de progresso e *Cancelar*.
- **GD-IMP-50** **Impressão com dados variáveis** (pós-Beta): fonte CSV/TXT (UTF-8); campos `«nome»` colocados como texto no desenho; campo numérico com formato (`000`, `0,0`) e incremento automático (início/fim); saída: imprimir direto ou gerar novo documento com uma página por registro. Barra de propriedades própria enquanto o documento tem fonte de dados: lista de campos + *Inserir*, registro anterior/próximo (prévia ao vivo), *Gerar documento*.
  - Aceite: CSV com 3 linhas e campo `«nome»` → documento novo com 3 páginas, cada uma com um nome.
- **GD-IMP-60** **Faixa e ilhós** (pós-Beta, liga com doc 17): gera **documento novo** com borda extra (cor da página, cor escolhida, esticar bordas da arte, espelhar bordas) e marcas de ilhós (círculos) por quantidade por lado ou por espaçamento, com margem até a borda; opção cantos/cima/baixo/esq/dir. Original intacto.
  - Aceite: faixa 3 × 1 m, ilhós a cada 500 mm, margem 20 mm, todos os lados → 7 em cima, 7 embaixo, 3 em cada lateral (cantos não duplicados) = 16.

---

## 5. Valores padrão (estilo "Padrão GraphDraw")

- **GD-IMP-70** Ao criar estilo novo ou *Restaurar padrões*:
  | Opção | Padrão |
  |---|---|
  | Posição | tamanho real, escala 100 %, proporção travada |
  | Mosaico | desligado; sobreposição 0; marcas de mosaico desligadas |
  | Sangria de saída | desligada; valor 3 mm (doc. métrico) ou 1/8 pol (doc. em polegadas) |
  | Marcas de corte | desligadas; *só borda externa* ligado; caixa das marcas automática |
  | Registro, barra de cores, escala de cinza, info, nº de página | desligados; escala de cinza: 0, 5, 25, 50, 75, 95, 100 % |
  | Negativo / espelhar | desligados |
  | Separações | desligadas; chapas vazias ocultas; texto sempre em preto desligado |
  | Preto sempre sobreimpresso | desligado; limiar 95 % |
  | Cor | preservar preto puro **ligado**; preservar números CMYK desligado; converter spots desligado; intenção colorimétrica relativa |
  | Reduzir resolução de imagens | desligado; cor/cinza 300 dpi; monocromático 1200 dpi |
  | PostScript | nível automático; sem compressão; degradê/curvas automáticos desligados; fontes incorporadas; planicidade 1,0; divisão de caminhos acima de 10 000 nós |
  | PDF (destino arquivo) | ver GD-IMP-18b |
  - Aceite: *Restaurar padrões* depois de mudar 5 opções → as 5 voltam aos valores da tabela; documento métrico mostra sangria 3,000 mm.
- **GD-IMP-71** Lineatura sugerida quando a tela é deixada em "automático" e o destino informa resolução: até 300 dpi → 60 lpi · 600 → 80 · 1000 → 100 · 1200–1693 → 133 · 2540 → 150 · ≥ 3386 → 200.

## 6. Limites dos campos

- **GD-IMP-72**
  | Campo | Mín. | Máx. | Passo |
  |---|---|---|---|
  | Cópias | 1 | 9999 | 1 |
  | Imprimir como imagem | 72 dpi | 2400 dpi | 1 |
  | Sangria, sobreposição de mosaico | 0 | 50 mm | 0,01 (3 casas) |
  | Tamanho do ladrilho (% da folha) | 0 | 25 % | 1 |
  | Peças H × V na imposição | 1 | 100 | 1 |
  | Cadernos | 1 | 16 | 1 |
  | Margens, calha, caixa das marcas | 0 (caixa: negativo permitido) | tamanho máximo de página (45,7 m) | 0,01 |
  | Reduzir resolução para | 1 dpi | 200 000 dpi | 1 |
  | Degraus da escala de cinza | 0 % | 100 % | 1 |
  | Qualidade JPEG/PDF | 1 | 100 | 1 (setas: 25) |
  | Subconjunto de fontes abaixo de | 0 % | 99 % | 1 |
  - Valor fora da faixa é recortado ao limite ao sair do campo, com dica mostrando a faixa.

## 7. Fora de escopo
Trapping automático e in-RIP (GD-IMP-14) · funções de tela personalizadas além de lpi/ângulo · conexão ODBC/Excel direta para dados variáveis (usar CSV) · formato de arquivo "para Macintosh".

---

## 8. Revisão do Time B (2026-10-19)

O doc 29 (GD-PRT, fase MVP) foi a primeira entrega da impressão e o doc 28
(GD-PRE, fase MVP) a da verificação prévia. Este doc os **estende**: onde
só acrescenta, os três valem juntos; onde o comportamento diverge, o
conflito fica em aberto (§8.3) até resposta do responsável pelo produto,
e a implementação daquele ponto espera.

### 8.1 Correspondência doc 29 (GD-PRT) ↔ doc 16 (GD-IMP)
| Doc 29 | Doc 16 | Relação |
|---|---|---|
| GD-PRT-01 Imprimir, Ctrl+P | §1 cabeçalho | igual |
| GD-PRT-02 envio rasterizado até 600 ppp | GD-IMP-05, GD-IMP-19 | **conflito** C-02 |
| GD-PRT-03 imprimir para arquivo PDF | GD-IMP-04, GD-IMP-18b | estende (PS, PRN, por página/chapa); **conflito** C-03 no padrão do PDF |
| GD-PRT-04 progresso e Cancelar | — | só no 29; vale |
| GD-PRT-05 opções guardadas na sessão | GD-IMP-06 | **conflito** C-09 |
| GD-PRT-10 impressora + Propriedades | GD-IMP-01 | estende (status, local, comentário) |
| GD-PRT-11 papel e orientação | GD-IMP-01 | estende (do driver, igual ao documento, personalizado) |
| GD-PRT-12 cópias e Agrupar | GD-IMP-01 | **conflito** C-07 (limite e rótulo) |
| GD-PRT-13 páginas | GD-IMP-01 | estende (pares/ímpares, `8-`, Seleção, Documentos abertos) |
| GD-PRT-20 escala real/ajustar/% | GD-IMP-07 | estende (Manual com L/A); **conflito** C-07 no limite da escala |
| GD-PRT-21 centralizar/canto + deslocamento | GD-IMP-07 | estende (âncora de 9 pontos, posição por página) |
| GD-PRT-22 girar automaticamente | GD-IMP-01 (*igual ao documento*), GD-IMP-03 (*orientação automática*) | compatível |
| GD-PRT-23 ladrilhar | GD-IMP-03 | estende (% da folha); **conflito** C-06 no padrão da sobreposição |
| GD-PRT-24 N-up | GD-IMP-20, GD-IMP-23 | estende (modelos, ordem, rotação); **conflito** C-01 (fase) |
| GD-PRT-30 marcas | GD-IMP-11, 11b, 11c | **conflito** C-04 (medidas) e C-05 (barra de cores) |
| GD-PRT-31 imprimir sangria | GD-IMP-10 | estende (valor próprio); **conflito** C-06 no padrão |
| GD-PRT-32 marcas que não cabem são omitidas | GD-IMP-11 (aviso), GD-IMP-30b (*Fora do papel*) | **conflito** C-04 |
| GD-PRT-40 espelhar | GD-IMP-16 | igual (16 acrescenta aviso) |
| GD-PRT-41 negativo | GD-IMP-16 | igual |
| GD-PRT-42 separações em cinza | GD-IMP-08, GD-IMP-12 | estende (tabela de chapas); **conflito** C-06 (chapas vazias) |
| GD-PRT-43 não imprimíveis não saem | — | só no 29; vale |
| GD-PRT-50 prévia na janela | GD-IMP-01 (miniatura), GD-IMP-02 (janela) | compatível |
| GD-PRT-51 resumo abaixo da prévia | — | só no 29; vale |
| GD-PRT-52 sem impressora | — | só no 29; vale |

### 8.2 Correspondência doc 28 (GD-PRE) ↔ doc 16 (GD-IMP)
| Doc 28 | Doc 16 | Relação |
|---|---|---|
| GD-PRE-01 painel Verificação contínuo | GD-IMP-30, 31, 44 | mesmo motor de regras (D-16-05); o painel roda na edição, a seção Problemas no diálogo |
| GD-PRE-02 limites por documento | GD-IMP-30b (limiares), GD-IMP-31 (perfil) | **conflito** C-08 |
| GD-PRE-03 cobertura de tinta 300 % | GD-IMP-30b (GraphDraw) 300 % | igual |
| GD-PRE-04 linha fina 0,25 pt / 0,5 pt | GD-IMP-30b contorno fino 0,1 mm | **conflito** C-08 |
| GD-PRE-05 zona de segurança do texto | — | só no 28; vale também em Problemas (D-16-15) |
| GD-PRE-06 encostado sem sangria | GD-IMP-30b *objeto fora da sangria* | regras diferentes; as duas valem |
| GD-PRE-07 preto sem sobreimpressão | GD-IMP-13 *preto sempre sobreimpresso* | complementares: um aponta o problema no objeto, o outro força na saída |
| GD-PRE-08 sobreimpressão na saída | GD-IMP-13 *Preservar* | igual; 16 acrescenta Ignorar e Simular |
| GD-PRE-10 estender até a sangria | — | só no 28 |
| GD-PRE-20/21 kerf | — | doc 17; fora da impressão |
| GD-PRE-30/31 aninhamento, mapa de tinta | — | só no 28 |

Itens do doc 16 sem par nos docs 28/29: GD-IMP-02 (janela), 05, 06, 10,
11b, 11c, 12–19, 18b, 21–23, 30b, 32, 33, 40–44, 50, 60, 70–72.

### 8.3 Conflitos em aberto
Cada um tem pergunta em `PERGUNTAS.md`; nada aqui foi decidido pelo Time B.

- **C-01 Fase (P-16).** Este doc põe GD-IMP-01..05 no M3 e o resto na
  Beta; o `MVP.md` da origem põe GD-IMP-01..03; o pedido de implementação
  fala em GD-IMP-01..07. O doc 29 inteiro é MVP e inclui N-up, separações,
  marcas e posição, cujos pares aqui (GD-IMP-20, 08/12, 11, 07) são Beta.
- **C-02 Caminho de envio (P-17).** GD-PRT-02 rasteriza sempre a folha
  (até 600 ppp). GD-IMP-05 trata rasterizar como saída de emergência
  opcional, o que supõe envio vetorial ao driver como padrão.
- **C-03 PDF de impressão (P-18).** GD-PRT-03 sai em PDF/X-1a, sem
  negativo (avisa). GD-IMP-18b tem padrão *Acrobat DC* (PDF 1.7) e não diz
  nada sobre negativo no PDF.
- **C-04 Marcas de corte (P-19).** GD-PRT-30: traços de 5 mm a 2 mm além da
  sangria impressa. GD-IMP-11: distância padrão 3 mm da página.
  GD-IMP-11b: 3,8 mm da linha de corte e 8,5 mm de comprimento (= sangria
  quando limitada e maior). Também: GD-PRT-32 omite marcas que não cabem;
  o doc 16 avisa e lista "Fora do papel" como problema, sem dizer se omite.
- **C-05 Barra de cores e escala (P-19).** GD-PRT-30: C, M, Y, K, CM, CY, MY
  e K 10–100 %, quadrados de 5 mm acima da página. GD-IMP-11: CMYK + spots
  de 0 a 100 % em passos de 10, mais escala de cinza de 7 degraus
  (GD-IMP-70) e uma escala por tinta na composta (GD-IMP-11c).
- **C-06 Valores padrão (P-20).** Sobreposição do mosaico: 10 mm
  (GD-PRT-23) × 0 (GD-IMP-70). Sangria de saída: igual à do documento
  (GD-PRT-31 e GD-IMP-10) × desligada com 3 mm / 1/8 pol (GD-IMP-70).
  Separações: as quatro chapas de processo sempre (GD-PRT-42) × chapas
  vazias ocultas (GD-IMP-12, 70).
- **C-07 Limites e rótulos (P-21).** Cópias 1–999 (GD-PRT-12) × 1–9999
  (GD-IMP-72). Rótulo *Agrupar* (GD-PRT-12, já no app) × *Intercalar*
  (GD-IMP-01). Escala 1–1000 % (GD-PRT-20) × sem limite em GD-IMP-72.
  "Tamanho do ladrilho (% da folha)" com máximo de 25 % (GD-IMP-72): não
  fica claro se é o tamanho do ladrilho ou a sobreposição em %.
- **C-08 Verificação (P-22).** Linha fina mínima 0,25 pt (GD-PRE-02) ×
  0,1 mm ≈ 0,28 pt (GD-IMP-30b). Limites guardados no documento
  (GD-PRE-02, `settings.preflight`) × regras e limiares por perfil de
  verificação independente do documento (GD-IMP-31).
- **C-09 Onde ficam as opções (P-24).** GD-PRT-05: opções por documento
  só durante a sessão, impressora entre sessões. GD-IMP-06: último estilo
  gravado no `.gdr`.
- **C-10 Sub-modos da Imposição (P-23).** GD-IMP-32 lista 4; o doc 28 da
  origem (§9, aqui doc 31) fala em 6.

### 8.4 Decisões
Valem para o que não está em conflito. As marcadas *provisória* seguem a
pergunta indicada.

- **D-16-01** Mosaico e imposição não se combinam (como no doc 29): ligar um desliga o outro.
- **D-16-02** *Provisória (P-21).* "Tamanho do ladrilho = % da largura da folha": o ladrilho tem p% da largura **e** p% da altura da área imprimível (mesma proporção da área).
- **D-16-03** *Ajustar à folha* reserva o espaço das marcas ligadas (como o doc 29).
- **D-16-04** A coluna *converter em processo* entra na tabela de Chapas (a seção Cor manda para lá, mas a tabela não a listava).
- **D-16-05** Problemas (GD-IMP-30/31) e o painel Verificação (GD-PRE-01) usam o mesmo motor de regras. *Provisória (P-22)* quanto a onde ficam regras e limiares.
- **D-16-06** *Provisória (P-15).* PostScript (GD-IMP-04/18) nesta fase sai **rasterizado** por folha (nível 2/3, imagem em faixas, cor RGB, cinza ou CMYK). As opções que só fazem sentido no vetorial (DSC, PPD, degradês, curvas, fontes, divisão acima de 10 000 nós, planicidade) ficam guardadas no estilo e desabilitadas com dica.
- **D-16-07** PRN = saída do driver da impressora escolhida gravada em arquivo (o spooler grava em vez de imprimir).
- **D-16-08** Livreto (GD-IMP-21, pós-Beta) entra já no modelo de imposição: grampo (cadernos encaixados), colada e empilhar-e-cortar, sem assistente de duplex manual nesta fase. Os modelos de caderno do GD-IMP-23 seguem a mesma fase do livreto.
- **D-16-09** GD-IMP-32 × GD-IMP-33: a prévia é a **janela** (GD-IMP-32). O painel compacto (GD-IMP-33) fica fora desta fase; o modelo de dados é o mesmo.
- **D-16-10** *Provisória (P-23).* Valem os **4** sub-modos do GD-IMP-32.
- **D-16-11** Dados variáveis (GD-IMP-50): o contexto da barra "dados variáveis" (GD-BPR-01) mostra campos ▾ · Inserir | ◀ registro n de N ▶ | Gerar documento.
- **D-16-12** Ilhós por espaçamento (GD-IMP-60): o espaçamento é o **máximo**; cada lado recebe `ceil(comprimento útil / espaçamento) + 1` ilhoses, distribuídos por igual, e os cantos não se repetem entre lados.
- **D-16-13** Opções do GD-IMP-18b que a exportação PDF (doc 15) ainda não gera (níveis de compatibilidade além dos existentes, LZW, JPEG 2000, EPS embutido, senhas e permissões) ficam guardadas no estilo e desabilitadas com dica até lá.
- **D-16-14** Sala limpa: menus e barras da prévia seguem a **função** listada aqui, com agrupamento, rótulos e ícones próprios (CLAUDE.md, regra 4); não se reproduz a disposição de nenhum produto.
- **D-16-15** A seção Problemas mostra também as regras do doc 28 sem par aqui (zona de segurança, encostado sem sangria, preto sem sobreimpressão), com a mesma correção do painel.
