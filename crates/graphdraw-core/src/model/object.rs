//! Objetos do documento (doc 02, GD-OBJ-01..05, GD-GDR-05).
//!
//! Serialização feita via `serde_json::Value` para (1) preservar campos
//! desconhecidos (GD-GDR-03) e (2) nunca falhar na leitura: um objeto
//! inválido vira `ObjectKind::Unknown` com a mensagem de erro, e o resto do
//! documento abre normalmente (GD-GDR-10).

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use super::geom::{FillRule, Subpath, Transform};
use super::paint::{Fill, Stroke};

/// Gera um ID novo (UUID v7, GD-GDR-04).
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub id: String,
    pub name: Option<String>,
    pub transform: Transform,
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
    pub opacity: Option<f64>,
    pub visible: bool,
    pub locked: bool,
    /// `None` = herda da camada (GD-OBJ-04).
    pub printable: Option<bool>,
    pub user_data: Option<BTreeMap<String, String>>,
    /// Modo de mesclagem com o que está embaixo (GD-TRA-02).
    pub blend: BlendMode,
    /// Afasta o texto de parágrafo (GD-TXT-10).
    pub wrap: Option<TextWrap>,
    /// Linha de construção (GD-TXT-09): aparece na tela, mas não sai na
    /// impressão, no PDF/SVG nem no corte (ex.: o traço que guia um texto).
    pub construction: bool,
    /// Estilo de objeto e estilo de texto ligados (doc 11).
    pub style: Option<String>,
    pub text_style: Option<String>,
    pub kind: ObjectKind,
}

/// Modos de mesclagem (GD-TRA-02): os 16 do PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl BlendMode {
    pub const ALL: [BlendMode; 16] = [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::ColorDodge,
        BlendMode::ColorBurn,
        BlendMode::HardLight,
        BlendMode::SoftLight,
        BlendMode::Difference,
        BlendMode::Exclusion,
        BlendMode::Hue,
        BlendMode::Saturation,
        BlendMode::Color,
        BlendMode::Luminosity,
    ];
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectKind {
    Path(PathData),
    Rectangle(RectData),
    Ellipse(EllipseData),
    Polygon(PolygonData),
    /// Espiral viva (GD-PRI-05): continua editável até virar curvas.
    Spiral(SpiralData),
    Group(GroupData),
    Text(TextData),
    Image(ImageData),
    /// Cota (GD-COT-01..03, doc 08).
    Dimension(DimensionData),
    /// Forma básica paramétrica (GD-PRI-07).
    Basic(BasicShapeData),
    /// Conector entre objetos (GD-FER-41, GD-CON-01).
    Connector(ConnectorData),
    /// B-spline cúbica uniforme (GD-BSP-01).
    BSpline(BSplineData),
    /// Tipo desconhecido ou objeto inválido: JSON original preservado.
    Unknown {
        raw: Value,
        error: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathData {
    pub subpaths: Vec<Subpath>,
    #[serde(default)]
    pub fill_rule: FillRule,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Canto de retângulo (GD-PRI-01). `kind`: `round`, `chamfer`, `scallop`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Corner {
    pub kind: String,
    pub radius: f64,
}

/// Retângulo paramétrico ocupando `[0,width]×[0,height]` no espaço local.
/// `corners`, quando presente, segue a ordem: inferior-esquerdo,
/// inferior-direito, superior-direito, superior-esquerdo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RectData {
    pub width: f64,
    pub height: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corners: Option<Vec<Corner>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Elipse paramétrica centrada na origem local (GD-PRI-02).
/// Ângulos em graus, anti-horário a partir de +X.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EllipseData {
    pub rx: f64,
    pub ry: f64,
    /// `ellipse`, `arc` ou `pie`.
    #[serde(default = "ellipse_mode")]
    pub mode: String,
    #[serde(default)]
    pub start_angle: f64,
    #[serde(default = "full_turn")]
    pub end_angle: f64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Polígono ou estrela paramétricos (GD-PRI-03/04), centrados na origem
/// local e inscritos na elipse de raios `rx`, `ry`; o primeiro vértice fica
/// em cima.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolygonData {
    pub rx: f64,
    pub ry: f64,
    /// Lados (polígono) ou pontas (estrelas), 3–500.
    pub sides: u32,
    /// `polygon`, `star` (pontas alternando raio externo e interno) ou
    /// `complex` (estrela de linhas cruzadas, com auto-interseção).
    #[serde(default = "polygon_mode")]
    pub mode: String,
    /// Nitidez da ponta, 0–1: na estrela, quanto o raio interno encolhe;
    /// na complexa, quantos vértices cada linha pula.
    #[serde(default = "half")]
    pub sharpness: f64,
    /// Curvatura simétrica dos lados: ponto de controle de cada lado em
    /// [posição ao longo do lado 0–1, afastamento perpendicular em fração
    /// do comprimento, positivo para fora]; `None` = lados retos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bend: Option<[f64; 2]>,
    /// Raio de arredondamento (µm) das pontas (vértices externos) e dos
    /// vales (vértices internos das estrelas). Vale com os lados retos.
    #[serde(default, skip_serializing_if = "poly_zero")]
    pub corner_outer: f64,
    #[serde(default, skip_serializing_if = "poly_zero")]
    pub corner_inner: f64,
    /// Estrela complexa: `true` preenche os cruzamentos (regra não-zero);
    /// `false` deixa o miolo vazado (par-ímpar).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fill_crossings: bool,
    /// Vértices tirados da simetria: índice na sequência de vértices
    /// (estrela: externos nos pares, internos nos ímpares) e posição local.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub free_nodes: Vec<FreeNode>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Espiral (GD-PRI-05) no quadro local (0, 0)–(width, height): começa no
/// centro e termina na borda depois de `turns` voltas; simétrica
/// (Arquimedes) ou logarítmica (o raio multiplica por `expansion` a cada
/// volta). Com `ribbon` = [largura no centro, largura na borda] (µm) vira
/// uma fita fechada, pronta para corte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpiralData {
    pub width: f64,
    pub height: f64,
    pub turns: f64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub logarithmic: bool,
    #[serde(default = "spiral_expansion")]
    pub expansion: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ribbon: Option<[f64; 2]>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn spiral_expansion() -> f64 {
    2.0
}

/// Vértice do polígono fora da simetria (posição local própria).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreeNode {
    pub index: u32,
    pub at: [f64; 2],
}

fn poly_zero(v: &f64) -> bool {
    *v == 0.0
}

fn polygon_mode() -> String {
    "polygon".into()
}

fn half() -> f64 {
    0.5
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupData {
    pub children: Vec<Object>,
    /// Booleana não destrutiva: `weld`, `trim` (os de cima aparam o de
    /// baixo) ou `intersect`; o grupo mostra só o resultado, com o estilo do
    /// filho de baixo, e os filhos continuam editáveis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boolean: Option<String>,
    /// Instância do símbolo (doc 11): os filhos são cópias do mestre.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// Efeito dinâmico (doc 10): o grupo desenha o resultado gerado.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<Effect>,
    /// Simetria ao vivo (GD-FER-33): o grupo desenha também as cópias.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symmetry: Option<Symmetry>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Efeito dinâmico do grupo (doc 10). Medidas em µm, ângulos em graus,
/// acelerações de −100 a 100 (0 = linear).
/// - `contour`: `direction` = `outside`/`inside`/`center` (para dentro até
///   fechar), `steps` anéis a cada `distance`, cantos `corners`
///   (`round`/`miter`/`bevel`), cor da origem até `endColor` com `accel`;
/// - `blend`: `steps` passos do primeiro ao segundo filho, `rotation` total
///   dos passos, `accel` no espaçamento e `colorAccel` na cor;
/// - `drop-shadow`: `offset`, `blur`, `color`, `opacity`, `scale` (tamanho
///   da sombra em relação à forma);
/// - `block`: volume na direção `angle` com `depth`, `color` na frente até
///   `endColor` no fundo (degradê) e `taper` (escala do fundo; 1 = paralelo);
/// - `envelope`: a caixa `frame` (x0, y0, x1, y1) deformada pelos 8 `points`
///   (cantos e meios, do canto de baixo à esquerda, sentido anti-horário).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Effect {
    #[serde(rename_all = "camelCase")]
    Contour {
        #[serde(default = "outside")]
        direction: String,
        steps: u32,
        distance: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        end_color: Option<crate::model::Color>,
        #[serde(default = "round", skip_serializing_if = "is_round")]
        corners: String,
        #[serde(default, skip_serializing_if = "fx_zero")]
        accel: f64,
    },
    #[serde(rename_all = "camelCase")]
    Blend {
        steps: u32,
        #[serde(default, skip_serializing_if = "fx_zero")]
        rotation: f64,
        #[serde(default, skip_serializing_if = "fx_zero")]
        accel: f64,
        #[serde(default, skip_serializing_if = "fx_zero")]
        color_accel: f64,
    },
    #[serde(rename_all = "camelCase")]
    DropShadow {
        offset: [f64; 2],
        blur: f64,
        color: crate::model::Color,
        #[serde(default = "half")]
        opacity: f64,
        #[serde(default = "fx_one", skip_serializing_if = "fx_is_one")]
        scale: f64,
    },
    #[serde(rename_all = "camelCase")]
    Block {
        angle: f64,
        depth: f64,
        color: crate::model::Color,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        end_color: Option<crate::model::Color>,
        #[serde(default = "fx_one", skip_serializing_if = "fx_is_one")]
        taper: f64,
    },
    Envelope {
        frame: [f64; 4],
        points: [[f64; 2]; 8],
    },
}

impl Effect {
    /// Efeito com os valores iniciais (doc 10) pelo nome do tipo.
    pub fn initial(kind: &str) -> Option<Effect> {
        use crate::model::Color;
        Some(match kind {
            "contour" => Effect::Contour {
                direction: "outside".into(),
                steps: 3,
                distance: 2_000.0,
                end_color: None,
                corners: "round".into(),
                accel: 0.0,
            },
            "blend" => Effect::Blend {
                steps: 8,
                rotation: 0.0,
                accel: 0.0,
                color_accel: 0.0,
            },
            "drop-shadow" => Effect::DropShadow {
                offset: [2_000.0, -2_000.0],
                blur: 3_000.0,
                color: Color::cmyk(0.0, 0.0, 0.0, 1.0),
                opacity: 0.5,
                scale: 1.0,
            },
            "block" => Effect::Block {
                angle: -45.0,
                depth: 5_000.0,
                color: Color::cmyk(0.0, 0.0, 0.0, 0.6),
                end_color: None,
                taper: 1.0,
            },
            "envelope" => Effect::Envelope {
                frame: [0.0; 4],
                points: [[0.0; 2]; 8],
            },
            _ => return None,
        })
    }

    /// Nome do tipo (`contour`, `blend`, …).
    pub fn kind(&self) -> &'static str {
        match self {
            Effect::Contour { .. } => "contour",
            Effect::Blend { .. } => "blend",
            Effect::DropShadow { .. } => "drop-shadow",
            Effect::Block { .. } => "block",
            Effect::Envelope { .. } => "envelope",
        }
    }
}

fn round() -> String {
    "round".into()
}
fn is_round(s: &str) -> bool {
    s == "round"
}
fn fx_zero(v: &f64) -> bool {
    *v == 0.0
}
fn fx_one() -> f64 {
    1.0
}
fn fx_is_one(v: &f64) -> bool {
    *v == 1.0
}

fn outside() -> String {
    "outside".into()
}

/// Simetria do grupo (GD-FER-33): `mirror` = `count` eixos de espelho
/// (1 = um espelho; N = caleidoscópio de N eixos), `radial` = `count`
/// cópias giradas. Centro e ângulo do primeiro eixo no espaço do grupo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Symmetry {
    pub kind: String,
    pub count: u32,
    pub center: [f64; 2],
    #[serde(default)]
    pub angle: f64,
}

/// Fonte de um trecho de texto. `size` em µm (corpo).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontSpec {
    pub family: String,
    pub size: f64,
    #[serde(default = "normal_weight")]
    pub weight: u16,
    #[serde(default)]
    pub italic: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextSpan {
    pub text: String,
    pub font: FontSpec,
    /// Sobrepõe o preenchimento do objeto, se presente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Fill>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paragraph {
    /// `left`, `center`, `right`, `justify`, `forceJustify`.
    #[serde(default = "left")]
    pub align: String,
    pub spans: Vec<TextSpan>,
    /// Hifenização no quadro de parágrafo (GD-TXT-07).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hyphenate: bool,
    /// Idioma (BCP 47, ex.: `pt-BR`); ausente = português do Brasil.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Texto (doc 06). A origem local é o canto superior esquerdo do bloco;
/// as linhas avançam para baixo (−Y). `frame` = largura/altura do quadro
/// (texto de parágrafo) ou da caixa usada para alinhamento.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextData {
    /// `artistic` ou `paragraph`.
    #[serde(default = "artistic")]
    pub text_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<[f64; 2]>,
    pub paragraphs: Vec<Paragraph>,
    /// Entrelinha como múltiplo do corpo.
    #[serde(default = "line_spacing")]
    pub line_spacing: f64,
    /// Texto em caminho (GD-TXT-09).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<TextPath>,
    /// Caminho de antes de endireitar o texto ("caminho fantasma"): permite
    /// reativar o caminho de origem depois.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_memory: Option<TextPath>,
    /// Áreas que afastam o texto de parágrafo (GD-TXT-10), no espaço do
    /// texto. Derivado dos objetos com `wrap`; não é gravado.
    #[serde(skip)]
    pub exclusions: Exclusions,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Áreas derivadas que afastam o texto (GD-TXT-10). Não entram na
/// comparação: são recalculadas a partir dos objetos.
#[derive(Debug, Clone, Default)]
pub struct Exclusions(pub Vec<WrapZone>);

impl PartialEq for Exclusions {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// Área que afasta o texto: polígonos (espaço do texto), lado e distância.
#[derive(Debug, Clone, PartialEq)]
pub struct WrapZone {
    pub polys: Vec<Vec<[f64; 2]>>,
    pub side: String,
    pub distance: f64,
}

/// Afastamento do texto de parágrafo pelo objeto (GD-TXT-10): `style` =
/// `contour` (a forma) ou `square` (a caixa); `side` = `both`, `left`
/// (o texto fica à esquerda), `right` ou `above-below`; `distance` em µm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextWrap {
    pub style: String,
    pub side: String,
    #[serde(default)]
    pub distance: f64,
}

/// Caminho do texto (GD-TXT-09), em coordenadas locais do texto. A linha
/// de base corre sobre o primeiro subcaminho (por comprimento de arco),
/// deslocada `offset` (µm) a partir do alinhamento; `distance` afasta do
/// caminho; `side` = `above` ou `below` (o texto passa para o outro lado
/// sem inverter a leitura); `orientation` = `rotate` (gira cada letra),
/// `skew-v`/`skew-h` (perspectiva: inclina mantendo as verticais ou as
/// horizontais), `upright` (de pé) ou `warp` (deformação plena: a base
/// segue o traço e o topo o arco externo); `mirror` percorre o caminho ao
/// contrário. `align` = `start`, `center`, `end` ou `justify` dentro do
/// intervalo `range` [início, fim] (frações do comprimento); `vertical` =
/// linha que casa com o traço (`baseline`, `descender`, `x-height`,
/// `cap`, `center`); `corners` = nos cantos vivos, `jump` (o glifo
/// pula para a face seguinte), `wrap` (o glifo dobra) ou `split` (cada
/// face recebe uma parte igual); `relax` abre o espaço nos côncavos para
/// os topos não se tocarem. Em caminho fechado o texto dá a volta. `guide`
/// = ID do objeto de onde a forma veio (vínculo vivo: mexer nele refaz o
/// texto); `detached` = já separado dele (GD-CMB-03): o texto mantém a
/// curva, independente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPath {
    pub subpaths: Vec<Subpath>,
    #[serde(default)]
    pub offset: f64,
    #[serde(default)]
    pub distance: f64,
    #[serde(default = "above")]
    pub side: String,
    #[serde(default = "rotate")]
    pub orientation: String,
    #[serde(default)]
    pub mirror: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guide: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub detached: bool,
    #[serde(default = "tp_start", skip_serializing_if = "tp_is_start")]
    pub align: String,
    #[serde(default = "tp_baseline", skip_serializing_if = "tp_is_baseline")]
    pub vertical: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<[f64; 2]>,
    #[serde(default = "tp_jump", skip_serializing_if = "tp_is_jump")]
    pub corners: String,
    #[serde(default = "tp_yes", skip_serializing_if = "tp_is_yes")]
    pub relax: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl TextPath {
    /// Caminho novo com os valores iniciais.
    pub fn new(subpaths: Vec<Subpath>, guide: Option<String>) -> TextPath {
        TextPath {
            subpaths,
            offset: 0.0,
            distance: 0.0,
            side: "above".into(),
            orientation: "rotate".into(),
            mirror: false,
            guide,
            detached: false,
            align: "start".into(),
            vertical: "baseline".into(),
            range: None,
            corners: "jump".into(),
            relax: true,
            extra: Map::new(),
        }
    }
}

fn tp_start() -> String {
    "start".into()
}
fn tp_is_start(s: &str) -> bool {
    s == "start"
}
fn tp_baseline() -> String {
    "baseline".into()
}
fn tp_is_baseline(s: &str) -> bool {
    s == "baseline"
}
fn tp_jump() -> String {
    "jump".into()
}
fn tp_is_jump(s: &str) -> bool {
    s == "jump"
}
fn tp_yes() -> bool {
    true
}
fn tp_is_yes(v: &bool) -> bool {
    *v
}

fn above() -> String {
    "above".into()
}
fn rotate() -> String {
    "rotate".into()
}

/// Cota (GD-COT-01..03). `dim_kind`: `horizontal`, `vertical`,
/// `aligned` (paralela), `angular`, `radius`, `diameter` ou `segment`
/// (comprimento de um segmento de curva). `points` no espaço local:
/// lineares e segmento `[a, b]`; angular `[vértice, a, b]`; raio e
/// diâmetro `[centro, ponto da borda]`. `offset` (µm): distância da linha
/// de cota (lineares), raio do arco (angular) ou guia além da borda (raio).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DimensionData {
    pub dim_kind: String,
    pub points: Vec<[f64; 2]>,
    #[serde(default)]
    pub offset: f64,
    /// Nó ligado a cada ponto (GD-COT-02); `None` = ponto livre.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<Option<DimLink>>,
    /// Segmento medido (tipo `segment`): `node` = índice do segmento.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment: Option<DimLink>,
    #[serde(default)]
    pub format: DimFormat,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Forma básica (GD-PRI-07): `shape` da biblioteca (`render::basic`), caixa
/// local `[0,width]×[0,height]` e parâmetros de ajuste (0–1; a ponta dos
/// balões pode sair da faixa).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BasicShapeData {
    pub shape: String,
    pub width: f64,
    pub height: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<f64>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// B-spline (GD-BSP-01): pontos de controle; os fixados a curva atravessa,
/// os flutuantes a puxam. Aberta, a curva começa e termina nas pontas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BSplineData {
    pub points: Vec<BSplinePoint>,
    #[serde(default)]
    pub closed: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Ponto de controle da B-spline (espaço local).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BSplinePoint {
    pub p: [f64; 2],
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fixed: bool,
}

/// Conector (GD-CON-01): `style` = `straight`, `elbow` (ângulo reto, com
/// `radius` nos cantos) ou `curved`; pontas na página.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorData {
    pub style: String,
    pub from: ConnEnd,
    pub to: ConnEnd,
    #[serde(default)]
    pub radius: f64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Ponta do conector: o ponto (página) e, se presa, o objeto e o encaixe
/// relativo à caixa dele (0–1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnEnd {
    pub point: [f64; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<[f64; 2]>,
}

/// Nó (ou segmento) de outro objeto a que a cota está ligada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DimLink {
    pub object: String,
    pub subpath: usize,
    pub node: usize,
}

/// Formato da cota (GD-COT-03).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DimFormat {
    /// `mm`, `cm`, `m`, `in`, `pt`.
    pub unit: String,
    pub precision: u32,
    pub prefix: String,
    pub suffix: String,
    pub show_unit: bool,
    /// `above` (acima da linha) ou `inline` (sobre a linha).
    pub text_position: String,
    /// Ponta: `triangle`, `bar`, `circle` ou `none`.
    pub arrow: String,
    pub extension_lines: bool,
    /// Corpo do texto (µm) e família.
    pub text_size: f64,
    pub font: String,
}

impl Default for DimFormat {
    fn default() -> Self {
        DimFormat {
            unit: "mm".into(),
            precision: 1,
            prefix: String::new(),
            suffix: String::new(),
            show_unit: true,
            text_position: "above".into(),
            arrow: "triangle".into(),
            extension_lines: true,
            text_size: 3_500.0,
            font: "Arial".into(),
        }
    }
}

/// Imagem ocupando `[0,width]×[0,height]` no espaço local; `src` aponta
/// para `resources/images/<sha256>.<ext>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageData {
    pub width: f64,
    pub height: f64,
    pub src: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn ellipse_mode() -> String {
    "ellipse".into()
}
fn full_turn() -> f64 {
    360.0
}
fn normal_weight() -> u16 {
    400
}
fn left() -> String {
    "left".into()
}
fn artistic() -> String {
    "artistic".into()
}
fn line_spacing() -> f64 {
    1.2
}

const COMMON_KEYS: &[&str] = &[
    "id",
    "type",
    "name",
    "transform",
    "fill",
    "stroke",
    "opacity",
    "visible",
    "locked",
    "printable",
    "userData",
    "blendMode",
    "textWrap",
    "objectStyle",
    "construction",
    "textStyle",
];

impl Object {
    pub fn new(kind: ObjectKind) -> Object {
        Object {
            id: new_id(),
            name: None,
            transform: Transform::IDENTITY,
            fill: None,
            stroke: None,
            opacity: None,
            visible: true,
            locked: false,
            printable: None,
            user_data: None,
            blend: BlendMode::Normal,
            wrap: None,
            construction: false,
            style: None,
            text_style: None,
            kind,
        }
    }

    pub fn type_name(&self) -> &str {
        match &self.kind {
            ObjectKind::Path(_) => "path",
            ObjectKind::Rectangle(_) => "rectangle",
            ObjectKind::Ellipse(_) => "ellipse",
            ObjectKind::Polygon(_) => "polygon",
            ObjectKind::Spiral(_) => "spiral",
            ObjectKind::Group(_) => "group",
            ObjectKind::Text(_) => "text",
            ObjectKind::Image(_) => "image",
            ObjectKind::Dimension(_) => "dimension",
            ObjectKind::Basic(_) => "basic",
            ObjectKind::Connector(_) => "connector",
            ObjectKind::BSpline(_) => "bspline",
            ObjectKind::Unknown { raw, .. } => {
                raw.get("type").and_then(Value::as_str).unwrap_or("?")
            }
        }
    }

    /// Visita este objeto e todos os descendentes.
    pub fn walk<'a>(&'a self, f: &mut dyn FnMut(&'a Object)) {
        f(self);
        if let ObjectKind::Group(g) = &self.kind {
            for c in &g.children {
                c.walk(f);
            }
        }
    }

    pub fn from_value(v: Value) -> Object {
        match Self::try_from_value(&v) {
            Ok(o) => o,
            Err(e) => {
                let id = v
                    .get("id")
                    .and_then(Value::as_str)
                    .map(String::from)
                    .unwrap_or_else(new_id);
                let mut o = Object::new(ObjectKind::Unknown {
                    raw: v,
                    error: Some(e),
                });
                o.id = id;
                o
            }
        }
    }

    fn try_from_value(v: &Value) -> Result<Object, String> {
        let map = v.as_object().ok_or("objeto não é um mapa JSON")?;
        let id = map
            .get("id")
            .and_then(Value::as_str)
            .ok_or("campo 'id' ausente")?
            .to_string();
        let ty = map
            .get("type")
            .and_then(Value::as_str)
            .ok_or("campo 'type' ausente")?;
        let de = |key: &str| -> Result<Option<Value>, String> {
            Ok(map.get(key).filter(|x| !x.is_null()).cloned())
        };
        let transform = match de("transform")? {
            Some(t) => serde_json::from_value(t).map_err(|e| format!("transform inválido: {e}"))?,
            None => Transform::IDENTITY,
        };
        let fill = de("fill")?.map(Fill::from_value);
        let stroke = match de("stroke")? {
            Some(s) => {
                Some(serde_json::from_value(s).map_err(|e| format!("stroke inválido: {e}"))?)
            }
            None => None,
        };
        let opacity = map.get("opacity").and_then(Value::as_f64);
        let visible = map.get("visible").and_then(Value::as_bool).unwrap_or(true);
        let locked = map.get("locked").and_then(Value::as_bool).unwrap_or(false);
        let printable = map.get("printable").and_then(Value::as_bool);
        let user_data = match de("userData")? {
            Some(u) => {
                Some(serde_json::from_value(u).map_err(|e| format!("userData inválido: {e}"))?)
            }
            None => None,
        };
        let name = map.get("name").and_then(Value::as_str).map(String::from);
        // Modo desconhecido (versão futura) cai para normal.
        let blend = de("blendMode")?
            .and_then(|b| serde_json::from_value(b).ok())
            .unwrap_or_default();
        let wrap = de("textWrap")?.and_then(|w| serde_json::from_value(w).ok());
        let construction = map
            .get("construction")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let style = map
            .get("objectStyle")
            .and_then(Value::as_str)
            .map(String::from);
        let text_style = map
            .get("textStyle")
            .and_then(Value::as_str)
            .map(String::from);

        let mut rest = map.clone();
        for k in COMMON_KEYS {
            rest.remove(*k);
        }
        let rest = Value::Object(rest);
        fn parse<T: serde::de::DeserializeOwned>(v: Value, ty: &str) -> Result<T, String> {
            serde_json::from_value(v).map_err(|e| format!("objeto '{ty}' inválido: {e}"))
        }
        let kind = match ty {
            "path" => ObjectKind::Path(parse(rest, ty)?),
            "rectangle" => ObjectKind::Rectangle(parse(rest, ty)?),
            "ellipse" => ObjectKind::Ellipse(parse(rest, ty)?),
            "polygon" => ObjectKind::Polygon(parse(rest, ty)?),
            "spiral" => ObjectKind::Spiral(parse(rest, ty)?),
            "group" => ObjectKind::Group(parse(rest, ty)?),
            "text" => ObjectKind::Text(parse(rest, ty)?),
            "image" => ObjectKind::Image(parse(rest, ty)?),
            "dimension" => ObjectKind::Dimension(parse(rest, ty)?),
            "basic" => ObjectKind::Basic(parse(rest, ty)?),
            "connector" => ObjectKind::Connector(parse(rest, ty)?),
            "bspline" => ObjectKind::BSpline(parse(rest, ty)?),
            _ => ObjectKind::Unknown {
                raw: v.clone(),
                error: None,
            },
        };
        Ok(Object {
            id,
            name,
            transform,
            fill,
            stroke,
            opacity,
            visible,
            locked,
            printable,
            user_data,
            blend,
            wrap,
            construction,
            style,
            text_style,
            kind,
        })
    }

    pub fn to_value(&self) -> Value {
        if let ObjectKind::Unknown { raw, .. } = &self.kind {
            return raw.clone();
        }
        let mut m = Map::new();
        m.insert("id".into(), self.id.clone().into());
        m.insert("type".into(), self.type_name().into());
        if let Some(n) = &self.name {
            m.insert("name".into(), n.clone().into());
        }
        m.insert(
            "transform".into(),
            serde_json::to_value(self.transform).unwrap(),
        );
        if let Some(f) = &self.fill {
            m.insert("fill".into(), f.to_value());
        }
        if let Some(s) = &self.stroke {
            m.insert("stroke".into(), serde_json::to_value(s).unwrap());
        }
        if let Some(o) = self.opacity {
            m.insert("opacity".into(), o.into());
        }
        m.insert("visible".into(), self.visible.into());
        m.insert("locked".into(), self.locked.into());
        if let Some(p) = self.printable {
            m.insert("printable".into(), p.into());
        }
        if let Some(u) = &self.user_data {
            m.insert("userData".into(), serde_json::to_value(u).unwrap());
        }
        if let Some(s) = &self.style {
            m.insert("objectStyle".into(), s.clone().into());
        }
        if self.construction {
            m.insert("construction".into(), true.into());
        }
        if let Some(s) = &self.text_style {
            m.insert("textStyle".into(), s.clone().into());
        }
        if let Some(w) = &self.wrap {
            m.insert("textWrap".into(), serde_json::to_value(w).unwrap());
        }
        if self.blend != BlendMode::Normal {
            m.insert(
                "blendMode".into(),
                serde_json::to_value(self.blend).unwrap(),
            );
        }
        let kind = match &self.kind {
            ObjectKind::Path(d) => serde_json::to_value(d),
            ObjectKind::Rectangle(d) => serde_json::to_value(d),
            ObjectKind::Ellipse(d) => serde_json::to_value(d),
            ObjectKind::Polygon(d) => serde_json::to_value(d),
            ObjectKind::Spiral(d) => serde_json::to_value(d),
            ObjectKind::Group(d) => serde_json::to_value(d),
            ObjectKind::Text(d) => serde_json::to_value(d),
            ObjectKind::Image(d) => serde_json::to_value(d),
            ObjectKind::Dimension(d) => serde_json::to_value(d),
            ObjectKind::Basic(d) => serde_json::to_value(d),
            ObjectKind::Connector(d) => serde_json::to_value(d),
            ObjectKind::BSpline(d) => serde_json::to_value(d),
            ObjectKind::Unknown { .. } => unreachable!(),
        };
        if let Ok(Value::Object(k)) = kind {
            for (key, val) in k {
                m.entry(key).or_insert(val);
            }
        }
        Value::Object(m)
    }
}

impl Serialize for Object {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.to_value().serialize(s)
    }
}

impl<'de> Deserialize<'de> for Object {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Object::from_value(Value::deserialize(d)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unknown_fields_and_types_are_preserved() {
        let v = json!({
            "id": "0192a000-0000-7000-8000-000000000001", "type": "rectangle",
            "transform": [1,0,0,1,0,0], "visible": true, "locked": false,
            "width": 1000.0, "height": 2000.0, "futureField": {"x": 1}
        });
        let o = Object::from_value(v.clone());
        assert!(matches!(o.kind, ObjectKind::Rectangle(_)));
        assert_eq!(
            o.to_value(),
            json!({
                "id": "0192a000-0000-7000-8000-000000000001", "type": "rectangle",
                "transform": [1.0,0.0,0.0,1.0,0.0,0.0], "visible": true, "locked": false,
                "width": 1000.0, "height": 2000.0, "futureField": {"x": 1}
            })
        );

        let alien = json!({"id": "x", "type": "hologram", "beams": 3});
        assert_eq!(Object::from_value(alien.clone()).to_value(), alien);
    }

    #[test]
    fn invalid_object_becomes_unknown_with_error() {
        let bad = json!({"id": "x", "type": "rectangle", "width": "largo"});
        let o = Object::from_value(bad.clone());
        match &o.kind {
            ObjectKind::Unknown { error: Some(e), .. } => assert!(e.contains("rectangle")),
            other => panic!("esperado Unknown, veio {other:?}"),
        }
        assert_eq!(o.to_value(), bad);
    }
}
