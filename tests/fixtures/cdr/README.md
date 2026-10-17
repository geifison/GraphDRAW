# Amostras `.cdr` / `.cmx`

Arquivos de licença livre usados nos testes de importação (doc 14). Nenhum
foi criado ou alterado pelo projeto.

| Arquivo | Origem | Licença | O que exercita |
|---|---|---|---|
| `fdo65220-2.cdr` | LibreOffice, `writerperfect/qa/unit/data/draw/libcdr/pass/` | MPL-2.0 | `.cdr` em contêiner ZIP (versão 15 do formato), retângulos, contorno, transparência uniforme |
| `fdo63782-1.cmx` | LibreOffice, `writerperfect/qa/unit/data/draw/libcdr-cmx/pass/` | MPL-2.0 | `.cmx`, 116 caminhos com Bézier, contornos largos |
| `EDB-35217-2.cdr` | LibreOffice, `writerperfect/qa/unit/data/draw/libcdr/pass/` | MPL-2.0 | Arquivo não suportado/corrompido (GD-CDR-06) |

Baixados em 2026-10-16 de
`https://github.com/LibreOffice/core/tree/master/writerperfect/qa/unit/data/draw`.

`expected/fdo63782-1.png` e `expected/fdo65220-2.png` (esta de 2026-10-17)
são referências de renderização (72 ppp, fundo branco) aprovadas por
inspeção visual — servem de regressão, não de alvo de fidelidade; o teste de fidelidade compara a
renderização atual com ela. A spec pede referências exportadas pelo autor
do arquivo — não disponíveis para estas amostras (ver
`docs/spec/PERGUNTAS.md`, P-02).
