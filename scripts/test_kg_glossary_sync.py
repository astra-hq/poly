#!/usr/bin/env python3
"""
LightRAG glossary sync test script (v2 — corrected API contract).

Mirrors the Rust logic in api_sync_glossary_to_knowledge_graph to surface
race-condition / sequencing bugs with the live LightRAG endpoint.

Usage:
    export LIGHTRAG_URL="http://localhost:9621"
    export LIGHTRAG_API_KEY="your-key"   # optional
    python3 test_kg_glossary_sync.py

What it does:
    1. Query all documents via GET /documents, looking for file_path == "poly/glossary.yml"
    2. If found, delete by doc_id via DELETE /documents/delete_document
       If NOT found, STILL try delete with doc_id = "poly/glossary.yml"
       (this mirrors the current Rust fallback logic)
    3. Immediately POST /documents/text to re-insert
    4. Poll GET /documents/track_status/{track_id} until final

Expected bug to surface:
    When no document exists, step 2 triggers a pipeline job. Step 3 races
    into it and returns 409 Conflict:
    "Pipeline is clearing or deleting documents..."
"""

import os
import sys
import time
import json
import requests

LIGHTRAG_URL = os.environ.get("LIGHTRAG_URL", "http://localhost:9621").rstrip("/")
API_KEY = os.environ.get("LIGHTRAG_API_KEY", "")
FILE_SOURCE = "poly/glossary.yml"
TIMEOUT = 30


def headers():
    h = {"Content-Type": "application/json"}
    if API_KEY:
        h["X-API-Key"] = API_KEY
    return h


def get(path):
    url = f"{LIGHTRAG_URL}/{path}"
    r = requests.get(url, headers=headers(), timeout=TIMEOUT)
    print(f"GET {url} -> {r.status_code}")
    if r.text:
        try:
            body = r.json()
            print(json.dumps(body, indent=2))
            return r, body
        except Exception:
            print(r.text)
    return r, None


def post(path, payload):
    url = f"{LIGHTRAG_URL}/{path}"
    r = requests.post(url, json=payload, headers=headers(), timeout=TIMEOUT)
    print(f"POST {url} -> {r.status_code}")
    if r.text:
        try:
            body = r.json()
            print(json.dumps(body, indent=2))
            return r, body
        except Exception:
            print(r.text)
    return r, None


def delete(path, payload):
    url = f"{LIGHTRAG_URL}/{path}"
    r = requests.delete(url, json=payload, headers=headers(), timeout=TIMEOUT)
    print(f"DELETE {url} -> {r.status_code}")
    if r.text:
        try:
            body = r.json()
            print(json.dumps(body, indent=2))
            return r, body
        except Exception:
            print(r.text)
    return r, None


def delete_by_doc_ids(doc_ids):
    """Mirrors Rust delete_by_doc_ids logic exactly."""
    return delete("documents/delete_document", {
        "doc_ids": doc_ids,
        "delete_file": False,
        "delete_llm_cache": False,
    })


def find_document_by_file_source(file_source):
    """Mirrors Rust delete_by_file_source query logic (GET /documents)."""
    r, data = get("documents")
    if r.status_code != 200:
        print(f"Document query failed: {r.status_code}")
        return None
    for status_key, docs in data.get("statuses", {}).items():
        for doc in docs:
            if doc.get("file_path") == file_source:
                print(f"Found document in status '{status_key}': id={doc.get('id')}")
                return doc.get("id")
    print(f"No document found with file_path='{file_source}'")
    return None


def insert_glossary(text):
    """Mirrors Rust insert_text logic with corrected field name (file_source not source)."""
    return post("documents/text", {
        "text": text,
        "file_source": FILE_SOURCE,
        "chunking": None,
    })


def poll_track_status(track_id, max_wait=120):
    """Poll /documents/track_status/{track_id} until PROCESSED or FAILED."""
    start = time.time()
    while time.time() - start < max_wait:
        r, data = get(f"documents/track_status/{track_id}")
        if r.status_code != 200:
            print(f"Track status poll failed: {r.status_code}")
            time.sleep(1)
            continue
        docs = data.get("documents", [])
        if not docs:
            print("Track has no documents yet — continuing to poll")
            time.sleep(1)
            continue
        statuses = [d.get("status", "").upper() for d in docs]
        print(f"Document statuses: {statuses}")
        if all(s in ("PROCESSED", "FAILED") for s in statuses):
            return data
        time.sleep(1)
    raise RuntimeError(f"Timeout polling track {track_id}")


def main():
    print(f"LightRAG URL: {LIGHTRAG_URL}")
    print(f"API Key present: {'yes' if API_KEY else 'no'}")
    print()

    # ── STEP 1: See what is already there ────────────────────────────
    print("=== STEP 1: Query existing documents ===")
    existing_doc_id = find_document_by_file_source(FILE_SOURCE)
    print()

    # ── STEP 2: Delete (mirrors Rust exactly) ──────────────────────────
    print("=== STEP 2: Delete existing glossary document ===")
    if existing_doc_id:
        doc_id = existing_doc_id
        print(f"Found existing document, deleting by doc_id='{doc_id}'")
    else:
        # THIS IS THE RUST FALLBACK BEHAVIOUR
        doc_id = FILE_SOURCE
        print(f"No existing document — falling back to delete_by_doc_ids(['{doc_id}'])")

    del_r, del_data = delete_by_doc_ids([doc_id])
    if del_r.status_code != 200:
        print(f"Delete returned non-200: {del_r.status_code}")
    elif del_data and del_data.get("status") == "busy":
        print(f"Delete returned status='busy' — pipeline is occupied")
    print()

    # ── STEP 3: Immediately re-insert (the bug) ────────────────────────
    print("=== STEP 3: Immediately re-insert glossary ===")
    glossary_text = "# Glossary\n\n- Alice: person, Team lead\n"
    ins_r, ins_data = insert_glossary(glossary_text)
    if ins_r.status_code == 409:
        print("\nBUG REPRODUCED: 409 Conflict on re-insert after delete!")
        print("LightRAG pipeline is busy clearing documents.")
        sys.exit(1)
    if ins_r.status_code != 200:
        print(f"Insert returned non-200: {ins_r.status_code}")
        sys.exit(1)
    print()

    # ── STEP 4: Poll track status ──────────────────────────────────────
    track_id = ins_data.get("track_id") if ins_data else None
    if not track_id:
        print("No track_id in insert response")
        sys.exit(1)
    print(f"=== STEP 4: Poll track_status for {track_id} ===")
    final = poll_track_status(track_id)
    print()

    # ── Summary ───────────────────────────────────────────────────────
    failed = [d for d in final.get("documents", []) if d.get("status", "").upper() == "FAILED"]
    if failed:
        print(f"FAILED documents: {failed}")
        sys.exit(1)
    processed = [d for d in final.get("documents", []) if d.get("status", "").upper() == "PROCESSED"]
    print(f"SUCCESS: {len(processed)} document(s) PROCESSED")


if __name__ == "__main__":
    main()
