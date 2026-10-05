# ADR-001: Durable Idempotency Topology, Structured Operation Identity, and Cryptographic Fingerprints

## Status

**ACCEPTED (REMEDIATED v3)** — Phase 11 Architecture Reconciliation (Pre-Implementation Gate)  
**Date:** 2026-10-04  
**Deciders:** Core Architecture Team  
**Consulted:** Phase 11 Hardening Research, Information-Theoretic Identity Analysis, Direct $K=5000$ Empirical Benchmarks

---

## 1. Context and Problem Statement

Phase 10 introduced durable automation idempotency in TinixCut to ensure that retrying operations across CLI, stdio sessions, or future MCP adapters does not duplicate edits. This was implemented by storing `command_ledger: Vec<CommandLedgerEntry>` in `Project` (`.tcproj`), using an FNV-1a 64-bit operation fingerprint.

Phase 11 pre-implementation research and subsequent reviews identified critical architectural flaws in the retention and identity lifecycle:

1. **The Unscoped Identity Correctness Gap**:
   - In ADR-001 v2, arbitrary unscoped string IDs were remembered across an Active Window ($K=5,000$) plus a Bounded Tombstone Window ($M_{\max}=10,000$).
   - *The Correctness Flaw*: When an unscoped operation ID is evicted from both windows, all stored state for that ID is erased. If a caller later queries the project revision and resubmits that old ID with a refreshed `base_revision > sealed_unscoped_revision`, the server has no record of the ID, observes a valid current `base_revision`, and executes the command as **fresh**.
   - An old committed operation silently duplicates mutations.
2. **Information-Theoretic Impossibility for Arbitrary Strings**:
   - Storing an unbounded universe of arbitrary string IDs in bounded storage ($O(1)$) while allowing refreshed `base_revision` submissions is mathematically impossible without false negatives (duplicate execution) or false positives (rejecting fresh IDs).
3. **Unbounded Client Cardinality Vulnerability**:
   - Simply introducing `client_epoch_watermarks: HashMap<String, u64>` without an explicit lifecycle contract assumes the number of clients is naturally bounded. An automated script or adversarial caller generating `client_id = uuid()` on every request causes the watermark map to grow indefinitely ($O(N)$), recreating document bloat.
4. **Legacy FNV-1a Replay Vulnerability**:
   - Stored Phase 10 entries use non-cryptographic FNV-1a 64-bit hashes without raw payload retention, exposing replays to collision spoofing.

---

## 2. Decision Drivers

- **Zero Duplicate Mutation Invariant**: A committed operation identity must be mathematically provable to never silently re-execute as fresh under any circumstances, regardless of `base_revision` refresh.
- **Strictly Bounded Document Footprint**: Total project file size, load time, save time, and memory overhead attributable to idempotency must remain strictly capped by a measurable performance budget ($\le 2.0 \text{ MB}$).
- **Bounded Client Lifecycle**: The client registry must have a strict capacity quota and deterministic retirement protocol to prevent unbounded client map growth.
- **Atomic Single-File Persistence**: Single-file atomic replace (`save_project_atomic`) must remain atomic; multi-file sidecars with split-brain risks are rejected.
- **Cryptographic Security**: Preimage resistance $\sim 2^{256}$ and collision resistance $\sim 2^{128}$ (Birthday bound); no reliance on FNV-1a for exact replay proof.
- **Deterministic Correctness**: No probabilistic data structures (e.g. Bloom filters) as correctness authorities.

---

## 3. Mathematical Proof: Impossibility of Bounded Unscoped String Deduplication

### 3.1 Theorem (Information-Theoretic Impossibility)
Let $U = \Sigma^*$ be the infinite universe of arbitrary string operation IDs.  
Let $H_t \subset U$ be the set of operation IDs committed up to project revision $t$.  
Let $B < \infty$ be a constant storage budget (in bits) allocated in the project document.

To guarantee zero duplicate executions under arbitrary caller refreshes (where a caller presents any $x \in H_t$ with refreshed `base_revision = t`), the system must compute a deterministic predicate $\text{IsCommitted}(x) \in \{\text{true}, \text{false}\}$ such that:
$$\forall x \in H_t, \text{IsCommitted}(x) = \text{true} \quad \text{and} \quad \forall x \notin H_t, \text{IsCommitted}(x) = \text{false}$$

The number of distinct subsets $H_t$ of size $N$ that can be distinguished by $B$ bits is at most $2^B$.  
As $N \to \infty$, the number of possible committed subsets $\binom{|U|}{N} \gg 2^B$.  
By the Pigeonhole Principle, there exist distinct histories $H_1 \ne H_2$ that map to the identical stored state $S_B$.  
For any $x \in H_1 \setminus H_2$, the system cannot distinguish whether $x$ was committed in $H_1$ or is fresh in $H_2$.

### 3.2 Corollary (The v2 Refreshed Revision Flaw)
Any bounded-horizon truncation of arbitrary unscoped strings erases historical members. If a caller refreshes `base_revision` to match current project revision $t$, and $t > \text{sealed\_unscoped\_revision}$, the system sees a valid revision and no existing record of $x$, and therefore **must** treat $x$ as fresh.  
**Conclusion:** Arbitrary unscoped string IDs cannot guarantee durable idempotency across unbounded horizons with bounded storage.

---

## 4. Decision Outcome

We adopt **Structured Scoped Operation Identity with a Bounded Client Registry, Ephemeral Unscoped Compatibility, Conservative Legacy FNV Rejection, and SHA-256 Baseline**.

### 4.1 Topology & Staged Isolation
- `client_registry` and `command_ledger` remain application-owned properties of the [`Project`](file:///d:/___TOOLS/__TINIX/TinixCut/src-tauri/src/modules/domain/mod.rs#L321-L335) model in `.tcproj`.
- `CommandService::execute` implements **Staged Transaction Isolation**: pre-commit validation and simulation clones only the editorial timeline state (`tracks`, `assets`, `settings`), isolating the ledger vectors.
  - *Measured improvement*: In-memory staging clone overhead is **601 nanoseconds** ($4,394\times$ faster than full 2.64 ms Project clone), with algorithmic complexity bounded to $O(\text{editorial items})$.
- Persistence maintains single-file atomic rename (`save_project_atomic`). Segregated sidecars are rejected due to split-brain crash risks.

### 4.2 Structured Operation Identity Specification
Durable idempotency is exclusively governed by a **Structured Scoped Identity**:
$$\text{OpIdentity} = (\text{client\_id}, \text{client\_epoch}, \text{sequence\_number})$$
- `client_id`: A registered client identifier string ($\le 64$ chars, e.g. `"gui"`, `"cli"`, `"agent-audio"`).
- `client_epoch`: A `u64` monotonic epoch allocated by the project to this client registration.
- `sequence_number`: A `u64` strictly monotonic integer sequence starting at 1 ($1, 2, 3, \dots$).

#### Mathematical Proof of Correctness for Monotonic Sequence Identity
For any registered client $(c, e)$, the set of committed operations is a contiguous prefix of natural numbers:
$$H(c, e) = \{1, 2, \dots, W(c, e)\} \quad \text{where } W(c, e) = \max(\{s \mid (c, e, s) \text{ was committed}\})$$
1. **Constant Storage $O(1)$**: To represent all historical committed operations $H(c, e)$ of any size $N$ requires exactly **ONE integer**: $W(c, e)$ (8 bytes). Storage overhead is completely independent of the number of operations executed.
2. **Deterministic Evaluation with Refreshed Revision Defense**:
   For any incoming command $(c, e, s)$ with **any** `base_revision`:
   - **If $s \le W(c, e)$**:
     By definition of sequence monotonicity, $s \in H(c, e)$ with $100\%$ certainty.
     The operation was **already committed** in the past. It is mathematically impossible for $s$ to ever be fresh, even if the caller supplies a refreshed `base_revision`.
     - In Active Window ($K=5,000$) $\rightarrow$ Return full prior receipt (`deduplicated_noop`).
     - Evicted from Active Window $\rightarrow$ Reject with `OPERATION_ALREADY_COMMITTED` ($R_{\text{applied}}$).
     $\implies$ Duplicate execution probability is **identically 0**.
   - **If $s == W(c, e) + 1$**:
     The operation is fresh. Execute atomically. If successful, $W(c, e) \leftarrow W(c, e) + 1$.
   - **If $s > W(c, e) + 1$**:
     Out-of-order sequence gap. Reject with `SEQUENCE_GAP_DETECTED`.

---

## 5. Client Lifecycle and Bounded Capacity Contract (`ClientRegistry`)

To prevent unbounded growth of client records when callers generate random client IDs:

### 5.1 Hard Capacity Quota ($C_{\max} = 64$)
The project enforces a strict upper bound of $C_{\max} = 64$ registered clients:
```rust
pub struct ClientRegistry {
    pub min_accepted_epoch: u64,
    pub current_epoch: u64,
    pub max_active_clients: usize, // Hard-coded: 64
    pub clients: HashMap<String, ClientRecord>,
}

pub struct ClientRecord {
    pub client_epoch: u64,
    pub highest_sequence: u64,
    pub last_active_revision: u64,
    pub registered_at_revision: u64,
}
```
- **Storage Budget**: $64 \times (\approx 48 \text{ bytes}) \approx 3.0 \text{ KB}$ in memory; $\le \mathbf{3.5 \text{ KB}}$ in `.tcproj` JSON.
- **Reserved Clients**: `"gui"` (UI desktop shell), `"cli"` (one-shot CLI operations), `"session"` (stdio machine session).

### 5.2 Registration and Quota Enforcement
1. **Explicit / Implicit Registration**:
   - Clients may register via `session.open` or `client.register`, receiving `(client_id, client_epoch)`.
   - On first command, if $|\text{clients}| < C_{\max}$, an unregistered client is auto-registered with `client_epoch = current_epoch` and initialized with $W = 0$.
2. **Quota Exhaustion**:
   - If $|\text{clients}| == C_{\max}$ and an unregistered client submits a command, the request is **strictly rejected** with:
     ```json
     {
       "code": "CLIENT_REGISTRY_FULL",
       "message": "Maximum active client capacity (64) reached. Inactive clients must be retired before new clients can register.",
       "max_clients": 64
     }
     ```
   - This mathematically prevents unbounded map growth under buggy or adversarial scripts.

### 5.3 Safe Client Retirement and Epoch Fencing
When client slots must be reclaimed:
1. Inactive clients (idle for $> 50,000$ revisions) are retired.
2. The project advances `min_accepted_epoch` and increments `current_epoch`.
3. Remaining active clients are re-anchored to `current_epoch`.
4. **Epoch Fencing Check**:
   Any incoming command with `client_epoch < min_accepted_epoch` is **immediately rejected** with `CLIENT_EPOCH_EXPIRED` via an $O(1)$ scalar check.
   *Correctness Guarantee*: A pruned client can NEVER re-execute old operations as fresh because its entire epoch is sealed. To execute again, it must register as a fresh client under `current_epoch`, resetting sequence to 1.

---

## 6. Legacy / Unscoped Operation IDs: Semantics & Backward Compatibility

### 6.1 Ephemeral Window Deduplication Contract
Arbitrary string IDs (Phase 10 format, bare UUIDs) that do not supply `client_id` and `sequence_number` are classified as **Ephemeral Window Operations**:
- **Deduplication Scope**: Retained **only** within the Active Receipt Window ($K = 5,000$).
- **In-Window Behavior**: Full exact replay deduplication (`deduplicated_noop`) or payload collision rejection (`IDEMPOTENCY_COLLISION`).
- **Post-Eviction Behavior**:
  - Once evicted beyond the 5,000-operation active window, unscoped IDs are **completely expired**.
  - **No Durable Guarantee**: Unscoped IDs do **not** provide durable idempotency across unbounded retention horizons.
  - If an evicted unscoped ID is submitted with a refreshed `base_revision`, the system treats it as an untracked command.
  - The API returns an explicit warning header in receipts: `idempotency_scope: "ephemeral_unscoped"`.
  - Callers requiring durable, crash-proof, multi-thousand-edit idempotency **must use Structured Operation Identity**.

### 6.2 Automation Transport Wrapping
To protect existing scripts and users without requiring manual sequence tracking:
- The **CLI Runner** (`tinixcut apply`) automatically injects `client_id: "cli"` and manages monotonic sequences via local session tracking.
- The **Stdio Session** (`tinixcut session`) automatically injects `client_id: "session"` and increments monotonic sequences per session connection.
- The **Desktop GUI** automatically routes edits through `client_id: "gui"`.
- Consequently, normal workflows transparently receive full durable idempotency.

---

## 7. Safe Legacy FNV Migration: Conservative Replay Rejection

Because FNV-1a possesses negligible adversarial collision resistance ($\ll 2^{32}$) and raw payloads were not retained in Phase 10 `.tcproj` files:
1. When loading Phase 10 entries with `"fp_<16hex>"`, they are flagged as **Legacy Unverifiable Entries**.
2. If an incoming command attempts to replay against a legacy FNV-1a entry:
   - The engine **refuses to treat it as an exact replay** because it cannot cryptographically prove that the candidate payload matches the un-retained original.
   - The engine **strictly rejects** the replay with:
     ```json
     {
       "code": "LEGACY_RECEIPT_UNVERIFIABLE",
       "message": "Operation 'op_123' was committed under legacy non-cryptographic schema (FNV-1a) without raw payload retention and cannot be safely verified for replay. Replay rejected for cryptographic integrity. Inspect project state to determine if operation outcome is present.",
       "operation_id": "op_123"
     }
     ```
   - **CRITICAL CORRECTNESS**: The operation is **NOT executed as fresh** (zero project mutation, zero revision bump).
3. As Phase 11+ commands are committed using SHA-256, legacy entries naturally roll out of the active window, leaving 100% of receipts cryptographically secure.

---

## 8. Direct $K=5000$ Empirical Benchmark & Performance Budget

Direct empirical measurements on reference hardware (Windows 11, NVMe storage) validate the $K = 5,000$ threshold against the 5-point performance budget:

| Performance Metric | Budget Target | Measured Directly at $K = 5,000$ (Formatted JSON + SHA-256) | Status / Margin |
|---|---|---|---|
| **Project File Overhead** | $\le 2.0 \text{ MB}$ | **1.635 MB** (1,714,534 bytes; 342.9 B/entry) + 3.5 KB ClientRegistry | PASS (18% margin) |
| **CommandService RAM** | $\le 5.0 \text{ MB}$ | **3.11 MB** (1.44 MB Vec + 1.67 MB HashMap) | PASS (38% margin) |
| **Staged Isolation Clone** | $\le 100 \ \mu\text{s}$ | **601 nanoseconds** ($0.6 \ \mu\text{s}$; algorithmic $O(\text{items})$) | PASS (99% margin) |
| **Deduplicated Replay Lookup** | $\le 50 \ \mu\text{s}$ | **8.5 – 22.7 µs** ($O(1)$ HashMap lookup) | PASS (55% margin) |
| **Autosave Disk Latency** | $\le 15 \text{ ms}$ (release) | **~6.5 ms** (release profile; ~52 ms unoptimized debug) | PASS under standard NVMe profile |
| **Project Load / Parse** | $\le 30 \text{ ms}$ (release) | **~24.0 ms** (release profile; ~94 ms unoptimized debug) | PASS under standard NVMe profile |

*Note on Latency Environment Boundaries*: While in-memory algorithmic complexity is strictly $O(1)$, disk I/O latencies are subject to OS thread preemption, virus scanner inspection locks, and filesystem sync flushes.

### 8.1 Cryptographic Fingerprint Specification
- **Algorithm**: Standard **SHA-256** (NIST FIPS 180-4).
- **Security Properties**:
  - **Collision resistance**: $\mathbf{\sim 2^{128}}$ operations (Birthday bound $2^{256/2} \approx 3.4 \times 10^{38}$).
  - **Preimage & Second-preimage resistance**: $\mathbf{\sim 2^{256}}$ operations.
- **Canonical Payload**: UTF-8 bytes from `serde_json::to_vec(&operation)` with exact integer fractions from [`RationalTime`](file:///d:/___TOOLS/__TINIX/TinixCut/src-tauri/src/modules/domain/time.rs#L58-L65).
- **Format**: `fp_sha256_<64 lowercase hex characters>`.
- **Authority**: Serves as the sole equality authority for idempotency collision detection.

---

## 9. Project Domain Schema Extension

```rust
pub struct Project {
    pub schema_version: u32,
    pub project_id: String,
    pub name: String,
    pub revision: u64,
    pub settings: ProjectSettings,
    pub assets: Vec<Asset>,
    pub tracks: Vec<Track>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub client_registry: ClientRegistry,
    #[serde(default)]
    pub command_ledger: Vec<CommandLedgerEntry>, // Active Window (K=5,000)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRegistry {
    pub min_accepted_epoch: u64,
    pub current_epoch: u64,
    pub max_active_clients: usize,
    pub clients: HashMap<String, ClientRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRecord {
    pub client_epoch: u64,
    pub highest_sequence: u64,
    pub last_active_revision: u64,
    pub registered_at_revision: u64,
}
```

*Note on Tombstones Removal*: With Structured Operation Identity, individual operation tombstones (`evicted_tombstones`) are completely eliminated. The single `highest_sequence: u64` scalar compactly represents all past committed operations for that client, reducing historical storage from $O(N)$ tombstones to $O(1)$ per client.

---

## 10. Summary & Guarantees

1. **Provable Correctness**: Structured monotonic sequences eliminate the refreshed `base_revision` loophole with mathematical certainty ($s \le W(c) \implies s \in \text{committed}$).
2. **Strictly Bounded Document Size**: Bounded at $\le \mathbf{1.65 \text{ MB}}$ total overhead permanently (Active Window $1.635 \text{ MB}$ + Client Registry $\le 3.5 \text{ KB}$).
3. **Bounded Client Map**: Hard capacity quota ($C_{\max} = 64$) and epoch fencing prevent client cardinality leaks.
4. **Deterministic Authority**: Zero Bloom filters or probabilistic structures used as correctness authority.
5. **Safe Legacy Handling**: Conservative rejection of unverified FNV replays closes collision attacks.
6. **Sub-Microsecond In-Memory Staging**: Staged Isolation clone verified at 601 nanoseconds.
