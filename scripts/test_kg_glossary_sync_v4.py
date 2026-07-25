#!/usr/bin/env python3
"""
LightRAG glossary sync test script (v4 — test with wait).

Same as v3 but adds a pipeline_status poll between delete and insert
to see if we can detect when it's safe to insert again.
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
            print(f"  {json.dumps(body, indent=2)[:500]}")
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


def wait_for_pipeline_clear(max_wait=60):
    """Poll /documents/pipeline_status until destructive_busy is false."""
    start = time.time()
    while time.time() - start < max_wait:
        r, data = get("documents/pipeline_status")
        if r.status_code != 200:
            time.sleep(1)
            continue
        # The response structure may vary — print it so we can inspect
        if data:
            # Check various possible fields
            busy = data.get("busy", False)
            destructive = data.get("destructive_busy", False)
            scanning = data.get("scanning", False)
            pending = data.get("pending_enqueues", 0)
            print(f"  pipeline: busy={busy}, destructive_busy={destructive}, scanning={scanning}, pending={pending}")
            if not busy and not destructive and not scanning and pending == 0:
                print("  Pipeline is clear!")
                return True
        time.sleep(1)
    print("  Timeout waiting for pipeline clear")
    return False


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
        print("Delete returned status='busy'")
    print()

    # ── STEP 3: WAIT for pipeline to clear ───────────────────────────
    print("=== STEP 3: Wait for pipeline to clear ===")
    wait_for_pipeline_clear()
    print()

    # ── STEP 4: Re-insert after waiting ──────────────────────────────
    print("=== STEP 4: Re-insert glossary after waiting ===")
    ins2_r, ins2_data = insert_glossary("# Glossary v2\n\n- Alice: person, Team lead\n")
    if ins2_r.status_code == 409:
        print("Still got 409 even after waiting!")
        sys.exit(1)
    if ins2_r.status_code != 200:
        print(f"Re-insert failed: {ins2_r.status_code}")
        sys.exit(1)
    print()

    # ── STEP 5: Verify ───────────────────────────────────────────────
    track2 = ins2_data.get("track_id")
    print(f"=== STEP 5: Poll track_status for {track2} ===")
    final2 = poll_track_status(track2)
    failed = [d for d in final2.get("documents", []) if d.get("status", "").upper() == "FAILED"]
    if failed:
        print(f"FAILED: {failed}")
        sys.exit(1)
    processed = [d for d in final2.get("documents", []) if d.get("status", "").upper() == "PROCESSED"]
    print(f"SUCCESS: {len(processed)} document(s) PROCESSED")

    # Cleanup
    print("\n=== Cleanup ===")
    final_doc_id = processed[0]["id"]
    delete_by_doc_ids([final_doc_id])
    print("Done.")


if __name__ == "__main__":
    main()
