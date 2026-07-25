#!/usr/bin/env python3
"""
LightRAG glossary sync test script (v3 — reproduce the actual race condition).

This version FIRST inserts a document, THEN deletes it, THEN immediately
re-inserts — mirroring what happens when a glossary already exists in the KG
and the user adds a new entry (which triggers delete+re-insert).

Usage:
    export LIGHTRAG_URL="http://localhost:9621"
    export LIGHTRAG_API_KEY="your-key"
    python3 test_kg_glossary_sync_v3.py
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
            # print(json.dumps(body, indent=2))  # Keep output shorter
            print(f"  (response body length: {len(str(body))})")
            return r, body
        except Exception:
            print(f"  {r.text[:200]}")
    return r, None


def post(path, payload):
    url = f"{LIGHTRAG_URL}/{path}"
    r = requests.post(url, json=payload, headers=headers(), timeout=TIMEOUT)
    print(f"POST {url} -> {r.status_code}")
    if r.text:
        try:
            body = r.json()
            print(f"  {json.dumps(body, indent=2)[:500]}")
            return r, body
        except Exception:
            print(f"  {r.text[:200]}")
    return r, None


def delete(path, payload):
    url = f"{LIGHTRAG_URL}/{path}"
    r = requests.delete(url, json=payload, headers=headers(), timeout=TIMEOUT)
    print(f"DELETE {url} -> {r.status_code}")
    if r.text:
        try:
            body = r.json()
            print(f"  {json.dumps(body, indent=2)[:500]}")
            return r, body
        except Exception:
            print(f"  {r.text[:200]}")
    return r, None


def delete_by_doc_ids(doc_ids):
    return delete("documents/delete_document", {
        "doc_ids": doc_ids,
        "delete_file": False,
        "delete_llm_cache": False,
    })


def find_document_by_file_source(file_source):
    r, data = get("documents")
    if r.status_code != 200:
        return None
    for status_key, docs in data.get("statuses", {}).items():
        for doc in docs:
            if doc.get("file_path") == file_source:
                print(f"  Found in status '{status_key}': id={doc.get('id')}")
                return doc.get("id")
    return None


def insert_glossary(text):
    return post("documents/text", {
        "text": text,
        "file_source": FILE_SOURCE,
        "chunking": None,
    })


def poll_track_status(track_id, max_wait=120):
    start = time.time()
    while time.time() - start < max_wait:
        r, data = get(f"documents/track_status/{track_id}")
        if r.status_code != 200:
            time.sleep(1)
            continue
        docs = data.get("documents", [])
        if not docs:
            time.sleep(1)
            continue
        statuses = [d.get("status", "").upper() for d in docs]
        if all(s in ("PROCESSED", "FAILED") for s in statuses):
            return data
        time.sleep(1)
    raise RuntimeError(f"Timeout polling track {track_id}")


def main():
    print(f"LightRAG URL: {LIGHTRAG_URL}")
    print()

    # ── STEP 1: Insert a document first ────────────────────────────────
    print("=== STEP 1: Insert initial glossary document ===")
    ins1_r, ins1_data = insert_glossary("# Glossary v1\n\n- Bob: person\n")
    if ins1_r.status_code != 200:
        print(f"Initial insert failed: {ins1_r.status_code}")
        sys.exit(1)
    track1 = ins1_data.get("track_id")
    print(f"Waiting for track {track1}...")
    final1 = poll_track_status(track1)
    doc_id = final1["documents"][0]["id"]
    print(f"Initial document processed: id={doc_id}")
    print()

    # ── STEP 2: Delete the existing document ─────────────────────────
    print("=== STEP 2: Delete existing document ===")
    print(f"Deleting by doc_id='{doc_id}'")
    del_r, del_data = delete_by_doc_ids([doc_id])
    if del_r.status_code != 200:
        print(f"Delete failed: {del_r.status_code}")
    elif del_data and del_data.get("status") == "busy":
        print("Delete returned status='busy' — pipeline is occupied")
    print()

    # ── STEP 3: Immediately re-insert (the actual bug scenario) ──────
    print("=== STEP 3: Immediately re-insert glossary ===")
    ins2_r, ins2_data = insert_glossary("# Glossary v2\n\n- Alice: person, Team lead\n")
    if ins2_r.status_code == 409:
        print("\n*** BUG REPRODUCED ***")
        print("409 Conflict on re-insert after delete of EXISTING document!")
        print("LightRAG pipeline is busy clearing documents.")
        sys.exit(1)
    if ins2_r.status_code != 200:
        print(f"Re-insert failed: {ins2_r.status_code}")
        sys.exit(1)
    print()

    # ── STEP 4: Verify the new document ──────────────────────────────
    track2 = ins2_data.get("track_id")
    print(f"=== STEP 4: Poll track_status for {track2} ===")
    final2 = poll_track_status(track2)
    failed = [d for d in final2.get("documents", []) if d.get("status", "").upper() == "FAILED"]
    if failed:
        print(f"FAILED: {failed}")
        sys.exit(1)
    processed = [d for d in final2.get("documents", []) if d.get("status", "").upper() == "PROCESSED"]
    print(f"SUCCESS: {len(processed)} document(s) PROCESSED")

    # ── Cleanup ──────────────────────────────────────────────────────
    print("\n=== Cleanup: delete test document ===")
    final_doc_id = processed[0]["id"]
    delete_by_doc_ids([final_doc_id])
    print("Done.")


if __name__ == "__main__":
    main()
