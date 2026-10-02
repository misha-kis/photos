<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import GridView from "$lib/components/GridView.svelte";
    import Image from "$lib/components/Image.svelte";
    import type { PersonCluster, Photo } from "$lib/types";

    let peopleLoading = $state(false);
    let peopleError = $state<string | null>(null);
    let people = $state<PersonCluster[]>([]);
    let selectedPerson = $state<PersonCluster | null>(null);
    let personPhotos = $state<Photo[]>([]);
    let personLoading = $state(false);
    let personError = $state<string | null>(null);

    async function loadPeople() {
        peopleError = null;
        peopleLoading = true;
        try {
            people = await invoke<PersonCluster[]>("get_people");
        } catch (error) {
            people = [];
            peopleError = String(error);
        } finally {
            peopleLoading = false;
        }
    }

    async function openPerson(person: PersonCluster) {
        selectedPerson = person;
        personError = null;
        personLoading = true;
        try {
            const records = await invoke<[string, string, string][]>(
                "get_person_photos",
                { detectionIds: person.detection_ids },
            );
            personPhotos = records.map(([id, thumbnailPath, originalPath]) => ({
                id,
                thumbnailPath,
                originalPath,
            }));
        } catch (error) {
            personPhotos = [];
            personError = String(error);
        } finally {
            personLoading = false;
        }
    }

    $effect(() => {
        void loadPeople();
    });
</script>

<div class="flex min-h-0 flex-1 -m-8">
    <aside
        class="w-44 shrink-0 overflow-y-auto border-l border-zinc-800 bg-zinc-950 p-4"
    >
        <h2 class="mb-4 text-lg font-semibold">People</h2>
        {#if peopleLoading}
            <p class="text-sm text-zinc-400">Loading people…</p>
        {:else if peopleError}
            <p class="text-sm text-red-400">{peopleError}</p>
        {:else if !people.length}
            <p class="text-sm text-zinc-400">No people found yet</p>
        {:else}
            <nav class="space-y-2" aria-label="People">
                {#each people as person (person.id)}
                    <button
                        type="button"
                        class="flex w-full items-center gap-3 rounded-lg p-2 text-left hover:bg-zinc-800"
                        class:bg-zinc-800={selectedPerson?.id === person.id}
                        onclick={() => openPerson(person)}
                    >
                        <span
                            class="h-12 w-12 shrink-0 overflow-hidden rounded-full bg-zinc-800"
                        >
                            <Image src={person.thumbnail_path} alt="" />
                        </span>
                        <span class="text-sm text-zinc-300"
                            >{person.photo_count}</span
                        >
                    </button>
                {/each}
            </nav>
        {/if}
    </aside>
    <div class="flex min-h-0 min-w-0 flex-1 flex-col p-8">
        {#if !selectedPerson}
            <p class="text-zinc-400">Select a person to view their photos.</p>
        {:else if personLoading}
            <p>Loading photos…</p>
        {:else if personError}
            <p class="text-red-400">{personError}</p>
        {:else}
            <GridView getPhotos={async () => personPhotos} />
        {/if}
    </div>
</div>
