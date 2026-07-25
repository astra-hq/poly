#!/usr/bin/env python3
"""
LightRAG meeting summary delete+ingest test script.

Tests the meeting summary regeneration flow:
  1. Insert a summary document
  2. Delete it (mirrors api_delete_summary_from_knowledge_graph)
  3. Wait for deletion confirmation (mirrors wait_for_document_deletion)
  4. Immediately re-insert (mirrors api_ingest_summary_to_knowledge_graph)

This reproduces the race condition that exists in the meeting summary flow
when the LLM call is fast and the delete is still in progress.
"""

import os
import sys
import time
import json
import requests

LIGHTRAG_URL = os.environ.get("LIGHTRAG_URL", "http://localhost:9621").rstrip("/")
API_KEY = os.environ.get("LIGHTRAG_API_KEY", "")
MEETING_ID = "test-meeting-123"
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


def insert_summary(text, file_source):
    return post("documents/text", {
        "text": text,
        "file_source": file_source,
        "chunking": None,
    })


def delete_by_doc_id(doc_id):
    return delete("documents/delete_document", {
        "doc_ids": [doc_id],
        "delete_file": False,
        "delete_llm_cache": False,
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


def wait_for_document_deletion(track_id, doc_id, max_wait=120):
    """Mirrors Rust wait_for_document_deletion logic."""
    start = time.time()
    while time.time() - start < max_wait:
        r, data = get(f"documents/track_status/{track_id}")
        if r.status_code == 200:
            still_exists = any(d.get("id") == doc_id for d in data.get("documents", []))
            if not still_exists:
                print(f"  Document {doc_id} no longer in track — deletion confirmed")
                return True
        elif r.status_code == 404:
            print(f"  Track {track_id} returned 404 — deletion confirmed")
            return True
        time.sleep(1)
    print(f"  Timeout waiting for document {doc_id} deletion")
    return False


def wait_for_pipeline_clear(max_wait=60):
    start = time.time()
    while time.time() - start < max_wait:
        r, data = get("documents/pipeline_status")
        if r.status_code == 200 and data:
            busy = data.get("busy", False)
            destructive = data.get("destructive_busy", False)
            scanning = data.get("scanning_exclusive", False)
            pending = data.get("pending_enqueues", 0)
            print(f"  pipeline: busy={busy}, destructive_busy={destructive}, scanning_exclusive={scanning}, pending={pending}")
            if not busy and not destructive and not scanning and pending == 0:
                print("  Pipeline is clear!")
                return True
        time.sleep(1)
    return False


def main():
    print(f"LightRAG URL: {LIGHTRAG_URL}")
    print()

    file_source = f"poly/meetings/{MEETING_ID}/summary.md"

    # ── STEP 1: Insert initial summary ───────────────────────────────
    print("=== STEP 1: Insert initial meeting summary ===")
    ins1_r, ins1_data = insert_summary("# Meeting Summary\n\nTest meeting content\n", file_source)
    if ins1_r.status_code != 200:
        print(f"Initial insert failed: {ins1_r.status_code}")
        sys.exit(1)
    track1 = ins1_data.get("track_id")
    print(f"Waiting for track {track1}...")
    final1 = poll_track_status(track1)
    doc_id = final1["documents"][0]["id"]
    print(f"Initial document processed: id={doc_id}")
    print()

    # ── STEP 2: Delete by doc_id (as Rust does) ──────────────────────
    print("=== STEP 2: Delete summary by doc_id ===")
    del_r, del_data = delete_by_doc_id(doc_id)
    if del_r.status_code != 200:
        print(f"Delete failed: {del_r.status_code}")
        sys.exit(1)
    print()

    # ── STEP 3: Wait for document deletion (as Rust does) ──────────
    print("=== STEP 3: Wait for document deletion confirmation ===")
    wait_for_document_deletion(track1, doc_id)
    print()

    # ── STEP 4: Check pipeline status ────────────────────────────────
    print("=== STEP 4: Check pipeline status after deletion ===")
    wait_for_pipeline_clear()
    print()

    # ── STEP 5: Re-insert summary ──────────────────────────────────
    print("=== STEP 5: Re-insert meeting summary ===")
    ins2_r, ins2_data = insert_summary("# Meeting Summary v2\n\nUpdated content\n", file_source)
    if ins2_r.status_code == 409:
        print("\n*** BUG: 409 Conflict on re-insert after meeting summary delete! ***")
        sys.exit(1)
    if ins2_r.status_code != 200:
        print(f"Re-insert failed: {ins2_r.status_code}")
        sys.exit(1)
    print()

    # ── STEP 6: Verify ───────────────────────────────────────────────
    track2 = ins2_data.get("track_id")
    print(f"=== STEP 6: Poll track_status for {track2} ===")
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
    delete_by_doc_id(final_doc_id)
    print("Done.")


if __name__ == "__main__":
    main()
