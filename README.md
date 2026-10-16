# GraphDraw

Editor de ilustração vetorial **open source** para gráficas rápidas, comunicação visual e corte em plotter/laser.

- Abre arquivos `.cdr` legados (via [libcdr](https://wiki.documentfoundation.org/DLP/Libraries/libcdr))
- Formato nativo aberto e documentado: `.gdr` ([descrição](docs/format/gdr-1.0.md), [schema](docs/format/gdr-1.0.schema.json))
- Exporta PDF, SVG e PNG (DXF e PLT/HPGL previstos)

> Status: marco **M0 — visualizador** implementado. Ver [arquitetura](docs/ARCHITECTURE.md).

## Uso

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

```bash
graphdraw-viewer desenho.cdr
```

No visualizador: roda do mouse = zoom no cursor, arrastar = mover, `0` ajusta à página,
`1` tamanho real, `PageUp`/`PageDown` trocam de página, `Ctrl+O` abre.

## Compilação

Resumo (detalhes em [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#3-compilação)):

1. Rust estável, alvo `x86_64-pc-windows-gnu` no Windows.
2. MSYS2 com `libcdr`, `gcc`, `pkgconf` (UCRT64) e `binutils` (MINGW64, no fim do `PATH`).
3. `bash native/cdr-bridge/build.sh` (no shell UCRT64) gera a ponte de importação `.cdr`.
4. `cargo build --release` e `cargo test --workspace`.

## Procedência
Desenvolvido em sala limpa: o código é escrito exclusivamente a partir da especificação funcional em [`docs/spec/`](docs/spec/), sem acesso a código, binários ou ativos de terceiros. Ver [protocolo](docs/spec/00-PROTOCOLO-CLEANROOM.md) e [CONTRIBUTING](CONTRIBUTING.md). Dependências em [THIRD_PARTY.md](THIRD_PARTY.md).

GraphDraw não é afiliado a nenhum fabricante de software. Marcas citadas pertencem aos seus titulares e são usadas apenas para indicar compatibilidade de formatos.

## Roteiro
Ver [docs/spec/MVP.md](docs/spec/MVP.md).
