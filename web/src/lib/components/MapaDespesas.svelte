<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { browser } from '$app/environment';
	import type { PontoDespesaGeo } from '$lib/types';

	export let pontos: PontoDespesaGeo[] = [];
	export let politicoNome = 'Parlamentar';
	export let politicoUf = 'BR';

	let mapContainer: HTMLElement;
	let map: any = null;
	let markerClusterGroup: any = null;
	let L: any = null;

	// Filtros do mapa
	let filtroCategoria = 'TODAS';
	let apenasForaUf = false;
	let buscaFornecedor = '';

	// Categorias únicas disponíveis nos pontos
	$: categoriasDisponiveis = [
		'TODAS',
		...Array.from(new Set(pontos.map((p) => p.categoria))).filter(Boolean)
	];

	// Pontos filtrados
	$: pontosFiltrados = pontos.filter((p) => {
		if (filtroCategoria !== 'TODAS' && p.categoria !== filtroCategoria) {
			return false;
		}
		if (apenasForaUf && !p.fora_uf_origem) {
			return false;
		}
		if (buscaFornecedor.trim()) {
			const term = buscaFornecedor.toLowerCase();
			const matchNome = p.fornecedor_nome.toLowerCase().includes(term);
			const matchCnpj = p.fornecedor_cnpj.includes(term);
			const matchMun = p.municipio.toLowerCase().includes(term);
			if (!matchNome && !matchCnpj && !matchMun) return false;
		}
		return true;
	});

	$: totalValorFiltrado = pontosFiltrados.reduce((acc, p) => acc + p.valor, 0);
	$: totalForaUfFiltrado = pontosFiltrados.filter((p) => p.fora_uf_origem).length;

	function formatarMoeda(val: number): string {
		return val.toLocaleString('pt-BR', { style: 'currency', currency: 'BRL' });
	}

	function formatarData(dataStr: string): string {
		if (!dataStr) return '-';
		const parts = dataStr.split('-');
		if (parts.length === 3) {
			return `${parts[2]}/${parts[1]}/${parts[0]}`;
		}
		return dataStr;
	}

	function formatarCnpj(doc: string): string {
		const clean = doc.replace(/\D/g, '');
		if (clean.length === 14) {
			return clean.replace(/^(\d{2})(\d{3})(\d{3})(\d{4})(\d{2})$/, '$1.$2.$3/$4-$5');
		}
		return doc;
	}

	function criarIcone(ponto: PontoDespesaGeo) {
		const isAlerta = ponto.alerta_distancia || ponto.fora_uf_origem;
		const isCombustivel = ponto.categoria.toUpperCase().includes('COMBUST');

		const bgClass = isAlerta ? 'bg-rose-500' : isCombustivel ? 'bg-amber-500' : 'bg-emerald-500';
		const borderClass = isAlerta ? 'border-rose-300 ring-rose-400' : 'border-emerald-300 ring-emerald-400';
		const pulseHtml = isAlerta ? `<span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-rose-400 opacity-75"></span>` : '';

		const emoji = isAlerta ? '⚠️' : isCombustivel ? '⛽' : '📍';

		const html = `
			<div class="relative flex items-center justify-center w-8 h-8 cursor-pointer group">
				${pulseHtml}
				<div class="relative w-7 h-7 rounded-full ${bgClass} border-2 ${borderClass} shadow-lg flex items-center justify-center text-xs font-bold text-white transition-transform group-hover:scale-125">
					<span class="text-[11px]">${emoji}</span>
				</div>
			</div>
		`;

		return L.divIcon({
			className: 'custom-geo-marker',
			html,
			iconSize: [32, 32],
			iconAnchor: [16, 32],
			popupAnchor: [0, -32]
		});
	}

	function renderizarMarcadores() {
		if (!map || !markerClusterGroup || !L) return;

		markerClusterGroup.clearLayers();

		const bounds = L.latLngBounds([]);

		for (const ponto of pontosFiltrados) {
			if (!ponto.latitude || !ponto.longitude || isNaN(ponto.latitude) || isNaN(ponto.longitude)) {
				continue;
			}

			const latLng = [ponto.latitude, ponto.longitude];
			bounds.extend(latLng);

			const icon = criarIcone(ponto);
			const marker = L.marker(latLng, { icon });

			const alertaBadge = ponto.fora_uf_origem
				? `<div class="p-2 mb-2 rounded bg-rose-500/20 border border-rose-500/40 text-rose-300 text-[11px] leading-tight">
						<span class="font-bold flex items-center gap-1">⚠️ Alerta de Distância / Fora da UF</span>
						<span>${ponto.motivo_alerta || `Despesa a ${ponto.distancia_origem_km.toFixed(0)} km da base eleitoral (${politicoUf})`}</span>
				   </div>`
				: '';

			const litrosHtml = ponto.litros
				? `<div class="flex justify-between text-xs py-0.5 border-b border-slate-700/60">
						<span class="text-slate-400">Litros / Volume:</span>
						<span class="font-mono font-semibold text-amber-300">${ponto.litros.toFixed(1)} L</span>
				   </div>`
				: '';

			const popupContent = `
				<div class="p-1 max-w-[280px] font-sans text-slate-200">
					${alertaBadge}
					<div class="font-bold text-sm text-white leading-snug mb-1 truncate" title="${ponto.fornecedor_nome}">
						${ponto.fornecedor_nome}
					</div>
					<div class="text-[11px] text-slate-400 mb-2 font-mono">
						CNPJ: ${formatarCnpj(ponto.fornecedor_cnpj)}
					</div>

					<div class="space-y-1 my-2 bg-slate-900/90 p-2.5 rounded-lg border border-slate-800">
						<div class="flex justify-between text-xs py-0.5 border-b border-slate-800">
							<span class="text-slate-400">Valor Pago:</span>
							<span class="font-semibold text-emerald-400 font-mono">${formatarMoeda(ponto.valor)}</span>
						</div>
						<div class="flex justify-between text-xs py-0.5 border-b border-slate-800">
							<span class="text-slate-400">Data:</span>
							<span class="text-slate-300 font-mono">${formatarData(ponto.data)}</span>
						</div>
						<div class="flex justify-between text-xs py-0.5 border-b border-slate-800">
							<span class="text-slate-400">Localização:</span>
							<span class="text-slate-200">${ponto.municipio} - ${ponto.uf}</span>
						</div>
						<div class="flex justify-between text-xs py-0.5 border-b border-slate-800">
							<span class="text-slate-400">Categoria:</span>
							<span class="text-cyan-300 text-[11px] truncate max-w-[150px]" title="${ponto.categoria}">${ponto.categoria}</span>
						</div>
						${litrosHtml}
					</div>

					<div class="pt-1 flex items-center justify-between gap-2">
						<a href="/dossie/cnpj/${ponto.fornecedor_cnpj}" target="_blank"
						   class="w-full text-center px-3 py-1.5 rounded-md bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-semibold shadow-sm transition-colors block">
							Ver Dossiê da Empresa 🔍
						</a>
					</div>
				</div>
			`;

			marker.bindPopup(popupContent, {
				className: 'radar-map-popup',
				maxWidth: 320
			});

			markerClusterGroup.addLayer(marker);
		}

		if (bounds.isValid()) {
			map.fitBounds(bounds, { padding: [50, 50], maxZoom: 14 });
		}
	}

	$: if (map && markerClusterGroup && pontosFiltrados) {
		renderizarMarcadores();
	}

	import 'leaflet/dist/leaflet.css';
	import 'leaflet.markercluster/dist/MarkerCluster.css';
	import 'leaflet.markercluster/dist/MarkerCluster.Default.css';

	onMount(async () => {
		if (!browser) return;

		try {
			const leafletModule = await import('leaflet');
			L = leafletModule.default || leafletModule;

			// Define L nos escopos globais requeridos por plugins legados UMD do Leaflet
			if (typeof window !== 'undefined') {
				(window as any).L = L;
			}
			if (typeof globalThis !== 'undefined') {
				(globalThis as any).L = L;
			}

			try {
				// @ts-ignore
				await import('leaflet.markercluster');
			} catch (errCluster) {
				console.warn('leaflet.markercluster não carregou, usando agrupamento nativo:', errCluster);
			}

			if (!mapContainer) return;

			// Inicializa o mapa com foco no Brasil
			map = L.map(mapContainer, {
				center: [-15.793889, -47.882778],
				zoom: 4,
				zoomControl: true,
				attributionControl: true
			});

			// Camada de mapa OpenStreetMap em tons escuros
			L.tileLayer('https://{s}.basemaps.cartocdn.com/rastertiles/voyager/{z}/{x}/{y}{r}.png', {
				attribution: '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> &copy; <a href="https://carto.com/attributions">CARTO</a>',
				subdomains: 'abcd',
				maxZoom: 19
			}).addTo(map);

			if (L && typeof L.markerClusterGroup === 'function') {
				// @ts-ignore
				markerClusterGroup = L.markerClusterGroup({
					showCoverageOnHover: false,
					spiderfyOnMaxZoom: true,
					maxClusterRadius: 40,
					iconCreateFunction: (cluster: any) => {
						const count = cluster.getChildCount();
						let bg = 'bg-emerald-600/90 text-white border-emerald-400';
						if (count > 20) {
							bg = 'bg-amber-600/90 text-white border-amber-400';
						}
						if (count > 50) {
							bg = 'bg-indigo-600/90 text-white border-indigo-400';
						}

						return L.divIcon({
							html: `<div class="w-9 h-9 rounded-full ${bg} border-2 flex items-center justify-center font-bold text-xs shadow-xl ring-2 ring-slate-900 font-mono">${count}</div>`,
							className: 'marker-cluster-custom',
							iconSize: [36, 36]
						});
					}
				});
			} else {
				// Fallback caso markercluster não esteja disponível
				markerClusterGroup = L.layerGroup();
			}

			map.addLayer(markerClusterGroup);

			renderizarMarcadores();

			setTimeout(() => {
				if (map) map.invalidateSize();
			}, 300);
		} catch (err) {
			console.error('Erro ao inicializar Leaflet:', err);
		}
	});

	onDestroy(() => {
		if (map) {
			map.remove();
			map = null;
		}
	});

	function resetarZoom() {
		if (map && markerClusterGroup) {
			if (typeof markerClusterGroup.getBounds === 'function') {
				const bounds = markerClusterGroup.getBounds();
				if (bounds && typeof bounds.isValid === 'function' && bounds.isValid()) {
					map.fitBounds(bounds, { padding: [50, 50], maxZoom: 14 });
					return;
				}
			}
			map.setView([-15.793889, -47.882778], 4);
		}
	}
</script>

<svelte:head>
	<link rel="stylesheet" href="https://unpkg.com/leaflet@1.9.4/dist/leaflet.css" />
	<link rel="stylesheet" href="https://unpkg.com/leaflet.markercluster@1.5.3/dist/MarkerCluster.css" />
	<link rel="stylesheet" href="https://unpkg.com/leaflet.markercluster@1.5.3/dist/MarkerCluster.Default.css" />
</svelte:head>

<div class="bg-slate-900 border border-slate-800 rounded-2xl overflow-hidden shadow-xl space-y-4 p-5">
	<!-- Barra Superior de Controle e Filtros -->
	<div class="flex flex-col lg:flex-row lg:items-center justify-between gap-4 pb-3 border-b border-slate-800">
		<div class="space-y-1">
			<div class="flex items-center gap-2">
				<span class="p-1.5 rounded-lg bg-emerald-500/10 border border-emerald-500/30 text-emerald-400">
					<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 20l-5.447-2.724A1 1 0 013 16.382V5.618a1 1 0 011.447-.894L9 7m0 13l6-3m-6 3V7m6 10l4.553 2.276A1 1 0 0021 18.382V7.618a1 1 0 00-.553-.894L15 4m0 13V4m0 0L9 7" />
					</svg>
				</span>
				<h3 class="text-base font-bold text-white">Mapa Georreferenciado de Gastos Parlamentares</h3>
			</div>
			<p class="text-xs text-slate-400">
				Visualização geográfica de notas fiscais, postos de combustíveis e deslocamentos de <span class="text-slate-200 font-semibold">{politicoNome}</span> ({politicoUf})
			</p>
		</div>

		<!-- Resumo estatístico do mapa -->
		<div class="flex flex-wrap items-center gap-2 text-xs">
			<div class="px-3 py-1.5 rounded-lg bg-slate-800/80 border border-slate-700/60 flex items-center gap-1.5">
				<span class="w-2 h-2 rounded-full bg-emerald-400"></span>
				<span class="text-slate-400">Visíveis:</span>
				<strong class="text-white font-mono">{pontosFiltrados.length}</strong>
			</div>

			<div class="px-3 py-1.5 rounded-lg bg-slate-800/80 border border-slate-700/60 flex items-center gap-1.5">
				<span class="text-slate-400">Valor Filtrado:</span>
				<strong class="text-emerald-400 font-mono">{formatarMoeda(totalValorFiltrado)}</strong>
			</div>

			{#if totalForaUfFiltrado > 0}
				<div class="px-3 py-1.5 rounded-lg bg-rose-500/10 border border-rose-500/30 flex items-center gap-1.5 text-rose-300">
					<span class="w-2 h-2 rounded-full bg-rose-500 animate-pulse"></span>
					<span>Fora da UF:</span>
					<strong class="font-mono font-bold text-rose-200">{totalForaUfFiltrado} notas</strong>
				</div>
			{/if}
		</div>
	</div>

	<!-- Barra de Filtros e Busca Rápida -->
	<div class="grid grid-cols-1 sm:grid-cols-3 lg:grid-cols-4 gap-3 text-xs">
		<!-- Busca por Estabelecimento / Município -->
		<div>
			<label for="filtro-busca-mapa" class="block text-[11px] text-slate-400 mb-1 font-medium">Buscar Estabelecimento / Cidade:</label>
			<input
				id="filtro-busca-mapa"
				type="text"
				bind:value={buscaFornecedor}
				placeholder="Ex: Posto Cascol, Caxias, CNPJ..."
				class="w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-1.5 text-slate-200 placeholder-slate-500 focus:ring-emerald-500 focus:border-emerald-500"
			/>
		</div>

		<!-- Filtro por Categoria -->
		<div>
			<label for="filtro-cat-mapa" class="block text-[11px] text-slate-400 mb-1 font-medium">Categoria de Despesa:</label>
			<select
				id="filtro-cat-mapa"
				bind:value={filtroCategoria}
				class="w-full bg-slate-950 border border-slate-700 rounded-lg px-2.5 py-1.5 text-slate-200 focus:ring-emerald-500 focus:border-emerald-500 truncate"
			>
				{#each categoriasDisponiveis as cat}
					<option value={cat}>{cat}</option>
				{/each}
			</select>
		</div>

		<!-- Toggle Fora da UF -->
		<div class="flex items-end">
			<label class="flex items-center gap-2 cursor-pointer p-2 bg-slate-950/60 rounded-lg border border-slate-800 w-full hover:border-slate-700 transition-colors select-none">
				<input
					type="checkbox"
					bind:checked={apenasForaUf}
					class="rounded bg-slate-900 border-slate-700 text-rose-500 focus:ring-rose-500 w-4 h-4 cursor-pointer"
				/>
				<span class="text-[11px] text-slate-300 font-medium flex items-center gap-1">
					<span>Apenas Fora da UF ({politicoUf})</span>
					<span class="px-1.5 py-0.2 rounded text-[10px] bg-rose-500/20 text-rose-300">⚠️ Alerta</span>
				</span>
			</label>
		</div>

		<!-- Botão Resetar Visão -->
		<div class="flex items-end sm:col-span-3 lg:col-span-1">
			<button
				type="button"
				on:click={resetarZoom}
				class="w-full px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-200 font-medium rounded-lg border border-slate-700 transition-colors flex items-center justify-center gap-1.5"
			>
				<svg class="w-3.5 h-3.5 text-slate-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4" />
				</svg>
				<span>Enquadrar Todos os Pontos</span>
			</button>
		</div>
	</div>

	<!-- Container do Mapa Leaflet -->
	<div class="relative w-full h-[480px] rounded-xl overflow-hidden border border-slate-800 shadow-inner bg-slate-950">
		<div bind:this={mapContainer} class="w-full h-full z-0"></div>

		<!-- Legenda Flutuante no Canto Inferior Esquerdo -->
		<div class="absolute bottom-3 left-3 z-[1000] bg-slate-950/90 backdrop-blur border border-slate-800 rounded-lg p-2.5 shadow-lg text-[11px] space-y-1.5 max-w-[220px]">
			<span class="font-bold text-white block text-[10px] uppercase tracking-wider">Legenda de Marcadores</span>
			<div class="flex items-center gap-2 text-slate-300">
				<span class="w-3 h-3 rounded-full bg-emerald-500 border border-emerald-300 flex-shrink-0"></span>
				<span>Base Eleitoral / Brasília</span>
			</div>
			<div class="flex items-center gap-2 text-slate-300">
				<span class="w-3 h-3 rounded-full bg-amber-500 border border-amber-300 flex-shrink-0"></span>
				<span>Combustíveis & Lubrificantes</span>
			</div>
			<div class="flex items-center gap-2 text-rose-300">
				<span class="w-3 h-3 rounded-full bg-rose-500 border border-rose-300 animate-pulse flex-shrink-0"></span>
				<span class="font-medium">Alerta de Distância (> 250km / Fora UF)</span>
			</div>
		</div>
	</div>
</div>

<style>
	:global(.radar-map-popup .leaflet-popup-content-wrapper) {
		background-color: #0f172a !important;
		border: 1px solid #334155 !important;
		border-radius: 0.75rem !important;
		box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.5) !important;
		padding: 0.25rem !important;
	}

	:global(.radar-map-popup .leaflet-popup-tip) {
		background-color: #0f172a !important;
		border: 1px solid #334155 !important;
	}

	:global(.radar-map-popup .leaflet-popup-close-button) {
		color: #94a3b8 !important;
		padding: 4px !important;
	}

	:global(.radar-map-popup .leaflet-popup-close-button:hover) {
		color: #ffffff !important;
	}
</style>
