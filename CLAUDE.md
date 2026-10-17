# GraphDraw — instruções para sessões de desenvolvimento (Time B / sala limpa)

GraphDraw é um editor de ilustração vetorial open source para gráficas, comunicação visual e corte (plotter/laser). Abre `.cdr` (via libcdr), formato nativo `.gdr`.

## Regras de sala limpa (obrigatórias)
1. A **única** fonte de requisitos é `docs/spec/`. Comece por `docs/spec/MVP.md`.
2. **Nunca** ler, listar ou abrir `../timeA-evidencias/` nem nada fora deste repositório, exceto `../spec/` quando o usuário pedir para sincronizar a spec.
3. **Nunca** abrir, inspecionar, descompilar ou executar binários de softwares comerciais de terceiros, nem buscar informações sobre suas implementações internas.
4. Não usar nomes, ícones, textos ou layout de produtos de terceiros. Identidade visual e textos são próprios.
5. Dúvida sobre comportamento → perguntar ao usuário ou registrar em `docs/spec/PERGUNTAS.md`; não deduzir a partir de produtos de terceiros.
6. Dúvida sobre o formato `.cdr` → somente código/documentação da libcdr (open source) e arquivos de teste com licença livre.
7. Só dependências com licença compatível com a licença do projeto; registrar cada uma em `THIRD_PARTY.md`.

## Design (obrigatório)
- **`docs/design/DESIGN-SYSTEM.md` é a fonte única de aparência e interação.** Ler antes de criar ou alterar qualquer tela, painel, janela, ícone, cursor ou controle, e seguir os tokens dele (cor, espaçamento, tipografia, raios, tamanhos de botão).
- Valores visuais só no módulo de tema (`crates/graphdraw-app/src/theme.rs`), com os nomes dos tokens; nunca cor ou tamanho escrito direto em outro arquivo.
- Se o caso não estiver coberto, atualizar o documento de design antes do código, no mesmo commit.
- Ao corrigir um item da seção 8 (conformidade) do documento, atualizar a tabela no mesmo commit.
- Antes de concluir, passar pelo checklist da seção 9.

## Rastreabilidade
- Cada commit/PR cita os IDs de requisito implementados (ex.: `GD-GEO-01`).
- Cada requisito implementado tem teste com o critério de aceitação da spec.

## Ordem de trabalho
Marcos em `docs/spec/MVP.md`: M0 (visualizador .cdr/.gdr + exportação SVG/PDF/PNG) → M1 → M2 → M3.
