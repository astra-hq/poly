# Knowledge Graph Remote Deployment (Helm)

This guide explains how to use the `poly-kg` Helm chart to deploy the Knowledge Graph stack (LightRAG + Neo4j) to a Kubernetes cluster. The chart is in `charts/poly` and is a **deployment scaffold** it contains placeholder values for secrets and endpoints that you must replace before deploying to a real cluster.

> **Scope:** This chart is for remote LightRAG deployments. For local development with Docker Compose, see [LOCAL_SETUP.md](LOCAL_SETUP.md).

## Prerequisites

- Kubernetes 1.25+
- Helm 3.12+
- An external S3-compatible blob store (AWS S3, MinIO, Ceph RGW, etc.)
- (Optional) An existing Kubernetes Secret if you already manage credentials externally

## What This Chart Deploys

| Component | Kind | Notes |
|-----------|------|-------|
| LightRAG | Deployment + Service | Query API on port 9621 |
| Neo4j | StatefulSet + Service | Graph DB on ports 7687 (bolt) and 7474 (http) |
| ConfigMap | ConfigMap | LightRAG runtime config (S3 endpoint, Neo4j URL, embedding model) |
| Secret | Secret | Placeholder credentials (API key, Neo4j auth, S3 keys) |

> **Note:** The local `rustfs` S3-compatible service from `docker-compose.kg.yml` is intentionally excluded from this remote chart. Replace it with an external S3 bucket.

## Values Overview

The main configurable sections in `values.yaml` are:

| Section | Purpose |
|---------|---------|
| `s3` | External S3 endpoint, bucket, region, and credentials |
| `lightrag` | LightRAG image, replica count, API key, embedding model, resources |
| `neo4j` | Neo4j image, auth, persistence, resources |
| `ingress` | Ingress configuration (disabled by default) |

### S3 Placeholder Setup

Before deploying, replace the S3 placeholders in your custom values file:

```yaml
s3:
  endpoint: "https://s3.amazonaws.com"    # or your MinIO/Ceph endpoint
  bucket: "my-lightrag-bucket"
  region: "us-east-1"
  useSsl: true
  existingSecret: ""                      # set to an existing K8s secret name, OR
  existingSecretAccessKeyId: "YOUR_S3_ACCESS_KEY_ID"    # fill in here (not recommended for production)
  existingSecretSecretAccessKey: "YOUR_S3_SECRET_ACCESS_KEY"  # fill in here (not recommended for production)
```

**Production recommendation:** Create a Kubernetes Secret and reference it via `s3.existingSecret`:

```bash
kubectl create secret generic poly-kg-s3-secrets \
  --from-literal=access-key-id=YOUR_KEY \
  --from-literal=secret-access-key=YOUR_SECRET
```

Then set `s3.existingSecret: "poly-kg-s3-secrets"` in your values.

## Render (Validate Without Deploying)

Render the templates locally to validate correctness without deploying:

```bash
helm template poly-kg charts/poly \
  --values charts/poly/values.yaml \
  > /tmp/poly-kg-rendered.yaml
```

Verify the output contains LightRAG and Neo4j resources and no real secrets:

```bash
grep -c "kind:" /tmp/poly-kg-rendered.yaml
grep "CHANGE_ME" /tmp/poly-kg-rendered.yaml
```

## Install

```bash
# With default placeholder values (not functional — for validation only)
helm install poly-kg charts/poly

# With custom values file
helm install poly-kg charts/poly \
  --values my-custom-values.yaml

# With inline overrides
helm install poly-kg charts/poly \
  --set s3.endpoint=https://s3.amazonaws.com \
  --set s3.bucket=my-bucket \
  --set lightrag.apiKey.value=my-secure-key \
  --set neo4j.auth.password=my-neo4j-password
```

## Upgrade

```bash
helm upgrade poly-kg charts/poly \
  --values my-custom-values.yaml
```

## Uninstall and Reset

```bash
helm uninstall poly-kg
```

> **Warning:** PersistentVolumeClaims created by the Neo4j StatefulSet are **not** deleted by `helm uninstall`. To remove them:
>
> ```bash
> kubectl delete pvc -l app.kubernetes.io/instance=poly-kg
> ```

To do a full reset (uninstall and delete persistent data):

```bash
helm uninstall poly-kg
kubectl delete pvc -l app.kubernetes.io/instance=poly-kg
```

## Configuration Reference

See `charts/poly/values.yaml` for all configurable parameters. Key fields:

| Field | Default | Description |
|-------|---------|-------------|
| `s3.endpoint` | `CHANGE_ME_S3_ENDPOINT` | S3-compatible endpoint URL |
| `s3.bucket` | `CHANGE_ME_S3_BUCKET` | Bucket name for LightRAG storage |
| `s3.region` | `CHANGE_ME_S3_REGION` | S3 region |
| `s3.existingSecret` | `""` | Name of an existing K8s secret with S3 credentials |
| `lightrag.apiKey.value` | `CHANGE_ME_LIGHTRAG_API_KEY` | API key for LightRAG endpoint |
| `lightrag.embedding.model` | `bge-m3` | Embedding model identifier |
| `neo4j.auth.password` | `CHANGE_ME_NEO4J_PASSWORD` | Neo4j password |
| `neo4j.persistence.size` | `20Gi` | Data volume size |
| `ingress.enabled` | `false` | Ingress is disabled by default |

## Non-Goals

This chart is a **scaffold only**. It does **not**:

- Provision real S3 buckets or IAM roles
- Automate TLS certificate issuance
- Deploy to a cluster automatically
- Include the local `rustfs` blob storage service
- Provide production-ready network policies or pod security standards
- Include Insight Engine, live ingestion, or graph visualization services
- Document production ingress or TLS automation

For local development, use [LOCAL_SETUP.md](LOCAL_SETUP.md) instead.
