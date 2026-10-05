<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { browser } from '$app/environment';
	import type { SubgrafoData } from '$lib/types';

	export let dados: SubgrafoData;
	let container: HTMLElement;
	let cyInstance: any = null;
	let elementoSelecionado: any = null;

	function formatarMoeda(valor: number): string {
		return new Intl.NumberFormat('pt-BR', { style: 'currency', currency: 'BRL' }).format(valor);
	}

	async function inicializarGrafo() {
		if (!browser || !container) return;

		const cytoscape = (await import('cytoscape')).default;

		const elements = [
			...dados.cytoscape.nodes.map((n) => ({
				group: 'nodes' as const,
				data: {
					...n.data,
					isRoot: n.data.id === dados.raiz_id
				}
			})),
			...dados.cytoscape.edges.map((e) => ({
				group: 'edges' as const,
				data: e.data
			}))
		];

		cyInstance = cytoscape({
			container,
			elements,
			style: [
				{
					selector: 'node',
					style: {
						label: 'data(label)',
						'text-valign': 'bottom',
						'text-margin-y': 6,
						'font-size': '11px',
						color: '#e2e8f0',
						'text-outline-color': '#0f172a',
						'text-outline-width': 2,
						'background-color': '#64748b',
						width: 32,
						height: 32
					}
				},
				{
					selector: 'node[tipo = "POLITICO"]',
					style: {
						'background-color': '#0ea5e9',
						width: 40,
						height: 40
					}
				},
				{
					selector: 'node[tipo = "PESSOA_FISICA"]',
					style: {
						'background-color': '#10b981',
						width: 32,
						height: 32
					}
				},
				{
					selector: 'node[tipo = "EMPRESA"]',
					style: {
						'background-color': '#f59e0b',
						width: 36,
						height: 36
					}
				},
				{
					selector: 'node[tipo = "PARTIDO"]',
					style: {
						'background-color': '#a855f7',
						width: 34,
						height: 34
					}
				},
				{
					selector: 'node[?isRoot]',
					style: {
						'border-width': 3,
						'border-color': '#38bdf8',
						width: 48,
						height: 48
					}
				},
				{
					selector: 'edge',
					style: {
						width: 2,
						'line-color': '#475569',
						'target-arrow-color': '#475569',
						'target-arrow-shape': 'triangle',
						'curve-style': 'bezier',
						opacity: 0.8
					}
				},
				{
					selector: 'edge[?anomalia]',
					style: {
						width: 4,
						'line-color': '#f43f5e',
						'target-arrow-color': '#f43f5e',
						'target-arrow-shape': 'triangle',
						'line-style': 'dashed',
						opacity: 1
					}
				},
				{
					selector: ':selected',
					style: {
						'border-width': 4,
						'border-color': '#f8fafc',
						'line-color': '#38bdf8',
						'target-arrow-color': '#38bdf8'
					}
				}
			],
			layout: {
				name: 'cose',
				animate: false,
				nodeDimensionsIncludeLabels: true,
				padding: 30
			}
		});

		cyInstance.on('tap', 'node', (evt: any) => {
			const node = evt.target;
			elementoSelecionado = {
				tipo: 'NO',
				dados: node.data()
			};
		});

		cyInstance.on('tap', 'edge', (evt: any) => {
			const edge = evt.target;
			elementoSelecionado = {
				tipo: 'ARESTA',
				dados: edge.data()
			};
		});

		cyInstance.on('tap', (evt: any) => {
			if (evt.target === cyInstance) {
				elementoSelecionado = null;
			}
		});
	}

	function resetarZoom() {
		if (cyInstance) cyInstance.fit();
	}

	function zoomIn() {
		if (cyInstance) cyInstance.zoom(cyInstance.zoom() * 1.2);
	}

	function zoomOut() {
		if (cyInstance) cyInstance.zoom(cyInstance.zoom() * 0.8);
	}

	onMount(() => {
		inicializarGrafo();
	});

	onDestroy(() => {
		if (cyInstance) {
			cyInstance.destroy();
		}
	});
</script>

<div class="relative w-full h-[600px] bg-slate-950 border border-slate-800 rounded-2xl overflow-hidden shadow-2xl">
	<!-- Canvas Cytoscape -->
	<div bind:this={container} class="w-full h-full"></div>

	<!-- Controles Flutuantes -->
	<div class="absolute top-4 right-4 flex flex-col gap-2 z-10">
		<button
			on:click={zoomIn}
			title="Aumentar Zoom"
			class="w-9 h-9 bg-slate-800/90 hover:bg-slate-700 text-slate-200 border border-slate-700 rounded-lg flex items-center justify-center font-bold text-lg shadow backdrop-blur"
		>
			+
		</button>
		<button
			on:click={zoomOut}
			title="Diminuir Zoom"
			class="w-9 h-9 bg-slate-800/90 hover:bg-slate-700 text-slate-200 border border-slate-700 rounded-lg flex items-center justify-center font-bold text-lg shadow backdrop-blur"
		>
			-
		</button>
		<button
			on:click={resetarZoom}
			title="Centralizar Grafo"
			class="w-9 h-9 bg-slate-800/90 hover:bg-slate-700 text-slate-200 border border-slate-700 rounded-lg flex items-center justify-center text-xs shadow backdrop-blur"
		>
			[ ]
		</button>
	</div>

	<!-- Legenda -->
	<div class="absolute bottom-4 left-4 bg-slate-900/90 border border-slate-800 rounded-xl p-3 text-xs shadow-lg backdrop-blur z-10 flex flex-wrap items-center gap-4">
		<div class="flex items-center gap-1.5">
			<span class="w-3 h-3 rounded-full bg-sky-500"></span>
			<span class="text-slate-300">Político</span>
		</div>
		<div class="flex items-center gap-1.5">
			<span class="w-3 h-3 rounded-full bg-emerald-500"></span>
			<span class="text-slate-300">Doador / Pessoa Física</span>
		</div>
		<div class="flex items-center gap-1.5">
			<span class="w-3 h-3 rounded-full bg-amber-500"></span>
			<span class="text-slate-300">Fornecedor / PJ</span>
		</div>
		<div class="flex items-center gap-1.5">
			<span class="w-4 h-0.5 border-t-2 border-rose-500 border-dashed"></span>
			<span class="text-rose-400 font-semibold">Anomalia / Alerta</span>
		</div>
	</div>

	<!-- Inspetor de Elemento Selecionado -->
	{#if elementoSelecionado}
		<div class="absolute top-4 left-4 max-w-sm w-full bg-slate-900/95 border border-slate-700 rounded-xl p-4 text-xs shadow-2xl backdrop-blur z-20">
			<div class="flex items-center justify-between pb-2 border-b border-slate-800">
				<span class="font-bold text-slate-200 uppercase tracking-wide">
					{elementoSelecionado.tipo === 'NO' ? 'Entidade da Rede' : 'Relação Financeira / Vínculo'}
				</span>
				<button on:click={() => (elementoSelecionado = null)} class="text-slate-400 hover:text-white">✕</button>
			</div>

			<div class="mt-3 space-y-2 text-slate-300">
				{#if elementoSelecionado.tipo === 'NO'}
					<p><span class="text-slate-500">Nome:</span> <strong class="text-slate-100">{elementoSelecionado.dados.label}</strong></p>
					<p><span class="text-slate-500">Tipo:</span> <span class="px-1.5 py-0.5 rounded bg-slate-800 text-emerald-400 font-mono">{elementoSelecionado.dados.tipo}</span></p>
					{#if elementoSelecionado.dados.documento}
						<p><span class="text-slate-500">Doc:</span> <span class="font-mono">{elementoSelecionado.dados.documento}</span></p>
					{/if}
					{#if elementoSelecionado.dados.municipio || elementoSelecionado.dados.uf}
						<p><span class="text-slate-500">Localização:</span> {elementoSelecionado.dados.municipio || ''} {elementoSelecionado.dados.uf ? `(${elementoSelecionado.dados.uf})` : ''}</p>
					{/if}
				{:else}
					<p><span class="text-slate-500">Relação:</span> <strong class="text-slate-100">{elementoSelecionado.dados.tipo_relacao}</strong></p>
					{#if elementoSelecionado.dados.valor}
						<p><span class="text-slate-500">Valor:</span> <span class="font-mono text-emerald-400 font-semibold">{formatarMoeda(elementoSelecionado.dados.valor)}</span></p>
					{/if}
					<p><span class="text-slate-500">Ano:</span> {elementoSelecionado.dados.ano}</p>
					<p><span class="text-slate-500">Fonte:</span> {elementoSelecionado.dados.fonte_dado}</p>
					{#if elementoSelecionado.dados.anomalia}
						<div class="p-2 rounded bg-rose-500/10 border border-rose-500/30 text-rose-300 font-semibold mt-2">
							⚠️ Relação sinalizada pelo motor de auditoria!
						</div>
					{/if}
				{/if}
			</div>
		</div>
	{/if}
</div>
