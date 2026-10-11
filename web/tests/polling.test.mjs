import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';
const source = fs.readFileSync(new URL('../src/lib/polling.ts', import.meta.url), 'utf8');
const code = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
const { startPolling } = await import(`data:text/javascript;base64,${Buffer.from(code).toString('base64')}`);

function timers(t) {
    let sequence = 0;
    const pending = new Map();
    t.mock.method(globalThis, 'setTimeout', (callback, delay) => { const id = ++sequence; pending.set(id, { callback, delay }); return id; });
    t.mock.method(globalThis, 'clearTimeout', (id) => pending.delete(id));
    return pending;
}
async function settle() { for (let i = 0; i < 8; i++) await Promise.resolve(); }
function fire(pending, delay) {
    const item = [...pending].find(([, value]) => value.delay === delay);
    assert.ok(item, `Timer de ${delay} ms não encontrado`);
    pending.delete(item[0]);
    item[1].callback();
}

test('aguarda resposta, não sobrepõe consultas e cancela ao sair', async (t) => {
    const pending = timers(t);
    let calls = 0;
    let resolve;
    let signal;
    const stop = startPolling((current) => {
        calls++;
        signal = current;
        return new Promise((done) => { resolve = done; });
    }, { intervalMs: 1000 });
    assert.equal(calls, 1);
    assert.equal(pending.size, 1); // Somente prazo; nenhuma segunda consulta agendada.
    resolve(true);
    await settle();
    assert.ok(signal.aborted);
    assert.equal(pending.size, 1);
    fire(pending, 1000);
    assert.equal(calls, 2);
    stop();
    assert.ok(signal.aborted);
    resolve(true);
    await settle();
    assert.equal(pending.size, 0);
});

test('reduz frequência após falhas e restaura intervalo ao recuperar', async (t) => {
    const pending = timers(t);
    let calls = 0;
    let errors = 0;
    const stop = startPolling(async () => {
        calls++;
        if (calls <= 2) throw new Error('HTTP 500');
        return calls < 4;
    }, { intervalMs: 1000, onError: () => { errors++; } });
    await settle();
    fire(pending, 2000);
    await settle();
    fire(pending, 4000);
    await settle();
    fire(pending, 1000);
    await settle();
    assert.equal(calls, 4);
    assert.equal(errors, 2);
    assert.equal(pending.size, 0);
    stop();
});

test('prazo de consulta aborta a requisição e agenda nova tentativa', async (t) => {
    const pending = timers(t);
    let errors = 0;
    let signal;
    const stop = startPolling((current) => {
        signal = current;
        return new Promise((resolve, reject) => current.addEventListener('abort', () => reject(new Error('timeout')), { once: true }));
    }, { intervalMs: 1000, timeoutMs: 5000, onError: () => { errors++; } });
    fire(pending, 5000);
    await settle();
    assert.ok(signal.aborted);
    assert.equal(errors, 1);
    assert.equal([...pending.values()][0].delay, 2000);
    stop();
    assert.equal(pending.size, 0);
});
