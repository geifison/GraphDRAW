# Mudanças

Formato: uma seção por versão; dentro dela, o que entrou por área. Os IDs
entre parênteses são os requisitos de `docs/spec/`.

## [Não lançada] — caminho para a 1.0

### Produção, gráfica e corte
- Verificação prévia (painel Verificação, doc 28):
  - cobertura de tinta, linha fina e linha clara fina;
  - texto fora da zona de segurança;
  - objeto encostado no corte sem sangria;
  - preto 100% K sem sobreimpressão.

  Cada problema traz o botão Corrigir.
- Sobreimpressão de preenchimento e contorno no PDF (`/op`, `/OP`, `OPM 1`).
- Estender até a sangria (GD-PRE-10).
- Compensação de kerf para laser/CNC na saída de corte (GD-PRE-20).
- Prévia de separações por chapa e composta, com sobreimpressão simulada
  (GD-SEP-01).
- Perfis de cor de trabalho e política ao abrir arquivo com outros perfis
  (GD-CMS-04).

### Ferramentas
- Corte: faca, recortar, borracha, excluir segmento virtual
  (GD-FER-16..19).
- Linhas: mão livre com suavização, polilinha, linha 2 pontos
  (livre/tangente/perpendicular), curva 3 pontos (GD-FER-20/21/25/26).
- Transformação livre: girar, refletir, escalar e inclinar em torno do pivô
  (GD-FER-03).
- Cotas:
  - horizontal, vertical, paralela, angular, raio, diâmetro e segmento;
  - ligadas aos nós medidos, com a escala de desenho (GD-FER-40,
    GD-COT-01..03).
- B-spline: pontos de controle fixados ou flutuantes, aberta ou fechada;
  editar arrastando os pontos na ferramenta Formato (GD-FER-24,
  GD-BSP-01).
- Conectores reto, em ângulo reto (cantos arredondados) e curvo, presos
  aos encaixes dos objetos e refeitos no mesmo passo ao movê-los
  (GD-FER-41, GD-CON-01).
- Conta-gotas:
  - amostrar e despejar, com lupa e média de área;
  - atributos, rampas de gradiente e histórico (doc 27).
- Botão direito:
  - menus por alvo, com ícones;
  - arrastar com o direito para mover, copiar ou combinar;
  - objeto bloqueado alcançado só pelo direito (doc 26).

### Preenchimento
- Padrões vetorial, de imagem e de duas cores:
  - deslocamento de linha/coluna e espelhamento;
  - "transformar com o objeto" (GD-FIL-04..06, GD-FIL-10).
- Opção do documento "Preencher caminhos abertos" (GD-FIL-12).

### Segurança do trabalho
- Salvamento automático a cada 2 min e recuperação depois de queda.
- Registro de falhas.
- Ao salvar por cima, o arquivo anterior fica como `.gdr.bak`.

### Distribuição
- Ícones do programa e do `.gdr` feitos a partir da logo do projeto.
- Instalador Windows (NSIS, só para o usuário, associa `.gdr`), gerado no CI
  por tag.
- Janela Sobre: versão, licença, componentes de terceiros e pastas de
  suporte.

## 0.1.0 — marcos M0 a M3
- **M0 — visualizador**: abre `.cdr` (libcdr) e `.gdr`, exporta SVG, PDF e
  PNG.
- **M1 — editor básico**:
  - seleção, transformações, retângulo, elipse, Bézier, nós;
  - preenchimento e contorno, desfazer, salvar `.gdr`.
- **M2 — produção**:
  - texto artístico e de parágrafo, booleanas, deslocamento;
  - linha de corte, PLT/DXF, PDF/X-1a com LittleCMS.
- **M3 — polimento**:
  - paletas, guias e réguas, gradientes, alinhar e distribuir;
  - polígonos e estrelas, camadas, páginas em abas;
  - texto em caminho, modos de mesclagem, pontas de seta, caneta
    caligráfica.
