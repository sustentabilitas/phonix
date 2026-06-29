# Deploying phonix-recall to GKE

Template manifests for running [`phonix-recall`](../../crates/phonix-recall) behind a
GKE HTTP(S) Load Balancer with a managed TLS cert, so Recall.ai can reach it over
`wss://`.

| File | What it creates |
|---|---|
| `deployment.yaml` | The `phonix-recall` Deployment (2 replicas, `/healthz` probes, resources) |
| `service.yaml` | A `ClusterIP` Service **+ a `BackendConfig`** that raises the LB timeout |
| `ingress.yaml` | A GKE `Ingress` **+ `ManagedCertificate`** terminating TLS |

## The one thing not to skip

GKE's HTTP(S) LB default backend timeout is **30 s** and will silently drop a
long-lived WebSocket mid-meeting. `service.yaml` sets `BackendConfig.timeoutSec: 3600`
to prevent that. (phonix-recall also answers WS pings, but the LB timeout is the real
killer.)

## Image

`deployment.yaml` defaults to the published image `sustentabilitas/phonix-recall`,
built and pushed by CI: `:X.Y.Z` + `:latest` on each `v*` release tag, and a rolling
`:edge` on every push to `main`. Pin to `:X.Y.Z` for production. To build your own
instead, run `./crates/phonix/models/fetch.sh` first (so the model is in the build
context), then `docker build -f crates/phonix-recall/Dockerfile -t <your-image> .` and
update the manifest.

## Steps

```bash
# 1. Edit the placeholders:
#    ingress.yaml → domain (x2); deployment.yaml → image (only if not using the published one)

# 2. Apply
kubectl apply -f scripts/k8s/deployment.yaml
kubectl apply -f scripts/k8s/service.yaml
kubectl apply -f scripts/k8s/ingress.yaml

# 3. Get the ingress IP, point your domain's DNS at it, then wait for the cert
kubectl get ingress phonix-recall -w
kubectl describe managedcertificate phonix-recall-cert   # wait for status: Active
```

Once the cert is `Active`, your endpoint is `wss://<your-domain>/ws`. Send a bot at it:

```bash
scripts/recall/join.sh "<teams_meeting_url>" "wss://<your-domain>/ws"
```

## Notes

- A custom wake model (`PHONIX_WAKE_MODEL`): bake it into the image alongside
  `silero_vad.onnx`, or mount it via a volume, and uncomment the env in `deployment.yaml`.
- Memory scales with concurrent participants (each loads its own Silero VAD model) —
  tune `resources` and `replicas` for your expected concurrency.
- Reserve a global static IP and reference it in `ingress.yaml` so DNS stays stable
  across re-creates.
