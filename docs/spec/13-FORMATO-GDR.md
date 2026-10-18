# 13 — Formato Nativo `.gdr`

Formato **aberto e documentado** (publicado junto do código). Projetado pelo GraphDraw; não deriva de formato de terceiros.

## Contêiner
- **GD-GDR-01** Arquivo ZIP (deflate) com extensão `.gdr`. Primeiro membro: `mimetype` sem compressão contendo `application/vnd.graphdraw.document` (permite detecção por bytes iniciais).
- **GD-GDR-02** Estrutura:
```
mimetype
manifest.json        versão do formato, app gerador, lista de membros + SHA-256
document.json        configurações, páginas, camadas, árvore de objetos
resources/
  colors.json        cores nomeadas/globais, paletas do documento
  styles.json        estilos de objeto e de texto, símbolos (só com conteúdo)
  images/<sha256>.<ext>   bitmaps (PNG/JPEG/TIFF/WebP), deduplicados por hash
  fonts/<sha256>.<ext>    fontes inteiras incorporadas (otf/ttf/ttc, GD-FNT-03)
thumbnails/
  page-1.png         miniatura 512 px da página 1 (para o explorador de arquivos)
metadata.json        título, autor, datas, palavras-chave, notas
```
- **GD-GDR-03** Versão semântica do formato (`formatVersion: "1.0"`). Leitor aceita mesma versão maior; campos desconhecidos são **preservados** ao salvar (round-trip).

## Documento
- **GD-GDR-04** JSON UTF-8. Coordenadas em micrômetros (número). IDs de objeto: UUID v7 em string.
- **GD-GDR-05** Objeto: `{ "id", "type", "name?", "transform": [a,b,c,d,e,f], "fill?", "stroke?", "opacity?", "visible", "locked", "userData?", ...campos do tipo }`.
- **GD-GDR-06** Caminho: `"subpaths": [{ "closed": bool, "nodes": [{ "p":[x,y], "in?":[x,y], "out?":[x,y], "kind":"corner|smooth|symmetric" }] }]`, `"fillRule": "evenodd|nonzero"`.
- **GD-GDR-07** Cor: `{ "model": "rgb|cmyk|gray|lab|spot", "values": [...], "name?", "ref?": "<id da cor global>" }`.
- **GD-GDR-08** Schema JSON publicado em `docs/format/gdr-1.0.schema.json`; CI valida todos os arquivos de teste contra o schema.

## Robustez
- **GD-GDR-09** Salvamento atômico: grava em arquivo temporário no mesmo diretório e renomeia.
- **GD-GDR-10** Leitura tolerante: membro corrompido/ausente → abre o resto, avisa o usuário, nunca trava.
- **GD-GDR-11** Arquivos grandes: `document.json` pode ser dividido em `pages/page-<n>.json` (carregamento sob demanda) — obrigatório acima de 20 MB.
- **GD-GDR-12** Pré-visualização no explorador de arquivos: no Windows, a DLL `graphdraw_thumbs.dll` (gerador de miniaturas do shell, registrada por usuário pelo instalador ou `regsvr32`) mostra no ícone do `.gdr` a miniatura `thumbnails/page-1.png` — a página 1 com o desenho, para achar o arquivo pelo conteúdo. No canto inferior direito vai o ícone do GraphDraw (cerca de 1/4 da largura, de 12 a 96 px; some em miniaturas pequenas demais), desenhado só na exibição — o arquivo não muda. Arquivo sem miniatura ou danificado mostra o ícone comum (a DLL nunca derruba o Explorador).
- Aceite: abrir → salvar → abrir um `.gdr` produz `document.json` semanticamente idêntico (ignorando ordem de chaves).
