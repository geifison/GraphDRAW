# 12 — Desfazer/Refazer e Transações

- **GD-UND-01** Toda alteração do documento acontece dentro de uma **transação** com nome legível ("Mover 3 objetos"). Uma ação do usuário = uma transação (inclusive arrastos longos: um único passo ao soltar).
- **GD-UND-02** Pilha de desfazer por documento; nível configurável (padrão 100; 0 = ilimitado limitado por memória).
- **GD-UND-03** Refazer descartado ao executar nova ação após desfazer.
- **GD-UND-04** Painel de histórico: lista de transações, clicar volta a qualquer ponto.
- **GD-UND-05** Transações aninhadas: comandos compostos (ex.: script) agrupam subtransações em um único passo.
- **GD-UND-06** Seleção não entra no histórico; zoom/rolagem não entram.
- **GD-UND-07** Documento marcado "modificado" se a posição na pilha difere da do último salvamento.
- **GD-UND-08** Implementação recomendada: comandos reversíveis ou snapshots de estrutura imutável (persistent data structures) — escolha do Time B.
- Aceite: criar 3 objetos, mover 1, desfazer 2× → só os 3 objetos na posição original de criação, documento "modificado"; refazer 1× → objeto movido de novo.
