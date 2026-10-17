# 27 — Conta-gotas e transferência de atributos

Detalha a ferramenta Conta-gotas do grupo Preenchimento (doc 07). Ela tem
duas camadas: amostra exata de cor (pixel a pixel, média de área ou valor
nativo do vetor) e cópia de propriedades vetoriais de um objeto para outros.

Fase por item: **MVP** ou **Beta** (depende de requisito fora do MVP). Item
Beta aparece desabilitado na interface, com o motivo na dica (GD-BDR-04).

## 1. Modos — MVP
- **GD-CTG-01** Dois modos na barra de propriedades:
  - **Cor**: pega só o valor da cor (RGB, CMYK, HEX ou Lab, no modelo da
    cor). Não copia espessura, fonte nem estrutura.
  - **Atributos**: pega o conjunto de parâmetros do objeto (ver
    GD-CTG-08).
  - Trocar de modo esvazia o balde.

## 2. Amostragem e lupa
- **GD-CTG-02** Lupa flutuante (HUD) junto ao cursor. **MVP**
  - Grade ampliada de 8× a 16× com os pixels visíveis e retículo no ponto
    de coleta.
  - Pílula de dados: HEX, CMYK, RGB e o nome da cor na paleta do
    documento, quando é a mesma cor.
- **GD-CTG-03** Raio de coleta. **MVP**
  - **Ponto 1×1**: o pixel sob a ponta.
  - **Média 3×3** ou **5×5**: média da área (fotos, ruído, texturas).
  - **Vetor nativo** (padrão): o valor do modelo do objeto, sem desvio de
    anti-serrilhado. Imagens e áreas vazias caem no pixel da tela.
- **GD-CTG-04** Amostrar fora da janela: com o botão pressionado, o
  ponteiro sai do programa e pega a cor de outra janela ou monitor.
  **Beta** (precisa de captura de tela do sistema; ver P-12).

## 3. Ciclo "amostrar e despejar" — MVP
- **GD-CTG-05** Sem trocar de ferramenta:
  1. **Pipeta**: o clique coleta da origem.
  2. Logo depois, o cursor vira **balde** e o balde mostra o que leva.
  3. **Balde**: o clique aplica nos objetos de destino.
  4. **Shift + clique** coleta de novo sem aplicar, e o balde continua
     ativo (bom para encher a paleta).
  5. **Alt + clique** troca pipeta ↔ balde sem coletar nem aplicar.
  6. **Esc** esvazia o balde e volta à pipeta.

## 4. Botões e modificadores ao despejar — MVP
- **GD-CTG-06** Botões:
  - **Esquerdo** em objeto fechado: a cor vai para o **preenchimento**.
    Em linha aberta, vai para o traço. Em texto, vai para a caixa
    inteira.
  - **Direito** em qualquer vetor: a cor vai para o **contorno**; o
    preenchimento fica igual. Com o conta-gotas, o direito não abre menu
    (exceção a GD-BDR-03).
  - Quem não tinha contorno recebe o contorno padrão dos novos objetos,
    com a cor coletada.
- **GD-CTG-07** **Ctrl + clique** atinge o objeto de dentro de um grupo. Sem
  Ctrl, o grupo inteiro recebe.

## 5. Filtros de atributos
- **GD-CTG-08** No modo Atributos, uma barra marca as famílias que passam:

| Família | Atributos | Fase |
|---|---|---|
| Preenchimento | sólido, gradiente (reposicionado na caixa do destino), transparência do preenchimento | MVP |
| Contorno | espessura, cor, tracejado, cantos e pontas, setas, caneta caligráfica | MVP |
| Efeitos visuais | sombra, brilho, contorno externo, extrusão, chanfro | Beta (doc 10) |
| Tipografia | família, peso, itálico, corpo, entrelinha | MVP |
| Transformações | escala X/Y, giro e inclinação; o centro do destino fica parado | MVP |

  - Ajuste entre letras (kerning/tracking) entra quando o modelo de texto
    tiver esse campo.

## 6. Gradientes — MVP
- **GD-CTG-09**
  - **Clique simples**: pega a cor do ponto exato da rampa.
  - **Alt + clique** no gradiente: clona a rampa inteira (paradas,
    posições e ângulo), reposicionada na caixa de cada destino.
  - Com o balde de cor sobre um objeto com gradiente, as paradas aparecem.
    O clique numa parada troca só a cor dela.

## 7. Atalhos e microinterações — MVP
- **GD-CTG-10** Botão direito na lupa: a cor vai para a paleta do
  documento.
  - Para alcançar a lupa: parado por 0,4 s, a lupa para e espera o cursor.
    Ela volta a seguir quando o cursor se afasta.
- **GD-CTG-11** Os últimos 5 tons coletados aparecem como círculos sob a
  lupa, sem repetir. O clique num deles carrega o balde.
- **GD-CTG-12** Ao despejar, o destino ganha um contorno de destaque de
  150 ms que some aos poucos.

## Aceite
- Pipeta no quadrado vermelho, clique no azul: o azul fica vermelho e é 1
  passo de desfazer. Direito no verde: o contorno fica vermelho e o
  preenchimento continua verde.
- Shift + clique no verde: nada muda no documento e o balde fica verde.
- Alt + clique duas vezes: pipeta, depois balde, sem passo de desfazer.
- Esc: o balde fica vazio.
- Alt + clique num gradiente linear de 100 mm e clique num retângulo de
  400 × 200 mm: o eixo vai de borda a borda do retângulo.
