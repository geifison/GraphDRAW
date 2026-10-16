# 17 — Saída para Corte (plotter de recorte, laser, CNC)

Diferencial do produto para comunicação visual.

## Preparação
- **GD-CRT-01** Camada/atributo "linha de corte": objetos marcados (ou cor especial nomeada configurável, ex. `CutContour`) são enviados ao corte; o resto só imprime.
- **GD-CRT-02** Gerar linha de corte ao redor da seleção: offset externo d mm (doc 09 GD-OFS-01) sobre a silhueta (união dos objetos), cantos arredondados opcionais, remoção de furos internos opcional.
- **GD-CRT-03** Marcas de registro para impressão-e-corte (pontos/cantos configuráveis) fora da área útil.
- **GD-CRT-04** Ordem de corte: interno antes de externo (furos primeiro), minimizar deslocamento ocioso (heurística vizinho mais próximo), direção (horário/anti-horário).
- **GD-CRT-05** Opções de vinil: linhas de "weed" (descasque) — caixa ao redor + linhas de divisão; sobreposição (overcut) mm; compensação de lâmina (offset de faca) mm.
- **GD-CRT-06** Prévia de corte animada (percurso em ordem).

## Saída
- **GD-CRT-10** HPGL/PLT: unidades por mm configurável (padrão 40 u/mm), PU/PD/PA, velocidade/força (VS/FS) opcionais, origem e espelhamento configuráveis.
- **GD-CRT-11** DXF (R12) com só linhas/arcos/polilinhas — para laser/CNC.
- **GD-CRT-12** SVG/PDF com camada de corte separada e cor spot nomeada — para RIPs de impressão-e-corte.
- **GD-CRT-13** Curvas convertidas em segmentos com tolerância (padrão 0,02 mm) quando o destino não suporta Bézier.
- **GD-CRT-14** Envio direto para porta serial/USB/rede: pós-MVP (drivers por fabricante via plugins).

## Aceite
- Retângulo 100×50 mm, exportar PLT 40 u/mm → coordenadas finais 4000×2000; texto convertido; furo da letra "O" cortado antes do contorno externo.
