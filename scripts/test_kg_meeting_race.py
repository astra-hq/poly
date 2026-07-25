#!/usr/bin/env python3
"""
LightRAG meeting summary race condition test (no pipeline wait).

Same as test_kg_meeting_sync.py but SKIPS the pipeline_status wait
between delete and insert, to see if the 409 race reproduces.

This simulates a fast LLM call where the delete is still in progress.
"""

import os
import sys
import time
import json
import requests

LIGHTRAG_URL = os.environ.get("LIGHTRAG_URL", "http://localhost:9621").rstrip("/")
API_KEY = os.environ.get("LIGHTRAG_API_KEY", "")
MEETING_ID = "test-meeting-456"
TIMEOUT = 30


def headers():
    h = {"Content-Type": "application/json"}
    if API_KEY:
        h["X-API-Key"] = API_KEY
    return h


def get(path):
    url = f"{LIGHTRAG_URL}/{path}"
    r = requests.get(url, headers=headers(), timeout=TIMEOUT)
    if r.text:
        try:
            return r, r.json()
        except Exception:
            pass
    return r, None


def post(path, payload):
    url = f"{LIGHTRAG_URL}/{path}"
    r = requests.post(url, json=payload, headers=headers(), timeout=TIMEOUT)
    if r.text:
        try:
            return r, r.json()
        except Exception:
            pass
    return r, None


def delete(path, payload):
    url = f"{LIGHTRAG_URL}/{path}"
    r = requests.delete(url, json=payload, headers=headers(), timeout=TIMEOUT)
    if r.text:
        try:
            return r, r.json()
        except Exception:
            pass
    return r, None


def insert_summary(text, file_source):
    return post("documents/text", {"text": text, "file_source": file_source, "chunking": None})


def delete_by_doc_id(doc_id):
    return delete("documents/delete_document", {"doc_ids": [doc_id], "delete_file": False, "delete_llm_cache": False})


def poll_track_status(track_id, max_wait=120):
    start = time.time()
    while time.time() - start < max_wait:
        r, data = get(f"documents/track_status/{track_id}")
        if r.status_code == 200:
            docs = data.get("documents", [])
            if docs and all(d.get("status", "").upper() in ("PROCESSED", "FAILED") for d in docs):
                return data
        time.sleep(1)
    raise RuntimeError(f"Timeout")


def wait_for_document_deletion(track_id, doc_id, max_wait=120):
    start = time.time()
    while time.time() - start < max_wait:
        r, data = get(f"documents/track_status/{track_id}")
        if r.status_code == 200:
            if not any(d.get("id") == doc_id for d in data.get("documents", [])):
                return True
        elif r.status_code == 404:
            return True
        time.sleep(1)
    return False


def main():
    file_source = f"poly/meetings/{MEETING_ID}/summary.md"

    # Insert
    ins1_r, ins1_data = insert_summary("# Meeting Summary\n\nTest\n", file_source)
    if ins1_r.status_code != 200:
        sys.exit(1)
    track1 = ins1_data.get("track_id")
    final1 = poll_track_status(track1)
    doc_id = final1["documents"][0]["id"]

    # Delete
    delete_by_doc_id(doc_id)

    # Wait for document to disappear (as Rust does)
    wait_for_document_deletion(track1, doc_id)

    # DO NOT wait for pipeline — immediately insert (simulate fast LLM)
    print("Immediately re-inserting (no pipeline wait)...")
    ins2_r, _ = insert_summary("# Meeting Summary v2\n\nUpdated\n", file_source)
    if ins2_r.status_code == 409:
        print("\n*** RACE BUG REPRODUCED: 409 even after wait_for_document_deletion! ***")
        print("The Rust wait_for_document_deletion is NOT sufficient.")
        sys.exit(1)
    if ins2_r.status_code != 200:
        print(f"Re-insert failed: {ins2_r.status_code}")
        sys.exit(1)

    print("No 409 — the race did not occur this time (delete was fast enough)")

    # Cleanup
    final2 = poll_track_status(ins2_r.json().get("track_id"))
    delete_by_doc_id(final2["documents"][0]["id"])


if __name__ == "__main__":
    main()
