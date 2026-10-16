# 05 — Cor

## Modelos de cor
- **GD-COR-01** Modelos suportados: RGB, CMYK, Escala de cinza, Lab, HSB/HSV, HLS, Registro (100% em todas as chapas), Cor especial (spot) com tinta nomeada e cor de prévia alternativa, Multitinta (DeviceN) opcional.
- **GD-COR-02** Cada cor guarda **o modelo original e os valores originais**; conversões são só para exibição/saída. Editar CMYK nunca grava RGB convertido.
- **GD-COR-03** Precisão: componentes em float 0–1 internamente; UI exibe 0–255 (RGB) e 0–100 (CMYK, %).

## Paletas
- **GD-PAL-01** Paleta do documento (cores usadas/nomeadas, salva no `.gdr`).
- **GD-PAL-02** Paletas de usuário importáveis/exportáveis: formato próprio (JSON), ASE (Adobe Swatch Exchange), GPL (GIMP), CSV; importação de `.xml`/`.cpl` de terceiros quando o formato for documentado.
- **GD-PAL-03** Cores globais (vínculo): alterar a definição na paleta atualiza todos os objetos que a usam.
- **GD-PAL-04** Bibliotecas de cores especiais de fabricantes **não são distribuídas** (licença de terceiros); usuário importa as suas.
- **GD-PAL-05** Barra de paleta na UI: clique esquerdo = preenchimento, direito = contorno; arrastar para objeto; modificador para mesclar com cor atual (tint).

## Gerenciamento de cor (ICC)
- **GD-CMS-01** Motor: LittleCMS 2. Perfis padrão: sRGB IEC61966-2.1, perfil CMYK FOGRA39/GRACoL-equivalente aberto (ex.: perfis livres ECI quando licença permitir) — validar licenças.
- **GD-CMS-02** Perfis por documento: RGB, CMYK, Cinza. Intenção de renderização: perceptual, colorimétrica relativa (padrão), saturação, colorimétrica absoluta; compensação de ponto preto.
- **GD-CMS-03** Prova de cor na tela (soft-proof) com perfil de saída escolhido e aviso de fora de gama.
- **GD-CMS-04** Política ao abrir/importar: manter perfil incorporado, converter para o do documento, ou ignorar — configurável e perguntado quando divergente.

## Seletor de cor
- **GD-SEL-COR-01** Modos: visual (quadrado/roda), valores numéricos por modelo, hexadecimal, conta-gotas na tela (inclusive fora da janela), harmonias (complementar, triádica…), histórico recente.

## Separações (uso no doc 16)
- **GD-SEP-01** Prévia de separações por chapa (C, M, Y, K, cada spot), sobreimpressão simulada.
- **GD-SEP-02** Preto rico vs. preto 100% K: opção de sobreimpressão automática de preto em texto pequeno/linhas finas.
