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

| `type` | Campos | Geometria local |
|---|---|---|
| `path` | `subpaths [{closed, nodes [{p, in?, out?, kind}]}]`, `fillRule` (`evenodd` padrão / `nonzero`) | coordenadas dos nós |
| `rectangle` | `width`, `height`, `corners?` (4 × `{kind: round\|chamfer\|scallop, radius}`) | `[0,width]×[0,height]`; cantos: inf.-esq., inf.-dir., sup.-dir., sup.-esq. |
| `ellipse` | `rx`, `ry`, `mode` (`ellipse`/`arc`/`pie`), `startAngle`, `endAngle` (graus, anti-horário a partir de +X) | centrada em (0,0) |
| `polygon` | `rx`, `ry`, `sides` (3–500), `mode` (`polygon`/`star`/`complex`), `sharpness` (0–1) | centrado em (0,0), primeiro vértice em cima; `complex` = estrela {n/k} com par-ímpar |
| `group` | `children [objeto]` | — |
| `text` | `textKind` (`artistic`/`paragraph`), `frame? [w,h]`, `lineSpacing` (múltiplo do corpo, padrão 1,2), `paragraphs [{align, spans [{text, font {family, size, weight, italic}, fill?}]}]` | origem no canto superior esquerdo do bloco; linhas descem (−Y) |
| `image` | `width`, `height`, `src` (`resources/images/<sha256>.<ext>`) | `[0,width]×[0,height]`, topo da imagem em `y = height` |

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

Paradas: `{"offset": 0–1, "color": cor, "opacity?": 0–1}`. Coordenadas do
gradiente no espaço local do objeto. Tipos desconhecidos são preservados e
desenhados como "nenhum".

## Contorno

`{ "width", "color", "opacity?", "dash?": [traço, espaço, ...], "dashOffset?",
"join?": "miter|round|bevel", "miterLimit?", "cap?": "butt|round|square",
"position?": "center|inside|outside", "scaleWithObject?": true,
"behindFill?": false, "startArrow?": arrow, "endArrow?": arrow, "nibStretch?": 1, "nibAngle?": 0 }` — comprimentos em µm; `width: 0` = fio de cabelo.
Ponta de seta (GD-OUT-05, só em subcaminhos abertos): `{ "shape": "triangle|open|barbed|circle|square|bar",
"scale?": 1, "offset?": 0, "rotation?": 0, "mirror?": false }` — tamanho padrão 4 × a espessura
(mínimo 0,25 mm), `offset` em µm, `rotation` em graus; o traço recua para não passar da ponta.
Caneta caligráfica (GD-OUT-06): com `nibStretch` < 1 o traço é a área varrida por um bico elíptico
de largura `width`, achatado a `nibStretch` e girado `nibAngle` graus (pontas e cantos arredondados).
