# Protocolo Clean Room — GraphDraw

Versão 1 · 2026-10-16

## Objetivo
Garantir que o GraphDraw seja obra original, sem código, texto, ícones ou outros ativos protegidos de terceiros, mesmo tendo estudado o comportamento de softwares existentes.

## Papéis
| Papel | Pode ver | Produz | Não pode |
|---|---|---|---|
| **Time A (sujo)** | Programas de terceiros instalados, binários, descompilação, documentação pública | Evidências (`timeA-evidencias/`) e especificação funcional (`spec/`) | Escrever código do produto |
| **Revisor** | Evidências + especificação | Aprovação da spec | Repassar evidências ao Time B |
| **Time B (limpo)** | Somente `spec/` aprovada e fontes abertas de terceiros com licença compatível | Código em `graphdraw/` | Ver `timeA-evidencias/`, binários ou descompilação de terceiros |

## Regras para IA
1. Sessões do Time A e do Time B são **sessões separadas**, com diretórios de trabalho diferentes. A sessão do Time B abre só `graphdraw/` e `spec/`.
2. Nenhuma sessão que tenha lido `timeA-evidencias/` escreve código em `graphdraw/`.
3. Logs das sessões são guardados como prova de procedência.
4. A spec descreve **o quê** (comportamento, entradas, saídas, limites), nunca **como** o original implementa (sem endereços, nomes internos, pseudocódigo descompilado, estruturas de memória).

## Regras para a especificação
- Linguagem própria. Não copiar frases da ajuda/documentação de terceiros.
- Proibido incluir: nomes de classes/funções internas, endereços, trechos de assembly/pseudocódigo, strings copiadas, ícones/capturas de tela.
- Permitido: comportamento observável, formatos de arquivo públicos ou já documentados por projetos abertos (ex.: libcdr), padrões abertos (PDF, SVG, ICC, OpenType).
- Cada item da spec tem ID (`GD-<área>-<nº>`) e critério de aceitação testável.

## Marca e identidade
- Nome do produto: **GraphDraw**. Formato nativo: **.gdr**.
- Não usar nomes, logotipos, ícones, esquemas de cores ou layout idêntico de terceiros.
- Importação de `.cdr` é recurso de interoperabilidade, implementado sobre biblioteca aberta (libcdr, MPL-2.0) ou spec própria derivada de fontes abertas.

## Pendências jurídicas
- [ ] Parecer de advogado de PI sobre a análise de binários (EULA × Lei 9.609/98)
- [ ] Busca de marca "GraphDraw" no INPI (classe 9)
- [ ] Verificar colisão da extensão `.gdr`
