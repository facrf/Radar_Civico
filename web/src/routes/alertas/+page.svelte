<script lang="ts">
	import PainelAlertas from '$lib/components/PainelAlertas.svelte';
</script>

<div class="space-y-6">
	<div>
		<h1 class="text-2xl sm:text-3xl font-bold text-white tracking-tight">Painel de Alertas do Auditor</h1>
		<p class="text-sm text-slate-400 mt-1">
			Lista de anomalias detectadas pelo motor heurístico e analítico do Radar Cívico com indicação das fontes primárias.
		</p>
	</div>

	<PainelAlertas />
</div>
