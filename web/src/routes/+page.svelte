<script lang="ts">
	import Busca from '$lib/components/Busca.svelte';
	import PainelAlertas from '$lib/components/PainelAlertas.svelte';
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
</div>
