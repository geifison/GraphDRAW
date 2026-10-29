# Mapa de ícones (Passo 3b)

Cada comando ou ferramenta da base e o desenho SVG do conjunto de ícones do
GraphDraw (`apps/graphdraw/assets/icons`) que o representa. Onde ha
equivalente, o SVG é desenhado; onde não há ("sem equivalente"), fica o
ícone pintado em código (`ui/icons.rs`, `ui/propbar.rs`,
`ui/menu_glyphs.rs`, `ui/menu_pics.rs`, `ui/envelope_ui.rs`), que
continua no código como reserva.

A tabela vem de `apps/graphdraw/src/ui/svg_icons.rs` (`svg_for`), a
única consulta. A função `svg_icons::paint` é o ponto de entrada único:
`icons::draw`, `icons::draw_action`, `propbar::draw_pic` e
`menu_glyphs::draw` passam por ela, e as linhas de menu resolvem a chave
do menu para o mesmo ícone (`menus::menu_icon_ref`). Assim o mesmo
comando mostra o mesmo ícone na caixa de ferramentas, nas barras, nos
menus, no menu de contexto, nas dicas e na lista de atalhos.

Os SVGs são monocromáticos (branco); a interface aplica a cor do tema
(ícone normal, desabilitado esmaecido, cor do grupo com ícones coloridos).
São rasterizados com resvg no tamanho exato em pixels físicos (grades de
16, 20 e 24 px) e guardados como texturas.

Exceção: os nomes `color/...` (`assets/icons/color/`) são figuras em cores
próprias, desenhadas sem a cor do tema quando habilitadas e esmaecidas
quando não. Alinhar, Distribuir, Pra Frente, Pra Trás, Combinar e Separar usam essas figuras
em todo lugar (barra de propriedades, menus, menu de contexto, dockers). Regra: um ícone trocado vale em todo lugar; não há segundo mapa por tela.

## Ferramentas (`Tool`: caixa de ferramentas, flyouts, dicas, cursores, dockers)

| id na base | SVG do GraphDraw | observações |
|---|---|---|
| `Tool::Pick` | `tool-select.svg` |  |
| `Tool::FreeformPick` | sem equivalente | `cur-select-pick` é um cursor, não a ferramenta de seleção à mão livre |
| `Tool::FreeTransform` | `tool-free-transform.svg` |  |
| `Tool::Shape` | `tool-shape.svg` |  |
| `Tool::Smooth` | sem equivalente |  |
| `Tool::Smear` | sem equivalente |  |
| `Tool::Twirl` | sem equivalente |  |
| `Tool::AttractRepel` | sem equivalente |  |
| `Tool::Smudge` | sem equivalente |  |
| `Tool::Roughen` | sem equivalente |  |
| `Tool::VariableOutline` | `tool-width.svg` | contorno de largura variável |
| `Tool::Crop` | `tool-crop.svg` |  |
| `Tool::Knife` | `tool-knife.svg` |  |
| `Tool::SegmentDelete` | `tool-virtual-delete.svg` | apagar segmento virtual |
| `Tool::Eraser` | `tool-eraser.svg` |  |
| `Tool::Zoom` | `tool-zoom.svg` |  |
| `Tool::Pan` | `tool-pan.svg` |  |
| `Tool::Freehand` | `tool-freehand.svg` |  |
| `Tool::TwoPointLine` | `tool-line2.svg` |  |
| `Tool::Bezier` | `tool-bezier.svg` |  |
| `Tool::Pen` | sem equivalente | o GraphDraw só tem o desenho da Bézier; usá-lo deixaria Caneta e Bézier iguais |
| `Tool::BSpline` | `tool-bspline.svg` |  |
| `Tool::Polyline` | `tool-polyline.svg` |  |
| `Tool::ThreePointCurve` | `tool-curve3.svg` |  |
| `Tool::BrushStrokes` | `tool-artistic.svg` | pincel; no GraphDraw antigo era a mídia artística; duvidoso, ver abaixo |
| `Tool::PainterlyBrush` | sem equivalente |  |
| `Tool::ShapeRecognition` | `tool-smart-draw.svg` | desenho inteligente (reconhece formas) |
| `Tool::Sketch` | sem equivalente |  |
| `Tool::Rectangle` | `tool-rect.svg` |  |
| `Tool::ThreePointRectangle` | `tool-rect-3pt.svg` |  |
| `Tool::Ellipse` | `tool-ellipse.svg` |  |
| `Tool::ThreePointEllipse` | `tool-ellipse-3pt.svg` |  |
| `Tool::Polygon` | `tool-polygon.svg` |  |
| `Tool::Star` | `poly-star.svg` | mesmo desenho da figura `Pic::Star` da barra de propriedades |
| `Tool::Spiral` | `tool-spiral.svg` |  |
| `Tool::CommonShapes` | `shape-basic.svg` | formas básicas |
| `Tool::ActionLines` | sem equivalente |  |
| `Tool::GraphPaper` | `tool-graph.svg` | grade de papel quadriculado |
| `Tool::Text` | `tool-text.svg` |  |
| `Tool::Table` | sem equivalente | `grid` e `tool-graph` são grades sem cabeçalho; a tabela pintada é mais clara |
| `Tool::ParallelDimension` | sem equivalente | `tool-dimension` é uma cota horizontal; foi para Horizontal/Vertical |
| `Tool::HorizontalVerticalDimension` | `tool-dimension.svg` | cota horizontal com setas e linhas de chamada |
| `Tool::AngularDimension` | sem equivalente |  |
| `Tool::SegmentDimension` | sem equivalente |  |
| `Tool::Callout` | sem equivalente |  |
| `Tool::Connector` | `tool-connector.svg` | conector de linha reta |
| `Tool::RightAngleConnector` | sem equivalente |  |
| `Tool::RoundedConnector` | sem equivalente |  |
| `Tool::AnchorEditing` | sem equivalente |  |
| `Tool::DropShadow` | `tool-shadow.svg` |  |
| `Tool::Contour` | `tool-contour.svg` |  |
| `Tool::Blend` | `tool-blend.svg` |  |
| `Tool::Distort` | sem equivalente | `tool-envelope` e `tp-warp` são outros comandos |
| `Tool::Envelope` | `tool-envelope.svg` |  |
| `Tool::Extrude` | `tool-extrude.svg` |  |
| `Tool::BlockShadow` | sem equivalente |  |
| `Tool::Transparency` | sem equivalente |  |
| `Tool::ColorEyedropper` | `tool-eyedropper.svg` |  |
| `Tool::AttributesEyedropper` | sem equivalente | `tool-eyedropper` fica só com o conta-gotas de cor |
| `Tool::InteractiveFill` | `tool-gradient.svg` | retângulo com vetor de degradê (preenchimento interativo); duvidoso, ver abaixo |
| `Tool::AreaFill` | `tool-smart-fill.svg` | preenche a área fechada entre objetos (preenchimento inteligente) |
| `Tool::MeshFill` | `tool-mesh.svg` |  |
| `Tool::OutlinePen` | `outline-pen.svg` | mesmo desenho de `Pic::Pen` |
| `Tool::OutlineColor` | sem equivalente | `outline-pen` já é a caneta de contorno |

## Ações da barra padrão (`icons::Action`: barra padrão e menus)

| id na base | SVG do GraphDraw | observações |
|---|---|---|
| `Action::New` | `file-new.svg` |  |
| `Action::Open` | `file-open.svg` |  |
| `Action::Save` | `file-save.svg` |  |
| `Action::Print` | `file-print.svg` |  |
| `Action::Cut` | `cut.svg` |  |
| `Action::Copy` | `copy.svg` |  |
| `Action::Paste` | `paste.svg` |  |
| `Action::Undo` | `undo.svg` |  |
| `Action::Redo` | `redo.svg` |  |
| `Action::Import` | `file-import.svg` |  |
| `Action::Export` | `file-export.svg` |  |
| `Action::Pdf` | `file-pdf.svg` | importar, exportar, PDF, desfazer e refazer vêm da folha desenhada pelo responsável pelo produto, via `split-file.mjs` |
| `Action::SnapOff` | sem equivalente | o ícone pintado mostra o estado (cortado em vermelho); `snap` perderia o estado |
| `Action::Options` | sem equivalente | não há engrenagem no conjunto |
| `Action::Welcome` | sem equivalente |  |
| `Action::Fullscreen` | sem equivalente |  |
| `Action::ShowRulers` | sem equivalente |  |
| `Action::ShowGrid` | `grid.svg` | grade; o estado ligado aparece no botão pressionado |
| `Action::ShowGuidelines` | sem equivalente | `guide-object` é outro comando (guia a partir de objeto) |
| `Action::Launch` | sem equivalente |  |
| `Action::GitPull` | sem equivalente |  |
| `Action::GitPush` | sem equivalente |  |

## Figuras da barra de propriedades (`propbar::Pic`: barras, dockers e menus)

| id na base | SVG do GraphDraw | observações |
|---|---|---|
| `Pic::Width` | sem equivalente |  |
| `Pic::Height` | sem equivalente |  |
| `Pic::PageWidth` | sem equivalente |  |
| `Pic::PageHeight` | sem equivalente |  |
| `Pic::Portrait` | `page-portrait.svg` |  |
| `Pic::Landscape` | `page-landscape.svg` |  |
| `Pic::AllPages` | sem equivalente |  |
| `Pic::CurrentPage` | sem equivalente |  |
| `Pic::Nudge` | `nudge.svg` |  |
| `Pic::DupX` | sem equivalente |  |
| `Pic::DupY` | sem equivalente |  |
| `Pic::TreatFilled` | sem equivalente |  |
| `Pic::Rotate` | `rotate-cw.svg` | girar (campo de ângulo e página Girar do docker Transformações), como no GraphDraw antigo |
| `Pic::Lock(true)` | `lock-on.svg` | versão de 16 px nos menus |
| `Pic::Lock(false)` | `lock-off.svg` | versão de 16 px nos menus |
| `Pic::MirrorH` | `mirror-h.svg` |  |
| `Pic::MirrorV` | `mirror-v.svg` |  |
| `Pic::Pen` | `outline-pen.svg` | largura do contorno |
| `Pic::ZoomIn` | `zoom-in.svg` |  |
| `Pic::ZoomOut` | `zoom-out.svg` |  |
| `Pic::ZoomSelected` | `fit-selection.svg` | ajustar à seleção |
| `Pic::ZoomAll` | `fit-art.svg` | ajustar a todos os objetos (Ver > Ajustar) |
| `Pic::ZoomPage` | `fit-page.svg` | ajustar à página |
| `Pic::ZoomWidth` | sem equivalente |  |
| `Pic::ZoomHeight` | sem equivalente |  |
| `Pic::Ellipse` | `ellipse-whole.svg` |  |
| `Pic::Pie` | `ellipse-pie.svg` |  |
| `Pic::Arc` | `ellipse-arc.svg` |  |
| `Pic::ArcDirection` | sem equivalente |  |
| `Pic::CornerRound` | `corner-round.svg` |  |
| `Pic::CornerScallop` | `corner-scallop.svg` |  |
| `Pic::CornerChamfer` | `corner-chamfer.svg` |  |
| `Pic::CornerScaling` | sem equivalente |  |
| `Pic::ToFront` | `color/order-front.svg` |  |
| `Pic::ToBack` | `color/order-back.svg` |  |
| `Pic::ToCurves` | `to-curves.svg` |  |
| `Pic::AddNode` | `node-add.svg` |  |
| `Pic::DeleteNode` | `node-delete.svg` |  |
| `Pic::JoinNodes` | `node-join.svg` |  |
| `Pic::BreakCurve` | `node-break.svg` |  |
| `Pic::ToLine` | `seg-line.svg` |  |
| `Pic::ToCurve` | `seg-curve.svg` |  |
| `Pic::NodeCusp` | `node-corner.svg` |  |
| `Pic::NodeSmooth` | `node-smooth.svg` |  |
| `Pic::NodeSymmetrical` | `node-symmetric.svg` |  |
| `Pic::ReverseDirection` | `path-reverse.svg` |  |
| `Pic::ExtendToClose` | `node-connect.svg` | liga as pontas com um segmento; duvidoso, ver abaixo |
| `Pic::ExtractSubpath` | `node-extract.svg` |  |
| `Pic::CloseCurve` | `path-close.svg` | fechar curva |
| `Pic::StretchNodes` | sem equivalente |  |
| `Pic::RotateNodes` | sem equivalente |  |
| `Pic::AlignNodes` | sem equivalente | `node-align-h`/`node-align-v` alinham numa direção; o botão abre a escolha |
| `Pic::ReflectH` | sem equivalente | `mirror-h` é espelhar objeto; refletir nós fica com o pintado |
| `Pic::ReflectV` | sem equivalente | `mirror-v` é espelhar objeto; refletir nós fica com o pintado |
| `Pic::Elastic` | sem equivalente |  |
| `Pic::SelectAllNodes` | sem equivalente | `select-all` é selecionar todos os objetos |
| `Pic::ReduceNodes` | `node-reduce.svg` |  |
| `Pic::Star` | `poly-star.svg` |  |
| `Pic::ComplexStar` | `poly-complex.svg` |  |
| `Pic::Group` | `group.svg` |  |
| `Pic::Ungroup` | `ungroup.svg` |  |
| `Pic::UngroupAll` | `ungroup-all.svg` |  |
| `Pic::Combine` | `color/combine.svg` |  |
| `Pic::BreakApart` | `color/break-apart.svg` |  |
| `Pic::Weld` | `bool-weld.svg` |  |
| `Pic::Trim` | `bool-trim.svg` |  |
| `Pic::Intersect` | `bool-intersect.svg` |  |
| `Pic::Simplify` | `bool-simplify.svg` |  |
| `Pic::FrontMinusBack` | `bool-front-minus.svg` |  |
| `Pic::BackMinusFront` | `bool-back-minus.svg` |  |
| `Pic::Boundary` | `bool-boundary.svg` | as sete figuras de Modelar (`bool-weld` a `bool-boundary`) vêm da folha desenhada pelo responsável pelo produto, via `split-shaping.mjs` |
| `Pic::AlignDistribute` | sem equivalente |  |
| `Pic::TextLeft` | `text-left.svg` |  |
| `Pic::TextCenter` | `text-center.svg` |  |
| `Pic::TextRight` | `text-right.svg` |  |
| `Pic::TextJustify` | `text-justify.svg` |  |
| `Pic::TextNone` | sem equivalente |  |
| `Pic::TextForceJustify` | sem equivalente |  |
| `Pic::Bullets` | sem equivalente |  |
| `Pic::DropCap` | sem equivalente |  |
| `Pic::TextProperties` | `text-props.svg` |  |
| `Pic::EditText` | `text-edit.svg` |  |
| `Pic::WrapText` | sem equivalente |  |
| `Pic::Position` | `cur-move.svg` | seta de mover em quatro direções (página Posição), como no GraphDraw antigo |
| `Pic::ScaleMirror` | sem equivalente | `scale-xy` é o campo de escala em %, não a página do docker |
| `Pic::SizeBox` | sem equivalente |  |
| `Pic::SkewBox` | sem equivalente |  |
| `Pic::EnvelopeStraight` | sem equivalente |  |
| `Pic::EnvelopeSingleArc` | sem equivalente |  |
| `Pic::EnvelopeDoubleArc` | sem equivalente |  |
| `Pic::EnvelopeUnconstrained` | sem equivalente |  |
| `Pic::EnvelopeAddNew` | sem equivalente |  |
| `Pic::EnvelopeClear` | sem equivalente |  |
| `Pic::EnvelopeCopy` | sem equivalente |  |
| `Pic::KeepLines` | sem equivalente |  |
| `Pic::Tabs` | sem equivalente |  |
| `Pic::Columns` | sem equivalente |  |
| `Pic::TextStatistics` | sem equivalente |  |
| `Pic::ConvertText` | `text-convert.svg` | converter texto artístico/parágrafo |
| `Pic::FitTextToPath` | `text-path.svg` |  |
| `Pic::StraightenText` | `tp-straighten.svg` | endireitar texto que está num caminho (`tp-straighten`) |
| `Pic::AlignBaseline` | `align-baseline.svg` | Texto > Alinhar à linha de base |
| `Pic::ChangeCase` | `text-case.svg` |  |
| `Pic::Encode` | sem equivalente |  |
| `Pic::ConvertToBitmap` | `to-bitmap.svg` |  |
| `Pic::StraightenImage` | sem equivalente |  |
| `Pic::EditBitmap` | sem equivalente |  |
| `Pic::CropBitmap` | sem equivalente | `tool-crop` é a ferramenta Cortar; o comando de bitmap tem figura própria |
| `Pic::Resample` | sem equivalente |  |
| `Pic::BreakLink` | sem equivalente |  |
| `Pic::UpdateLink` | sem equivalente |  |
| `Pic::QuickTrace` | sem equivalente |  |
| `Pic::NewWindow` | sem equivalente |  |
| `Pic::RefreshWindow` | sem equivalente |  |
| `Pic::CloseWindow` | sem equivalente |  |
| `Pic::CloseAll` | sem equivalente |  |
| `Pic::Cascade` | sem equivalente |  |
| `Pic::TileHorizontally` | sem equivalente |  |
| `Pic::TileVertically` | sem equivalente |  |
| `Pic::CombineWindows` | sem equivalente |  |
| `Pic::DockWindow` | sem equivalente |  |

## Figuras das linhas de menu (`menu_glyphs::Glyph`: menus, menu de contexto, docker Alinhar)

| id na base | SVG do GraphDraw | observações |
|---|---|---|
| `Glyph::Help` | sem equivalente |  |
| `Glyph::Video` | sem equivalente |  |
| `Glyph::QuickStart` | sem equivalente |  |
| `Glyph::UserGuide` | sem equivalente |  |
| `Glyph::WhatsNew` | sem equivalente |  |
| `Glyph::Updates` | sem equivalente |  |
| `Glyph::MessageSettings` | sem equivalente |  |
| `Glyph::Community` | sem equivalente |  |
| `Glyph::Info` | sem equivalente |  |
| `Glyph::SaveAs` | `file-save-as.svg` |  |
| `Glyph::CloseDoc` | `doc-close.svg` |  |
| `Glyph::CloseAllDocs` | sem equivalente |  |
| `Glyph::Revert` | sem equivalente |  |
| `Glyph::PrintPreview` | sem equivalente |  |
| `Glyph::Preflight` | `preflight.svg` |  |
| `Glyph::DocProperties` | sem equivalente | `doc` é o ícone de documento das listas, não as propriedades |
| `Glyph::Exit` | sem equivalente |  |
| `Glyph::Mail` | sem equivalente |  |
| `Glyph::Acquire` | sem equivalente |  |
| `Glyph::FromTemplate` | sem equivalente |  |
| `Glyph::Delete` | `delete.svg` |  |
| `Glyph::Duplicate` | `duplicate.svg` |  |
| `Glyph::Clone` | `instance.svg` | cópia ligada (instância); duvidoso, ver abaixo |
| `Glyph::SelectAll` | `select-all.svg` |  |
| `Glyph::Find` | sem equivalente |  |
| `Glyph::StepRepeat` | sem equivalente | `repeat` é Editar > Repetir |
| `Glyph::CopyProperties` | `copy-props.svg` |  |
| `Glyph::PasteSpecial` | sem equivalente |  |
| `Glyph::Repeat` | `repeat.svg` |  |
| `Glyph::PageInsert` | `page-add.svg` | também o botão Inserir página do navegador de páginas |
| `Glyph::PageDelete` | sem equivalente |  |
| `Glyph::PageRename` | `page-rename.svg` |  |
| `Glyph::PageGoto` | `page-jump.svg` |  |
| `Glyph::PageDuplicate` | sem equivalente |  |
| `Glyph::PageOrientation` | sem equivalente | `page-portrait`/`page-landscape` são as opções, não o comando alternar |
| `Glyph::PageNumber` | sem equivalente |  |
| `Glyph::DocOptions` | sem equivalente |  |
| `Glyph::PageBackground` | sem equivalente |  |
| `Glyph::PageSize` | sem equivalente |  |
| `Glyph::AlignLeft` | `color/align-left.svg` |  |
| `Glyph::AlignRight` | `color/align-right.svg` |  |
| `Glyph::AlignTop` | `color/align-top.svg` |  |
| `Glyph::AlignBottom` | `color/align-bottom.svg` |  |
| `Glyph::AlignCenterH` | `color/align-center-h.svg` | Align::CenterH alinha os centros numa linha horizontal (move em y): a figura com o eixo horizontal, a mesma do docker |
| `Glyph::AlignCenterV` | `color/align-center-v.svg` | Align::CenterV move em x: a figura com o eixo vertical, a mesma do docker |
| `Glyph::CenterPage` | `center-on.svg` | centralizar na página (também o botão do docker Alinhar) |
| `Glyph::DistLeft` | `color/dist-left.svg` |  |
| `Glyph::DistCenterH` | `color/dist-center-h.svg` |  |
| `Glyph::DistSpaceH` | `color/dist-space-h.svg` |  |
| `Glyph::DistRight` | `color/dist-right.svg` |  |
| `Glyph::DistTop` | `color/dist-top.svg` |  |
| `Glyph::DistCenterV` | `color/dist-center-v.svg` |  |
| `Glyph::DistSpaceV` | `color/dist-space-v.svg` |  |
| `Glyph::DistBottom` | `color/dist-bottom.svg` |  |
| `Glyph::Forward` | `order-forward.svg` |  |
| `Glyph::Backward` | `order-backward.svg` |  |
| `Glyph::InFront` | `order-in-front-of.svg` |  |
| `Glyph::Behind` | `order-behind-of.svg` |  |
| `Glyph::ReverseOrder` | sem equivalente |  |
| `Glyph::Hide` | `eye-off.svg` | olho cortado; versão de 16 px nos menus |
| `Glyph::ShowAll` | `show-all.svg` |  |
| `Glyph::Unlock` | `lock-off.svg` | mesmo desenho de `Pic::Lock(false)` |
| `Glyph::UnlockAll` | `unlock-all.svg` |  |
| `Glyph::OutlineToObject` | `outline-to-object.svg` |  |
| `Glyph::JoinCurves` | sem equivalente |  |
| `Glyph::Perspective` | sem equivalente |  |
| `Glyph::ClearTransform` | `clear-transform.svg` |  |
| `Glyph::Properties` | sem equivalente |  |
| `Glyph::ObjectsDocker` | `layers.svg` | pilha de camadas (docker Objetos); duvidoso, ver abaixo |
| `Glyph::ClipInside` | `clip-inside.svg` |  |
| `Glyph::ClipExtract` | `clip-extract.svg` | moldura com seta saindo por cima (menus e barra da moldura de recorte) |
| `Glyph::ClipEdit` | `clip-edit.svg` | lápis sobre a moldura tracejada |
| `Glyph::ClipFinish` | `clip-finish.svg` | seta saindo da moldura pela direita |
| `Glyph::ClipCenter` | `clip-center.svg` | conteúdo no centro, setas opostas |
| `Glyph::ClipFill` | `clip-fill.svg` | setas diagonais para fora |
| `Glyph::ClipFit` | `clip-fit.svg` | setas para dentro |
| `status.click_frame` | `cur-pick-path.svg` | cursor de Colocar dentro do clip frame: seta lateral do conjunto de cursores, ponta no ponto de clique |
| `Glyph::ClipExtract` | sem equivalente |  |
| `Glyph::ClipEdit` | sem equivalente |  |
| `Glyph::ClipFinish` | sem equivalente |  |
| `Glyph::Barcode` | sem equivalente |  |
| `Glyph::QrCode` | sem equivalente |  |
| `Glyph::Symmetry` | `symmetry.svg` |  |
| `Glyph::Symbol` | sem equivalente |  |
| `Glyph::Blend` | `tool-blend.svg` | mesmo desenho da ferramenta Mistura |
| `Glyph::Contour` | `tool-contour.svg` | mesmo desenho da ferramenta Contorno |
| `Glyph::Extrude` | `tool-extrude.svg` | mesmo desenho da ferramenta Extrusão |
| `Glyph::Lens` | sem equivalente |  |
| `Glyph::Bevel` | sem equivalente |  |
| `Glyph::BrushStrokes` | sem equivalente |  |
| `Glyph::Mosaic` | sem equivalente |  |
| `Glyph::Flatten` | sem equivalente |  |
| `Glyph::TableNew` | sem equivalente |  |
| `Glyph::TextToTable` | sem equivalente |  |
| `Glyph::TableToText` | sem equivalente |  |
| `Glyph::RowAbove` | sem equivalente |  |
| `Glyph::RowBelow` | sem equivalente |  |
| `Glyph::ColLeft` | sem equivalente |  |
| `Glyph::ColRight` | sem equivalente |  |
| `Glyph::DelRow` | sem equivalente |  |
| `Glyph::DelCol` | sem equivalente |  |
| `Glyph::DelTable` | sem equivalente |  |
| `Glyph::MergeCells` | sem equivalente |  |
| `Glyph::SplitCells` | sem equivalente |  |
| `Glyph::DistRows` | sem equivalente |  |
| `Glyph::DistCols` | sem equivalente |  |
| `Glyph::ColorManagement` | sem equivalente |  |
| `Glyph::FontManager` | sem equivalente |  |
| `Glyph::Ai` | sem equivalente |  |
| `Glyph::Clock` | sem equivalente |  |
| `Glyph::Script` | sem equivalente |  |
| `Glyph::Record` | sem equivalente |  |
| `Glyph::Grommet` | sem equivalente |  |
| `Glyph::SaveDefaults` | sem equivalente |  |

## Comandos sem figura pintada dos tipos comuns (pela chave do texto: linhas de menu, navegador de páginas, menu de arrastar com o botão direito)

| id na base | SVG do GraphDraw | observações |
|---|---|---|
| `menu.text.glyphs` | `glyphs.svg` | Texto > Glifos; a linha não tinha ícone |
| `menu.text.spell_check` | `spell.svg` | Texto > Verificar ortografia; a linha não tinha ícone |
| `status.first_page` | `page-first.svg` | navegador de páginas, Primeira página |
| `status.previous_page` | `page-prev.svg` | navegador de páginas, Página anterior |
| `status.next_page` | `page-next.svg` | navegador de páginas, Próxima página |
| `status.last_page` | `page-last.svg` | navegador de páginas, Última página |
| `drop.move_here` | `move-here.svg` | menu ao arrastar com o botão direito |
| `drop.copy_here` | `duplicate.svg` | mesmo desenho de Editar > Duplicar |
| `drop.copy_fill_here` | `copy-fill.svg` | menu ao arrastar com o botão direito |
| `drop.copy_outline_here` | `copy-stroke.svg` | menu ao arrastar com o botão direito |
| `drop.copy_all_properties` | `copy-props.svg` | mesmo desenho de Editar > Copiar propriedades |
| `drop.place_inside` | `clip-inside.svg` | mesmo desenho de Objeto > Recortar no quadro |
| `drop.cancel` | `cancel.svg` | menu ao arrastar com o botão direito |

## Janelas Editar preenchimento (F11) e Caneta de contorno (F12)

| id na base | SVG do GraphDraw | observações |
|---|---|---|
| `fillwin.mode_none` | `fill-none.svg` | Editar preenchimento: sem preenchimento |
| `fillwin.mode_solid` | `tool-fill-solid.svg` | Editar preenchimento: uniforme |
| `fillwin.mode_gradient` | `fill-gradient.svg` | Editar preenchimento: gradiente |
| `fillwin.mode_vector` | `fill-vector.svg` | Editar preenchimento: padrão vetorial |
| `fillwin.mode_bitmap` | `fill-bitmap.svg` | Editar preenchimento: padrão de imagem |
| `fillwin.mode_two_color` | `fill-two-color.svg` | Editar preenchimento: padrão de duas cores |
| `fillwin.mode_texture` | `fill-texture.svg` | Editar preenchimento: textura |
| `fillwin.mode_mesh` | `tool-mesh.svg` | mesmo desenho da ferramenta Malha |
| `fillwin.postscript_tip` | `fill-postscript.svg` | botão desabilitado (PostScript não oferecido) |
| `fill.linear` | `fill-linear.svg` | tipo de gradiente |
| `fill.radial` | `fill-radial.svg` | tipo de gradiente |
| `fillwin.elliptic` | `fill-elliptic.svg` | tipo de gradiente (próxima etapa) |
| `fill.conical` | `fill-conic.svg` | tipo de gradiente |
| `fill.square` | `fill-rectangular.svg` | tipo de gradiente |
| `fill.shaped` | `fill-shaped.svg` | tipo de gradiente da base; desenho novo no mesmo traço |
| `fillwin.steps_lock` | `lock-off.svg` | faixas do gradiente (próxima etapa) |
| `fillwin.reflect_tip` | `grad-reflect.svg` | repetir espelhado |
| `fillwin.repeat_tip` | `grad-repeat.svg` | repetir |
| `fillwin.blend_direct` | `blend-direct.svg` | mistura direta |
| `fillwin.blend_cw` | `blend-cw.svg` | mistura pela roda, horário (próxima etapa) |
| `fillwin.blend_ccw` | `blend-ccw.svg` | mistura pela roda, anti-horário (próxima etapa) |
| `fillwin.smooth_tip` | `grad-smooth.svg` | suavizar (próxima etapa) |
| `fillwin.reverse_tip` | `grad-reverse.svg` | inverter os nós |
| `stylewin.eyedropper_tip` | `tool-eyedropper.svg` | conta-gotas das janelas |
| `toolbar.eye_mode_color` | `tool-eyedropper.svg` | modo Cor do conta-gotas (o mesmo da ferramenta) |
| `toolbar.eye_empty` | `cancel.svg` | esvaziar o balde do conta-gotas (Esc) |
| `outlinewin.dash_standard` | `dash-standard.svg` | Caneta de contorno: tracejado nos cantos (próxima etapa) |
| `outlinewin.dash_align` | `dash-align.svg` | idem |
| `outlinewin.dash_fixed` | `dash-fixed.svg` | idem |
| `outlinewin.join_miter_tip` | `join-miter.svg` | Caneta de contorno: cantos |
| `outlinewin.join_round_tip` | `join-round.svg` | Caneta de contorno: cantos |
| `outlinewin.join_bevel_tip` | `join-bevel.svg` | Caneta de contorno: cantos |
| `outlinewin.cap_butt_tip` | `cap-butt.svg` | Caneta de contorno: pontas |
| `outlinewin.cap_round_tip` | `cap-round.svg` | Caneta de contorno: pontas |
| `outlinewin.cap_square_tip` | `cap-square.svg` | Caneta de contorno: pontas |
| `outlinewin.pos_outside_tip` | `pos-outside.svg` | posição do contorno (próxima etapa) |
| `outlinewin.pos_center_tip` | `pos-center.svg` | idem |
| `outlinewin.pos_inside_tip` | `pos-inside.svg` | idem |
| `outlinewin.behind_fill_tip` | `behind-fill.svg` | atrás do preenchimento |
| `outlinewin.scale_tip` | `scale-stroke.svg` | escalar com o objeto |
| `toolbar.path_rotate` | `tp-rotate.svg` | texto em caminho: letras giradas |
| `toolbar.path_upright` | `tp-upright.svg` | letras de pé |
| `toolbar.path_vertical_skew` | `tp-skew-v.svg` | perspectiva pelas verticais |
| `toolbar.path_horizontal_skew` | `tp-skew-h.svg` | perspectiva pelas horizontais |
| `toolbar.path_warp` | `tp-warp.svg` | deformar |
| `toolbar.path_on_baseline` | `tpv-baseline.svg` | traço na linha de base |
| `toolbar.path_on_descender` | `tpv-descender.svg` | traço nas descendentes |
| `toolbar.path_on_xheight` | `tpv-xheight.svg` | traço na altura-x |
| `toolbar.path_on_cap` | `tpv-cap.svg` | traço na altura das maiúsculas |
| `toolbar.path_on_center` | `tpv-center.svg` | traço no centro das letras |
| `toolbar.path_on_ascender` | `tpv-cap.svg` | sem desenho próprio: o mais perto, o topo das maiúsculas |
| `toolbar.path_place_left` | `text-left.svg` | início do caminho |
| `toolbar.path_place_center` | `text-center.svg` | centro do caminho |
| `toolbar.path_place_right` | `text-right.svg` | fim do caminho |
| `toolbar.path_place_justify` | `text-justify.svg` | justificar no caminho |
| `toolbar.path_mirror_h` | `tp-reverse.svg` | espelhar na horizontal (inverte a leitura no lugar) |
| `toolbar.path_mirror_v` | `tp-flip-side.svg` | espelhar na vertical (outro lado do traço); também a seta dupla na tela |
| `toolbar.path_offset` | `tp-offset-x.svg` | deslocar ao longo (ícone arrastável) |
| `toolbar.path_distance` | `tp-offset-y.svg` | distância do traço |
| `toolbar.path_corner_jump` | `tp-corner-jump.svg` | cantos: pular |
| `toolbar.path_corner_wrap` | `tp-corner-wrap.svg` | cantos: dobrar |
| `toolbar.path_corner_split` | `tp-corner-split.svg` | cantos: partes iguais por face |
| `toolbar.path_relax` | `tp-relax.svg` | anticolisão |
| `toolbar.letter_spacing` | `tp-tracking.svg` | espaçamento entre letras (ícone arrastável) |
| `toolbar.path_badge` | `tp-badge.svg` | selo superior e inferior |
| `toolbar.path_construction` | `tp-construction.svg` | traço como construção |
| `toolbar.path_unlink` | `tp-unlink.svg` | desvincular |
| `toolbar.path_straighten` | `tp-straighten.svg` | endireitar (o mesmo de `Pic::StraightenText`) |
| `context.put_text_on_path` | `text-path.svg` | o mesmo de Ajustar texto ao caminho |
| `context.reactivate_text_path` | `text-path.svg` | idem |
| `drop.fit_to_path` | `text-path.svg` | idem, no menu do arrasto direito |
| `status.click_text_path` | `cur-pick-path.svg` | cursor de Ajustar texto ao caminho esperando a guia |

## SVGs sem comando na base

63 dos 283 desenhos de 20 px não têm comando equivalente na base (ou o comando parecido tem outro sentido) e não são embutidos no programa. Ficam em `assets/icons` para quando o comando existir. As versões de 16 px (`assets/icons/16/`) embutidas são as de `eye-off`, `lock-off` e `lock-on`.

`bar-grip`, `bar-more`, `bar-overflow`, `bleed`, `bool-divide`, `bool-exclude`, `cur-grab`, `cur-pan`, `cur-pick-path`, `cur-rotate`, `cur-select`, `cur-select-pick`, `cur-shape`, `cur-zoom`, `cut-line`, `doc`, `export-selection`, `eye-on`, `fill-pattern`, `focus`, `guide-object`, `history`, `image-frame`, `layer-add`, `layer-move-to`, `nest`, `node-align-h`, `node-align-v`, `nudge-big`, `offset`, `page-grid`, `page-list`, `palette`, `panel-collapse`, `panel-expand`, `panel-undock`, `path-open`, `print-off`, `print-on`, `reg-marks`, `report`, `rotate-ccw`, `scale-xy`, `select-invert`, `select-same`, `snap`, `stroke-none`, `styles`, `text-all-caps`, `text-all-small-caps`, `text-bigger`, `text-bold`, `text-cap-spacing`, `text-frame`, `text-italic`, `text-path-detach`, `text-sample`, `text-small-caps`, `text-smaller`, `tool-artboard`, `tp-range`, `wireframe`, `zoom-actual`

## Casos duvidosos

- `Tool::BrushStrokes` -> `tool-artistic`: o desenho é um pincel com traço, que combina com Pinceladas, mas no GraphDraw antigo era a ferramenta de mídia artística.
- `Tool::InteractiveFill` -> `tool-gradient`: o pintado da base é um balde; o SVG mostra o vetor de degradê que a ferramenta edita. `tool-fill-solid` (balde) ficou sem comando.
- `Pic::ExtendToClose` -> `node-connect` (pontas ligadas por segmento tracejado) e `Pic::CloseCurve` -> `path-close`: os dois fecham a curva; conferir se a distinção ficou clara.
- `Glyph::AlignCenterH`/`AlignCenterV`: as figuras coloridas seguem o efeito do comando (`color/align-center-h` tem o eixo horizontal); menu e docker usam a mesma figura.
- `Glyph::ObjectsDocker` -> `layers`: o docker Objetos da base também lista páginas e camadas.
- `Glyph::Clone` -> `instance`: no GraphDraw antigo o desenho era a cópia ligada (instância), o mais perto do Clone da base.
- `Pic::Rotate` -> `rotate-cw` e `Pic::Position` -> `cur-move`: seguem o GraphDraw antigo (Transformar > Girar / Posição); `cur-move` nasceu como desenho de cursor.
- `Action::ShowGrid` -> `grid`: o pintado tinha um olho para indicar mostrar/ocultar.
- Ícones pintados com cores de destaque (vermelho, verde, azul) viram monocromáticos onde há SVG, como pede o conjunto do GraphDraw.
- Fora do mapa ficaram de propósito os ícones de estado do docker Objetos (olho, impressora, cadeado, lixeira das camadas) e os botões dos diálogos Laboratório de cor e Endireitar imagem: são estados ou controles locais, não comandos dos menus ou barras.
- A fonte de sentido para os casos acima foram as tabelas comando -> ícone do GraphDraw antigo (`keymap.rs`, `ctxmenu.rs`), lidas sem alterar nada.
