# Spec 07 — MCP server (stdio JSON-RPC)

**État**: pending

## Objectif

Serveur MCP minimaliste sur stdio. Pas de SDK Rust officiel mature → hand-roll JSON-RPC 2.0 + framing MCP. ~300 LOC.

## Protocole

- Transport: stdin/stdout. Framing JSON-RPC en LSP-style? Non — MCP utilise une ligne JSON par message (LSP-style headers seulement pour le mode HTTP). Pour stdio: une ligne `\n`-terminée par message.
- Méthodes obligatoires:
  - `initialize` → renvoie `serverInfo`, `capabilities.tools`, `instructions` (le contenu de `server-instructions.md`).
  - `initialized` (notification, no-op côté serveur).
  - `tools/list` → array de tools.
  - `tools/call` → invoque le tool.
  - `ping` → `{}`.
  - `shutdown` (optionnel selon agent).

## Tools

| Nom MCP | Handler | Args |
|---|---|---|
| `codegraph_search` | `db.search_nodes` | `{ query, limit?, kind? }` |
| `codegraph_node` | `db.node_by_id` ou by_name | `{ id?, name? }` |
| `codegraph_callers` | `traversal.callers` | `{ node, depth? }` |
| `codegraph_callees` | `traversal.callees` | `{ node, depth? }` |
| `codegraph_impact` | `traversal.impact_radius` | `{ node, max_depth? }` |
| `codegraph_context` | `context::build` | `{ query, depth?, include_source?, format? }` |
| `codegraph_explore` | `context::explore` | `{ paths[], depth? }` |
| `codegraph_files` | `db.files_under` | `{ path? }` |
| `codegraph_status` | `db.stats` | `{}` |

Chaque tool a un JSON Schema `inputSchema` exposé dans `tools/list`.

## Architecture

```rust
pub struct McpServer {
    db: Arc<Db>,
    traversal: Arc<Traversal<'static>>, // ... ou re-create par call
}

impl McpServer {
    pub async fn run(self, stdin: impl AsyncBufRead, stdout: impl AsyncWrite) -> Result<()>;
}
```

Boucle:
1. `read_line` → parse `JsonRpcMessage` (request/notification).
2. Dispatch async via `tokio::spawn` (un task par call — concurrence).
3. Réponse écrite avec `Mutex<stdout>` pour sérialisation des writes.

## Server instructions

`include_str!("server-instructions.md")` — contenu identique à `archive/src/mcp/server-instructions.ts`. Renvoyé dans `initialize.result.instructions`.

À garder en sync avec `instructions-template` de l'installer (spec 08).

## Erreurs

JSON-RPC 2.0 standard:
- `-32700` parse error
- `-32600` invalid request
- `-32601` method not found
- `-32602` invalid params
- `-32603` internal error
- `-32000..-32099` server-defined (NotInitialized → `-32001`)

## Tests

- Integration: spawn `codegraph serve --mcp` sur fixture indexé, écris séquence `initialize` → `tools/call codegraph_search`, assert response.
- Pas de SDK client — fabrique requêtes JSON à la main.

## Pièges

- `tracing_subscriber` doit écrire sur **stderr** (jamais stdout — corrompt le protocole).
- Si DB pas init (`.codegraph/` absent): `initialize` OK mais tous tools renvoient `-32001 NotInitialized` avec message guidant `codegraph init`.
- Multi-instance: lockfile sur `.codegraph/db.sqlite` pour éviter writer concurrent.
