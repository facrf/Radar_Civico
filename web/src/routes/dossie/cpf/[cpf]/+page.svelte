<script lang="ts">
	import { page } from '#lib/page';
	import { onMount } from 'svelte';
	import DossieCpfView from '#lib/components/DossieCpfView.svelte';
	import type { DossieCpf } from '#lib/types';

	let cpfParam = ($page.params.cpf ?? "");
	let dossie: DossieCpf | null = null;
	let loading = true;
	let erro: string | null = null;

	async function carregarDossie() {
		loading = true;
		erro = null;
		try {
			const res = await fetch(`/api/v1/dossie/cpf/${encodeURIComponent(cpfParam)}`);
			if (res.ok) {
				dossie = await res.json();
			} else if (res.status === 404) {
				erro = 'Pessoa Física não encontrada ou sem vínculos cruzados cadastrados.';
			} else {
				erro = 'Falha ao carregar dossiê analítico da pessoa física.';
			}
		} catch (err) {
			erro = 'Erro de comunicação com a API do Radar Cívico.';
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		carregarDossie();
	});
</script>

<svelte:head>
	<title>{dossie ? `${dossie.nome} | Radar Cívico` : 'Dossiê Pessoa Física | Radar Cívico'}</title>
</svelte:head>

<div>
	<div class="mb-6 flex items-center justify-between print:hidden">
		<a href="/" class="text-xs font-medium text-slate-400 hover:text-emerald-400 flex items-center gap-1.5 transition-colors">
			<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18" />
			</svg>
			<span>Voltar à Busca</span>
		</a>

		<button
			type="button"
			on:click={() => window.print()}
			class="text-xs font-semibold px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 border border-slate-700 text-slate-200 hover:text-white transition-colors flex items-center gap-1.5 shadow-sm"
			title="Imprimir ou exportar dossiê em PDF"
		>
			<svg class="w-3.5 h-3.5 text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4a2 2 0 00-2-2H9a2 2 0 00-2 2v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
			</svg>
			<span>Imprimir / Exportar PDF</span>
		</button>
	</div>

	<!-- Cabeçalho Oficial para Impressão -->
	<div class="hidden print:block border-b-2 border-slate-900 pb-4 mb-4">
		<div class="flex items-center justify-between">
			<div class="flex items-center gap-3">
				<div class="w-10 h-10 rounded-xl bg-slate-900 text-white flex items-center justify-center font-black text-xl">
					RC
				</div>
				<div>
					<h1 class="text-xl font-black text-slate-900 tracking-tight">RADAR CÍVICO • DOSSIÊ PESSOA FÍSICA</h1>
					<p class="text-[10px] text-slate-600 font-mono tracking-wider uppercase">Auditoria Cívica de Vínculos Societários e Eleitorais</p>
				</div>
			</div>
			<div class="text-right text-[10px] text-slate-700 font-mono space-y-0.5">
				<div>Emissão: {new Date().toLocaleDateString('pt-BR')} às {new Date().toLocaleTimeString('pt-BR')}</div>
				<div>Documento Oficial Auditado</div>
			</div>
		</div>
	</div>

	{#if loading}
		<div class="py-24 flex flex-col justify-center items-center gap-3">
			<div class="w-10 h-10 border-3 border-purple-500 border-t-transparent rounded-full animate-spin"></div>
			<p class="text-sm text-slate-300 font-medium">Cruzando registros de CPF no QSA, TSE, CEAP e OAB...</p>
			<p class="text-xs text-slate-500">Buscando participações societárias e histórico de doações eleitorais</p>
		</div>
	{:else if erro}
		<div class="p-8 text-center bg-rose-500/10 border border-rose-500/30 rounded-2xl text-rose-300 text-sm max-w-xl mx-auto space-y-3">
			<p class="font-semibold">{erro}</p>
			<a href="/" class="inline-block px-4 py-2 bg-slate-800 hover:bg-slate-700 text-xs text-white rounded-lg transition-colors">
				Voltar para pesquisa principal
			</a>
		</div>
	{:else if dossie}
		<DossieCpfView {dossie} />
	{/if}
</div>

<style>
	@media print {
		@page {
			size: A4;
			margin: 12mm 15mm;
		}
		:global(body) {
			background: #ffffff !important;
			color: #000000 !important;
		}
		:global(nav),
		:global(header),
		:global(footer),
		button,
		a {
			display: none !important;
		}
		:global(.bg-slate-900),
		:global(.bg-slate-950),
		:global(.bg-slate-800) {
			background: #ffffff !important;
			color: #000000 !important;
			border-color: #cbd5e1 !important;
		}
		:global(.text-white),
		:global(.text-slate-200),
		:global(.text-slate-300) {
			color: #0f172a !important;
		}
		:global(.text-slate-400),
		:global(.text-slate-500) {
			color: #475569 !important;
		}
	}
</style>
