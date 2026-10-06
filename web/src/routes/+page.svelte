<script lang="ts">
	import Busca from '$lib/components/Busca.svelte';
	import PainelAlertas from '$lib/components/PainelAlertas.svelte';
	import { versionStore } from '$lib/version';
</script>

<div class="space-y-12">
	<!-- Hero / Apresentação -->
	<div class="text-center max-w-3xl mx-auto pt-4 pb-2">
		<span class="px-3 py-1 text-xs font-semibold rounded-full bg-emerald-500/10 border border-emerald-500/30 text-emerald-400">
			Auditoria Cívica Determinística em Tempo Real
		</span>
		<h1 class="text-3xl sm:text-5xl font-extrabold text-white tracking-tight mt-4">
			Rastreie recursos, doações e anomalias eleitorais
		</h1>
		<p class="text-base sm:text-lg text-slate-400 mt-4 leading-relaxed">
			Cruzamentos analíticos automáticos entre dados do TSE, notas fiscais da CEAP, contratos do PNCP, Diários Oficiais e registros da OAB.
		</p>
	</div>

	<!-- Barra de Busca Unificada -->
	<section class="max-w-4xl mx-auto">
		<Busca />
	</section>

	<!-- Painel de Irregularidades / Alertas -->
	<section class="pt-6 border-t border-slate-800">
		<PainelAlertas />
	</section>

	<!-- Rodapé Informativo da Tela Inicial -->
	<footer class="pt-6 border-t border-slate-800/80 flex flex-col sm:flex-row items-center justify-between gap-4 text-xs text-slate-500">
		<div class="flex items-center gap-2">
			<span class="inline-block w-2 h-2 rounded-full bg-emerald-400"></span>
			<span>Sistema de Auditoria Cívica Operacional</span>
		</div>
		<div class="flex items-center gap-2">
			<span class="text-slate-400">Versão:</span>
			<span
				class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-slate-800/90 border border-slate-700/70 font-mono text-xs font-semibold text-emerald-400 shadow-sm cursor-help hover:border-emerald-500/50 transition-colors"
				title={`Commit Git: ${$versionStore.commit}`}
				aria-label={`Versão ${$versionStore.version}, commit ${$versionStore.commit}`}
			>
				<span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
				<span>{$versionStore.version}</span>
				<span class="text-slate-400 text-[11px] font-normal">({$versionStore.commit})</span>
			</span>
		</div>
	</footer>
</div>
