<script lang="ts">
	import '../app.css';
	import { onMount } from 'svelte';
	import { versionStore, sincronizarVersaoServidor } from '$lib/version';

	let brandTimestamp = Date.now();
	let iconLoadFailed = false;
	let totalAlertas = 0;

	function atualizarIdentidade() {
		brandTimestamp = Date.now();
		iconLoadFailed = false;
	}

	async function verificarAlertas() {
		try {
			const res = await fetch('/api/v1/auditoria/alertas?limit=1');
			if (res.ok) {
				const data = await res.json();
				totalAlertas = data.total || 0;
			}
		} catch (err) {
			console.error('Erro ao verificar alertas:', err);
		}
	}

	onMount(() => {
		window.addEventListener('radar-icon-updated', atualizarIdentidade);
		window.addEventListener('radar-alertas-updated', verificarAlertas);
		verificarAlertas();
		sincronizarVersaoServidor();
		return () => {
			window.removeEventListener('radar-icon-updated', atualizarIdentidade);
			window.removeEventListener('radar-alertas-updated', verificarAlertas);
		};
	});
</script>

<svelte:head>
	<link rel="icon" href={`/api/v1/config/favicon?v=${brandTimestamp}`} />
</svelte:head>

<div class="min-h-screen flex flex-col bg-slate-900 text-slate-100 font-sans">
	<header class="border-b border-slate-800 bg-slate-950/80 backdrop-blur sticky top-0 z-50">
		<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 h-16 flex items-center justify-between">
			<a href="/" class="flex items-center gap-3 group">
				<div class="w-9 h-9 rounded-lg bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-400 group-hover:scale-105 transition-transform overflow-hidden relative">
					<img
						src={`/api/v1/config/icone?v=${brandTimestamp}`}
						alt="Radar Cívico Logo"
						class="w-full h-full object-contain p-1"
						on:error={() => (iconLoadFailed = true)}
						class:hidden={iconLoadFailed}
					/>
					{#if iconLoadFailed}
						<svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
						</svg>
					{/if}
				</div>
				<div>
					<span class="text-lg font-bold tracking-tight text-white">Radar<span class="text-emerald-400">Cívico</span></span>
					<span class="hidden sm:inline-block ml-2 px-1.5 py-0.5 text-xs bg-slate-800 text-slate-400 rounded">Auditoria Pública</span>
				</div>
			</a>

			<nav class="flex items-center gap-1 sm:gap-2">
				<a href="/" class="px-3 py-2 text-sm font-medium text-slate-300 hover:text-white hover:bg-slate-800/60 rounded-md transition-colors">
					Painel & Busca
				</a>
				<a href="/politicos" class="px-3 py-2 text-sm font-medium text-slate-300 hover:text-white hover:bg-slate-800/60 rounded-md transition-colors flex items-center gap-1.5">
					<svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
					</svg>
					<span>Políticos</span>
				</a>
				<a href="/alertas" class="px-3 py-2 text-sm font-medium text-slate-300 hover:text-white hover:bg-slate-800/60 rounded-md transition-colors flex items-center gap-1.5">
					{#if totalAlertas > 0}
						<span class="w-2 h-2 rounded-full bg-rose-500 animate-pulse" title={`${totalAlertas} anomalias detectadas`}></span>
						<span>Alertas</span>
						<span class="px-1.5 py-0.2 text-[10px] font-bold rounded-full bg-rose-500/20 text-rose-300 border border-rose-500/40 leading-none">
							{totalAlertas > 999 ? '999+' : totalAlertas}
						</span>
					{:else}
						<span class="w-2 h-2 rounded-full bg-emerald-500/60" title="Nenhuma irregularidade ativa detectada"></span>
						<span>Alertas</span>
					{/if}
				</a>
				<a href="/configuracoes" class="px-3 py-2 text-sm font-medium text-slate-300 hover:text-white hover:bg-slate-800/60 rounded-md transition-colors flex items-center gap-1.5">
					<svg class="w-4 h-4 text-slate-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
					</svg>
					Configurações
				</a>
			</nav>
		</div>
	</header>

	<main class="flex-1 max-w-7xl w-full mx-auto px-4 sm:px-6 lg:px-8 py-8">
		<slot />
	</main>

	<footer class="border-t border-slate-800 bg-slate-950 py-6 text-xs text-slate-500">
		<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 flex flex-col sm:flex-row items-center justify-between gap-4">
			<div class="text-center sm:text-left">
				<p>Radar Cívico - Plataforma Aberta de Inteligência, Cruzamento e Auditoria de Dados Públicos</p>
				<p class="mt-1 text-slate-600">Fontes Primárias: TSE, Câmara dos Deputados (CEAP), PNCP, Querido Diário e OAB/CNA.</p>
			</div>
			<div class="flex items-center gap-2 flex-shrink-0">
				<span
					class="inline-flex items-center gap-1.5 px-3 py-1 rounded-lg bg-slate-900 border border-slate-800 font-mono text-xs text-slate-300 shadow-sm cursor-help hover:border-emerald-500/50 hover:text-emerald-400 transition-colors"
					title={`Commit Git: ${$versionStore.commit}`}
					aria-label={`Versão ${$versionStore.version}, commit ${$versionStore.commit}`}
				>
					<span class="w-1.5 h-1.5 rounded-full bg-emerald-500"></span>
					<span>{$versionStore.version}</span>
					<span class="text-slate-500 text-[11px]">({$versionStore.commit})</span>
				</span>
			</div>
		</div>
	</footer>
</div>
