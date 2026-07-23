# Glossary

The Poly glossary improves transcript accuracy and summary quality by teaching the app about your domain-specific terms, people, projects, and acronyms.

## How it works

- Glossary entries are injected as context into chunk and final summary prompts.
- They are **not** used for translation or combine prompts in the current version.
- An empty glossary means no extra context is injected — the app behaves as before.

## File location

The canonical glossary file is:

```
~/.poly/glossary.yml
```

- It is a plain YAML file with no secrets.
- If the file is missing or empty, the app treats it as an empty glossary.

## Entry schema

Each entry contains the following fields:

| Field | Required | Description |
|---|---|---|
| `term` | Yes | Canonical name (e.g. "Parakeet") |
| `id` | Yes | Stable entry ID. Older files without IDs are assigned IDs on load. |
| `kind` | Yes | One of: `person`, `team`, `project`, `code_name`, `component`, `acronym`, `other` |
| `aliases` | No | Comma-separated alternative names (e.g. "PK, Parakeet TDT") |
| `pronunciation` | No | Phonetic hint for transcription |
| `definition` | No | Short description |
| `notes` | No | Additional context for summarization |
| `references` | No | HTTP(S) links, one URL per line in the Settings UI |

## Knowledge Graph sync

- Saving glossary changes syncs changed entries to the active Knowledge Graph profile.
- Sync uses one Knowledge Graph document per entry, so adding one entry does not delete and reupload the whole glossary.
- If no active KG profile is configured, the sync is skipped with a clear reason in the UI.

## Safety semantics

- Glossary context is tagged as `<glossary_context>` in prompts with an interpretive note: the model must not introduce a glossary entry as a meeting fact unless the source text explicitly supports it.
- The summary cache is fingerprinted by glossary content, so adding or removing entries invalidates prior cached summaries.
