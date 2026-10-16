/** Erros HTTP permanecem distintos de uma consulta válida sem resultados. */
export class ApiError extends Error {
    constructor(message: string, public status: number) { super(message); this.name = 'ApiError'; }
}
export async function fetchJson<T>(url: string, options?: RequestInit): Promise<T> {
    const response = await fetch(url, options);
    if (!response.ok) {
        const payload = await response.json().catch(() => null);
        const detail = payload?.mensagem ?? payload?.erro;
        throw new ApiError(typeof detail === 'string' ? detail : `Consulta indisponível (HTTP ${response.status}). Tente novamente.`, response.status);
    }
    return response.json() as Promise<T>;
}
