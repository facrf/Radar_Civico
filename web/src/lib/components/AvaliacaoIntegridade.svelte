<script lang="ts">
    import type { AvaliacaoIntegridade } from '#lib/types';
    export let avaliacao: AvaliacaoIntegridade | null | undefined = undefined;
    const situacoes: Record<string,string> = { SEM_DADOS:'Dados insuficientes', NAO_AUDITADO:'Auditoria pendente', DESATUALIZADA:'Dados alterados; nova auditoria pendente' };
    function formatarData(value:string):string {
        const normalized=value.includes('T') ? value : value.replace(' ','T')+'Z';
        const date=new Date(normalized);
        return Number.isNaN(date.getTime()) ? value : new Intl.DateTimeFormat('pt-BR',{dateStyle:'short',timeStyle:'short'}).format(date);
    }
    $: explicacao = `${avaliacao?.cobertura ?? 'Avaliação indisponível.'} ${situacoes[avaliacao?.situacao ?? ''] ?? ''} ${avaliacao?.ultima_auditoria ? `Última auditoria: ${formatarData(avaliacao.ultima_auditoria)}.` : ''}`;
</script>
<span class="inline-flex items-center gap-1 rounded border border-slate-600 px-2 py-1 text-xs" style:color={avaliacao?.cor_hex ?? '#94a3b8'} title={explicacao}>
    {#if avaliacao?.score != null}
        {avaliacao.score}/100 · {avaliacao.nivel_risco}
    {:else}
        Não avaliado · {situacoes[avaliacao?.situacao ?? ''] ?? 'Dados insuficientes'}
    {/if}
</span>
