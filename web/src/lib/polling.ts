/** Agenda a próxima consulta somente depois da anterior terminar. */
export function startPolling(
	task: (signal: AbortSignal) => Promise<boolean>,
	options: { intervalMs: number; maxIntervalMs?: number; timeoutMs?: number; onError?: (error: unknown) => void }
): () => void {
	let stopped = false;
	let timer: ReturnType<typeof setTimeout> | undefined;
	let controller: AbortController | undefined;
	let failures = 0;
	const run = async () => {
		if (stopped) return;
		controller = new AbortController();
		const current = controller;
		const deadline = setTimeout(() => current.abort(), options.timeoutMs ?? 30000);
		let keepPolling = true;
		try {
			keepPolling = await task(current.signal);
			failures = 0;
		} catch (error) {
			failures += 1;
			if (!stopped) options.onError?.(error);
		} finally {
			clearTimeout(deadline);
			current.abort();
			if (!stopped && keepPolling) {
				const delay = Math.min(options.intervalMs * 2 ** Math.min(failures, 5), options.maxIntervalMs ?? 15000);
				timer = setTimeout(run, delay);
			}
		}
	};
	void run();
	return () => {
		stopped = true;
		if (timer !== undefined) clearTimeout(timer);
		controller?.abort();
	};
}
