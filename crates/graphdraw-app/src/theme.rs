//! Tema do GraphDraw: os tokens de `docs/design/DESIGN-SYSTEM.md` com os
//! mesmos nomes, e a aplicação deles no `Style` do egui. Nenhum outro arquivo
//! escreve cor, espaçamento, raio, tamanho de controle ou fonte direto.
//!
//! O módulo traz todos os tokens do documento, inclusive os que ainda não
//! têm tela que os use (ex.: botão Prominent, faixa de painel recolhido).
#![allow(dead_code)]

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow,
    Stroke, Style, TextStyle, Theme, Vec2, Visuals,
};

// ---- espaçamento (2.1) --------------------------------------------------------

pub const SPACE_050: f32 = 2.0;
pub const SPACE_100: f32 = 4.0;
pub const SPACE_150: f32 = 6.0;
pub const SPACE_200: f32 = 8.0;
pub const SPACE_300: f32 = 12.0;
pub const SPACE_400: f32 = 16.0;
pub const SPACE_600: f32 = 24.0;
pub const SPACE_800: f32 = 32.0;

// ---- cores iguais nos dois temas (2.2) ----------------------------------------

pub const PAGE_PAPER: Color32 = Color32::WHITE;
pub const BORDER_FOCUS: Color32 = Color32::from_rgb(0x63, 0x66, 0xF1);
pub const FG_ON_ACCENT: Color32 = Color32::WHITE;
pub const ACCENT_PRIMARY: Color32 = Color32::from_rgb(0x63, 0x66, 0xF1);
pub const ACCENT_PRIMARY_HOVER: Color32 = Color32::from_rgb(0x7A, 0x7D, 0xF5);
pub const ACCENT_SUCCESS: Color32 = Color32::from_rgb(0x10, 0xB9, 0x81);
pub const ACCENT_WARNING: Color32 = Color32::from_rgb(0xF5, 0x9E, 0x0B);
pub const ACCENT_DANGER: Color32 = Color32::from_rgb(0xEF, 0x44, 0x44);
pub const GUIDE_OBJECT: Color32 = Color32::from_rgb(0xEC, 0x48, 0x99);
pub const GUIDE_PAGE: Color32 = Color32::from_rgb(0x06, 0xB6, 0xD4);
/// Guias do usuário (GD-GUI-01); travadas ficam mais apagadas.
pub const GUIDE_USER: Color32 = Color32::from_rgb(0x3B, 0x82, 0xF6);
pub const GUIDE_LOCKED_ALPHA: u8 = 110;

/// Sobreposições de seleção sobre a arte (5.1).
pub const SELECTION_OBJECT: Color32 = Color32::from_rgba_unmultiplied_const(0x63, 0x66, 0xF1, 110);
pub const SELECTION_FILL: Color32 = Color32::from_rgba_unmultiplied_const(0x63, 0x66, 0xF1, 26);
pub const GRID_LINE: Color32 = Color32::from_rgba_unmultiplied_const(0x63, 0x66, 0xF1, 40);

/// Xadrez atrás de amostras com transparência.
pub const CHECKER_LIGHT: Color32 = Color32::WHITE;
pub const CHECKER_DARK: Color32 = Color32::from_rgb(0xD9, 0xD9, 0xDF);

// ---- cores que mudam com o tema (2.2) ------------------------------------------

/// Superfícies, bordas e texto de um tema.
pub struct Palette {
    pub bg_canvas: Color32,
    pub bg_surface_0: Color32,
    pub bg_surface_1: Color32,
    pub bg_surface_2: Color32,
    pub bg_surface_hover: Color32,
    pub bg_surface_active: Color32,
    pub bg_surface_toggled: Color32,
    pub border_subtle: Color32,
    pub border_default: Color32,
    pub fg_primary: Color32,
    pub fg_strong: Color32,
    pub fg_muted: Color32,
    /// Ícone pressionado: `fg-on-accent` no escuro; no claro, `fg-strong`
    /// (branco não aparece sobre o fundo claro pressionado).
    pub fg_pressed: Color32,
    shadow_page_alpha: u8,
    shadow_popover_alpha: u8,
}

pub const DARK: Palette = Palette {
    bg_canvas: Color32::from_rgb(0x10, 0x10, 0x12),
    bg_surface_0: Color32::from_rgb(0x16, 0x16, 0x19),
    bg_surface_1: Color32::from_rgb(0x1E, 0x1E, 0x23),
    bg_surface_2: Color32::from_rgb(0x27, 0x27, 0x2E),
    bg_surface_hover: Color32::from_rgb(0x32, 0x32, 0x3B),
    bg_surface_active: Color32::from_rgb(0x3D, 0x3D, 0x48),
    bg_surface_toggled: Color32::from_rgb(0x2A, 0x2D, 0x4A),
    border_subtle: Color32::from_rgb(0x26, 0x26, 0x2E),
    border_default: Color32::from_rgb(0x36, 0x36, 0x42),
    fg_primary: Color32::from_rgb(0xE2, 0xE2, 0xE8),
    fg_strong: Color32::from_rgb(0xF4, 0xF4, 0xF6),
    fg_muted: Color32::from_rgb(0xA0, 0xA0, 0xAB),
    fg_pressed: FG_ON_ACCENT,
    shadow_page_alpha: 89,
    shadow_popover_alpha: 128,
};

pub const LIGHT: Palette = Palette {
    bg_canvas: Color32::from_rgb(0xD9, 0xD9, 0xDF),
    bg_surface_0: Color32::from_rgb(0xED, 0xED, 0xF1),
    bg_surface_1: Color32::from_rgb(0xF6, 0xF6, 0xF8),
    bg_surface_2: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    bg_surface_hover: Color32::from_rgb(0xE6, 0xE6, 0xEC),
    bg_surface_active: Color32::from_rgb(0xD8, 0xD8, 0xE0),
    bg_surface_toggled: Color32::from_rgb(0xE2, 0xE3, 0xFC),
    border_subtle: Color32::from_rgb(0xDE, 0xDE, 0xE5),
    border_default: Color32::from_rgb(0xC6, 0xC6, 0xD0),
    fg_primary: Color32::from_rgb(0x26, 0x26, 0x2E),
    fg_strong: Color32::from_rgb(0x10, 0x10, 0x12),
    fg_muted: Color32::from_rgb(0x62, 0x62, 0x6D),
    fg_pressed: Color32::from_rgb(0x10, 0x10, 0x12),
    shadow_page_alpha: 46,
    shadow_popover_alpha: 51,
};

/// Paleta do tema em uso.
pub fn palette(ui: &egui::Ui) -> &'static Palette {
    palette_for(ui.visuals())
}

pub fn palette_for(v: &Visuals) -> &'static Palette {
    if v.dark_mode { &DARK } else { &LIGHT }
}

impl Palette {
    /// `shadow-page`: deslocamento 0/8 px, desfoque 30 px.
    pub fn shadow_page(&self) -> Shadow {
        Shadow {
            offset: [0, 8],
            blur: 30,
            spread: 0,
            color: Color32::from_black_alpha(self.shadow_page_alpha),
        }
    }

    /// `shadow-popover`: deslocamento 0/10 px, desfoque 25 px (recuo 0; o
    /// egui não aceita recuo negativo).
    pub fn shadow_popover(&self) -> Shadow {
        Shadow {
            offset: [0, 10],
            blur: 25,
            spread: 0,
            color: Color32::from_black_alpha(self.shadow_popover_alpha),
        }
    }
}

// ---- traços da interface -------------------------------------------------------

pub const BORDER_WIDTH: f32 = 1.0;
pub const FOCUS_WIDTH: f32 = 1.5;

pub fn border_subtle(ui: &egui::Ui) -> Stroke {
    Stroke::new(BORDER_WIDTH, palette(ui).border_subtle)
}

pub fn border_default(ui: &egui::Ui) -> Stroke {
    Stroke::new(BORDER_WIDTH, palette(ui).border_default)
}

pub fn border_focus() -> Stroke {
    Stroke::new(FOCUS_WIDTH, BORDER_FOCUS)
}

// ---- tipografia (2.3) ----------------------------------------------------------

pub const TYPE_DISPLAY: f32 = 18.0;
pub const TYPE_SECTION: f32 = 13.0;
pub const TYPE_BODY: f32 = 12.0;
pub const TYPE_NUMERIC: f32 = 11.0;
pub const TYPE_MICRO: f32 = 10.0;

const INTER: &str = "inter";
const INTER_MEDIUM: &str = "inter-medium";
const INTER_SEMIBOLD: &str = "inter-semibold";

/// Estilo de texto `type-section` (títulos de painel).
pub fn section_style() -> TextStyle {
    TextStyle::Name("section".into())
}

pub fn font_display() -> FontId {
    FontId::new(TYPE_DISPLAY, FontFamily::Name(INTER_SEMIBOLD.into()))
}

pub fn font_section() -> FontId {
    FontId::new(TYPE_SECTION, FontFamily::Name(INTER_MEDIUM.into()))
}

pub fn font_body() -> FontId {
    FontId::proportional(TYPE_BODY)
}

pub fn font_numeric() -> FontId {
    FontId::monospace(TYPE_NUMERIC)
}

pub fn font_micro() -> FontId {
    FontId::proportional(TYPE_MICRO)
}

// ---- raios (2.4) ---------------------------------------------------------------

pub const RADIUS_NONE: u8 = 0;
pub const RADIUS_XS: u8 = 2;
pub const RADIUS_SM: u8 = 4;
pub const RADIUS_MD: u8 = 6;
pub const RADIUS_LG: u8 = 10;
pub const RADIUS_FULL: u8 = u8::MAX;

/// Raio só nos cantos de cima (abas de documento).
pub fn radius_top(r: u8) -> CornerRadius {
    CornerRadius {
        nw: r,
        ne: r,
        sw: 0,
        se: 0,
    }
}

// ---- movimento (2.5) -----------------------------------------------------------

pub const MOTION_FAST: f32 = 0.10;
pub const MOTION_BASE: f32 = 0.12;
pub const TOOLTIP_DELAY: f32 = 0.4;

// ---- iconografia (3) -----------------------------------------------------------

pub const ICON_UI: f32 = 20.0;
pub const ICON_MICRO: f32 = 16.0;
pub const ICON_HUB: f32 = 24.0;
pub const ICON_CURSOR: f32 = 24.0;
/// Miniatura 1:1 de objeto no painel de camadas (4.7).
pub const OBJECT_THUMB: f32 = 24.0;
/// Campo de texto dentro de menu de contexto (nome do objeto, 4.7).
pub const MENU_FIELD_WIDTH: f32 = 160.0;
/// Cursor sobre o canvas: tinta preta com halo branco de 1 px (3.2).
pub const CURSOR_INK: Color32 = Color32::from_rgb(0x10, 0x10, 0x12);
pub const CURSOR_HALO: Color32 = Color32::WHITE;
pub const CURSOR_HALO_WIDTH: f32 = 1.0;
/// Mira das ferramentas de desenho: vão central e comprimento dos braços.
pub const CROSS_GAP: f32 = 3.0;
pub const CROSS_ARM: f32 = 10.0;
/// Ícone da ferramenta ao lado da mira.
pub const CROSS_BADGE: f32 = 15.0;

// ---- componentes (4) -----------------------------------------------------------

pub const BUTTON_COMPACT: f32 = 24.0;
pub const BUTTON_DEFAULT: f32 = 28.0;
pub const BUTTON_PROMINENT: f32 = 36.0;
/// Densidade Ampla dos painéis (6): controles de 32 px.
pub const BUTTON_WIDE: f32 = 32.0;
pub const BUTTON_PAD_WIDE: f32 = 12.0;
pub const BUTTON_PAD_COMPACT: f32 = 6.0;
pub const BUTTON_PAD_DEFAULT: f32 = 10.0;
pub const BUTTON_PAD_PROMINENT: f32 = 16.0;
/// Largura mínima dos botões de rodapé de janela (OK, Cancelar).
pub const BUTTON_MIN_WIDTH: f32 = 80.0;
/// Botão pressionado desce meio pixel.
pub const PRESS_OFFSET: f32 = 0.5;
pub const MENU_ITEM_HEIGHT: f32 = 26.0;
/// Altura fixa da barra de propriedades (4.6).
pub const BAR_PROPERTY_HEIGHT: f32 = 2.0 * (BUTTON_COMPACT + 2.0 * SPACE_150);
/// Espaço entre as duas linhas de controles empilhados na barra (4.6).
pub const BAR_ROW_GAP: f32 = SPACE_300;
/// Abas de documento (4.5): altura da faixa, largura cheia e mínima.
pub const TAB_DOC_HEIGHT: f32 = BUTTON_DEFAULT + SPACE_100;
pub const TAB_DOC_WIDTH: f32 = 220.0;
pub const TAB_DOC_MIN_WIDTH: f32 = 72.0;
/// Faixa da etiqueta de cor na aba de página (4.5).
pub const TAB_TAG_STRIP: f32 = 3.0;
/// Janelas modais: altura máxima em fração da tela (4.4).
pub const MODAL_MAX_HEIGHT: f32 = 0.75;
/// Faixa de ícones de um painel recolhido (6).
pub const PANEL_RAIL: f32 = 36.0;
/// Coluna de painéis aberta: largura inicial e limites ao redimensionar (6).
pub const PANEL_WIDTH: f32 = 260.0;
pub const PANEL_MIN_WIDTH: f32 = 200.0;
pub const PANEL_MAX_WIDTH: f32 = 520.0;
/// Janela de painel solto: tamanho inicial (6).
pub const PANEL_FLOAT_SIZE: [f32; 2] = [260.0, 360.0];
/// Prévia do percurso de corte (doc 17, GD-CRT-06): tamanho e duração.
pub const CUT_PREVIEW: [f32; 2] = [420.0, 240.0];
pub const CUT_PREVIEW_SECONDS: f32 = 4.0;
/// Campo hexadecimal do editor de cor.
pub const HEX_FIELD_WIDTH: f32 = 72.0;
/// Seletor de cantos do retângulo (2 × 2): lado de cada célula e vão.
pub const CORNER_CELL: f32 = 10.0;
pub const CORNER_GAP: f32 = 4.0;
/// Tipo de canto (Redondo/Chanfro/Recorte).
pub const CORNER_KIND_WIDTH: f32 = 96.0;
/// Réguas (GD-REG-01): largura da faixa, espaço mínimo entre números e
/// comprimento das marcas.
pub const RULER_SIZE: f32 = 20.0;
pub const RULER_MAJOR_SPACING: f32 = 60.0;
pub const RULER_TICK_MAJOR: f32 = 8.0;
pub const RULER_TICK_MINOR: f32 = 4.0;
/// Escolha de família de fonte na barra de propriedades (doc 06).
pub const FONT_COMBO_WIDTH: f32 = 180.0;
/// Estilo da fonte (Regular, Bold…) na barra de texto.
pub const FONT_STYLE_WIDTH: f32 = 110.0;
/// Lista do seletor de fonte: largura, altura máxima, altura da prévia de
/// cada família e largura máxima da prévia (doc 06, GD-FNT-01).
pub const FONT_POPUP_WIDTH: f32 = 300.0;
pub const FONT_LIST_HEIGHT: f32 = 360.0;
pub const FONT_PREVIEW_HEIGHT: f32 = 22.0;
pub const FONT_PREVIEW_MAX_WIDTH: f32 = 270.0;
/// Amostras de cor (paleta e indicadores).
pub const SWATCH: f32 = 18.0;

// ---- canvas (5) ----------------------------------------------------------------

/// Âncora: quadrado de 6 × 6 px, borda de 1,5 px.
pub const HANDLE_SIZE: f32 = 6.0;
pub const HANDLE_STROKE: f32 = 1.5;
pub const HANDLE_FILL: Color32 = Color32::WHITE;
/// Distância de captura das alças (maior que o desenho, para acertar fácil).
pub const HANDLE_HIT: f32 = 8.0;
/// Nó suave e ponta de alça: círculo de 6 px.
pub const NODE_SIZE: f32 = 6.0;
pub const GUIDE_WIDTH: f32 = 1.0;
/// Raio de atração do encaixe, em pixels de tela (5.2).
pub const SNAP_RADIUS_PX: f64 = 6.0;
/// Marca de encaixe: círculo em volta do ponto encaixado.
pub const SNAP_MARK: f32 = 6.0;
/// Centro de rotação: círculo e braços da mira.
pub const ROTATION_CENTER: f32 = 6.0;
pub const ROTATION_CENTER_ARM: f32 = 9.0;
/// Sangria na tela (5.3): contorno tracejado e marcas de corte nos cantos.
pub const BLEED_DASH: f32 = 4.0;
pub const BLEED_GAP: f32 = 3.0;
pub const CROP_MARK: f32 = 12.0;
pub const CROP_MARK_OFFSET: f32 = 4.0;

// ---- aplicação no egui (7) -----------------------------------------------------

/// Registra as fontes e os estilos claro e escuro.
pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.set_style_of(Theme::Dark, style(&DARK, Visuals::dark()));
    ctx.set_style_of(Theme::Light, style(&LIGHT, Visuals::light()));
    ctx.set_theme(egui::ThemePreference::Dark);
}

fn fonts() -> FontDefinitions {
    let mut f = FontDefinitions::default();
    for (name, data) in [
        (
            INTER,
            &include_bytes!("../assets/fonts/Inter-Regular.ttf")[..],
        ),
        (
            INTER_MEDIUM,
            &include_bytes!("../assets/fonts/Inter-Medium.ttf")[..],
        ),
        (
            INTER_SEMIBOLD,
            &include_bytes!("../assets/fonts/Inter-SemiBold.ttf")[..],
        ),
    ] {
        f.font_data
            .insert(name.into(), FontData::from_static(data).into());
    }
    // As fontes padrão continuam como reserva (símbolos que a Inter não tem).
    let fallback = f.families[&FontFamily::Proportional].clone();
    f.families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, INTER.into());
    for name in [INTER_MEDIUM, INTER_SEMIBOLD] {
        let mut list = vec![name.to_string()];
        list.extend(fallback.iter().cloned());
        f.families.insert(FontFamily::Name(name.into()), list);
    }
    f
}

fn widget(bg: Color32, border: Stroke, fg: Stroke) -> egui::style::WidgetVisuals {
    egui::style::WidgetVisuals {
        bg_fill: bg,
        weak_bg_fill: bg,
        bg_stroke: border,
        corner_radius: CornerRadius::same(RADIUS_SM),
        fg_stroke: fg,
        expansion: 0.0,
    }
}

pub fn style(p: &Palette, base: Visuals) -> Style {
    let mut s = Style {
        visuals: base,
        ..Default::default()
    };
    s.text_styles = [
        (TextStyle::Heading, font_display()),
        (section_style(), font_section()),
        (TextStyle::Body, font_body()),
        (TextStyle::Button, font_body()),
        (TextStyle::Monospace, font_numeric()),
        (TextStyle::Small, font_micro()),
    ]
    .into();
    s.drag_value_text_style = TextStyle::Monospace;

    let v = &mut s.visuals;
    v.panel_fill = p.bg_surface_1;
    v.window_fill = p.bg_surface_1;
    v.faint_bg_color = p.bg_surface_0;
    v.extreme_bg_color = p.bg_surface_2;
    v.text_edit_bg_color = Some(p.bg_surface_2);
    v.code_bg_color = p.bg_surface_2;
    v.window_stroke = Stroke::new(BORDER_WIDTH, p.border_default);
    v.window_corner_radius = CornerRadius::same(RADIUS_LG);
    v.menu_corner_radius = CornerRadius::same(RADIUS_MD);
    v.window_shadow = p.shadow_popover();
    v.popup_shadow = p.shadow_popover();
    v.selection.bg_fill = ACCENT_PRIMARY;
    v.selection.stroke = Stroke::new(BORDER_WIDTH, FG_ON_ACCENT);
    v.hyperlink_color = ACCENT_PRIMARY_HOVER;
    v.warn_fg_color = ACCENT_WARNING;
    v.error_fg_color = ACCENT_DANGER;
    v.widgets.noninteractive = widget(
        p.bg_surface_1,
        Stroke::new(BORDER_WIDTH, p.border_subtle),
        Stroke::new(BORDER_WIDTH, p.fg_primary),
    );
    v.widgets.inactive = widget(
        p.bg_surface_2,
        Stroke::new(BORDER_WIDTH, p.border_default),
        Stroke::new(BORDER_WIDTH, p.fg_primary),
    );
    v.widgets.hovered = widget(
        p.bg_surface_hover,
        Stroke::new(BORDER_WIDTH, p.border_default),
        Stroke::new(FOCUS_WIDTH, p.fg_strong),
    );
    v.widgets.active = widget(
        p.bg_surface_active,
        border_focus(),
        Stroke::new(FOCUS_WIDTH, p.fg_strong),
    );
    v.widgets.open = widget(
        p.bg_surface_active,
        Stroke::new(BORDER_WIDTH, p.border_default),
        Stroke::new(BORDER_WIDTH, p.fg_strong),
    );

    let sp = &mut s.spacing;
    sp.item_spacing = Vec2::new(SPACE_200, SPACE_100);
    sp.button_padding = Vec2::new(BUTTON_PAD_COMPACT, SPACE_050);
    sp.interact_size.y = BUTTON_COMPACT;
    sp.window_margin = Margin::same(SPACE_600 as i8);
    sp.menu_margin = Margin::same(SPACE_100 as i8);
    sp.menu_spacing = SPACE_050;
    sp.indent = SPACE_400;

    s.interaction.tooltip_delay = TOOLTIP_DELAY;
    s.animation_time = MOTION_FAST;
    s
}

/// Leva o tema escolhido para a barra de título do sistema. O `set_theme`
/// do egui só muda o conteúdo; sem este comando a barra fica no tema do
/// sistema e, no Windows, os botões e o título podem sumir até a janela
/// perder e ganhar o foco. `last` guarda o que já foi enviado.
///
/// Só envia com a janela em foco: ao aplicar o tema, o Windows repinta a
/// barra conforme o foco do momento, e sem foco (primeiros quadros) a barra
/// ficaria presa com a aparência de janela inativa (título e botões
/// apagados).
pub fn sync_window_theme(ctx: &egui::Context, last: &mut Option<egui::ThemePreference>) {
    let pref = ctx.options(|o| o.theme_preference);
    if *last == Some(pref) {
        return;
    }
    if ctx.input(|i| i.viewport().focused) != Some(true) {
        return;
    }
    *last = Some(pref);
    ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(match pref {
        egui::ThemePreference::Dark => egui::SystemTheme::Dark,
        egui::ThemePreference::Light => egui::SystemTheme::Light,
        egui::ThemePreference::System => egui::SystemTheme::SystemDefault,
    }));
}

/// Densidade dos painéis laterais (DESIGN-SYSTEM 6). A barra de
/// propriedades, as barras de ferramentas e os menus não mudam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Density {
    /// Controles de 24 px (notebooks de 13").
    Compact,
    /// Controles de 28 px (padrão).
    #[default]
    Default,
    /// Controles de 32 px e espaçamentos maiores (tablets, toque).
    Wide,
}

impl Density {
    pub const ALL: [Density; 3] = [Density::Compact, Density::Default, Density::Wide];

    /// Chave estável na área de trabalho salva.
    pub fn key(self) -> &'static str {
        match self {
            Density::Compact => "compacta",
            Density::Default => "padrao",
            Density::Wide => "ampla",
        }
    }

    pub fn from_key(k: &str) -> Density {
        Density::ALL
            .into_iter()
            .find(|d| d.key() == k)
            .unwrap_or_default()
    }

    pub fn label(self) -> &'static str {
        match self {
            Density::Compact => crate::i18n::tr("Compacta", "Compact"),
            Density::Default => crate::i18n::tr("Padrão", "Default"),
            Density::Wide => crate::i18n::tr("Ampla", "Wide"),
        }
    }
}

fn density_id() -> egui::Id {
    egui::Id::unique("densidade-paineis")
}

/// Escolhe a densidade dos painéis (guardada no contexto do egui).
pub fn set_density(ctx: &egui::Context, d: Density) {
    ctx.data_mut(|m| m.insert_temp(density_id(), d));
}

pub fn density(ctx: &egui::Context) -> Density {
    ctx.data(|m| m.get_temp(density_id())).unwrap_or_default()
}

/// Medidas dos painéis laterais e do inspetor na densidade escolhida.
pub fn panel_density(ui: &mut egui::Ui) {
    let d = density(ui.ctx());
    let sp = ui.spacing_mut();
    let (h, pad) = match d {
        Density::Compact => (BUTTON_COMPACT, BUTTON_PAD_COMPACT),
        Density::Default => (BUTTON_DEFAULT, BUTTON_PAD_DEFAULT),
        Density::Wide => (BUTTON_WIDE, BUTTON_PAD_WIDE),
    };
    sp.interact_size.y = h;
    sp.button_padding.x = pad;
    if d == Density::Wide {
        sp.item_spacing = Vec2::new(SPACE_300, SPACE_200);
    }
}

/// Itens de menu com 26 px de altura (4.3).
pub fn menu_density(ui: &mut egui::Ui) {
    ui.spacing_mut().interact_size.y = MENU_ITEM_HEIGHT;
}

/// Contexto com o tema aplicado, para testes de layout com as medidas reais.
#[cfg(test)]
pub fn test_ctx() -> egui::Context {
    let ctx = egui::Context::default();
    apply(&ctx);
    ctx
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A densidade muda a altura dos controles dos painéis (24/28/32 px).
    #[test]
    fn panel_density_sets_control_height() {
        for (d, want) in [
            (Density::Compact, BUTTON_COMPACT),
            (Density::Default, BUTTON_DEFAULT),
            (Density::Wide, BUTTON_WIDE),
        ] {
            let ctx = test_ctx();
            set_density(&ctx, d);
            let mut h = 0.0;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(800.0, 600.0),
                )),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                panel_density(ui);
                h = ui.button("Ok").rect.height();
            });
            out.textures_delta.clear();
            assert_eq!(h, want, "{d:?}");
            assert_eq!(Density::from_key(d.key()), d);
        }
    }

    #[test]
    fn tokens_follow_the_design_system() {
        assert_eq!(BAR_PROPERTY_HEIGHT, 72.0);
        assert_eq!(ACCENT_PRIMARY, Color32::from_rgb(0x63, 0x66, 0xF1));
        assert_eq!(DARK.bg_surface_1, Color32::from_rgb(0x1E, 0x1E, 0x23));
        let s = style(&DARK, Visuals::dark());
        assert_eq!(s.visuals.panel_fill, DARK.bg_surface_1);
        assert_eq!(s.interaction.tooltip_delay, 0.4);
        assert_eq!(s.text_styles[&TextStyle::Monospace].size, TYPE_NUMERIC);
    }

    #[test]
    fn fonts_load() {
        let ctx = test_ctx();
        let mut out = ctx.run_ui(Default::default(), |ui| {
            ui.label("Inter");
            ui.label(egui::RichText::new("Seção").text_style(section_style()));
            ui.heading("Título");
        });
        out.textures_delta.clear();
    }
}
