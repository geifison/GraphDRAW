# Formato `.gdr` 1.0

Formato nativo, aberto e documentado do GraphDraw (requisitos no
[doc 13](../spec/13-FORMATO-GDR.md)). O schema JSON de `document.json` está
em [`gdr-1.0.schema.json`](gdr-1.0.schema.json).

## Pacote

Arquivo ZIP. O **primeiro membro** é `mimetype`, **sem compressão** e sem
campo extra, com o conteúdo exato `application/vnd.graphdraw.document`.
Assim, os bytes 30–37 do arquivo são `mimetype` e os seguintes, o tipo —
basta ler o começo do arquivo para reconhecê-lo.

| Membro | Conteúdo |
|---|---|
| `mimetype` | `application/vnd.graphdraw.document` |
| `manifest.json` | `formatVersion`, `generator {name, version}`, `members [{path, sha256, size}]` (todos os membros exceto `mimetype` e o próprio manifesto) |
| `document.json` | configurações, página mestra, páginas, camadas e objetos |
| `pages/page-<n>.json` | página `n` (quando o documento é dividido; obrigatório se `document.json` passar de 20 MB) |
| `metadata.json` | `title`, `author`, `keywords[]`, `notes`, `created`, `modified` (ISO 8601) |
| `resources/colors.json` | `colors [{id, name, color}]` — cores globais/nomeadas |
| `resources/images/<sha256>.<ext>` | imagens (PNG/JPEG/TIFF/WebP), nome = SHA-256 do conteúdo |
| `resources/fonts/<sha256>.<ext>` | fontes inteiras (OTF/TTF/TTC) dos textos, gravadas com `settings.embedFonts` = `true` e só quando o fsType permite (instalável ou editável); ao ler, valem para o texto (GD-FNT-03) |
| `thumbnails/page-1.png` | miniatura da primeira página, lado maior 256 px |

Membros desconhecidos devem ser preservados por quem regrava o arquivo.

## Convenções

- **Unidade:** micrômetro (µm), número JSON (`f64`).
- **Eixo Y para cima**, origem no canto inferior esquerdo da página.
- **Matriz** `transform: [a, b, c, d, e, f]` leva coordenadas locais para as
  da página (ou do grupo pai): `x' = a·x + c·y + e`, `y' = b·x + d·y + f`.
- **IDs:** UUID v7 em texto para objetos, páginas e camadas.
- **Versão:** `formatVersion` "maior.menor". Leitores aceitam a mesma versão
  maior e preservam campos desconhecidos ao salvar.

## `document.json`

Guias (GD-GUI-01): `settings.guides` valem em todas as páginas e
`page.guides` só na página. Cada uma tem `kind` (`horizontal`, `vertical`,
`angled`), `position` (y da horizontal ou x da vertical, µm), `point` +
`angle` (inclinada: ponto por onde passa e graus a partir de +X), `locked?`
e `color?`. Listas vazias não são gravadas.

**Cota** (`dimension`, GD-COT-01..03):
- `points` no espaço local:
  - lineares e `segment`: `[a, b]`;
  - `angular`: `[vértice, a, b]`;
  - `radius`/`diameter`: `[centro, ponto da borda]`.
- `offset` (µm): distância da linha de cota (lineares), raio do arco
  (angular) ou guia além da borda (raio).
- `links`: para cada ponto, `null` ou `{ "object", "subpath", "node" }` — o
  nó medido (GD-COT-02); `segment` liga o segmento medido.
- O leitor resolve as ligações ao desenhar. O editor grava os pontos novos
  no mesmo passo da alteração.
- `format`:
  - `unit` (`mm`, `cm`, `m`, `in`, `pt`), `precision`, `prefix`, `suffix`;
  - `showUnit`, `textPosition` (`above`/`inline`);
  - `arrow` (forma de ponta ou `none`), `extensionLines`;
  - `textSize` (µm), `font`.
- O valor usa `settings.drawingScale` (GD-DOC-03). A vírgula é o separador
  decimal; raio leva "R" e diâmetro "Ø".
- A cor das linhas e do texto é a do `stroke` do objeto.

`settings.preflight` (opcional; ausente = padrão) guarda os limites da verificação
prévia (GD-PRE-02, doc 28): `tac` (%, padrão 300), `minLine` e `minLineNegative`
(µm; 0,25 e 0,5 pt), `safeZone` (µm, 5 mm), `nearEdge` (µm, 1,5 mm) e
`blackTextSize` (µm, 12 pt).

`settings.color` (opcional; ausente = padrão) guarda o gerenciamento de cor do
documento (GD-CMS-02): `rgbProfile` e `cmykProfile` (caminho de um arquivo
ICC; ausente = sRGB embutido / conversão por fórmula), `intent`
(`perceptual`, `relative`, `saturation`, `absolute`; padrão `relative`) e
`blackPoint` (compensação de ponto preto, padrão `true`). Os perfis não vão
dentro do `.gdr`; perfil ausente ao abrir cai no padrão com aviso.

```json
{
  "formatVersion": "1.0",
  "settings": { "displayUnit": "mm", "origin": "bottom-left", "bleed": 3000,
                "drawingScale": [1, 1], "resolution": 300 },
  "masterPage": { "layers": [ ... ] },
  "pages": [
    { "id": "…", "name": "Capa", "width": 210000, "height": 297000,
      "background": { "model": "rgb", "values": [1, 1, 1] },
      "hideMaster": false,
      "layers": [
        { "id": "…", "name": "Camada 1", "visible": true, "printable": true,
          "locked": false, "objects": [ ... ] }
      ] },
    { "src": "pages/page-2.json" }
  ]
}
```

Campos opcionais da página: `bleed` (sangria própria em µm; ausente = a de
`settings`), `locked` (página bloqueada para edição) e `tagColor` (etiqueta
de cor da aba, só para organização; não é impressa).

## Objetos

Campos comuns: `id`, `type`, `name?`, `transform`, `fill?`, `stroke?`,
`opacity?` (0–1), `visible`, `locked`, `printable?` (ausente = herda da
camada), `userData?` (texto → texto), `blendMode?` (`normal` se ausente; os 16 modos do PDF em
kebab-case: `multiply`, `screen`, `overlay`, `darken`, `lighten`, `color-dodge`, `color-burn`,
`hard-light`, `soft-light`, `difference`, `exclusion`, `hue`, `saturation`, `color`, `luminosity`;
valor desconhecido é lido como `normal`; GD-TRA-02).
`textWrap?` = `{style: contour|square, side: both|left|right|above-below, distance?}` (µm): o objeto
afasta o texto de parágrafo de primeiro nível da mesma página (GD-TXT-10). As áreas são recalculadas
ao ler e a cada mudança; não são gravadas no texto.

| `type` | Campos | Geometria local |
|---|---|---|
| `path` | `subpaths [{closed, nodes [{p, in?, out?, kind}]}]`, `fillRule` (`evenodd` padrão / `nonzero`) | coordenadas dos nós |
| `rectangle` | `width`, `height`, `corners?` (4 × `{kind: round\|chamfer\|scallop, radius}`) | `[0,width]×[0,height]`; cantos: inf.-esq., inf.-dir., sup.-dir., sup.-esq. |
| `ellipse` | `rx`, `ry`, `mode` (`ellipse`/`arc`/`pie`), `startAngle`, `endAngle` (graus, anti-horário a partir de +X) | centrada em (0,0) |
| `polygon` | `rx`, `ry`, `sides` (3–500), `mode` (`polygon`/`star`/`complex`), `sharpness` (0–1) | centrado em (0,0), primeiro vértice em cima; `complex` = estrela {n/k} com par-ímpar |
| `group` | `children [objeto]`, `boolean?`, `symmetry?` | `boolean` = `weld`, `trim` (os filhos de cima aparam o de baixo) ou `intersect`: o grupo mostra só o resultado, com o estilo do filho de baixo, e os filhos continuam editáveis (quem lê sem suporte vê os filhos);  `symmetry` = `{kind, count, center, angle?}`: o grupo desenha também as cópias — `mirror` com `count` eixos de espelho (o primeiro a `angle` graus, os outros a cada 180°/`count`) ou `radial` com `count` cópias giradas; `center` no espaço do grupo; quem lê sem suporte vê só os filhos, GD-FER-33 |
| `text` | `textKind` (`artistic`/`paragraph`), `frame? [w,h]`, `lineSpacing` (múltiplo do corpo, padrão 1,2), `paragraphs [{align, hyphenate?, lang?, spans [{text, font {family, size, weight, italic}, fill?}]}]` (`hyphenate` liga a hifenização no quadro; `lang` em BCP 47, ausente = `pt-BR`, GD-TXT-07), `path? {subpaths, offset, distance, side above|below, orientation rotate|skew-v|skew-h|upright, mirror}` (texto em caminho, GD-TXT-09; caminho em coordenadas locais do texto) | origem no canto superior esquerdo do bloco; linhas descem (−Y) |
| `image` | `width`, `height`, `src` (`resources/images/<sha256>.<ext>`) | `[0,width]×[0,height]`, topo da imagem em `y = height` |
| `basic` | `shape` (biblioteca própria: `arrow`, `double-arrow`, `chevron`, `flow-process`, `flow-decision`, `flow-terminator`, `flow-data`, `flow-document`, `banner`, `speech`, `thought`, `plus`, `heart`), `width`, `height`, `params?` | `[0,width]×[0,height]`; parâmetros de ajuste em fração da caixa (a ponta dos balões pode sair dela), GD-PRI-07 |
| `connector` | `style` (`straight`, `elbow`, `curved`), `from`, `to`, `radius?` | pontas `{point, object?, anchor?}`: `point` no espaço do objeto; presa, `object` é o ID e `anchor` o encaixe relativo à caixa dele (0–1: cantos, meios dos lados, centro) e `point` é regravado quando o objeto se move; `radius` arredonda os cantos do `elbow`, GD-CON-01 |
| `bspline` | `points` (`{p, fixed?}`), `closed?` | B-spline cúbica uniforme pelos pontos de controle (espaço local); `fixed` = a curva passa pelo ponto (com quina); aberta, começa e termina nas pontas, GD-BSP-01 |
| `dimension` | `dimKind` (`horizontal`, `vertical`, `aligned`, `angular`, `radius`, `diameter`, `segment`), `points`, `offset?`, `links?`, `segment?`, `format?` | pontos medidos no espaço local (GD-COT-01..03; ver abaixo) |

Segmento entre dois nós: reto se nem `out` do primeiro nem `in` do segundo
existem; senão, Bézier cúbica (alça ausente = o próprio nó). `in`/`out` são
pontos absolutos no espaço local. Em subcaminho fechado há segmento do último
nó ao primeiro.

`size` da fonte em µm (12 pt = 4233,33 µm). Se a família não existir no
sistema, o texto é exibido com uma substituta e o nome original é mantido.

Tipos desconhecidos são preservados e não exibidos.

## Cor

`{ "model", "values", "name?", "ref?", "alternate?" }`

| `model` | `values` |
|---|---|
| `rgb` | 3 componentes 0–1 |
| `cmyk` | 4 componentes 0–1 |
| `gray` | 1 componente 0–1 (0 = preto) |
| `lab` | L* 0–100, a* e b* −128–127 (CIE, D50) |
| `hsb` | matiz, saturação, brilho, 0–1 (matiz 1 = 360°) |
| `hls` | matiz, luminosidade, saturação, 0–1 |
| `spot` | tinta 0–1; `name` obrigatório (nome da tinta); `alternate` = cor de prévia/saída alternativa |
| `registration` | tinta 0–1 (100% em todas as chapas) |

`ref` aponta para o `id` de uma cor em `resources/colors.json` (cor global).

## Preenchimento

- `{"type": "none"}`
- `{"type": "solid", "color": cor, "opacity?": 0–1, "overprint?": bool}`
- `{"type": "gradient", "kind": "linear", "start": [x,y], "end": [x,y], "stops": [...], "spread?": "pad|repeat|reflect"}`
- `{"type": "gradient", "kind": "radial", "center": [x,y], "radius": r, "stops": [...]}`
- `{"type": "gradient", "kind": "elliptic|conic|rectangular", "center": [x,y], "radius": rx, "radiusY?": ry, "angle?": graus, "stops": [...]}` —
  elíptico: t = distância elíptica (rx, ry); retangular: t = maior entre |u|/rx e |v|/ry; cônico: t = ângulo
  (0 em `angle`, sentido anti-horário) ÷ 360°. Eixos u, v girados por `angle`. Tipo desconhecido de
  gradiente é desenhado como linear.
- Opcionais de todo gradiente: `steps` (número de faixas de cor única; ausente = contínuo),
  `smooth` (pontilhado contra faixas na tela e no raster; o PDF e o SVG já são contínuos) e
  `colorPath` (`direct`, padrão, mistura em linha reta; `cw`/`ccw` pela roda de matiz, matiz
  crescente/decrescente). Cada parada pode ter `midpoint` (0–1, padrão 0,5): onde, no trecho até
  a parada seguinte, a mistura chega a 50/50. Quem lê sem suporte desenha a transição simples.

- `{"type": "pattern", "kind": "vector|bitmap|two-color|texture", "width", "height", ...}` — padrão
  (GD-FIL-04..06):
  - `texture` = textura procedural: `texture: {style: clouds|marble|wood|granite, seed, frequency
    (células por ladrilho), detail (oitavas, 1–8), contrast (0–1)}` entre `bg` e `fg`; o ruído se
    repete a cada ladrilho, então emenda;
  - ladrilho: `tile` (objetos em `[0,width]×[0,height]`), `src` (imagem dos
    recursos) ou `cells` (linhas de `0`/`1`, a de cima primeiro) com `fg`/`bg`;
  - `origin`, `scale`, `rotation` (graus);
  - `rowOffset`/`columnOffset` em fração do ladrilho;
  - `mirror` espelha ladrilhos alternados (xadrez);
  - `transformWithObject` (padrão `true`, GD-FIL-10) faz o padrão seguir a
    matriz do objeto; com `false`, ele fica preso à página;
  - `resolution` (ppp, padrão 300) é a da rasterização do ladrilho.

`settings.fillOpenPaths` (padrão `true`, GD-FIL-12): com `false`, caminhos
só com subcaminhos abertos não recebem preenchimento.

Paradas: `{"offset": 0–1, "color": cor, "opacity?": 0–1}`. Coordenadas do
gradiente no espaço local do objeto. Tipos desconhecidos são preservados e
desenhados como "nenhum".

## Contorno

`{ "width", "color", "opacity?", "dash?": [traço, espaço, ...], "dashOffset?",
"join?": "miter|round|bevel", "miterLimit?", "cap?": "butt|round|square",
"position?": "center|inside|outside", "scaleWithObject?": true,
"behindFill?": false, "startArrow?": arrow, "endArrow?": arrow, "nibStretch?": 1, "nibAngle?": 0, "overprint?": false, "dashCorners?": "standard", "widths?": [{at, left, right}] }` — comprimentos em µm; `width: 0` = fio de cabelo.
`overprint` (GD-PRE-08): o contorno sobreimprime (no PDF, `/OP true` com `OPM 1`); o
preenchimento sólido tem o mesmo campo.
Ponta de seta (GD-OUT-05, só em subcaminhos abertos): `{ "shape": "triangle|open|barbed|circle|square|bar",
"scale?": 1, "offset?": 0, "rotation?": 0, "mirror?": false }` — tamanho padrão 4 × a espessura
(mínimo 0,25 mm), `offset` em µm, `rotation` em graus; o traço recua para não passar da ponta.
Tracejado nos cantos (GD-OUT-03): `dashCorners` = `standard` (o padrão segue o caminho todo),
`align` (em cada trecho entre cantos o padrão estica ou encolhe para começar e terminar num traço:
um traço inteiro passa por cada canto) ou `fixed` (o padrão recomeça em cada canto, com os
comprimentos exatos). Quem lê sem suporte desenha `standard`.

Largura variável (GD-VAR-01, GD-OUT-08): `widths` são nós ao longo do caminho — `at` é a
posição (0–1 do comprimento de cada trecho), `left`/`right` multiplicam a meia espessura
de cada lado; entre os nós a largura muda suavemente (smoothstep), antes do primeiro e depois
do último fica a do nó. O traço é a área entre as duas bordas (nos fechados, um anel); tem
precedência sobre a caneta caligráfica.

Caneta caligráfica (GD-OUT-06): com `nibStretch` < 1 o traço é a área varrida por um bico elíptico
de largura `width`, achatado a `nibStretch` e girado `nibAngle` graus (pontas e cantos arredondados).
