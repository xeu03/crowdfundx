# Deployment Runbook

## Local / manual deployment

### Prerequisites

- [Stellar CLI](https://developers.stellar.org/docs/tools/cli) (v27+)
- Rust toolchain + `rustup target add wasm32v1-none`
- A funded account (testnet friendbot covers this automatically)

### One command

```bash
scripts/deploy.sh
```

What it does:

1. Builds all three contracts (`wasm32v1-none`, release)
2. Generates (or imports, via `DEPLOYER_SECRET`) the deployer identity and
   funds it from friendbot
3. Deploys the CFX token with the deployer as admin
4. Uploads the campaign wasm (`contract upload`) and deploys the factory
   configured with that wasm hash + creation fee
5. Runs the demo interaction: mint 1000 CFX → create campaign
   "Moonbase One" (2× 500 CFX milestones) → contribute 1000 CFX
6. Writes `deployment.json` (addresses + transaction hashes) and
   `frontend/.env.local`

### Verify the deployment

```bash
# Read factory stats (simulated view call)
stellar contract invoke --id <factory> --send=no --network testnet -- get_stats

# Stream live events (optional jq)
stellar events --start-ledger 1 --contract <factory> --network testnet
```

### Mainnet

```bash
DEPLOYER_SECRET=S… NETWORK=mainnet DEMO=0 scripts/deploy.sh
```

The script reads the network's RPC URL and passphrase from the Stellar CLI
network config (configure with `stellar network add`).

## CI/CD deployment (GitHub Actions)

`.github/workflows/deploy.yml` runs on `workflow_dispatch` with a
`testnet`/`mainnet` choice:

1. **contracts job** — installs the Stellar CLI, imports the deployer key from
   the `DEPLOYER_SECRET` repository secret, runs `scripts/deploy.sh`, and
   publishes every address + tx hash to the job summary.
2. **frontend job** — downloads `deployment.json`, builds the frontend with
   `VITE_FACTORY_ADDRESS` / `VITE_TOKEN_ADDRESS` from the fresh deployment,
   and publishes to Netlify.

Required repository secrets:

| Secret | Purpose |
| --- | --- |
| `DEPLOYER_SECRET` | Stellar secret key (S…) of the deployer account |
| `NETLIFY_AUTH_TOKEN` | Netlify personal access token |
| `NETLIFY_SITE_ID` | Target Netlify site |

## Frontend hosting notes

- The app uses **hash routing** — no SPA rewrite rules needed on any static host.
- Build-time env vars (`VITE_*`) are embedded in the bundle; change
  `.env.local` / Netlify build settings and redeploy to switch networks.
- CORS: `soroban-testnet.stellar.org` RPC allows browser origins. For a
  self-hosted RPC, allow your domain.

## Post-deployment checklist

- [ ] `deployment.json` written with token/factory/campaign addresses + tx hashes
- [ ] Frontend loads the Explore page and shows the demo campaign
- [ ] Contribution from a second account shows up in the live event feed
- [ ] Creator releases a milestone; payout appears in the event feed
- [ ] `stellar contract invoke … get_stats` shows the platform total
