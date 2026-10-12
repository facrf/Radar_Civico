<script lang="ts">
    import { onMount } from 'svelte';
    type Fonte = {nome:string;url:string;fonte:string;autor:string;licenca:string;licenca_url:string};
    let fontes: Fonte[] = [];
    let erro = '';
    onMount(async () => {
        try { const response = await fetch('/autoridades/fontes.json'); if (!response.ok) throw new Error('Catálogo indisponível'); fontes = await response.json(); }
        catch { erro = 'Não foi possível carregar as fontes das fotos.'; }
    });
</script>
<svelte:head><title>Fontes dos retratos — Radar Cívico</title></svelte:head>
<main class="max-w-6xl mx-auto p-6">
    <h1 class="text-2xl font-bold mb-3">Fontes dos retratos</h1>
    <p class="text-slate-400 mb-6">Retratos de referência de presidentes e ministros, com cópias locais. A imagem não indica exercício atual do cargo.</p>
    {#if erro}<p>{erro}</p>{/if}
    <div class="grid sm:grid-cols-2 lg:grid-cols-3 gap-4">
        {#each fontes as fonte}
            <article class="bg-slate-800 rounded-xl p-4 flex gap-4">
                <img src={fonte.url} alt={fonte.nome} class="w-20 h-28 object-cover rounded" loading="lazy" />
                <div><h2 class="font-semibold">{fonte.nome}</h2><p class="text-xs text-slate-400 my-2">{fonte.autor}</p>
                    <a href={fonte.fonte} class="text-sky-300 text-sm" target="_blank" rel="noreferrer">Fonte da foto</a>
                    <p>{#if fonte.licenca_url}<a href={fonte.licenca_url} class="text-sky-300 text-xs" target="_blank" rel="noreferrer">{fonte.licenca}</a>{:else}<span class="text-xs">{fonte.licenca}</span>{/if}</p>
                </div>
            </article>
        {/each}
    </div>
</main>
