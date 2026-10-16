import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';
const source = fs.readFileSync(new URL('../src/lib/api.ts', import.meta.url), 'utf8');
const code = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
const { fetchJson, ApiError } = await import(`data:text/javascript;base64,${Buffer.from(code).toString('base64')}`);
test('consulta vazia válida é distinta de uma indisponibilidade HTTP', async (t) => {
    t.mock.method(globalThis, 'fetch', async () => new Response(JSON.stringify({ resultados: [] }), { status: 200 }));
    assert.deepEqual(await fetchJson('/busca'), { resultados: [] });
    globalThis.fetch = async () => new Response(JSON.stringify({ erro: 'Banco ocupado' }), { status: 503 });
    await assert.rejects(fetchJson('/busca'), error => error instanceof ApiError && error.status === 503 && error.message === 'Banco ocupado');
});
test('erro HTML é legível e cancelamento preserva o erro original', async (t) => {
    t.mock.method(globalThis, 'fetch', async () => new Response('<html>erro</html>', { status: 502 }));
    await assert.rejects(fetchJson('/busca'), /HTTP 502/);
    const abort = new DOMException('Cancelado', 'AbortError');
    globalThis.fetch = async () => { throw abort; };
    await assert.rejects(fetchJson('/busca'), error => error === abort);
});
