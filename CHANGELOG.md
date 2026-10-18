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
- Janela Editar preenchimento refeita: barra de modos (nenhum, uniforme,
  gradiente, padrão vetorial, de imagem, de duas cores, textura), sub-barra
  de tipos de gradiente, rampa com nós e pontos médios (duplo clique cria e
  tira, arrastar move, amostra solta vira nó), ângulo em mostrador, centro,
  tamanho, faixas com cadeado, repetir/espelhar, mistura pela roda de
  cores, suavizar e inverter; seletor de cor com roda de matiz, canais,
  hexadecimal, paletas com busca e conta-gotas na janela; amostra original
  × nova; F11 e Shift+F11.
- Efeitos dinâmicos (menu Efeitos): contorno, mistura, sombra projetada,
  sombra de bloco e envelope; os parâmetros ficam na barra, o envelope se
  ajusta pelos 8 pontos na ferramenta Forma e Desagrupar congela o
  resultado (doc 10).
- Texto bidirecional: hebraico e árabe na ordem certa dentro da linha,
  moldados com a direção da direita para a esquerda (GD-TXT-06).
- Verificar ortografia (Texto): palavra desconhecida com o trecho em volta,
  sugestões, Substituir, Substituir todas, Ignorar, Ignorar todas e
  Adicionar ao dicionário, pelo corretor do Windows (GD-TXT-08).
- Desenho inteligente: o traço à mão vira retângulo, elipse, triângulo,
  reta ou seta, com nível de reconhecimento ajustável (GD-FER-27).
- Hifenização no texto de parágrafo, com idioma: dicionário hyphen
  quando instalado, regras do português sem ele (GD-TXT-07).
- Fontes: Arquivo → Incorporar fontes ao salvar leva as fontes inteiras no
  .gdr (se a licença permitir); ao abrir com fonte ausente, uma janela
  oferece trocar pela mais parecida, converter em curvas ou manter
  (GD-FNT-02/03).
- Janela Caneta de contorno refeita em duas colunas: cor com o seletor e
  conta-gotas, espessura com unidade e espessuras prontas, estilo de linha
  com editor de tracejado (estilos do usuário guardados), tracejado
  padrão/alinhado/fixo nos cantos, cantos, pontas, posição, pontas de seta
  com miniaturas e opções (trocar, tamanho, giro, afastamento), bico
  caligráfico interativo, largura variável e prévia; F12.
- Textura procedural como padrão (nuvens, mármore, madeira, granito).
- Booleanas não destrutivas: Soldar, Aparar e Interseção ao vivo; as
  formas continuam editáveis, a operação troca na barra, Converter em
  curvas fecha o resultado e Desagrupar devolve as formas.
- Aninhar na chapa: encaixa as peças pela forma real em chapas de tamanho
  dado, com rotações (nenhuma, 180°, 90°, livre), sentido da fibra, folga
  e margem; abre chapas novas e informa o aproveitamento (GD-PRE-30).
- Gradientes e padrões do documento na aba Paletas: clique aplica;
  toque longo com o direito abre o miniseletor de destino (GD-BDR-12).
- Texto contornando objeto: contorno ou quadrado, dos dois lados, à
  esquerda, à direita ou acima e abaixo, com distância (GD-TXT-10).
- Grades: de documento por espaçamento ou frequência, em linhas ou
  pontos; de linha base; de pixels (96 ppp); e grade imprimível numa
  camada nova (GD-GRD-01).
- Esboço contínuo na mão livre: traços seguidos, perto e a tempo, viram
  uma curva só (GD-FER-29).
- Largura variável do contorno: perfis prontos (afinar no fim, no início,
  nas pontas, bojo) e ferramenta Largura para ajustar ponto a ponto;
  Converter contorno em objeto respeita a largura (GD-VAR-01, GD-OUT-08).
- Simetria ao vivo: espelho de N eixos ou radial de N cópias; o desenho
  novo entra no grupo e editar um filho muda as cópias; Assar transforma
  as cópias em objetos (GD-FER-33).
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
