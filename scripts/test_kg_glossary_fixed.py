#!/usr/bin/env python3
"""
LightRAG glossary sync test (simulating the FIXED Rust logic).

This script simulates the fixed api_sync_glossary_to_knowledge_graph flow:
  1. Delete existing document by doc_id
  2. WAIT for pipeline_status to show destructive_busy=False
  3. Insert new document
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


def delete_by_doc_id(doc_id):
    return delete("documents/delete_document", {"doc_ids": [doc_id], "delete_file": False, "delete_llm_cache": False})


def insert_glossary(text):
    return post("documents/text", {"text": text, "file_source": FILE_SOURCE, "chunking": None})


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
    raise RuntimeError(f"Timeout")


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
                print("  Pipeline clear!")
                return True
        time.sleep(1)
    print("  Timeout waiting for pipeline clear")
    return False


def main():
    print(f"LightRAG URL: {LIGHTRAG_URL}")
    print()

    # Insert initial
    ins1_r, ins1_data = insert_glossary("# Glossary v1\n\n- Bob: person\n")
    if ins1_r.status_code != 200:
        sys.exit(1)
    track1 = ins1_data.get("track_id")
    final1 = poll_track_status(track1)
    doc_id = final1["documents"][0]["id"]
    print(f"Initial document: id={doc_id}")

    # Delete
    print("\n=== Delete ===")
    delete_by_doc_id(doc_id)

    # Wait for pipeline (THE FIX)
    print("\n=== Wait for pipeline clear ===")
    wait_for_pipeline_clear()

    # Re-insert
    print("\n=== Re-insert ===")
    ins2_r, ins2_data = insert_glossary("# Glossary v2\n\n- Alice: person, Team lead\n")
    if ins2_r.status_code == 409:
        print("\nBUG: Still got 409 after waiting!")
        sys.exit(1)
    if ins2_r.status_code != 200:
        print(f"Failed: {ins2_r.status_code}")
        sys.exit(1)

    # Verify
    track2 = ins2_data.get("track_id")
    final2 = poll_track_status(track2)
    processed = [d for d in final2.get("documents", []) if d.get("status", "").upper() == "PROCESSED"]
    print(f"\nSUCCESS: {len(processed)} document(s) PROCESSED")

    # Cleanup
    delete_by_doc_id(processed[0]["id"])


if __name__ == "__main__":
    main()
