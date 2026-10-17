//! Atalhos de teclado configuráveis (GD-UI-10).
//!
//! - Todo comando com atalho é um [`Command`]; a interface nunca testa teclas
//!   direto (exceto as fixas: setas, Delete, Enter e Esc).
//! - Perfis prontos: **GraphDraw** (padrão de fábrica, esquema próprio) e
//!   **Alternativo** (teclas de função e letras de alinhamento, escolhido
//!   pelo responsável pelo produto). O usuário pode editar qualquer atalho;
//!   aí o perfil vira "Personalizado".
//! - Importar/exportar em JSON; a escolha fica salva na pasta de
//!   configuração do usuário e vale sempre que o programa abrir.
//! - Contexto: um mesmo atalho pode ter um comando para quando há texto em
//!   edição e outro para o resto (ex.: Ctrl+R repetir × alinhar texto à
//!   direita no perfil Alternativo).

use std::path::PathBuf;

use eframe::egui::{self, Key, Modifiers};
use serde::{Deserialize, Serialize};

use crate::i18n::tr;

/// Situação em que um atalho vale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    Always,
    /// Há ao menos um objeto selecionado.
    Selection,
    /// Há dois ou mais objetos selecionados.
    Multi,
    /// Fora da edição de texto.
    NotText,
    /// Durante a edição de texto.
    Text,
}

/// Estado do programa usado para decidir o contexto.
#[derive(Debug, Clone, Copy, Default)]
pub struct State {
    pub selected: usize,
    pub editing_text: bool,
}

impl When {
    fn ok(self, s: State) -> bool {
        match self {
            When::Always => true,
            When::Selection => s.selected >= 1 && !s.editing_text,
            When::Multi => s.selected >= 2 && !s.editing_text,
            When::NotText => !s.editing_text,
            When::Text => s.editing_text,
        }
    }
}

macro_rules! commands {
    ($($v:ident = $id:literal, $pt:literal, $en:literal, $group:ident, $when:ident, $ready:literal;)*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Command { $($v,)* }

        impl Command {
            pub const ALL: &[Command] = &[$(Command::$v,)*];
            /// Identificador estável (usado no JSON).
            pub fn id(self) -> &'static str { match self { $(Command::$v => $id,)* } }
            pub fn label(self) -> &'static str { match self { $(Command::$v => tr($pt, $en),)* } }
            pub fn group(self) -> Group { match self { $(Command::$v => Group::$group,)* } }
            pub fn when(self) -> When { match self { $(Command::$v => When::$when,)* } }
            /// `false` = ainda sem a ferramenta/função; o atalho fica reservado.
            pub fn ready(self) -> bool { match self { $(Command::$v => $ready,)* } }
            pub fn from_id(id: &str) -> Option<Command> {
                match id { $($id => Some(Command::$v),)* _ => None }
            }
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    File,
    Edit,
    Object,
    Align,
    Order,
    View,
    Tools,
    Pages,
    Text,
}

impl Group {
    pub const ALL: [Group; 9] = [
        Group::File,
        Group::Edit,
        Group::Object,
        Group::Align,
        Group::Order,
        Group::View,
        Group::Tools,
        Group::Pages,
        Group::Text,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Group::File => tr("Arquivo", "File"),
            Group::Edit => tr("Edição", "Edit"),
            Group::Object => tr("Objeto", "Object"),
            Group::Align => tr("Alinhamento", "Alignment"),
            Group::Order => tr("Ordem dos objetos", "Object order"),
            Group::View => tr("Exibição", "View"),
            Group::Tools => tr("Ferramentas", "Tools"),
            Group::Pages => tr("Páginas e documentos", "Pages and documents"),
            Group::Text => tr("Texto", "Text"),
        }
    }
}

commands! {
    New = "file.new", "Novo documento", "New document", File, Always, true;
    Open = "file.open", "Abrir", "Open", File, Always, true;
    Save = "file.save", "Salvar", "Save", File, Always, true;
    SaveAs = "file.save_as", "Salvar como", "Save as", File, Always, true;
    Import = "file.import", "Importar para a página", "Import into page", File, NotText, true;
    Export = "file.export", "Exportar", "Export", File, NotText, true;
    Print = "file.print", "Imprimir", "Print", File, Always, false;
    Close = "file.close", "Fechar documento", "Close document", File, Always, true;
    NextDoc = "doc.next", "Próximo documento", "Next document", Pages, Always, true;
    PrevDoc = "doc.prev", "Documento anterior", "Previous document", Pages, Always, true;

    Undo = "edit.undo", "Desfazer", "Undo", Edit, NotText, true;
    Redo = "edit.redo", "Refazer", "Redo", Edit, NotText, true;
    Copy = "edit.copy", "Copiar", "Copy", Edit, Selection, true;
    Cut = "edit.cut", "Recortar", "Cut", Edit, Selection, true;
    Paste = "edit.paste", "Colar", "Paste", Edit, NotText, true;
    Duplicate = "edit.duplicate", "Duplicar", "Duplicate", Edit, Selection, true;
    PasteInPlace = "edit.paste_in_place", "Copiar no mesmo lugar", "Copy in place", Edit, Selection, true;
    Repeat = "edit.repeat", "Repetir a última ação", "Repeat last action", Edit, NotText, true;
    SelectAll = "edit.select_all", "Selecionar tudo", "Select all", Edit, NotText, true;
    InvertSelection = "edit.invert_selection", "Inverter seleção", "Invert selection", Edit, NotText, true;
    SelectSameKind = "edit.select_same_kind", "Selecionar mesmo tipo", "Select same type", Edit, Selection, true;
    SelectSameFill = "edit.select_same_fill", "Selecionar mesmo preenchimento", "Select same fill", Edit, Selection, true;
    SelectSameStroke = "edit.select_same_stroke", "Selecionar mesma cor de contorno", "Select same outline color", Edit, Selection, true;

    ToCurves = "obj.to_curves", "Converter em curvas", "Convert to curves", Object, Selection, true;
    OutlineToObject = "obj.outline_to_object", "Converter contorno em objeto", "Convert outline to object", Object, Selection, true;
    Group_ = "obj.group", "Agrupar", "Group", Object, Multi, true;
    Weld = "obj.weld", "Soldar", "Weld", Object, Multi, true;
    Trim = "obj.trim", "Aparar", "Trim", Object, Multi, true;
    Intersect = "obj.intersect", "Interseção", "Intersect", Object, Multi, true;
    Simplify = "obj.simplify", "Simplificar", "Simplify", Object, Multi, true;
    FrontMinusBack = "obj.front_minus_back", "Frente menos trás", "Front minus back", Object, Multi, true;
    BackMinusFront = "obj.back_minus_front", "Trás menos frente", "Back minus front", Object, Multi, true;
    Exclude = "obj.exclude", "Excluir sobreposição", "Exclude overlap", Object, Multi, true;
    Divide = "obj.divide", "Dividir", "Divide", Object, Multi, true;
    Boundary = "obj.boundary", "Limite", "Boundary", Object, Selection, true;
    Combine = "obj.combine", "Combinar", "Combine", Object, Multi, true;
    BreakApart = "obj.break_apart", "Separar", "Break apart", Object, Selection, true;
    Offset = "obj.offset", "Contorno de deslocamento", "Offset path", Object, Selection, true;
    CutLine = "cut.line", "Gerar linha de corte", "Create cut line", Object, Selection, true;
    RegistrationMarks = "cut.registration_marks", "Marcas de registro", "Registration marks", Object, NotText, true;
    Ungroup = "obj.ungroup", "Desagrupar", "Ungroup", Object, Selection, true;
    UngroupAll = "obj.ungroup_all", "Desagrupar tudo", "Ungroup all", Object, Selection, true;
    ExtendToBleed = "obj.extend_bleed", "Estender até a sangria", "Extend to bleed", Object, Selection, true;
    Preflight = "view.preflight", "Verificação prévia", "Preflight", View, Always, true;
    LockSelection = "obj.lock", "Bloquear", "Lock", Object, Selection, true;
    UnlockSelection = "obj.unlock", "Desbloquear", "Unlock", Object, Selection, true;
    HideSelection = "obj.hide", "Ocultar", "Hide", Object, Selection, true;
    ShowAll = "obj.show_all", "Mostrar todos", "Show all", Object, NotText, true;
    UnlockAll = "obj.unlock_all", "Desbloquear todos", "Unlock all", Object, NotText, true;
    InFrontOf = "obj.in_front_of", "À frente de…", "In front of…", Object, Selection, true;
    BehindOf = "obj.behind", "Atrás de…", "Behind…", Object, Selection, true;
    FocusMode = "view.focus", "Modo foco", "Focus mode", View, NotText, true;
    ExportSelection = "file.export_selection", "Exportar seleção…", "Export selection…", File, Selection, true;
    FitTextToPath = "text.fit_to_path", "Ajustar texto ao caminho", "Fit text to path", Object, Multi, true;
    DetachTextPath = "text.detach_path", "Separar texto do caminho", "Detach text from path", Object, Selection, true;
    ConvertTextKind = "text.convert_kind", "Converter texto artístico/parágrafo", "Convert artistic/paragraph text", Object, Selection, true;
    ConvertToBitmap = "obj.convert_to_bitmap", "Converter em bitmap", "Convert to bitmap", Object, Selection, true;
    ClearTransformations = "obj.clear_transformations", "Limpar transformações", "Clear transformations", Object, Selection, true;
    TransformPosition = "panel.transform_position", "Transformar: posição", "Transform: position", Object, Always, true;
    TransformRotate = "panel.transform_rotate", "Transformar: girar", "Transform: rotate", Object, Always, true;

    AlignTop = "align.top", "Alinhar pelo topo", "Align top", Align, Multi, true;
    AlignBottom = "align.bottom", "Alinhar pela base", "Align bottom", Align, Multi, true;
    AlignLeft = "align.left", "Alinhar pela esquerda", "Align left", Align, Multi, true;
    AlignRight = "align.right", "Alinhar pela direita", "Align right", Align, Multi, true;
    AlignCentersV = "align.centers_vertical", "Alinhar centros na vertical", "Align centers vertically", Align, Multi, true;
    AlignCentersH = "align.centers_horizontal", "Alinhar centros na horizontal", "Align centers horizontally", Align, Multi, true;
    CenterOnPage = "align.center_page", "Centralizar na página", "Center on page", Align, Selection, true;

    ToFront = "order.front", "Trazer para a frente de tudo", "Bring to front", Order, Selection, true;
    ToBack = "order.back", "Enviar para trás de tudo", "Send to back", Order, Selection, true;
    Forward = "order.forward", "Um passo à frente", "Bring forward", Order, Selection, true;
    Backward = "order.backward", "Um passo atrás", "Send backward", Order, Selection, true;

    ZoomObjects = "view.zoom_objects", "Zoom em todos os objetos", "Zoom to all objects", View, NotText, true;
    ZoomPage = "view.zoom_page", "Ajustar à página", "Fit page", View, NotText, true;
    ActualSize = "view.actual_size", "Tamanho real", "Actual size", View, NotText, true;
    ZoomIn = "view.zoom_in", "Aproximar", "Zoom in", View, NotText, true;
    ZoomOut = "view.zoom_out", "Afastar", "Zoom out", View, NotText, true;

    ToolSelect = "tool.select", "Ferramenta Seleção", "Select tool", Tools, NotText, true;
    ToolToggleSelect = "tool.toggle_select", "Alternar com a Seleção", "Toggle with Select", Tools, NotText, true;
    ToolShape = "tool.shape", "Ferramenta Forma (nós)", "Shape tool (nodes)", Tools, NotText, true;
    ToolZoom = "tool.zoom", "Ferramenta Zoom", "Zoom tool", Tools, NotText, true;
    ToolPan = "tool.pan", "Ferramenta Mão", "Hand tool", Tools, NotText, true;
    ToolBezier = "tool.bezier", "Ferramenta Bézier", "Bézier tool", Tools, NotText, true;
    ToolRect = "tool.rect", "Ferramenta Retângulo", "Rectangle tool", Tools, NotText, true;
    ToolEllipse = "tool.ellipse", "Ferramenta Elipse", "Ellipse tool", Tools, NotText, true;
    ToolArtboard = "tool.artboard", "Ferramenta Prancheta (tamanho da página)", "Artboard tool (page size)", Tools, NotText, true;
    ToolFreehand = "tool.freehand", "Ferramenta Mão Livre", "Freehand tool", Tools, NotText, false;
    ToolArtistic = "tool.artistic", "Ferramenta Mídia Artística", "Artistic media tool", Tools, NotText, false;
    ToolPolygon = "tool.polygon", "Ferramenta Polígono", "Polygon tool", Tools, NotText, true;
    ToolText = "tool.text", "Ferramenta Texto", "Text tool", Tools, NotText, true;
    ToolInteractiveFill = "tool.interactive_fill", "Preenchimento interativo", "Interactive fill", Tools, NotText, true;
    ToolSolidFill = "tool.solid_fill", "Preenchimento uniforme", "Uniform fill", Tools, NotText, true;
    ToolEyedropper = "tool.eyedropper", "Conta-gotas", "Eyedropper", Tools, NotText, true;
    ToolMeshFill = "tool.mesh_fill", "Preenchimento em malha", "Mesh fill", Tools, NotText, false;
    ToolEraser = "tool.eraser", "Ferramenta Borracha", "Eraser tool", Tools, NotText, false;

    PageNext = "page.next", "Próxima página", "Next page", Pages, NotText, true;
    PagePrev = "page.prev", "Página anterior", "Previous page", Pages, NotText, true;
    PageFirst = "page.first", "Primeira página", "First page", Pages, NotText, true;
    PageLast = "page.last", "Última página", "Last page", Pages, NotText, true;
    PageNew = "page.new", "Nova página após a atual", "New page after current", Pages, NotText, true;
    PageJump = "page.jump", "Ir para a página…", "Go to page…", Pages, NotText, true;
    PageRename = "page.rename", "Renomear página", "Rename page", Pages, NotText, true;

    TextProperties = "text.properties", "Propriedades de texto", "Text properties", Text, Always, false;
    TextGlyphs = "text.glyphs", "Glifos e caracteres especiais", "Glyphs and special characters", Text, Always, false;
    TextEdit = "text.edit", "Editar texto em janela", "Edit text in a window", Text, Always, false;
    TextBigger = "text.size_up", "Aumentar a fonte", "Increase font size", Text, Text, true;
    TextSmaller = "text.size_down", "Diminuir a fonte", "Decrease font size", Text, Text, true;
    TextAlignLeft = "text.align_left", "Texto à esquerda", "Align text left", Text, Text, true;
    TextAlignCenter = "text.align_center", "Centralizar texto", "Center text", Text, Text, true;
    TextAlignRight = "text.align_right", "Texto à direita", "Align text right", Text, Text, true;
    TextJustify = "text.justify", "Justificar texto", "Justify text", Text, Text, true;
    TextBold = "text.bold", "Negrito", "Bold", Text, Always, true;
    TextItalic = "text.italic", "Itálico", "Italic", Text, Always, true;
    TextToCurves = "text.to_curves", "Converter texto em curvas", "Convert text to curves", Text, Selection, true;
}

/// Combinação de teclas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chord {
    pub key: Key,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Chord {
    pub fn modifiers(self) -> Modifiers {
        Modifiers {
            alt: self.alt,
            ctrl: self.ctrl,
            shift: self.shift,
            mac_cmd: false,
            command: self.ctrl,
        }
    }

    fn weight(self) -> u8 {
        self.ctrl as u8 + self.shift as u8 + self.alt as u8
    }

    /// Texto como "Ctrl+Shift+Q", "Shift+F4", "Espaço".
    pub fn text(self) -> String {
        let mut s = String::new();
        if self.ctrl {
            s.push_str("Ctrl+");
        }
        if self.shift {
            s.push_str("Shift+");
        }
        if self.alt {
            s.push_str("Alt+");
        }
        s.push_str(&key_label(self.key));
        s
    }

    /// Forma estável para o arquivo: nome da tecla independente do idioma e
    /// do teclado ("Ctrl+Semicolon", não "Ctrl+Ç").
    pub fn code(self) -> String {
        let mut s = String::new();
        if self.ctrl {
            s.push_str("Ctrl+");
        }
        if self.shift {
            s.push_str("Shift+");
        }
        if self.alt {
            s.push_str("Alt+");
        }
        s.push_str(self.key.name());
        s
    }

    /// Lê "Ctrl+Shift+Q" (sem diferenciar maiúsculas; "Plus" para "+").
    /// Tecla modificadora sozinha (Ctrl, Shift, Alt, Win) não é atalho.
    pub fn parse(t: &str) -> Option<Chord> {
        let mut c = Chord {
            key: Key::A,
            ctrl: false,
            shift: false,
            alt: false,
        };
        let parts: Vec<&str> = t.split('+').map(str::trim).collect();
        let (last, mods) = parts.split_last()?;
        for m in mods {
            match m.to_lowercase().as_str() {
                "ctrl" | "cmd" | "control" => c.ctrl = true,
                "shift" => c.shift = true,
                "alt" | "option" => c.alt = true,
                "" => {}
                _ => return None,
            }
        }
        // "Ctrl++" chega como último pedaço vazio.
        let name = if last.is_empty() { "Plus" } else { last };
        c.key = match name.to_lowercase().as_str() {
            "espaço" | "space" => Key::Space,
            "pgup" | "pageup" => Key::PageUp,
            "pgdn" | "pagedown" => Key::PageDown,
            "+" | "plus" => Key::Plus,
            "-" | "minus" => Key::Minus,
            _ => Key::from_name(name)
                .or_else(|| Key::from_name(&name.to_uppercase()))
                .or_else(|| {
                    let mut ch = name.chars();
                    let first = ch.next()?.to_uppercase().collect::<String>();
                    Key::from_name(&(first + &ch.as_str().to_lowercase()))
                })?,
        };
        (!is_modifier(c.key)).then_some(c)
    }
}

/// Ctrl, Shift, Alt e Win chegam também como teclas próprias; sozinhas
/// não formam atalho.
pub fn is_modifier(k: Key) -> bool {
    matches!(
        k,
        Key::ShiftLeft
            | Key::ShiftRight
            | Key::ControlLeft
            | Key::ControlRight
            | Key::AltLeft
            | Key::AltRight
            | Key::SuperLeft
            | Key::SuperRight
    )
}

fn key_label(k: Key) -> String {
    match k {
        Key::Space => tr("Espaço", "Space").into(),
        Key::PageUp => "PgUp".into(),
        Key::PageDown => "PgDn".into(),
        Key::Plus => "+".into(),
        Key::Minus => "−".into(),
        other => layout_char(other).unwrap_or_else(|| other.name().to_string()),
    }
}

/// Caractere que a tecla de pontuação produz no teclado ativo (no ABNT2,
/// a tecla na posição do ";" americano é o "Ç"). Teclas que o egui não
/// conhece por letra chegam pela posição física.
#[cfg(windows)]
fn layout_char(k: Key) -> Option<String> {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn MapVirtualKeyW(code: u32, map_type: u32) -> u32;
    }
    // Código de varredura (conjunto 1) de cada posição física.
    let scan = match k {
        Key::Semicolon => 0x27,
        Key::Quote => 0x28,
        Key::Backtick => 0x29,
        Key::Backslash => 0x2B,
        Key::Comma => 0x33,
        Key::Period => 0x34,
        Key::Slash => 0x35,
        Key::Equals => 0x0D,
        Key::OpenBracket => 0x1A,
        Key::CloseBracket => 0x1B,
        Key::IntlBackslash => 0x56,
        _ => return None,
    };
    // SAFETY: função do Windows sem ponteiros; só converte números.
    let ch = unsafe {
        let vk = MapVirtualKeyW(scan, 1); // MAPVK_VSC_TO_VK
        MapVirtualKeyW(vk, 2) // MAPVK_VK_TO_CHAR
    } & 0x7FFF; // bit alto = tecla morta (acento)
    char::from_u32(ch)
        .filter(|c| !c.is_control() && !c.is_whitespace())
        .map(|c| c.to_uppercase().collect())
}

#[cfg(not(windows))]
fn layout_char(k: Key) -> Option<String> {
    Some(k.symbol_or_name().to_string()).filter(|s| s.chars().count() == 1)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    pub command: Command,
    pub chord: Chord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    GraphDraw,
    Alternative,
    Custom,
}

impl Profile {
    pub fn label(self) -> &'static str {
        match self {
            Profile::GraphDraw => tr("GraphDraw (padrão)", "GraphDraw (default)"),
            Profile::Alternative => tr(
                "Alternativo (teclas de função)",
                "Alternative (function keys)",
            ),
            Profile::Custom => tr("Personalizado", "Custom"),
        }
    }
    fn key(self) -> &'static str {
        match self {
            Profile::GraphDraw => "graphdraw",
            Profile::Alternative => "alternativo",
            Profile::Custom => "personalizado",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Keymap {
    pub profile: Profile,
    pub bindings: Vec<Binding>,
}

fn table(rows: &[(Command, &str)]) -> Vec<Binding> {
    rows.iter()
        .map(|(c, k)| Binding {
            command: *c,
            chord: Chord::parse(k).unwrap_or_else(|| panic!("atalho inválido: {k}")),
        })
        .collect()
}

impl Keymap {
    /// Esquema próprio do GraphDraw (padrão de fábrica).
    pub fn graphdraw() -> Keymap {
        use Command::*;
        Keymap {
            profile: Profile::GraphDraw,
            bindings: table(&[
                (New, "Ctrl+N"),
                (Open, "Ctrl+O"),
                (Save, "Ctrl+S"),
                (SaveAs, "Ctrl+Shift+S"),
                (Import, "Ctrl+I"),
                (Export, "Ctrl+E"),
                (Print, "Ctrl+P"),
                (Close, "Ctrl+W"),
                (NextDoc, "Ctrl+Tab"),
                (PrevDoc, "Ctrl+Shift+Tab"),
                (Undo, "Ctrl+Z"),
                (Redo, "Ctrl+Y"),
                (Redo, "Ctrl+Shift+Z"),
                (Copy, "Ctrl+C"),
                (Cut, "Ctrl+X"),
                (Paste, "Ctrl+V"),
                (Duplicate, "Ctrl+D"),
                (PasteInPlace, "Ctrl+Shift+D"),
                (Repeat, "Ctrl+Shift+R"),
                (SelectAll, "Ctrl+A"),
                (InvertSelection, "Ctrl+Shift+I"),
                (ToCurves, "Ctrl+Q"),
                (OutlineToObject, "Ctrl+Shift+Q"),
                (Group_, "Ctrl+G"),
                (Ungroup, "Ctrl+U"),
                (Combine, "Ctrl+L"),
                (BreakApart, "Ctrl+K"),
                (ToFront, "Shift+PageUp"),
                (ToBack, "Shift+PageDown"),
                (Forward, "Ctrl+PageUp"),
                (Backward, "Ctrl+PageDown"),
                (ZoomObjects, "Shift+0"),
                (ZoomPage, "0"),
                (ActualSize, "1"),
                (ZoomIn, "Plus"),
                (ZoomIn, "Equals"),
                (ZoomOut, "Minus"),
                (ToolSelect, "V"),
                (ToolShape, "N"),
                (ToolZoom, "Z"),
                (ToolPan, "H"),
                (ToolBezier, "B"),
                (ToolRect, "R"),
                (ToolEllipse, "E"),
                (ToolArtboard, "Shift+P"),
                (ToolText, "T"),
                (ToolPolygon, "Y"),
                (ToolInteractiveFill, "G"),
                (PageNext, "PageDown"),
                (PagePrev, "PageUp"),
                (PageFirst, "Home"),
                (PageLast, "End"),
                (PageNew, "Ctrl+Shift+N"),
                (PageJump, "Ctrl+J"),
                (PageRename, "F2"),
            ]),
        }
    }

    /// Perfil com teclas de função e letras de alinhamento, definido pelo
    /// responsável pelo produto (2026-10-16).
    pub fn alternative() -> Keymap {
        use Command::*;
        Keymap {
            profile: Profile::Alternative,
            bindings: table(&[
                // Essenciais
                (ZoomObjects, "F4"),
                (ZoomPage, "Shift+F4"),
                (CenterOnPage, "P"),
                (ToolZoom, "F2"),
                (ZoomOut, "F3"),
                (ToolToggleSelect, "Space"),
                (Group_, "Ctrl+G"),
                (Ungroup, "Ctrl+U"),
                // Ferramentas
                (ToolShape, "F10"),
                (ToolRect, "F6"),
                (ToolEllipse, "F7"),
                (ToolArtboard, "Shift+P"),
                (ToolFreehand, "F5"),
                (ToolArtistic, "I"),
                (ToolPolygon, "Y"),
                (ToolText, "F8"),
                (ToolInteractiveFill, "G"),
                (ToolMeshFill, "M"),
                (ToolPan, "H"),
                (ToolEraser, "X"),
                // Alinhamento (dois ou mais objetos)
                (AlignTop, "T"),
                (AlignBottom, "B"),
                (AlignLeft, "L"),
                (AlignRight, "R"),
                (AlignCentersV, "C"),
                (AlignCentersH, "E"),
                // Ordem
                (ToFront, "Shift+PageUp"),
                (ToBack, "Shift+PageDown"),
                (Forward, "Ctrl+PageUp"),
                (Backward, "Ctrl+PageDown"),
                // Edição de objetos
                (ToCurves, "Ctrl+Q"),
                (OutlineToObject, "Ctrl+Shift+Q"),
                (Duplicate, "Ctrl+D"),
                (PasteInPlace, "Plus"),
                (Repeat, "Ctrl+R"),
                (TransformPosition, "Alt+F7"),
                (TransformRotate, "Alt+F8"),
                // Texto
                (TextProperties, "Ctrl+T"),
                (TextGlyphs, "Ctrl+F11"),
                (TextEdit, "Ctrl+Shift+T"),
                (TextBigger, "Ctrl+8"),
                (TextSmaller, "Ctrl+2"),
                (TextAlignLeft, "Ctrl+L"),
                (TextAlignCenter, "Ctrl+E"),
                (TextAlignRight, "Ctrl+R"),
                (TextJustify, "Ctrl+J"),
                // Arquivo e sistema
                (New, "Ctrl+N"),
                (Open, "Ctrl+O"),
                (Save, "Ctrl+S"),
                (SaveAs, "Ctrl+Shift+S"),
                (Import, "Ctrl+I"),
                (Export, "Ctrl+E"),
                (Print, "Ctrl+P"),
                (Undo, "Ctrl+Z"),
                (Redo, "Ctrl+Shift+Z"),
                // Fora da lista, sem conflito com ela
                (SelectAll, "Ctrl+A"),
                (InvertSelection, "Ctrl+Shift+I"),
                (Close, "Ctrl+W"),
                (NextDoc, "Ctrl+Tab"),
                (PrevDoc, "Ctrl+Shift+Tab"),
                (PageNext, "PageDown"),
                (PagePrev, "PageUp"),
                (PageFirst, "Home"),
                (PageLast, "End"),
                (PageNew, "Ctrl+Shift+N"),
            ]),
        }
    }

    pub fn for_profile(p: Profile) -> Keymap {
        match p {
            Profile::Alternative => Keymap::alternative(),
            _ => Keymap::graphdraw(),
        }
    }

    /// Primeiro atalho do comando, como texto (para menus e dicas).
    pub fn label(&self, c: Command) -> Option<String> {
        self.bindings
            .iter()
            .find(|b| b.command == c)
            .map(|b| b.chord.text())
    }

    /// Espaço está livre para a mão temporária?
    pub fn space_is_free(&self) -> bool {
        !self
            .bindings
            .iter()
            .any(|b| b.chord.key == Key::Space && !b.chord.ctrl && !b.chord.alt && !b.chord.shift)
    }

    /// Define o atalho do comando (troca o primeiro; remove o mesmo atalho de
    /// outros comandos do mesmo contexto). O perfil vira "Personalizado".
    pub fn set(&mut self, c: Command, chord: Option<Chord>) {
        let same_ctx =
            |a: Command, b: Command| (a.when() == When::Text) == (b.when() == When::Text);
        if let Some(ch) = chord {
            self.bindings
                .retain(|b| !(b.chord == ch && b.command != c && same_ctx(b.command, c)));
        }
        let first = self.bindings.iter().position(|b| b.command == c);
        match (first, chord) {
            (Some(i), Some(ch)) => self.bindings[i].chord = ch,
            (None, Some(ch)) => self.bindings.push(Binding {
                command: c,
                chord: ch,
            }),
            (_, None) => self.bindings.retain(|b| b.command != c),
        }
        self.profile = Profile::Custom;
    }

    /// Comandos acionados neste quadro (as teclas usadas são consumidas).
    /// Combinações com mais modificadores são testadas antes (Ctrl+Shift+Z
    /// antes de Ctrl+Z). Atalhos sem Ctrl/Alt não valem enquanto se digita
    /// num campo.
    pub fn fired(&self, ctx: &egui::Context, state: State) -> Vec<Command> {
        let typing = ctx.egui_wants_keyboard_input();
        let mut order: Vec<&Binding> = self.bindings.iter().collect();
        order.sort_by_key(|b| std::cmp::Reverse(b.chord.weight()));
        let mut out = Vec::new();
        for b in order {
            if !b.command.when().ok(state) {
                continue;
            }
            if typing && !b.chord.ctrl && !b.chord.alt {
                continue;
            }
            let m = b.chord.modifiers();
            let hit = ctx.input_mut(|i| {
                let exact = i.modifiers.ctrl == m.ctrl
                    && i.modifiers.shift == m.shift
                    && i.modifiers.alt == m.alt;
                // "command" e "mac_cmd" vêm do próprio evento (no Windows o
                // Ctrl chega como ctrl e command ao mesmo tempo).
                let mm = Modifiers {
                    command: i.modifiers.command,
                    mac_cmd: i.modifiers.mac_cmd,
                    ..m
                };
                exact
                    && (i.consume_key(mm, b.chord.key)
                        || (!typing && take_clipboard_event(&mut i.events, b.chord)))
            });
            if hit && !out.contains(&b.command) {
                out.push(b.command);
            }
        }
        out
    }

    /// Combinação equivalente a um evento de área de transferência: o
    /// sistema entrega Ctrl+C/X/V (e Ctrl+Insert, Shift+Insert/Delete) como
    /// Copiar/Recortar/Colar, nunca como tecla.
    pub fn clipboard_chord(e: &egui::Event) -> Option<Chord> {
        let key = match e {
            egui::Event::Copy => Key::C,
            egui::Event::Cut => Key::X,
            egui::Event::Paste(_) | egui::Event::PasteImage(_) => Key::V,
            _ => return None,
        };
        Some(Chord {
            key,
            ctrl: true,
            shift: false,
            alt: false,
        })
    }

    // ---- JSON e arquivo de configuração ---------------------------------------

    pub fn to_json(&self) -> String {
        let f = KeymapFile {
            profile: self.profile.key().into(),
            bindings: self
                .bindings
                .iter()
                .map(|b| BindingFile {
                    command: b.command.id().into(),
                    keys: b.chord.code(),
                })
                .collect(),
        };
        serde_json::to_string_pretty(&f).unwrap_or_default()
    }

    /// Lê um mapa em JSON. Comandos e teclas desconhecidos são ignorados
    /// (e contados no segundo valor).
    pub fn from_json(text: &str) -> Result<(Keymap, usize), String> {
        let f: KeymapFile = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let profile = match f.profile.as_str() {
            "graphdraw" => Profile::GraphDraw,
            "alternativo" => Profile::Alternative,
            _ => Profile::Custom,
        };
        let mut skipped = 0;
        let mut bindings = Vec::new();
        for b in f.bindings {
            // "Espaço" é exibido traduzido; aceitar os dois.
            let keys = b.keys.replace("Espaço", "Space");
            match (Command::from_id(&b.command), Chord::parse(&keys)) {
                (Some(command), Some(chord)) => bindings.push(Binding { command, chord }),
                _ => skipped += 1,
            }
        }
        Ok((Keymap { profile, bindings }, skipped))
    }

    fn config_path() -> Option<PathBuf> {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("GraphDraw").join("atalhos.json"))
    }

    /// Mapa salvo do usuário; sem arquivo, o padrão de fábrica.
    pub fn load() -> Keymap {
        let Some(p) = Keymap::config_path() else {
            return Keymap::graphdraw();
        };
        match std::fs::read_to_string(&p)
            .ok()
            .map(|t| Keymap::from_json(&t))
        {
            Some(Ok((k, _))) => match k.profile {
                // Perfis prontos são relidos do código, para receber atalhos novos.
                Profile::Custom => k,
                other => Keymap::for_profile(other),
            },
            _ => Keymap::graphdraw(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let p = Keymap::config_path().ok_or("pasta de configuração não encontrada")?;
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(&p, self.to_json()).map_err(|e| e.to_string())
    }
}

/// Tira de `events` o evento de área de transferência que equivale a
/// `chord` (ver [`Keymap::clipboard_chord`]).
fn take_clipboard_event(events: &mut Vec<egui::Event>, chord: Chord) -> bool {
    match events
        .iter()
        .position(|e| Keymap::clipboard_chord(e) == Some(chord))
    {
        Some(i) => {
            events.remove(i);
            true
        }
        None => false,
    }
}

#[derive(Serialize, Deserialize)]
struct KeymapFile {
    profile: String,
    bindings: Vec<BindingFile>,
}

#[derive(Serialize, Deserialize)]
struct BindingFile {
    command: String,
    keys: String,
}

// ---- mapa ativo, para rótulos de menus e dicas ----------------------------------

use std::collections::HashMap;
use std::sync::{Mutex, RwLock};

static CURRENT: RwLock<Option<Keymap>> = RwLock::new(None);
static LABELS: Mutex<Option<HashMap<(String, bool), &'static str>>> = Mutex::new(None);

/// Torna `k` o mapa usado pelos rótulos (chamar ao carregar e ao mudar).
pub fn set_current(k: &Keymap) {
    *CURRENT.write().unwrap() = Some(k.clone());
}

fn intern(text: String, menu: bool) -> &'static str {
    let mut g = LABELS.lock().unwrap();
    let map = g.get_or_insert_with(HashMap::new);
    // Poucas combinações distintas (rótulo × atalho): o cache não cresce sem limite.
    map.entry((text.clone(), menu))
        .or_insert_with(|| Box::leak(text.into_boxed_str()))
}

fn current_label(c: Command) -> Option<String> {
    CURRENT.read().unwrap().as_ref().and_then(|k| k.label(c))
}

/// "Texto (atalho)" para dicas; só o texto se o comando não tiver atalho.
/// Texto de dica com vida longa (reaproveitado entre quadros).
pub fn intern_tip(text: String) -> &'static str {
    intern(text, false)
}

pub fn hint(text: &str, c: Command) -> &'static str {
    match current_label(c) {
        Some(k) => intern(format!("{text} ({k})"), false),
        None => intern(text.to_string(), false),
    }
}

/// "Texto  (atalho)" para itens de menu.
pub fn menu(text: &str, c: Command) -> &'static str {
    match current_label(c) {
        Some(k) => intern(format!("{text}  ({k})"), true),
        None => intern(text.to_string(), true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_parse_and_print() {
        for t in [
            "Ctrl+Shift+Q",
            "Shift+F4",
            "F10",
            "Alt+F7",
            "Ctrl+PgUp",
            "T",
        ] {
            let c = Chord::parse(t).unwrap_or_else(|| panic!("{t}"));
            assert_eq!(Chord::parse(&c.text()), Some(c), "{t}");
        }
        assert_eq!(Chord::parse("ctrl+shift+q"), Chord::parse("Ctrl+Shift+Q"));
        assert_eq!(Chord::parse("Plus").unwrap().key, Key::Plus);
        assert_eq!(Chord::parse("Space").unwrap().key, Key::Space);
        assert!(Chord::parse("Hiper+Q").is_none());
    }

    /// Os perfis prontos não têm o mesmo atalho para dois comandos do mesmo
    /// contexto (texto × resto podem repetir: Ctrl+R, Ctrl+E, Ctrl+J).
    #[test]
    fn built_in_profiles_have_no_conflicts() {
        for k in [Keymap::graphdraw(), Keymap::alternative()] {
            for (i, a) in k.bindings.iter().enumerate() {
                for b in &k.bindings[i + 1..] {
                    let text = |c: Command| c.when() == When::Text;
                    if a.chord == b.chord && a.command != b.command {
                        assert_ne!(
                            text(a.command),
                            text(b.command),
                            "{:?}: {:?} × {:?} em {:?}",
                            a.chord.text(),
                            a.command,
                            b.command,
                            k.profile
                        );
                    }
                }
            }
        }
    }

    /// A lista pedida está no perfil Alternativo.
    #[test]
    fn alternative_profile_matches_the_requested_list() {
        let k = Keymap::alternative();
        for (c, t) in [
            (Command::ZoomObjects, "F4"),
            (Command::ZoomPage, "Shift+F4"),
            (Command::CenterOnPage, "P"),
            (Command::ToolShape, "F10"),
            (Command::AlignCentersV, "C"),
            (Command::PasteInPlace, "+"),
            (Command::Repeat, "Ctrl+R"),
            (Command::TransformRotate, "Alt+F8"),
            (Command::Redo, "Ctrl+Shift+Z"),
        ] {
            assert_eq!(k.label(c).as_deref(), Some(t), "{c:?}");
        }
        assert!(!k.space_is_free(), "espaço alterna com a Seleção");
        assert!(
            Keymap::graphdraw().space_is_free(),
            "no padrão, espaço = mão"
        );
    }

    #[test]
    fn json_round_trip_and_editing() {
        let mut k = Keymap::alternative();
        k.set(Command::ToolEllipse, Chord::parse("Ctrl+Alt+E"));
        assert_eq!(k.profile, Profile::Custom);
        let (back, skipped) = Keymap::from_json(&k.to_json()).unwrap();
        assert_eq!(skipped, 0);
        assert_eq!(back, k);
        // Atribuir um atalho já usado tira do outro comando (mesmo contexto).
        k.set(Command::ToolRect, Chord::parse("F7"));
        assert_eq!(k.label(Command::ToolRect).as_deref(), Some("F7"));
        assert_ne!(k.label(Command::ToolEllipse).as_deref(), Some("F7"));
        let (_, skipped) =
            Keymap::from_json(r#"{"profile":"x","bindings":[{"command":"nada","keys":"F1"}]}"#)
                .unwrap();
        assert_eq!(skipped, 1);
    }

    /// Todo atalho salvo no arquivo volta igual ao reabrir o programa.
    #[test]
    fn every_key_survives_save_and_load() {
        for &key in Key::ALL {
            for (ctrl, shift, alt) in [
                (false, false, false),
                (true, false, false),
                (true, true, true),
            ] {
                let c = Chord {
                    key,
                    ctrl,
                    shift,
                    alt,
                };
                let want = (!is_modifier(key)).then_some(c);
                assert_eq!(Chord::parse(&c.code()), want, "{}", c.code());
            }
        }
        // Arquivo com o Ctrl gravado sozinho: ignorado, não vira atalho.
        let (k, skipped) = Keymap::from_json(
            r#"{"profile":"custom","bindings":[{"command":"obj.combine","keys":"Ctrl+ControlLeft"}]}"#,
        )
        .unwrap();
        assert_eq!((k.bindings.len(), skipped), (0, 1));
    }

    /// Ctrl+C/X/V chegam do sistema como eventos de área de transferência,
    /// não como tecla: precisam disparar os atalhos mesmo assim.
    #[test]
    fn clipboard_shortcuts_fire() {
        let k = Keymap::graphdraw();
        let ctrl = Modifiers::CTRL;
        for (ev, cmd) in [
            (egui::Event::Copy, Command::Copy),
            (egui::Event::Cut, Command::Cut),
            (egui::Event::Paste("x".into()), Command::Paste),
        ] {
            let got = fire(&k, vec![egui::Event::ModifiersChanged(ctrl), ev], 1);
            assert_eq!(got, [cmd]);
        }
        // Remapeado: Ctrl+C passa a ser outro comando.
        let mut k2 = k.clone();
        k2.set(Command::Duplicate, Chord::parse("Ctrl+C"));
        let got = fire(
            &k2,
            vec![egui::Event::ModifiersChanged(ctrl), egui::Event::Copy],
            1,
        );
        assert_eq!(got, [Command::Duplicate]);
    }

    /// Dispara comandos a partir de eventos de teclado de verdade.
    fn press(k: &Keymap, key: Key, m: Modifiers, selected: usize) -> Vec<Command> {
        let events = vec![
            egui::Event::ModifiersChanged(m),
            egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: m,
            },
        ];
        fire(k, events, selected)
    }

    fn fire(k: &Keymap, events: Vec<egui::Event>, selected: usize) -> Vec<Command> {
        let ctx = egui::Context::default();
        let mut got = Vec::new();
        let mut events = Some(events);
        for frame in 0..2 {
            let events = if frame == 1 {
                events.take().unwrap()
            } else {
                vec![]
            };
            let input = egui::RawInput {
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                if frame == 1 {
                    got = k.fired(
                        ui.ctx(),
                        State {
                            selected,
                            editing_text: false,
                        },
                    );
                }
            });
            out.textures_delta.clear();
        }
        got
    }

    #[test]
    fn keys_fire_the_right_commands() {
        let alt = Keymap::alternative();
        let none = Modifiers::NONE;
        let ctrl = Modifiers::CTRL;
        assert_eq!(press(&alt, Key::P, none, 1), [Command::CenterOnPage]);
        assert_eq!(press(&alt, Key::F4, none, 0), [Command::ZoomObjects]);
        assert_eq!(
            press(&alt, Key::F4, Modifiers::SHIFT, 0),
            [Command::ZoomPage]
        );
        assert_eq!(
            press(&alt, Key::Z, ctrl | Modifiers::SHIFT, 0),
            [Command::Redo]
        );
        assert_eq!(press(&alt, Key::Z, ctrl, 0), [Command::Undo]);
        assert_eq!(
            press(&alt, Key::R, ctrl, 0),
            [Command::Repeat],
            "fora do texto, Ctrl+R repete"
        );
        assert_eq!(press(&alt, Key::T, none, 2), [Command::AlignTop]);
        assert!(
            press(&alt, Key::T, none, 1).is_empty(),
            "alinhar pede 2 ou mais"
        );
        assert_eq!(
            press(&alt, Key::F7, Modifiers::ALT, 0),
            [Command::TransformPosition]
        );
        // Como o Windows entrega o Ctrl.
        let win_ctrl = Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        assert_eq!(press(&alt, Key::G, win_ctrl, 2), [Command::Group_]);
        let gd = Keymap::graphdraw();
        assert_eq!(press(&gd, Key::R, none, 0), [Command::ToolRect]);
        assert_eq!(
            press(&gd, Key::Z, ctrl | Modifiers::SHIFT, 0),
            [Command::Redo]
        );
    }

    #[test]
    fn context_rules() {
        let s = |n| State {
            selected: n,
            editing_text: false,
        };
        assert!(!When::Multi.ok(s(1)) && When::Multi.ok(s(2)));
        assert!(!When::Selection.ok(s(0)));
        assert!(!When::Text.ok(s(3)));
    }
}
