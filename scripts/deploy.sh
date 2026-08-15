#!/usr/bin/env bash
# ============================================================================
# CrowdfundX — full contract deployment + demo interaction script
#
# Deploys the CFX token and the factory (with the campaign wasm uploaded),
# then optionally runs a demo: mint → create campaign → contribute → stats.
# Writes every address and transaction hash to deployment.json and generates
# frontend/.env.local so `npm run dev` works immediately after.
#
# Requirements:
#   - stellar CLI (v27+)          https://developers.stellar.org/docs/tools/cli
#   - cargo + wasm32v1-none target
#
# Usage:
#   scripts/deploy.sh                          # generate a new testnet key
#   DEPLOYER_SECRET=S... scripts/deploy.sh     # use an existing key
#   NETWORK=mainnet DEMO=0 scripts/deploy.sh   # production, no demo txs
# ============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NETWORK="${NETWORK:-testnet}"
DEPLOYER="${DEPLOYER:-deployer}"
CREATION_FEE="${CREATION_FEE:-10}"
DEMO="${DEMO:-1}"
WASM_DIR="$ROOT/contracts/target/wasm32v1-none/release"

log()  { printf '\033[1;34m▸ %s\033[0m\n' "$*"; }
ok()   { printf '\033[1;32m✓ %s\033[0m\n' "$*"; }
fail() { printf '\033[1;31m✗ %s\033[0m\n' "$*" >&2; exit 1; }

command -v stellar >/dev/null 2>&1 || fail "stellar CLI not found — https://developers.stellar.org/docs/tools/cli"
command -v cargo >/dev/null 2>&1   || fail "cargo not found"
rustup target list --installed 2>/dev/null | grep -q wasm32v1-none \
  || fail "missing rust target — run: rustup target add wasm32v1-none"

# ---------------------------------------------------------------------------
# 1. Build
# ---------------------------------------------------------------------------
log "Building contracts (wasm32v1-none, release)"
(cd "$ROOT/contracts" && cargo build --target wasm32v1-none --release)

# ---------------------------------------------------------------------------
# 2. Identity + funding
# ---------------------------------------------------------------------------
if ! stellar keys ls 2>/dev/null | grep -q "^$DEPLOYER$"; then
  if [ -n "${DEPLOYER_SECRET:-}" ]; then
    log "Importing deployer identity from DEPLOYER_SECRET"
    stellar keys add "$DEPLOYER" --secret-key "$DEPLOYER_SECRET"
  else
    log "Generating new identity '$DEPLOYER'"
    stellar keys generate "$DEPLOYER"
  fi
fi
DEPLOYER_ADDR="$(stellar keys address "$DEPLOYER")"
ok "Deployer address: $DEPLOYER_ADDR"

log "Funding deployer on $NETWORK (friendbot — may already be funded)"
stellar keys fund "$DEPLOYER" --network "$NETWORK" 2>/dev/null \
  || log "Funding skipped (already funded or friendbot unavailable)"

# ---------------------------------------------------------------------------
# 3. Deploy token
# ---------------------------------------------------------------------------
log "Deploying CFX token"
TOKEN_ID="$(stellar contract deploy \
  --wasm "$WASM_DIR/cfx_token.wasm" \
  --source-account "$DEPLOYER" \
  --network "$NETWORK" \
  --alias cfx-token \
  -- --admin "$DEPLOYER_ADDR" --decimal 7 --name "CrowdfundX Token" --symbol CFX)"
ok "Token: $TOKEN_ID"

# ---------------------------------------------------------------------------
# 4. Upload campaign wasm + deploy factory
# ---------------------------------------------------------------------------
log "Uploading campaign wasm"
CAMPAIGN_WASM_HASH="$(stellar contract upload \
  --wasm "$WASM_DIR/cfx_campaign.wasm" \
  --source-account "$DEPLOYER" \
  --network "$NETWORK")"
ok "Campaign wasm hash: $CAMPAIGN_WASM_HASH"

log "Deploying factory (creation fee: $CREATION_FEE CFX)"
FACTORY_ID="$(stellar contract deploy \
  --wasm "$WASM_DIR/cfx_factory.wasm" \
  --source-account "$DEPLOYER" \
  --network "$NETWORK" \
  --alias cfx-factory \
  -- --admin "$DEPLOYER_ADDR" --campaign_wasm "$CAMPAIGN_WASM_HASH" --creation_fee "$CREATION_FEE")"
ok "Factory: $FACTORY_ID"

# ---------------------------------------------------------------------------
# 5. Demo interaction (mint → create campaign → contribute)
# ---------------------------------------------------------------------------
MINT_TX=""
CAMPAIGN_ID=""
CREATE_TX=""
CONTRIBUTE_TX=""

if [ "$DEMO" = "1" ]; then
  DEADLINE_UNIX="$(( $(date +%s) + 7 * 86400 ))"
  GOAL_RAW=10000000000        # 1000 CFX (7 decimals)
  MILESTONES_RAW='[5000000000,5000000000]'

  log "Demo: minting 1000 CFX to deployer"
  MINT_OUT="$(stellar contract invoke \
    --id "$TOKEN_ID" --source-account "$DEPLOYER" --network "$NETWORK" \
    -- mint --to "$DEPLOYER_ADDR" --amount "$GOAL_RAW")"
  MINT_TX="$(printf '%s\n' "$MINT_OUT" | grep -oE '[0-9a-f]{64}' | head -1 || true)"
  ok "Mint tx: ${MINT_TX:-n/a}"

  log "Demo: creating campaign 'Moonbase One' via the factory"
  CREATE_OUT="$(stellar contract invoke \
    --id "$FACTORY_ID" --source-account "$DEPLOYER" --network "$NETWORK" \
    -- create_campaign --creator "$DEPLOYER_ADDR" --name "Moonbase One" \
       --token "$TOKEN_ID" --goal "$GOAL_RAW" --deadline "$DEADLINE_UNIX" \
       --milestones "$MILESTONES_RAW")"
  CREATE_TX="$(printf '%s\n' "$CREATE_OUT" | grep -oE '[0-9a-f]{64}' | head -1 || true)"
  CAMPAIGN_ID="$(printf '%s\n' "$CREATE_OUT" | grep -oE 'C[0-9A-Z]{55}' | tail -1 || true)"
  ok "Create campaign tx: ${CREATE_TX:-n/a}"
  [ -n "$CAMPAIGN_ID" ] && ok "Campaign: $CAMPAIGN_ID"

  if [ -n "$CAMPAIGN_ID" ]; then
    log "Demo: contributing 1000 CFX (hits the goal)"
    CONTRIBUTE_OUT="$(stellar contract invoke \
      --id "$CAMPAIGN_ID" --source-account "$DEPLOYER" --network "$NETWORK" \
      -- contribute --from "$DEPLOYER_ADDR" --amount "$GOAL_RAW")"
    CONTRIBUTE_TX="$(printf '%s\n' "$CONTRIBUTE_OUT" | grep -oE '[0-9a-f]{64}' | head -1 || true)"
    ok "Contribute tx: ${CONTRIBUTE_TX:-n/a}"
  fi
fi

# ---------------------------------------------------------------------------
# 6. Artifacts
# ---------------------------------------------------------------------------
cat > "$ROOT/deployment.json" <<EOF
{
  "network": "$NETWORK",
  "deployer": "$DEPLOYER_ADDR",
  "token": "$TOKEN_ID",
  "factory": "$FACTORY_ID",
  "campaign_wasm_hash": "$CAMPAIGN_WASM_HASH",
  "creation_fee": "$CREATION_FEE",
  "demo_campaign": "$CAMPAIGN_ID",
  "transactions": {
    "mint": "$MINT_TX",
    "create_campaign": "$CREATE_TX",
    "contribute": "$CONTRIBUTE_TX"
  }
}
EOF
ok "Wrote deployment.json"

cat > "$ROOT/frontend/.env.local" <<EOF
VITE_NETWORK_PASSPHRASE=$(stellar network ls 2>/dev/null | grep -E "^\*? *$NETWORK " | awk '{print $2}' || echo "Test SDF Network ; September 2015")
VITE_RPC_URL=https://soroban-testnet.stellar.org
VITE_FACTORY_ADDRESS=$FACTORY_ID
VITE_TOKEN_ADDRESS=$TOKEN_ID
EOF
ok "Wrote frontend/.env.local"

printf '\n\033[1mDeployment summary\033[0m\n'
printf '  Token:    %s\n' "$TOKEN_ID"
printf '  Factory:  %s\n' "$FACTORY_ID"
printf '  Campaign: %s\n' "${CAMPAIGN_ID:-—}"
printf '  TXs:      %s\n' "$ROOT/deployment.json"
