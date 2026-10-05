<script lang="ts">
	import '../app.css';
	import { onMount } from 'svelte';

	let brandTimestamp = Date.now();
	let iconLoadFailed = false;

	function atualizarIdentidade() {
		brandTimestamp = Date.now();
		iconLoadFailed = false;
	}

	onMount(() => {
		window.addEventListener('radar-icon-updated', atualizarIdentidade);
		return () => {
			window.removeEventListener('radar-icon-updated', atualizarIdentidade);
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
				<a href="/alertas" class="px-3 py-2 text-sm font-medium text-slate-300 hover:text-white hover:bg-slate-800/60 rounded-md transition-colors flex items-center gap-1.5">
					<span class="w-2 h-2 rounded-full bg-rose-500 animate-pulse"></span>
					Alertas
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

	<footer class="border-t border-slate-800 bg-slate-950 py-6 text-center text-xs text-slate-500">
		<p>Radar Cívico - Plataforma Aberta de Inteligência, Cruzamento e Auditoria de Dados Públicos</p>
		<p class="mt-1">Fontes Primárias: TSE, Câmara dos Deputados (CEAP), PNCP, Querido Diário e OAB/CNA.</p>
	</footer>
</div>
