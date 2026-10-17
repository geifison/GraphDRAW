# 22 — Dimensionamento e gestão de páginas

Origem: requisito do responsável pelo produto (2026-10-16). Complementa
GD-PAG-01/02 (doc 02), GD-UI-04 (doc 20) e o doc 21. A parte de saída
(momento 3) depende do doc 16 (imposição), hoje fora do MVP.

## Linguagem visual
- **GD-DIM-01..04** Cores, sombra da página, tipografia e ícones seguem
  `docs/design/DESIGN-SYSTEM.md` (fonte única de design desde 2026-10-16).
  Ícones específicos deste documento, no estilo do guia: orientação (folha +
  seta curva; gira 90° em 150 ms ao clicar), sangria (contorno contínuo +
  contorno tracejado com marcas de corte), ajustar à arte (quatro cantoneiras
  em volta de uma forma), imposição (folha 2×2), escopo isolado (folha com
  trava) e vinculado (três folhas com elo).

## Momento 1 — Novo documento
- **GD-DIM-10** "Novo" abre um hub em duas colunas em vez de criar página
  em branco direto.
- **GD-DIM-11** Coluna esquerda: biblioteca de predefinições com busca em
  tempo real (`/` foca a busca), filtros por categoria (Impressão, Telas/UI,
  Social, Personalizado) e cartões proporcionais ao formato, com medidas,
  resolução recomendada e modelo de cor (ex.: "A5 · 148 × 210 mm · 300 ppp
  · CMYK").
- **GD-DIM-12** Coluna direita: prévia proporcional da folha; largura,
  altura, unidade (mm, cm, in, px, pt — converte o valor ao trocar),
  orientação, sangria, resolução e modelo de cor.
- **GD-DIM-13** Selo de aproveitamento: quantas unidades do formato cabem
  numa folha A4 e numa A3, já considerando girar 90° (ex.: "Cabe 2× em A4").
- **GD-DIM-14** O modelo de cor escolhido fica gravado no documento; o
  perfil ICC entra com o gerenciamento de cor (GD-CMS, M2).

## Momento 2 — Edição
- **GD-DIM-20** *(removido em 2026-10-16 a pedido do responsável pelo produto: os dados da página já ficam na barra de propriedades quando nada está selecionado.)*
- **GD-DIM-21** Sangria visível no canvas: contorno tracejado ao redor da
  página na distância da sangria, com marcas de corte nos cantos.
- **GD-DIM-22** Ajustar à arte: a página passa a ter o tamanho da caixa
  visual dos objetos (com contorno); os objetos são deslocados para que a
  arte comece no canto da página. Um passo no histórico.
- **GD-DIM-23** Ferramenta Prancheta (`Shift+P`): alças nos cantos e meios
  das bordas da página; arrastar redimensiona.
- **GD-DIM-24** Encaixe em formatos: ao passar perto de um formato
  conhecido (ISO A0–A6, B4/B5, Carta, Ofício), a borda gruda nele e um selo
  mostra "Encaixado em: ISO A5". Alt desliga o encaixe durante o arrasto.
- **GD-DIM-25** Ao soltar com objetos na página, escolher: *Manter posição*
  (objetos ficam onde estão em relação ao papel que não se moveu) ou
  *Escalar proporcional* (escala uniforme e centralização para caber na nova
  página). Vale para todas as páginas do documento (GD-PAG-01), cada uma com
  o mesmo ajuste nos objetos. Um passo só no histórico; Esc cancela.

## Momento 3 — Saída (depois do momento 2; ver doc 16)
- **GD-DIM-30** Simulador de aproveitamento: melhor encaixe (com rotação)
  de páginas numa folha de saída, com unidades por folha e % de refugo.
- **GD-DIM-31** Inspetor de linha de corte: destaca objetos a menos de
  1,5 mm da borda sem chegar à sangria.
- **GD-DIM-32** Ladrilhamento para grande formato: ladrilhos numerados (A1,
  A2…) com sobreposição configurável (padrão 10 mm) e guias de colagem.
- **GD-DIM-33** Marcas de corte e barras de cor na saída.

## Aceite
- Buscar "A5" no hub mostra o cartão A5; o selo diz "Cabe 2× em A4".
- Trocar a unidade de mm para px com 210 mm → 793,7 px (96 ppp); voltar dá
  210 mm.
- Prancheta: arrastar o canto de uma A4 até perto de 148 × 210 mm encaixa
  em A5 e mostra "Encaixado em: ISO A5".
- Redimensionar com objetos e escolher *Manter posição*: os objetos ficam
  no mesmo lugar em relação ao canto fixo; desfazer volta página e objetos
  num passo.
- Ajustar à arte numa página com um único retângulo de 50×30 mm sem
  contorno → página de 50×30 mm com o retângulo encostado nas bordas.
