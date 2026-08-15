# CrowdfundX Architecture

## Contract layer

Three Soroban contracts (soroban-sdk 27, `wasm32v1-none` target), one Rust
workspace.

### 1. `cfx-token` — platform token

SEP-41-style fungible token, 7 decimals, symbol `CFX`.

- `transfer` / `transfer_from` / `approve` / `allowance` with expiring
  allowances (zeroed allowances are removed from storage)
- admin-only `mint` / `burn` (the demo faucet flow uses mint)
- persistent storage with TTL bumping on every read/write
- events: `token/transfer`, `token/mint`, `token/burn`, `token/approve`,
  `token/set_admin`

### 2. `cfx-campaign` — all-or-nothing campaign

Deployed **only** by the factory (`deploy_v2` with constructor args).

State machine:

```
Active ──goal reached──▶ Funded ──all milestones released──▶ Closed
   │                          │
   └─deadline passed, short──▶ Refunding ──all refunds claimed──▶ Closed
```

- **Contribute**: pulls CFX from the contributor into the campaign vault,
  caps at the goal (no overfunding), records the per-contributor amount,
  emits `campaign/contributed` (+ `campaign/goal_reached`), and **notifies
  the factory** via a best-effort `try_record_contribution` call.
- **Milestones**: the constructor validates that milestone amounts sum exactly
  to the goal. The creator releases each milestone in order (`require_auth` on
  the creator + index in `require_auth_for_args`); the vault pays the creator
  through the token contract. Terminal state: `Closed`.
- **Refunds**: after a failed deadline, `close_failed` flips the campaign to
  `Refunding`; each backer claims exactly `contributed − already_claimed` from
  the vault.

### 3. `cfx-factory` — platform factory

- **Inter-contract deploy**: `create_campaign` deploys a fresh campaign via
  `Deployer::with_current_contract(salt).deploy_v2(wasm, ctor_args)` where the
  salt is the global campaign counter — deterministic, collision-free
  addresses per factory.
- **Creation fee**: charged in the campaign's token and held by the factory
  (platform economics); configurable by admin.
- **Inter-contract callback**: `record_contribution` aggregates platform-wide
  raised totals. Authorization is the **invoker-contract** pattern: a
  `require_auth` on the campaign's contract address only passes when that
  contract itself is the caller (the host's auth manager tracks contract
  invocations), plus a registry check for defense in depth.
- events: `factory/initialized`, `factory/config_updated`,
  `factory/campaign_created`, `factory/contribution_tracked`

### Events

All events use the SDK 27 `#[contractevent]` macro: two fixed symbol topics
(`["campaign", "contributed"]`) for cheap RPC filtering, with the remaining
fields either indexed (`#[topic]`) or packed into a named data map. The
frontend filters with `topics: [["campaign", "contributed"]]` and decodes the
data map directly.

### Authorization model

| Action | Auth |
| --- | --- |
| token mint/burn | token admin |
| token transfer | `from` |
| create_campaign | creator (account) + fee transfer |
| contribute | contributor |
| release_milestone | creator (`require_auth_for_args(index, timestamp)`) |
| close_failed | anyone (no auth) |
| refund | contributor |
| extend_deadline | creator |
| factory admin fns | factory admin |
| record_contribution | invoking campaign contract (invoker auth) + registry |

## Frontend layer

React 19 + TypeScript + Vite, `@stellar/stellar-sdk` v16, Freighter wallet.

- **`lib/rpc.ts`** — single shared RPC server instance (easy to mock in tests)
- **`lib/contracts.ts`** — the transaction pipeline: simulated reads
  (`simulateTransaction` on a dummy source), and
  build → prepare → wallet-sign → submit → poll with decoded failure reasons
- **`lib/wallet.ts`** — Freighter connection with friendly errors
- **`hooks/useEventStream.ts`** — cursor-paginated `getEvents` polling with
  auto-recovery after RPC errors; events are decoded into
  `{ name, data, topics }` and handed to the UI
- **`hooks/useCampaigns` / `useCampaign`** — data loading with refresh keys
  bumped by the event stream
- **`components/`** — ProgressBar, StatusBadge, CampaignCard, EventFeed,
  skeletons, error states, toasts, wallet button
- **`pages/`** — Explore (live registry + stats), CampaignDetail
  (contribute / milestones / refunds / creator panel), CreateCampaign
  (goal, deadline, milestone builder with sum validation)

### UX resilience

- **Loading**: skeleton grids and cards while simulating; per-button busy
  states during transactions
- **Errors**: ErrorBoundary for render crashes; toast notifications for
  transaction failures with decoded reasons; ErrorState + retry for data
  loads; the event stream silently retries after RPC outages
- **Responsive**: mobile-first CSS — single column under 640px, 2-col cards at
  640px, 3-col grid + sticky detail sidebar at 960px

## Testing strategy

- Contracts: integration tests against the real wasm (including
  `include_bytes!` cross-contract tests), covering happy paths, guards
  (`#[should_panic]` with exact contract error codes), events, and the full
  factory→campaign→token round trip.
- Frontend: Vitest + Testing Library — pure utils, presentational components,
  and the event-stream hook with mocked RPC (cursor reuse, cleanup, retry).
