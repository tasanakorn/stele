"""Bidirectional sync between two Stele server profiles."""

import json
import sys
import urllib.request
import urllib.error


def fetch_all_memories(base_url):
    """Fetch all memories from a server."""
    url = f"{base_url}/api/v1/memories?limit=1000"
    req = urllib.request.Request(url)
    with urllib.request.urlopen(req) as resp:
        return json.loads(resp.read())


def fetch_all_entities(base_url):
    """Fetch all entities via graph read for all scopes."""
    # First get all scopes
    url = f"{base_url}/api/v1/scopes"
    req = urllib.request.Request(url)
    with urllib.request.urlopen(req) as resp:
        scopes = json.loads(resp.read())

    all_entities = []
    all_relations = []
    seen_scopes = set()

    for scope_info in scopes:
        scope = scope_info["scope"]
        if scope in seen_scopes:
            continue
        seen_scopes.add(scope)
        url = f"{base_url}/api/v1/graph?scope={urllib.parse.quote(scope)}"
        req = urllib.request.Request(url)
        try:
            with urllib.request.urlopen(req) as resp:
                graph = json.loads(resp.read())
                all_entities.extend(graph.get("entities", []))
                all_relations.extend(graph.get("relations", []))
        except urllib.error.HTTPError:
            pass

    # Deduplicate by entity name+scope
    unique_entities = {}
    for e in all_entities:
        key = (e["name"], e["scope"])
        if key not in unique_entities:
            unique_entities[key] = e

    # Deduplicate relations
    unique_relations = {}
    for r in all_relations:
        key = (r["from_entity"], r["to_entity"], r["relation_type"])
        if key not in unique_relations:
            unique_relations[key] = r

    return list(unique_entities.values()), list(unique_relations.values())


def create_memory(base_url, memory):
    """Create a memory on target server."""
    url = f"{base_url}/api/v1/memories"
    payload = {
        "title": memory["title"],
        "content": memory["content"],
        "scope": memory["scope"],
        "memory_type": memory.get("memory_type", "knowledge"),
        "tags": memory.get("tags", []),
    }
    if memory.get("author"):
        payload["author"] = memory["author"]
    data = json.dumps(payload).encode()
    req = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req) as resp:
        return json.loads(resp.read())


def create_entities(base_url, entities):
    """Create entities on target server, one at a time to handle errors gracefully."""
    if not entities:
        return
    url = f"{base_url}/api/v1/graph/entities"
    created = 0
    for e in entities:
        payload = {
            "entities": [
                {
                    "name": e["name"],
                    "entity_type": e["entity_type"],
                    "scope": e["scope"],
                    "observations": [o["content"] for o in e.get("observations", [])],
                }
            ]
        }
        data = json.dumps(payload).encode()
        req = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(req) as resp:
                resp.read()
                created += 1
        except urllib.error.HTTPError as ex:
            print(f"    Warning: failed to create entity '{e['name']}' ({ex.code})")
    return created


def create_relations(base_url, relations):
    """Create relations on target server."""
    if not relations:
        return
    url = f"{base_url}/api/v1/graph/relations"
    payload = {
        "relations": [
            {
                "from_entity": r["from_entity"],
                "to_entity": r["to_entity"],
                "relation_type": r["relation_type"],
                "scope": r.get("scope", ""),
            }
            for r in relations
        ]
    }
    data = json.dumps(payload).encode()
    req = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req) as resp:
            return json.loads(resp.read())
    except urllib.error.HTTPError:
        # Relations may fail if entities don't exist on target yet
        pass


def sync_memories(source_url, target_url, source_name, target_name):
    """Sync memories from source to target, skipping duplicates by title+scope."""
    source_memories = fetch_all_memories(source_url)
    target_memories = fetch_all_memories(target_url)

    # Index target by (title, scope) to detect duplicates
    target_keys = {(m["title"], m["scope"]) for m in target_memories}

    new_memories = [
        m for m in source_memories if (m["title"], m["scope"]) not in target_keys
    ]

    for m in new_memories:
        create_memory(target_url, m)

    print(f"  {source_name} -> {target_name}: {len(new_memories)} memories synced ({len(source_memories) - len(new_memories)} already existed)")
    return len(new_memories)


def sync_graph(source_url, target_url, source_name, target_name):
    """Sync knowledge graph from source to target."""
    source_entities, source_relations = fetch_all_entities(source_url)
    target_entities, target_relations = fetch_all_entities(target_url)

    target_entity_keys = {(e["name"], e["scope"]) for e in target_entities}
    target_relation_keys = {
        (r["from_entity"], r["to_entity"], r["relation_type"]) for r in target_relations
    }

    new_entities = [
        e for e in source_entities if (e["name"], e["scope"]) not in target_entity_keys
    ]
    new_relations = [
        r for r in source_relations
        if (r["from_entity"], r["to_entity"], r["relation_type"]) not in target_relation_keys
    ]

    if new_entities:
        create_entities(target_url, new_entities)
    if new_relations:
        create_relations(target_url, new_relations)

    print(f"  {source_name} -> {target_name}: {len(new_entities)} entities, {len(new_relations)} relations synced")
    return len(new_entities) + len(new_relations)


def main():
    if len(sys.argv) != 5:
        print(f"Usage: {sys.argv[0]} <name1> <url1> <name2> <url2>")
        sys.exit(1)

    name1, url1, name2, url2 = sys.argv[1], sys.argv[2].rstrip("/"), sys.argv[3], sys.argv[4].rstrip("/")

    print(f"Syncing memories...")
    sync_memories(url1, url2, name1, name2)
    sync_memories(url2, url1, name2, name1)

    print(f"Syncing knowledge graph...")
    sync_graph(url1, url2, name1, name2)
    sync_graph(url2, url1, name2, name1)

    print("Done.")


if __name__ == "__main__":
    import urllib.parse
    main()
