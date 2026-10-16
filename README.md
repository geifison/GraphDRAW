# GraphDraw

Editor de ilustração vetorial **open source** para gráficas rápidas, comunicação visual e corte em plotter/laser.

- Abre arquivos `.cdr` legados (via [libcdr](https://wiki.documentfoundation.org/DLP/Libraries/libcdr))
- Formato nativo aberto e documentado: `.gdr` ([descrição](docs/format/gdr-1.0.md), [schema](docs/format/gdr-1.0.schema.json))
- Exporta PDF, SVG e PNG (DXF e PLT/HPGL previstos)

> Status: marco **M1 — editor básico** implementado e em validação de uso (M0, visualizador e conversão, concluído). Ver [arquitetura](docs/ARCHITECTURE.md).

## Editor

```bash
graphdraw-app desenho.cdr
```

O editor abre `.gdr`, `.cdr` e `.cmx` (diálogo ou arrastar-soltar) e salva em `.gdr`. Neste marco:

- Ferramentas: Seleção (`V`), Forma/nós (`N`), Zoom (`Z`), Mão (`H`, ou segurar espaço), Bézier (`B`), Retângulo (`R`) e Elipse (`E`).
- Mover, escalar, girar, inclinar e espelhar (clique de novo na seleção mostra as alças de girar/inclinar); setas deslocam 1 mm, com Shift 10 mm.
- Edição de nós: adicionar, excluir, unir, quebrar, fechar/abrir, inverter e tipo de nó.
- Preenchimento uniforme e contorno (espessura, cor, tracejado, junção, terminação, posição centrada/interna/externa). Na paleta, clique define o preenchimento e o botão direito define o contorno.
- Agrupar (`Ctrl+G`) e desagrupar (`Ctrl+U`); `Ctrl+clique` seleciona um objeto dentro do grupo sem desagrupar.
- Barra de páginas em abas, acima da barra de status: renomear com duplo clique ou `F2`, arrastar para reordenar, `Alt`+arrastar para duplicar, `Ctrl`/`Shift`+clique para selecionar várias, botão do meio para excluir (pede confirmação se a página tiver objetos), menu de contexto (configurar tamanho, orientação e sangria, bloquear, etiqueta de cor, exportar), busca por nome, visão geral com miniaturas e aba `[0] Mestra` para o fundo comum a todas as páginas. `Ctrl+J` pula para uma página; `Home`/`End`, `PgUp`/`PgDn` navegam; `Ctrl+Shift+N` cria página.
- Cor de preenchimento e de contorno à direita da barra de status; duplo clique abre a janela de edição.
- Painéis: Transformar, Alinhar, Camadas (visível, imprimível, bloqueada, esboço, ordem, mover seleção), Histórico, Páginas e Relatório de importação.
- Desfazer/refazer (`Ctrl+Z`/`Ctrl+Y`), encaixe em objetos, página e grade, e campos numéricos com contas e unidades (`10+5`, `2in`, `50%`).

A lista completa de atalhos fica em *Ajuda → Atalhos de teclado*.

## Linha de comando

```bash
graphdraw convert desenho.cdr desenho.pdf
```

```bash
graphdraw convert desenho.gdr capa.png --page 1 --dpi 300 --background white
```

```bash
graphdraw convert pasta-de-cdr pasta-de-pdf --format pdf --report relatorio.json
```

```bash
graphdraw info desenho.cdr
```

## Compilação

Resumo (detalhes em [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#3-compilação)):

1. Rust estável, alvo `x86_64-pc-windows-gnu` no Windows.
2. MSYS2 com `libcdr`, `gcc`, `pkgconf` (UCRT64) e `binutils` (MINGW64, no fim do `PATH`).
3. `bash native/cdr-bridge/build.sh` (no shell UCRT64) gera a ponte de importação `.cdr`.
4. `cargo build --release` e `cargo test --workspace`. Para abrir o editor durante o desenvolvimento: `cargo run --release -p graphdraw-app`.

Se o build parar com `error calling dlltool 'dlltool.exe': program not found`, a pasta `msys64/mingw64/bin` não está no `PATH` do terminal. No PowerShell, para a sessão atual:

```powershell
$env:PATH = "$env:PATH;C:\msys64\mingw64\bin"
```

Ajuste o caminho para onde o MSYS2 foi instalado. Para deixar a mudança permanente, inclua a pasta no `PATH` do usuário nas variáveis de ambiente do Windows.

## Procedência
Desenvolvido em sala limpa: o código é escrito exclusivamente a partir da especificação funcional em [`docs/spec/`](docs/spec/), sem acesso a código, binários ou ativos de terceiros. Ver [protocolo](docs/spec/00-PROTOCOLO-CLEANROOM.md) e [CONTRIBUTING](CONTRIBUTING.md). Dependências em [THIRD_PARTY.md](THIRD_PARTY.md).

GraphDraw não é afiliado a nenhum fabricante de software. Marcas citadas pertencem aos seus titulares e são usadas apenas para indicar compatibilidade de formatos.

## Roteiro
Ver [docs/spec/MVP.md](docs/spec/MVP.md).
