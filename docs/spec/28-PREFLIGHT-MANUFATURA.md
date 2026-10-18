# 28 — Verificação prévia (preflight) e preparação para manufatura

Origem: proposta de arquitetura do responsável pelo produto
(2026-10-17), avaliada em `docs/PRODUCAO.md`. Complementa o doc 16
(separações), o doc 17 (corte) e o doc 22 (GD-DIM-31).

Fase: **MVP** (entra na 1.0), exceto onde indicado.

## 1. Verificação prévia contínua
- **GD-PRE-01** A verificação roda sobre a página atual enquanto se edita
  e lista os problemas no painel **Verificação**.
  - Cada problema mostra o tipo, o objeto e o valor encontrado.
  - O clique seleciona o objeto; o botão **Corrigir** aplica a correção
    automática, quando há.
  - A barra de status mostra o total de problemas; o clique abre o painel.
- **GD-PRE-02** Limites por documento (`settings.preflight`):

| Limite | Padrão |
|---|---|
| Cobertura total de tinta (TAC) | 300% (papel revestido; 260% para não revestido) |
| Espessura mínima de linha positiva | 0,25 pt |
| Espessura mínima de linha negativa (clara sobre escura, ou branca) | 0,5 pt |
| Zona de segurança do texto até o corte | 5 mm |
| Distância da borda considerada "encostada" sem sangria | 1,5 mm (GD-DIM-31) |
| Corpo até o qual texto preto exige sobreimpressão | 12 pt |

  - Uma predefinição troca a TAC entre 300% e 260%; cada valor também é
    editável.
- **GD-PRE-03** **Cobertura de tinta**: o preenchimento ou o contorno com
  C+M+Y+K acima do limite é um problema.
  - A conta usa a cor CMYK do modelo; RGB/Lab convertem pelo perfil do
    documento (GD-CMS-01).
  - Gradiente: vale a parada mais carregada.
  - Correção: reduzir a cor até o limite, tirando do CMY e mantendo o K.
  - Imagens ficam para depois (Beta).
- **GD-PRE-04** **Linha fina**: a espessura efetiva menor que o mínimo é
  um problema; a espessura efetiva considera "escalar com o objeto".
  - Linha negativa é a de cor clara (luminância ≥ 0,8), ou que fura uma
    área escura; ela usa o mínimo negativo.
  - Correção: engrossar até o mínimo.
- **GD-PRE-05** **Zona de segurança**: um texto que entra na faixa entre a
  linha de corte e a zona de segurança é um problema. Não há correção
  automática.
- **GD-PRE-06** **Encostado sem sangria** (com sangria > 0): um objeto
  cuja borda fica a menos da distância de encostado do corte, sem chegar à
  sangria, é um problema. Correção: estender até a sangria (GD-PRE-10).
- **GD-PRE-07** **Preto sem sobreimpressão**: um texto 100% K até o corpo
  limite, ou uma linha 100% K, sem sobreimpressão é um problema.
  Correção: ligar a sobreimpressão.
- **GD-PRE-08** **Sobreimpressão na saída**:
  - Preenchimento e contorno têm a opção sobreimpressão.
  - No PDF, a sobreimpressão vai no estado gráfico (`OP`/`op` e `OPM 1`).
  - Na tela, a sobreimpressão simulada é a prévia de separações (doc 16,
    Beta).

## 2. Sangria automática
- **GD-PRE-10** **Estender até a sangria**, nos objetos selecionados (ou na
  correção de GD-PRE-06):
  - O lado encostado no corte (dentro da distância de encostado) vai até
    o corte mais a sangria.
  - O lado oposto fica parado.
  - Um passo de desfazer.
  - Imagens e formas esticam só no eixo do lado estendido.

## 3. Compensação de kerf (laser/CNC)
- **GD-PRE-20** Opção **Kerf** (largura do corte, mm) na saída para corte
  (doc 17). Cada contorno fechado é deslocado meio kerf:
  - para fora, nas peças (profundidade par de aninhamento);
  - para dentro, nos furos (profundidade ímpar).

  Linhas abertas não mudam. O desenho original não muda.
  - Aceite: quadrado de 100 mm com furo de 20 mm e kerf 0,2 mm → percurso
    externo de 100,2 mm e furo de 19,8 mm.
- **GD-PRE-21** A compensação de kerf é diferente da compensação de lâmina
  (GD-CRT-05), que trata o arrasto da faca de vinil. As duas podem ser
  usadas juntas.

## 4. Fora da 1.0
- **GD-PRE-30** Aninhamento (nesting) em chapa: rotações permitidas,
  sentido da fibra, folga da ferramenta. Feito: Objeto → Aninhar na chapa
  (grade de células, forma real com furos cheios, preenchimento
  inferior-esquerdo, várias chapas).
- **GD-PRE-31** Mapa de cobertura de tinta por pixel, em falso tom, para
  imagens e transparências: depende da prévia de separações (doc 16).
  Fase: Beta.

## Aceite
- Retângulo C100 M100 Y100 K100 → problema de TAC de 400%. Corrigir →
  TAC ≤ 300% com K 100%.
- Linha de 0,1 pt preta → linha fina; corrigir → 0,25 pt.
- Linha branca de 0,3 pt → linha fina (negativa).
- Texto preto de 8 pt sem sobreimpressão → problema; corrigir → o PDF
  traz `/OP true` no estado gráfico do texto.
- A4 com sangria de 3 mm e fundo a 1 mm da borda → encostado. Estender →
  a borda vai até −3 mm.
