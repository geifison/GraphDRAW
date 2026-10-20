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
| `Action::Pdf` | sem equivalente | não há desenho de PDF |
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
| `Pic::ToFront` | `order-front.svg` |  |
| `Pic::ToBack` | `order-back.svg` |  |
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
| `Pic::Combine` | `combine.svg` |  |
| `Pic::BreakApart` | `break-apart.svg` |  |
| `Pic::Weld` | `bool-weld.svg` |  |
| `Pic::Trim` | `bool-trim.svg` |  |
| `Pic::Intersect` | `bool-intersect.svg` |  |
| `Pic::Simplify` | `bool-simplify.svg` |  |
| `Pic::FrontMinusBack` | `bool-front-minus.svg` |  |
| `Pic::BackMinusFront` | `bool-back-minus.svg` |  |
| `Pic::Boundary` | `bool-boundary.svg` |  |
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
| `Glyph::AlignLeft` | `align-left.svg` |  |
| `Glyph::AlignRight` | `align-right.svg` |  |
| `Glyph::AlignTop` | `align-top.svg` |  |
| `Glyph::AlignBottom` | `align-bottom.svg` |  |
| `Glyph::AlignCenterH` | `align-center-v.svg` | Align::CenterH alinha os centros numa linha horizontal (move em y): o desenho com o eixo horizontal é `align-center-v` |
| `Glyph::AlignCenterV` | `align-center-h.svg` | Align::CenterV move em x: o desenho com o eixo vertical é `align-center-h` |
| `Glyph::CenterPage` | `center-on.svg` | centralizar na página (também o botão do docker Alinhar) |
| `Glyph::DistLeft` | `dist-left.svg` |  |
| `Glyph::DistCenterH` | `dist-center-h.svg` |  |
| `Glyph::DistSpaceH` | `dist-space-h.svg` |  |
| `Glyph::DistRight` | `dist-right.svg` |  |
| `Glyph::DistTop` | `dist-top.svg` |  |
| `Glyph::DistCenterV` | `dist-center-v.svg` |  |
| `Glyph::DistSpaceV` | `dist-space-v.svg` |  |
| `Glyph::DistBottom` | `dist-bottom.svg` |  |
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
| `status.click_frame` | `cur-pick-path.svg` | cursor de Colocar dentro do quadro: seta lateral do conjunto de cursores, ponta no ponto de clique |
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

## SVGs sem comando na base

119 dos 282 desenhos de 20 px não têm comando equivalente na base (ou o comando parecido tem outro sentido) e não são embutidos no programa. Ficam em `assets/icons` para quando o comando existir. As versões de 16 px (`assets/icons/16/`) embutidas são as de `eye-off`, `lock-off` e `lock-on`.

`bar-grip`, `bar-more`, `bar-overflow`, `behind-fill`, `bleed`, `blend-ccw`, `blend-cw`, `blend-direct`, `bool-divide`, `bool-exclude`, `cap-butt`, `cap-round`, `cap-square`, `cur-grab`, `cur-pan`, `cur-pick-path`, `cur-rotate`, `cur-select`, `cur-select-pick`, `cur-shape`, `cur-zoom`, `cut-line`, `dash-align`, `dash-fixed`, `dash-standard`, `doc`, `export-selection`, `eye-on`, `fill-bitmap`, `fill-conic`, `fill-elliptic`, `fill-gradient`, `fill-linear`, `fill-none`, `fill-pattern`, `fill-postscript`, `fill-radial`, `fill-rectangular`, `fill-texture`, `fill-two-color`, `fill-vector`, `focus`, `grad-reflect`, `grad-repeat`, `grad-reverse`, `grad-smooth`, `guide-object`, `history`, `image-frame`, `join-bevel`, `join-miter`, `join-round`, `layer-add`, `layer-move-to`, `nest`, `node-align-h`, `node-align-v`, `nudge-big`, `offset`, `page-grid`, `page-list`, `palette`, `panel-collapse`, `panel-expand`, `panel-undock`, `path-open`, `pos-center`, `pos-inside`, `pos-outside`, `print-off`, `print-on`, `reg-marks`, `report`, `rotate-ccw`, `scale-stroke`, `scale-xy`, `select-invert`, `select-same`, `snap`, `stroke-none`, `styles`, `text-all-caps`, `text-all-small-caps`, `text-bigger`, `text-bold`, `text-cap-spacing`, `text-frame`, `text-italic`, `text-path-detach`, `text-sample`, `text-small-caps`, `text-smaller`, `tool-artboard`, `tool-fill-solid`, `tp-badge`, `tp-construction`, `tp-corner-jump`, `tp-corner-split`, `tp-corner-wrap`, `tp-flip-side`, `tp-offset-x`, `tp-offset-y`, `tp-range`, `tp-relax`, `tp-reverse`, `tp-rotate`, `tp-skew-h`, `tp-skew-v`, `tp-tracking`, `tp-unlink`, `tp-upright`, `tp-warp`, `tpv-baseline`, `tpv-cap`, `tpv-center`, `tpv-descender`, `tpv-xheight`, `wireframe`, `zoom-actual`

## Casos duvidosos

- `Tool::BrushStrokes` -> `tool-artistic`: o desenho é um pincel com traço, que combina com Pinceladas, mas no GraphDraw antigo era a ferramenta de mídia artística.
- `Tool::InteractiveFill` -> `tool-gradient`: o pintado da base é um balde; o SVG mostra o vetor de degradê que a ferramenta edita. `tool-fill-solid` (balde) ficou sem comando.
- `Pic::ExtendToClose` -> `node-connect` (pontas ligadas por segmento tracejado) e `Pic::CloseCurve` -> `path-close`: os dois fecham a curva; conferir se a distinção ficou clara.
- `Glyph::AlignCenterH`/`AlignCenterV`: mapeados cruzados de propósito pelo efeito do comando (ver notas). A figura pintada `Glyph::AlignCenterH` da base desenha o eixo vertical, ao contrário do docker Alinhar e do efeito; com o SVG o menu passa a concordar com o docker.
- `Glyph::ObjectsDocker` -> `layers`: o docker Objetos da base também lista páginas e camadas.
- `Glyph::Clone` -> `instance`: no GraphDraw antigo o desenho era a cópia ligada (instância), o mais perto do Clone da base.
- `Pic::Rotate` -> `rotate-cw` e `Pic::Position` -> `cur-move`: seguem o GraphDraw antigo (Transformar > Girar / Posição); `cur-move` nasceu como desenho de cursor.
- `Action::ShowGrid` -> `grid`: o pintado tinha um olho para indicar mostrar/ocultar.
- Ícones pintados com cores de destaque (vermelho, verde, azul) viram monocromáticos onde há SVG, como pede o conjunto do GraphDraw.
- Fora do mapa ficaram de propósito os ícones de estado do docker Objetos (olho, impressora, cadeado, lixeira das camadas) e os botões dos diálogos Laboratório de cor e Endireitar imagem: são estados ou controles locais, não comandos dos menus ou barras.
- A fonte de sentido para os casos acima foram as tabelas comando -> ícone do GraphDraw antigo (`keymap.rs`, `ctxmenu.rs`), lidas sem alterar nada.
