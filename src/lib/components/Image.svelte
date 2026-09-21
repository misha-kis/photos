<script lang="ts">
    type Item = {
        id: number;
        title: string;
    };

    export let item: Item;
</script>

<img src="/svelte.svg" alt={item.title} />
