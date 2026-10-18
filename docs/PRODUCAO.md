# Caminho até a produção (versão 1.0)

Lista de trabalho para lançar o GraphDraw para gráficas, comunicação visual
e corte. Fonte: requisitos dos docs 02–09 sem implementação, cruzados com o
código em 2026-10-17, mais o que falta para distribuir o programa.

Marque `[x]` no mesmo commit que fecha o item, citando os IDs.

Prioridade:
- **P0** — bloqueia o lançamento.
- **P1** — entra na 1.0 se der tempo; senão, 1.1.
- **P2** — proposta de ficar fora da 1.0 (precisa de aprovação).

## P0 — Segurança do trabalho do usuário
- [x] **Salvamento automático e recuperação**:
  - cópia periódica do documento alterado numa pasta de recuperação;
  - ao abrir depois de uma queda, oferecer recuperar;
  - salvar com sucesso ou fechar sem salvar apaga a cópia.
- [x] **Registro de falhas**: em pânico, gravar o log (versão, sistema,
  pilha) na pasta do usuário e mostrar onde está no próximo início.
- [x] **Salvar seguro**: gravar num arquivo temporário e trocar no fim
  (queda no meio não corrompe o `.gdr`); manter uma cópia `.bak` do
  anterior.

## P0 — Ferramentas de produção (gráfica e corte)
- [x] Faca: corta ao longo de reta, mão livre ou Bézier, com lacuna
  opcional (GD-FER-17).
- [x] Recortar: o retângulo apaga tudo fora dele (GD-FER-16).
- [x] Borracha (GD-FER-18).
- [x] Excluir segmento virtual: apaga o trecho entre interseções
  (GD-FER-19).
- [x] Mão livre com suavização (GD-FER-20).
- [x] Polilinha (GD-FER-25).
- [x] Linha 2 pontos (GD-FER-21).
- [x] Curva 3 pontos (GD-FER-26).
- [x] Caneta com prévia do próximo segmento (GD-FER-23): a Bézier já faz isso.
- [x] Transformação livre: girar, refletir, escalar e inclinar em torno
  de um ponto (GD-FER-03).
- [x] Cotas: horizontal, vertical, paralela e angular, ligadas aos nós,
  no formato da escala de desenho (GD-FER-40, GD-COT-01..03).

## P0 — Preenchimento e saída
- [x] Padrão vetorial, padrão bitmap e padrão de duas cores
  (GD-FIL-04/05/06), com PDF e SVG.
- [x] O padrão acompanha a transformação do objeto (GD-FIL-10).
- [x] Opção de preencher caminho aberto (GD-FIL-12).
- [x] Prévia de separações por chapa, com sobreimpressão simulada
  (GD-SEP-01).
- [x] Preto 100% K com sobreimpressão automática em texto pequeno e linha
  fina (GD-SEP-02): coberto pela verificação (GD-PRE-07/08).
- [x] O que fazer com o perfil de cor ao abrir ou importar (GD-CMS-04): ao abrir `.gdr`; imagens com ICC e `.cdr` ficam para depois.

## P0 — Distribuição
- [x] Ícone do programa e do arquivo `.gdr`, feitos a partir da logo do
  projeto e embutidos no executável.
- [x] Instalador Windows (NSIS, só para o usuário):
  - menu Iniciar;
  - associação de `.gdr` (e `.cdr` opcional);
  - desinstalador;
  - a ponte libcdr e as DLLs junto.
- [x] ~~Pacote Linux~~ — decisão do responsável (2026-10-17): a 1.0 é só
  Windows; o Linux fica para depois.
- [x] Release no CI: gerar o instalador por tag, com o checksum (artefato
  do job; publicar como release do GitHub fica para quando houver o
  repositório público).
- [x] Janela "Sobre": versão, licença e THIRD_PARTY.
- [ ] Versão 1.0.0 no workspace e um CHANGELOG. O CHANGELOG já existe; a
  versão sobe quando o responsável decidir o lançamento.
- [x] Atualizar o README (ainda diz "M1 em validação").

## P0 — Qualidade
- [x] Definição de pronto do MVP: um arquivo `.gdr` de teste em
  `tests/fixtures/` por requisito implementado. Auditoria (2026-10-17):
  todo recurso que grava dado no documento tem o seu (24 arquivos, todos
  validados pelo schema); os requisitos só de comportamento (desfazer,
  encaixe, interface) são cobertos pelos testes automatizados listados no
  ARCHITECTURE.
- [ ] Rodada de uso real com arquivos de clientes. O relatório de
  fidelidade fica privado (GD-FID-31); no repositório só números
  agregados.
- [x] Benchmark GD-NF-01/02 sem regressão depois das ferramentas novas
  (2026-10-17: abrir 100 mil objetos 1,3 s; redesenho 15,6 ms/quadro, 26,4 ms
  com recursos pesados).

## P0 — Pré-impressão e manufatura (avaliação de 2026-10-17)
Proposta de arquitetura trazida pelo responsável pelo produto, avaliada em
2026-10-17. Estes itens entram porque são o diferencial para gráfica e
corte; cada um vira requisito em spec nova (doc 28) antes do código.
- [x] Verificação prévia (preflight) contínua (doc 28, GD-PRE-01..08):
  - cobertura total de tinta acima do limite (300–320% em papel revestido,
    260% em não revestido), em falso tom por canal;
  - linha fina demais (menor que 0,25 pt em positivo, 0,5 pt em negativo),
    com botão de engrossar;
  - objeto na zona de segurança do corte;
  - preto 100% K sem sobreimpressão.

  Junta GD-SEP-02 e GD-DIM-31. Os limites acima são o padrão e ficam
  configuráveis por documento.
- [x] Compensação de kerf para laser/CNC (GD-PRE-20): deslocamento para fora nas
  peças e para dentro nos furos, calculado na exportação sem mexer no
  vetor original. É diferente da compensação de lâmina do vinil
  (GD-CRT-05).
- [x] Sangria automática (GD-PRE-10): objeto encostado na borda estende até a linha de
  sangria (opcional, por objeto).

## P1 — Da mesma avaliação
- [x] Aninhamento (nesting): encaixar as peças numa chapa de tamanho dado,
  com rotações permitidas, sentido da fibra e folga da ferramenta.
- [x] Booleanas não destrutivas: soldar, aparar e interseção como
  modificador. As formas originais continuam editáveis; "converter em
  curvas" fecha o resultado.

## Fora da 1.0 — da mesma avaliação
Para depois da 1.0, cada um com o motivo:
- **Renderização em GPU por compute shaders:** a renderização em CPU com
  faixas paralelas já cumpre GD-NF-02 (26 ms por quadro com 10 mil
  objetos). Trocar o motor agora atrasaria o lançamento sem ganho para o
  público-alvo. Fica no radar se arquivos reais passarem de 30 mil
  objetos.
- **Redes vetoriais (nó com 3+ arestas):** mudaria o modelo do `.gdr`, a
  importação `.cdr`, PDF, SVG, PLT/DXF e a edição de nós. Nenhum formato de
  saída da gráfica representa isso. O preenchimento inteligente (GD-FER-52,
  P1) cobre o caso de "pintar a região entre linhas".
- **Curvas de Euler (G²):** possível como ferramenta ou opção de suavizar,
  sem mudar o modelo (vira Bézier na saída). Pós-1.0.
- **Vetorização de bitmap:** já está fora do MVP (GD-VET-01).
- **Geração paramétrica (Voronoi, empacotamento de círculos):** fora do MVP.
- **Colaboração em tempo real (CRDT):** o produto é de uso local. O doc 19
  (comentários) está fora do MVP; colaboração exige servidor, conta e
  modelo de dados novo.

## P1
- [x] Conector reto, em ângulo e curvo, que segue os objetos (GD-FER-41,
  GD-CON-01).
- [x] Chamada (balão com seta, GD-FER-42): balão de fala das formas básicas.
- [x] Formas básicas: setas, fluxograma, balões (GD-PRI-07).
- [x] Preenchimento inteligente: área entre objetos vira objeto
  (GD-FER-52).
- [x] Simetria ao vivo (GD-FER-33).
- [x] B-spline (GD-BSP-01, GD-FER-24).
- [x] Largura variável (GD-VAR-01, GD-OUT-08).
- [x] Esboço contínuo (GD-FER-29).
- [x] Objeto vira guia (GD-GUI-03).
- [x] Grade de linha base e de pixels (GD-GRD-01).
- [x] Texto contornando objeto (GD-TXT-10).
- [x] Toque longo em amostra composta (GD-BDR-12).

## P2 — Proposta: depois da 1.0
- [ ] Unicode bidirecional e scripts complexos (GD-TXT-06).
- [x] Hifenização (GD-TXT-07): por parágrafo, com idioma; dicionário
  hyphen (`hyph_<idioma>.dic`) quando instalado, regras do português sem
  ele.
- [x] Corretor ortográfico (GD-TXT-08): Texto → Verificar ortografia,
  pelo corretor do Windows com os idiomas instalados nele.
- [x] Desenho inteligente (GD-FER-27): retângulo, elipse, triângulo, reta
  e seta, com nível de reconhecimento.
- [x] Fontes incorporadas no `.gdr` (GD-FNT-03, P-11 a): fonte inteira,
  opcional por documento; ao abrir, janela de fontes ausentes (trocar,
  curvas ou manter).
- [x] ~~Amostrar fora da janela~~ (GD-CTG-04) — P-12: fica só dentro da
  janela do programa; fora da 1.0.
- [ ] Efeitos (doc 10), estilos (doc 11) e o resto que o MVP.md deixa de
  fora.
