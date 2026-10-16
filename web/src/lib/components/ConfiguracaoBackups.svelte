<script lang="ts">
    import { onMount } from 'svelte';
    import { fetchJson } from '#lib/api';
    import type { BackupItem,JobInfo } from '#lib/config-types';
    export let job:JobInfo|null=null;
    export let iniciarPollingJob:(id:string)=>void;
    let ultimaConclusao:string|null=null;
    $: if (job?.fonte==='BACKUP' && !job.persistencia_pendente && ['CONCLUIDO','ERRO','INTERROMPIDO'].includes(job.status) && ultimaConclusao!==job.job_id) {
        ultimaConclusao=job.job_id;
        if (job.status==='CONCLUIDO') {msgBackup=job.mensagem;void carregarBackups();}
        else {erroBackup=job.mensagem;}
    }
	// Exportação
	let tabelaExportar = 'politicos';
	let formatoExportar = 'csv';
	let exportando = false;

	const tabelasDisponiveis = [
		{ id: 'politicos', nome: 'Políticos (TSE)' },
		{ id: 'candidaturas', nome: 'Candidaturas' },
		{ id: 'receitas_campanha', nome: 'Receitas de Campanha' },
		{ id: 'despesas_campanha', nome: 'Despesas de Campanha' },
		{ id: 'despesas_parlamentares', nome: 'Despesas CEAP (Câmara)' },
		{ id: 'contratos_publicos', nome: 'Contratos Públicos (PNCP)' },
		{ id: 'emendas_parlamentares', nome: 'Emendas Parlamentares (SIOP/Transparência)' },
		{ id: 'alertas_auditoria', nome: 'Alertas de Auditoria' },
		{ id: 'empresas_qsa', nome: 'Quadro Societário (QSA)' },
		{ id: 'registros_profissionais', nome: 'Registros Profissionais (OAB)' },
		{ id: 'bens_candidato', nome: 'Bens Declarados' },
		{ id: 'nos_rede', nome: 'Nós do Grafo Relacional' },
		{ id: 'conexoes_rede', nome: 'Arestas / Vínculos de Rede' },
		{ id: 'historico_sincronizacao', nome: 'Histórico de Sincronizações' },
		{ id: 'configuracoes_sistema', nome: 'Configurações de Identidade' }
	];
	function baixarBanco() {
		void criarBackupQuente();
	}

	function baixarTabela() {
		window.location.href = `/api/v1/config/exportar/tabela/${tabelaExportar}?formato=${formatoExportar}`;
	}

	// Hot SQLite Snapshots & Backups

	let backups: BackupItem[] = [];
	let carregandoBackups = false;
	let criandoBackup = false;
	let msgBackup: string | null = null;
	let erroBackup: string | null = null;

	async function carregarBackups() {
		carregandoBackups = true;
		erroBackup = null;
		try {
            backups = await fetchJson<BackupItem[]>('/api/v1/config/backups');
		} catch (e: any) {
			erroBackup = 'Falha ao listar backups disponíveis';
		} finally {
			carregandoBackups = false;
		}
	}

	async function criarBackupQuente() {
		criandoBackup = true;
		msgBackup = null;
		erroBackup = null;
		try {
			const res = await fetch('/api/v1/config/backup', { method: 'POST' });
			const data = await res.json();
			if (res.ok && data.job_id) {
                msgBackup = 'Backup iniciado. Acompanhe a tarefa; o download ficará disponível ao concluir.';
                iniciarPollingJob(data.job_id);
            } else if (res.ok && data.status === 'sucesso') {
				msgBackup = `Backup ${data.nome_arquivo} (${data.tamanho_formatado}) gerado com sucesso!`;
				await carregarBackups();
			} else {
				erroBackup = data.mensagem || 'Falha ao executar snapshot';
			}
		} catch (e: any) {
			erroBackup = e.message || 'Erro de rede ao criar backup';
		} finally {
			criandoBackup = false;
		}
	}

	// Webhooks
	let webhookUrl = '';
	let testandoWebhook = false;
	let resultadoWebhook: { sucesso: boolean; status_code?: number; mensagem: string } | null = null;

	async function testarWebhook() {
		if (!webhookUrl.trim()) return;
		testandoWebhook = true;
		resultadoWebhook = null;
		try {
			const res = await fetch('/api/v1/config/webhook/test', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ url: webhookUrl.trim() })
			});
			resultadoWebhook = await res.json();
		} catch (e: any) {
			resultadoWebhook = {
				sucesso: false,
				mensagem: `Erro de rede: ${e.message}`
			};
		} finally {
			testandoWebhook = false;
		}
	}


    onMount(()=>{void carregarBackups();});
</script>
	<!-- Seção 5: Área de Backup e Exportação -->
	<div>
		<h2 class="text-base font-semibold text-slate-200 mb-4 flex items-center gap-2">
			<span class="w-2 h-2 rounded-full bg-amber-400"></span>
			Backup e Exportação de Dumps
		</h2>

		<div class="grid grid-cols-1 md:grid-cols-2 gap-6">
			<!-- Backup Completo do SQLite -->
			<div class="p-6 bg-slate-800/50 border border-slate-700/60 rounded-xl space-y-4">
				<div class="flex items-center gap-3">
					<div class="w-10 h-10 rounded-lg bg-amber-500/10 border border-amber-500/30 flex items-center justify-center text-amber-400">
						<svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4" />
						</svg>
					</div>
					<div>
						<h3 class="font-medium text-white">Snapshot do Banco SQLite Completo</h3>
						<p class="text-xs text-slate-400">Cópia íntegra de produção (.sqlite) gerada em tempo real com WAL online</p>
					</div>
				</div>

				<div class="flex flex-wrap items-center gap-2 pt-2">
					<button
						on:click={baixarBanco}
						class="px-4 py-2 bg-amber-600 hover:bg-amber-500 text-slate-950 font-semibold text-xs rounded-lg transition-colors flex items-center justify-center gap-1.5 shadow-sm"
					>
						<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
						</svg>
						<span>Baixar Arquivo Agora</span>
					</button>

					<button
						on:click={criarBackupQuente}
						disabled={criandoBackup || (job?.fonte === 'BACKUP' && job.status === 'PROCESSANDO')}
						class="px-4 py-2 bg-slate-800 hover:bg-slate-700 text-amber-300 hover:text-white border border-amber-500/30 font-semibold text-xs rounded-lg transition-colors flex items-center justify-center gap-1.5 disabled:opacity-50"
					>
						{#if criandoBackup}
							<div class="w-3 h-3 border-2 border-amber-400 border-t-transparent rounded-full animate-spin"></div>
							<span>Salvando Snapshot...</span>
						{:else}
							<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4" />
							</svg>
							<span>Criar Snapshot no Servidor</span>
						{/if}
					</button>
				</div>

				{#if msgBackup}
					<div class="p-2.5 rounded-lg bg-emerald-500/10 border border-emerald-500/30 text-emerald-300 text-xs">
						✅ {msgBackup}
					</div>
				{/if}
				{#if erroBackup}
					<div class="p-2.5 rounded-lg bg-rose-500/10 border border-rose-500/30 text-rose-300 text-xs">
						❌ {erroBackup}
					</div>
				{/if}

				<!-- Listagem de Backups Salvos -->
				{#if backups.length > 0}
					<div class="pt-2 border-t border-slate-700/60 space-y-2">
						<span class="text-[11px] font-semibold text-slate-400 uppercase tracking-wider block">
							Snapshots Salvos em Servidor ({backups.length}):
						</span>
						<div class="max-h-36 overflow-y-auto space-y-1.5 pr-1">
							{#each backups as b}
								<div class="p-2 bg-slate-900/80 border border-slate-700/50 rounded-lg flex items-center justify-between text-[11px]">
									<div>
										<span class="font-mono text-slate-200 block">{b.nome_arquivo}</span>
										<span class="text-slate-500 text-[10px]">{b.tamanho_formatado} &bull; {new Date(b.criado_em).toLocaleDateString('pt-BR')} {new Date(b.criado_em).toLocaleTimeString('pt-BR')}</span>
									</div>
									<a
										href={b.download_url}
										class="px-2.5 py-1 rounded bg-slate-800 hover:bg-slate-700 text-amber-300 text-[11px] font-medium transition-colors"
										download
									>
										Download
									</a>
								</div>
							{/each}
						</div>
					</div>
				{/if}
			</div>

			<!-- Exportação de Tabela em CSV ou JSON -->
			<div class="p-6 bg-slate-800/50 border border-slate-700/60 rounded-xl space-y-4">
				<div class="flex items-center gap-3">
					<div class="w-10 h-10 rounded-lg bg-indigo-500/10 border border-indigo-500/30 flex items-center justify-center text-indigo-400">
						<svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 17v-2m3 2v-4m3 4v-6m2 10H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
						</svg>
					</div>
					<div>
						<h3 class="font-medium text-white">Exportação de Tabela Específica</h3>
						<p class="text-xs text-slate-400">Download em lote de qualquer tabela para análise em Python/Pandas</p>
					</div>
				</div>

				<div class="flex flex-wrap items-center gap-3 pt-2">
					<select
						bind:value={tabelaExportar}
						class="bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-3 py-2 focus:ring-emerald-500 focus:border-emerald-500"
					>
						{#each tabelasDisponiveis as t}
							<option value={t.id}>{t.nome}</option>
						{/each}
					</select>

					<div class="flex items-center bg-slate-900 rounded-lg border border-slate-700 p-0.5 text-xs">
						<button
							type="button"
							on:click={() => (formatoExportar = 'csv')}
							class="px-2.5 py-1.5 rounded-md font-medium transition-colors {formatoExportar === 'csv' ? 'bg-indigo-600 text-white' : 'text-slate-400 hover:text-white'}"
						>
							CSV
						</button>
						<button
							type="button"
							on:click={() => (formatoExportar = 'json')}
							class="px-2.5 py-1.5 rounded-md font-medium transition-colors {formatoExportar === 'json' ? 'bg-indigo-600 text-white' : 'text-slate-400 hover:text-white'}"
						>
							JSON
						</button>
					</div>

					<button
						on:click={baixarTabela}
						class="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white font-semibold text-xs rounded-lg transition-colors flex items-center gap-2"
					>
						<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
						</svg>
						<span>Baixar {formatoExportar.toUpperCase()}</span>
					</button>
				</div>
			</div>
		</div>

		<!-- Integração de Webhooks -->
		<div class="mt-6 p-6 bg-slate-800/40 border border-slate-700/60 rounded-xl space-y-4">
			<div class="flex items-center gap-3">
				<div class="w-10 h-10 rounded-lg bg-teal-500/10 border border-teal-500/30 flex items-center justify-center text-teal-400">
					<svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
					</svg>
				</div>
				<div>
					<h3 class="font-medium text-white">Notificações Externas via Webhook</h3>
					<p class="text-xs text-slate-400">Envio de disparos HTTP POST para Slack, Discord ou endpoints de monitoramento</p>
				</div>
			</div>

			<div class="flex flex-col sm:flex-row items-stretch sm:items-center gap-2 pt-1">
				<input
					type="url"
					bind:value={webhookUrl}
					placeholder="https://exemplo.com/api/webhook ou https://discord.com/api/webhooks/..."
					class="flex-1 bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-3 py-2 focus:ring-teal-500 focus:border-teal-500 font-mono"
				/>
				<button
					type="button"
					on:click={testarWebhook}
					disabled={testandoWebhook || !webhookUrl.trim()}
					class="px-4 py-2 bg-teal-600 hover:bg-teal-500 text-slate-950 font-semibold text-xs rounded-lg transition-colors flex items-center justify-center gap-2 disabled:opacity-50"
				>
					{#if testandoWebhook}
						<div class="w-3.5 h-3.5 border-2 border-slate-950 border-t-transparent rounded-full animate-spin"></div>
						<span>Testando...</span>
					{:else}
						<span>Disparar Webhook de Teste</span>
					{/if}
				</button>
			</div>

			{#if resultadoWebhook}
				<div class="p-3 rounded-lg text-xs flex items-center gap-2 animate-fade-in {resultadoWebhook.sucesso ? 'bg-emerald-500/10 border border-emerald-500/30 text-emerald-300' : 'bg-rose-500/10 border border-rose-500/30 text-rose-300'}">
					<span>{resultadoWebhook.sucesso ? '✅' : '❌'}</span>
					<span>{resultadoWebhook.mensagem}</span>
					{#if resultadoWebhook.status_code}
						<span class="font-mono text-[11px] px-1.5 py-0.5 rounded bg-slate-900 border border-slate-700">HTTP {resultadoWebhook.status_code}</span>
					{/if}
				</div>
			{/if}
		</div>
	</div>

