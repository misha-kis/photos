# Photos App — a desktop Photo Library app \[work in progress 🏗️\]

Photos App is a desktop application for managing and viewing your photo library. Mac's Photos app doesn't analyze the libraries if they are on external drives. To solve this problem, I decided to create my own photo library app that can analyze and manage photos anywhere you wish.

It is now in a rather tech demo state and supports:

- having a library;
- importing images into the library;
- background facial detection, embedding and clustering.

### Demo

https://github.com/user-attachments/assets/e6478503-4f33-490a-887a-e8d54df8a0de

# Building and Running

> Mind you that the project is developed for MacOS and _doesn't support_ Linux or Windows. The part specific to MacOS is face detection, which uses Apple's proprietary face detection API. While I plan to implement a cross-platform detector module as well, currently it isn't my priority.

To build and run the Photos App, you need to have [Rust](https://rustup.rs/) and [Bun](https://bun.com/docs/installation).

To run the app, run:

```bash
git clone https://github.com/misha-kis/photos.git
bun i
bun run tauri dev
```

After that, you will be able to select a directory for your first photo library and start using the app.

# Architecture

The app uses hexagonal architecture, where Application (crates/photos-app) is the core, and it depends on and wires together other components: Filesystem Image Repository (crates/photos-infra-fs-repository), Metadata Repository (crates/photos-infra-sqlite-image-metadata-repository), CV Services (crates/photos-infra-cv), etc.

For the UI, I used Svelte. The previous version used `egui`, I enjoyed working with it but found Svelte + Tauri to be easier to maintain and work with.

Here is the component diagram of the app:

![Photos App component diagram](assets/components.svg)

### Task management

First of all, the app needs to do a lot of different things: render the thumbnails and originals, import photos, analyze photos in the background, aggregate metadata, etc. All of that needs to be done in a way, that keeps the UI responsive, that allows to add new features, and that allows to use parallelization effectively.

At first, I thought about processing background and UI-related tasks separately to avoid handling task priorities, but I figured out that I would need one anyway, since even background tasks can have different priorities (for example, import > background analysis).

So I decided to stick with a common task system for everything that the app does. The task system is based on three task queues for different priority levels. The diagram of the task system is shown below:

![Task Queue](assets/task_queue.svg)

This system handles different priority levels and task parallelization. Now to make implementing new features easier, I added a trait system, which allows to define new task types easily.

First, there are two foundational traits: `OneshotDispatchable` and `Dispatchable`. The former is used for UI tasks, which have a result that needs to be sent back to the UI, while the latter is used for background tasks, which don't have an immediate UI result.

Then, we have complex pipelines of tasks, for example, when analyzing the images, we first need to know, which images to analyze, then process them, and probably aggregate the results. Or spin up another job, for example, a different stage of processing. This is why the Expand-Map-Reduce system was born.

> I don't know, how real developers call this, so I ended up with this term

The basis is like that:

1. `Expand<I, O>`: one `I` → many `O`
1. `Map<I, O>`: one `I` → one `O`
1. `Reduce<I, O>`: many `I` → one `O`

Thus, a chain of Expand, Map, and Reduce makes a "one `I` → one `O`" task too.

`Map<I, O>` implements `OneshotDispatchable<I, O>` to allow for UI tasks with `Map` trait.

`ExpandMapReduce<I, M1, M2, ()>` implements `Dispatchable<I, ()>` for dispatching mass processing jobs. To allow job chaining, `(Arc<J1>, Arc<J2>) where J1: Dispatchable<I, ()>, J2: Dispatchable<(), ()>` implements `Dispatchable<I, ()>` too.

![Task Trait System](assets/task_traits.svg)

### ML

Right now the app supports face detection and creating embeddings for faces. This allows to make clusters of faces and add make photo collections based on the people in the pictures.

The current version uses

- a proprietary Apple on-device model for face detection
- [facenet](https://github.com/davidsandberg/facenet) for creating the embeddings
- HDBSCAN for clustering

## Core Libraries

| Library      | Purpose                                       |
| ------------ | --------------------------------------------- |
| Tauri        | Binding Backend & Frontend                    |
| Tokio        | Workflows and stuff                           |
| Svelte       | Frontend                                      |
| apple-vision | Crate exposing Apple face detection model API |
| ort          | ONNX runtime for Rust                         |
| sqlx         | SQL for Rust                                  |
