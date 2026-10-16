# 14 — Importação `.cdr`

Recurso de **interoperabilidade**. Implementação exclusivamente sobre fontes abertas:
- **libcdr** (MPL-2.0, projeto Document Liberation) — leitura de `.cdr` (versões 1–X da era moderna, conforme suporte da biblioteca) e `.cmx`.
- **librevenge** (MPL-2.0/LGPL) — interface de desenho que a libcdr emite.

Nenhuma informação sobre o formato vem de análise de binários de terceiros; dúvidas sobre o formato são resolvidas pelo código/documentação da libcdr, testes com arquivos de exemplo públicos ou contribuição upstream.

## Requisitos
- **GD-CDR-01** Abrir `.cdr` pelo menu Abrir e por arrastar-soltar; detecção pelo conteúdo, não só extensão.
- **GD-CDR-02** Mapear chamadas da librevenge para o modelo GraphDraw: páginas, camadas, grupos, caminhos, retângulos/elipses (como caminhos se primitiva não for exposta), texto (como texto editável quando possível, senão curvas), imagens, preenchimentos uniforme/gradiente, contornos (espessura, cor, tracejado, junção, terminação), transparência uniforme.
- **GD-CDR-03** O que a biblioteca não suportar vira **aproximação visual** (ex.: efeito convertido em curvas/bitmap) e é listado no **relatório de importação** (painel com avisos por página/objeto).
- **GD-CDR-04** Cores CMYK/spot preservadas no modelo original (doc 05 GD-COR-02).
- **GD-CDR-05** Fontes ausentes: doc 06 GD-FNT-02.
- **GD-CDR-06** Arquivo protegido/corrompido/versão não suportada: mensagem clara com a versão detectada; nunca travar.
- **GD-CDR-07** Importação em lote: pasta de `.cdr` → `.gdr`/PDF/SVG (linha de comando `graphdraw convert`), para migração de acervo de gráficas.
- **GD-CDR-08** Exportar `.cdr`: **fora de escopo** (sem biblioteca aberta de escrita).

## Testes
- Corpus de teste: arquivos `.cdr` próprios ou de licença livre (ex.: corpus de testes da libcdr), por versão.
- Teste de fidelidade: renderizar PNG e comparar com referência (diferença perceptual ≤ limiar) — referências geradas por exportação PDF do arquivo pelo autor do arquivo de teste.
- Contribuir correções de volta para a libcdr upstream.
