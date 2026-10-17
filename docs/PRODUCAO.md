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
- [ ] Faca: corta ao longo de reta, mão livre ou Bézier, com lacuna
  opcional (GD-FER-17).
- [ ] Recortar: o retângulo apaga tudo fora dele (GD-FER-16).
- [ ] Borracha (GD-FER-18).
- [ ] Excluir segmento virtual: apaga o trecho entre interseções
  (GD-FER-19).
- [ ] Mão livre com suavização (GD-FER-20).
- [ ] Polilinha (GD-FER-25).
- [ ] Linha 2 pontos (GD-FER-21).
- [ ] Curva 3 pontos (GD-FER-26).
- [ ] Caneta com prévia do próximo segmento (GD-FER-23).
- [ ] Transformação livre: girar, refletir, escalar e inclinar em torno
  de um ponto (GD-FER-03).
- [ ] Cotas: horizontal, vertical, paralela e angular, ligadas aos nós,
  no formato da escala de desenho (GD-FER-40, GD-COT-01..03).

## P0 — Preenchimento e saída
- [ ] Padrão vetorial, padrão bitmap e padrão de duas cores
  (GD-FIL-04/05/06), com PDF e SVG.
- [ ] O padrão acompanha a transformação do objeto (GD-FIL-10).
- [ ] Opção de preencher caminho aberto (GD-FIL-12).
- [ ] Prévia de separações por chapa, com sobreimpressão simulada
  (GD-SEP-01).
- [ ] Preto 100% K com sobreimpressão automática em texto pequeno e linha
  fina (GD-SEP-02).
- [ ] O que fazer com o perfil de cor ao abrir ou importar (GD-CMS-04).

## P0 — Distribuição
- [x] Ícone do programa e do arquivo `.gdr`, feitos a partir da logo do
  projeto e embutidos no executável.
- [ ] Instalador Windows:
  - menu Iniciar;
  - associação de `.gdr` (e `.cdr` opcional);
  - desinstalador;
  - a ponte libcdr e as DLLs junto.
- [ ] Pacote Linux (AppImage ou .deb), se o Linux entrar na 1.0.
- [ ] Release no CI: gerar o instalador por tag, com o checksum.
- [ ] Janela "Sobre": versão, licença e THIRD_PARTY.
- [ ] Versão 1.0.0 no workspace e um CHANGELOG.
- [x] Atualizar o README (ainda diz "M1 em validação").

## P0 — Qualidade
- [ ] Definição de pronto do MVP: um arquivo `.gdr` de teste em
  `tests/fixtures/` por requisito implementado.
- [ ] Rodada de uso real com arquivos de clientes. O relatório de
  fidelidade fica privado (GD-FID-31); no repositório só números
  agregados.
- [ ] Benchmark GD-NF-01/02 sem regressão depois das ferramentas novas.

## P1
- [ ] Conector reto, em ângulo e curvo, que segue os objetos (GD-FER-41,
  GD-CON-01).
- [ ] Chamada (balão com seta, GD-FER-42).
- [ ] Formas básicas: setas, fluxograma, balões (GD-PRI-07).
- [ ] Preenchimento inteligente: área entre objetos vira objeto
  (GD-FER-52).
- [ ] Simetria ao vivo (GD-FER-33).
- [ ] B-spline (GD-BSP-01, GD-FER-24).
- [ ] Largura variável (GD-VAR-01, GD-OUT-08).
- [ ] Esboço contínuo (GD-FER-29).
- [ ] Objeto vira guia (GD-GUI-03).
- [ ] Grade de linha base e de pixels (GD-GRD-01).
- [ ] Texto contornando objeto (GD-TXT-10).
- [ ] Toque longo em amostra composta (GD-BDR-12).

## P2 — Proposta: depois da 1.0
- [ ] Unicode bidirecional e scripts complexos (GD-TXT-06).
- [ ] Hifenização (GD-TXT-07).
- [ ] Corretor ortográfico (GD-TXT-08).
- [ ] Desenho inteligente (GD-FER-27).
- [ ] Fontes incorporadas no `.gdr` (GD-FNT-03; espera a resposta da
  P-11).
- [ ] Amostrar fora da janela (GD-CTG-04; espera a resposta da P-12).
- [ ] Efeitos (doc 10), estilos (doc 11) e o resto que o MVP.md deixa de
  fora.
