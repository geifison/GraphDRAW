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
- **GD-PAL-06** Paletas de produção prontas: **CMYK de processo** (tintas, secundárias e seus tons, cinzas em K, preto rico e cores comuns, todas definidas em CMYK e com no máximo 300% de tinta total, então os valores vão direto às chapas) e **RGB seguro para impressão** (matizes em vários tons já levados para dentro da gama do FOGRA39, ida e volta com ΔE2000 ≤ 1, sem quase-repetidas). Aparecem no painel Paletas (com "Pôr no documento") e na barra de paleta.

- **GD-PAL-07** Tons derivados: clicar e segurar (0,5 s) numa amostra (barra ou painel) abre um mosaico 6×6 de tons da cor (blocos colados, sem linhas divisórias; só o tom sob o mouse ganha contorno), do mais claro (canto superior esquerdo) ao mais escuro (canto inferior direito), com a própria cor no meio; CMYK fica em CMYK e escurece com K sem passar de 300% de tinta; outros modelos viram RGB. Cor só de preto (K puro, cinza, registro, RGB neutro) e cor especial mostram só 6 tons, da cor como está até zero dela. Escolher um tom: clicar nele (esquerdo = preenchimento, direito = contorno) ou, ainda segurando, deslizar até ele e soltar; escolher fecha o mosaico (também clique fora ou Esc). O mosaico abre abaixo da amostra, ou acima quando ela está na metade de baixo da tela, sem cobri-la; soltar sobre a amostra original não aplica cor.

## Gerenciamento de cor (ICC)
- **GD-CMS-01** Motor: LittleCMS 2. Perfis padrão: sRGB IEC61966-2.1 (embutido no motor) e perfis CMYK embutidos `builtin:fogra39` (FOGRA39L_coated, OpenICC, zlib — offset couché ISO 12647-2, padrão dos documentos novos) e `builtin:gracol2013` (GRACoL2013_CRPC6, ICC/IDEAlliance). Licenças verificadas em THIRD_PARTY.md; perfis ECI ficam de fora (redistribuição exige permissão). No `.gdr`, o perfil embutido é gravado pelo identificador e abre igual em qualquer computador; documentos antigos sem perfil continuam com a fórmula. Ao exportar PDF/X, a condição de impressão acompanha o perfil embutido do documento.
- **GD-CMS-02** Perfis por documento: RGB, CMYK, Cinza. Intenção de renderização: perceptual, colorimétrica relativa (padrão), saturação, colorimétrica absoluta; compensação de ponto preto.
- **GD-CMS-03** Prova de cor na tela (soft-proof) com perfil de saída escolhido e aviso de fora de gama.
- **GD-CMS-04** Política ao abrir/importar: manter perfil incorporado, converter para o do documento, ou ignorar — configurável e perguntado quando divergente.

## Seletor de cor
- **GD-SEL-COR-01** Modos: visual (quadrado/roda), valores numéricos por modelo, hexadecimal, conta-gotas na tela (inclusive fora da janela), harmonias (complementar, triádica…), histórico recente.

## Separações (uso no doc 16)
- **GD-SEP-01** Prévia de separações por chapa (C, M, Y, K, cada spot), sobreimpressão simulada.
- **GD-SEP-02** Preto rico vs. preto 100% K: opção de sobreimpressão automática de preto em texto pequeno/linhas finas.
