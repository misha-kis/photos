<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import SideBar from "$lib/components/SideBar.svelte";
    import GridView from "$lib/components/GridView.svelte";
    import ImportView from "$lib/components/ImportView.svelte";
    import type { PersonCluster, Photo } from "$lib/types";

    let showImport = $state(false);
    let view = $state<"gallery" | "people">("gallery");
    let peopleLoading = $state(false);
    let peopleError = $state<string | null>(null);
    let people = $state<PersonCluster[]>([]);
    let selectedPerson = $state<PersonCluster | null>(null);
    let personPhotos = $state<Photo[]>([]);
    let personLoading = $state(false);
    let personError = $state<string | null>(null);

    async function getPhotos(): Promise<Photo[]> {
        const records = await invoke<[string, string, string][]>(
            "get_image_ids_with_paths",
        );
        return records.map(([id, thumbnailPath, originalPath]) => ({
            id,
            thumbnailPath,
            originalPath,
        }));
    }

    async function navigate(viewName: "gallery" | "people") {
        view = viewName;
        if (viewName !== "people") return;

        selectedPerson = null;
        personPhotos = [];
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

    function personThumbnail(person: PersonCluster) {
        return person.thumbnail_path;
    }
</script>

<div class="flex h-screen overflow-hidden">
    <SideBar active={view} onnavigate={navigate} />
    <div
        class="flex min-h-0 min-w-0 flex-1 flex-col bg-zinc-900 p-8 text-zinc-100"
    >
        {#if showImport}
            <ImportView
                oncancel={() => (showImport = false)}
                oncomplete={() => (showImport = false)}
            />
        {:else if view === "people"}
            {#if selectedPerson}
                <div class="mb-4 flex items-center gap-4">
                    <button
                        type="button"
                        class="rounded bg-zinc-700 px-4 py-2 text-white hover:bg-zinc-600"
                        onclick={() => (selectedPerson = null)}
                        >Back to People</button
                    >
                    <h1 class="text-xl font-semibold">Person</h1>
                </div>
                {#if personLoading}
                    <p>Loading photos…</p>
                {:else if personError}
                    <p class="text-red-400">{personError}</p>
                {:else}
                    <GridView getPhotos={async () => personPhotos} />
                {/if}
            {:else if peopleLoading}
                <p>Loading people…</p>
            {:else if peopleError}
                <p class="text-red-400">{peopleError}</p>
            {:else if !people.length}
                <p>No people found yet</p>
            {:else}
                <GridView
                    initialItems={people.map(personThumbnail)}
                    showFullscreen={false}
                    itemLabels={Object.fromEntries(
                        people.map((person) => [
                            personThumbnail(person),
                            `${person.photo_count} photos`,
                        ]),
                    )}
                    onItemClick={(_, index) => openPerson(people[index])}
                />
            {/if}
        {:else}
            <div class="mb-4 flex items-center justify-between">
                <p>Library</p>
                <button
                    type="button"
                    class="rounded bg-blue-600 px-4 py-2 text-white hover:bg-blue-700"
                    onclick={() => (showImport = true)}>Import photos</button
                >
            </div>
            <GridView {getPhotos} />
        {/if}
    </div>
</div>
