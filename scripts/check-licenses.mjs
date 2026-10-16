#!/usr/bin/env node
// Verifica se todas as dependências Rust (normais e de build, transitivas)
// têm licença compatível com MPL-2.0. Uso: node scripts/check-licenses.mjs
import { execFileSync } from "node:child_process";

// Expressões SPDX aceitas: cada crate precisa ter ao menos uma licença desta lista
// em alguma alternativa "OR" (e todas as partes de um "AND").
const ALLOWED = new Set([
  "MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "Zlib", "0BSD", "Unlicense",
  "CC0-1.0", "BSL-1.0", "ISC", "MPL-2.0", "Unicode-3.0", "Unicode-DFS-2016", "OFL-1.1",
  "Apache-2.0 WITH LLVM-exception",
]);

const meta = JSON.parse(execFileSync("cargo", ["metadata", "--format-version", "1"], { maxBuffer: 1 << 28 }).toString());
const ws = new Set(meta.workspace_members);
const nodes = new Map(meta.resolve.nodes.map((n) => [n.id, n]));
const pkgs = new Map(meta.packages.map((p) => [p.id, p]));

const seen = new Set();
const stack = [...ws];
while (stack.length) {
  const id = stack.pop();
  if (seen.has(id)) continue;
  seen.add(id);
  for (const d of nodes.get(id).deps) {
    if (d.dep_kinds.some((k) => k.kind === null || k.kind === "build")) stack.push(d.pkg);
  }
}

function ok(expr) {
  const norm = expr.replace(/\//g, " OR ").replace(/[()]/g, " ");
  return norm.split(/\s+OR\s+/).some((alt) =>
    alt.split(/\s+AND\s+/).every((l) => ALLOWED.has(l.trim())));
}

const bad = [];
for (const id of seen) {
  if (ws.has(id)) continue;
  const p = pkgs.get(id);
  if (!p.license || !ok(p.license)) bad.push(`${p.name} ${p.version}: ${p.license ?? "(sem licença declarada)"}`);
}
if (bad.length) {
  console.error("Dependências com licença não permitida:\n  " + bad.join("\n  "));
  process.exit(1);
}
console.log(`ok: ${seen.size - ws.size} dependências com licenças compatíveis`);
