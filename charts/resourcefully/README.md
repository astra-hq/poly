# resourcefully-kg Helm Chart

Remote Knowledge Graph stack — **LightRAG** (retrieval-augmented generation API) + **Neo4j** (graph database) with **external S3-compatible blob storage** placeholders.

This chart is a deployment scaffold. It does **not** contain real credentials, production TLS/ingress automation, or the local `rustfs` service. It is designed to be customised before deploying to a Kubernetes cluster.

## Prerequisites

- Kubernetes 1.25+
- Helm 3.12+
- An external S3-compatible blob store (AWS S3, MinIO, Ceph RGW, etc.) — replace the `s3.*` placeholders in `values.yaml`
- (Optional) An existing Kubernetes Secret if you already manage credentials externally

## What this chart deploys

| Component | Kind | Notes |
|-----------|------|-------|
| LightRAG | Deployment + Service | Query API on port 9621 |
| Neo4j | StatefulSet + Service | Graph DB on ports 7687 (bolt) and 7474 (http) |
| ConfigMap | ConfigMap | LightRAG runtime config (S3 endpoint, Neo4j URL, embedding model) |
| Secret | Secret | Placeholder credentials (API key, Neo4j auth, S3 keys) |

> **Note:** The local `rustfs` S3-compatible service from `docker-compose.kg.yml` is intentionally excluded from this remote chart. Replace it with an external S3 bucket.

## Excluded by design

- `rustfs` workload (use external S3)
- Production TLS/ingress automation (ingress is disabled by default)
- Production secrets/credentials
- Live Insight Engine services (not implemented)
- Cluster deployment scripts

## Quick start (render only)

Render the templates locally to validate correctness without deploying:

```bash
helm template resourcefully-kg charts/resourcefully-kg \
  --values charts/resourcefully-kg/values.yaml \
  > /tmp/resourcefully-kg-rendered.yaml
```

Verify the output contains LightRAG and Neo4j resources and no real secrets:

```bash
grep -c "kind:" /tmp/resourcefully-kg-rendered.yaml
grep "CHANGE_ME" /tmp/resourcefully-kg-rendered.yaml
```

## S3 placeholder setup

Before deploying, replace the S3 placeholders in `values.yaml`:

```yaml
s3:
  endpoint: "https://s3.amazonaws.com"    # or your MinIO/Ceph endpoint
  bucket: "my-lightrag-bucket"
  region: "us-east-1"
  useSsl: true
  existingSecret: ""                      # set to an existing K8s secret name, OR
  existingSecretAccessKeyId: "AKIA..."    # fill in here (NOT recommended for production)
  existingSecretSecretAccessKey: "wJalr..." # fill in here (NOT recommended for production)
```

**Production recommendation:** Create a Kubernetes Secret and reference it via `s3.existingSecret`:

```bash
kubectl create secret generic resourcefully-kg-s3-secrets \
  --from-literal=access-key-id=YOUR_KEY \
  --from-literal=secret-access-key=YOUR_SECRET
```

Then set `s3.existingSecret: "resourcefully-kg-s3-secrets"` in your values.

## Install

```bash
# With default placeholder values (not functional)
helm install resourcefully-kg charts/resourcefully-kg

# With custom values file
helm install resourcefully-kg charts/resourcefully-kg \
  --values my-custom-values.yaml

# With inline overrides
helm install resourcefully-kg charts/resourcefully-kg \
  --set s3.endpoint=https://s3.amazonaws.com \
  --set s3.bucket=my-bucket \
  --set lightrag.apiKey.value=my-secure-key \
  --set neo4j.auth.password=my-neo4j-password
```

## Upgrade

```bash
helm upgrade resourcefully-kg charts/resourcefully-kg \
  --values my-custom-values.yaml
```

## Uninstall

```bash
helm uninstall resourcefully-kg
```

> **Note:** PersistentVolumeClaims created by the Neo4j StatefulSet are **not** deleted by `helm uninstall`. To remove them:
> ```bash
> kubectl delete pvc -l app.kubernetes.io/instance=resourcefully-kg
> ```

## Configuration reference

See `values.yaml` for all configurable parameters. Key sections:

| Section | Purpose |
|---------|---------|
| `s3` | External S3 endpoint, bucket, region, credentials |
| `lightrag` | LightRAG image, resources, API key, embedding model |
| `neo4j` | Neo4j image, resources, auth, persistence |
| `ingress` | Ingress configuration (disabled by default) |

## Non-goals (Week 2)

This chart is a **scaffold only**. It does not:

- Provision real S3 buckets or IAM roles
- Automate TLS certificate issuance
- Deploy to a cluster automatically
- Include the local `rustfs` blob storage service
- Provide production-ready network policies or pod security standards
- Include Insight Engine, live ingestion, or graph visualization services
