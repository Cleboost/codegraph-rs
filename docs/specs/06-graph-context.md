# Spec 06 — Graph traversal + context builder

**État**: pending

## Objectif

Requêtes graphe haut niveau pour MCP/CLI: callers, callees, impact radius. Builder qui compose tout en markdown/json pour l'agent.

## Crate `codegraph-graph`

```rust
pub struct Traversal<'a> { db: &'a Db }

impl<'a> Traversal<'a> {
    pub fn callers(&self, node: NodeId, depth: u32) -> Result<Vec<CallerHit>>;
    pub fn callees(&self, node: NodeId, depth: u32) -> Result<Vec<CalleeHit>>;
    pub fn impact_radius(&self, node: NodeId, max_depth: u32) -> Result<ImpactReport>;
    pub fn path(&self, from: NodeId, to: NodeId, max_depth: u32) -> Result<Option<Vec<NodeId>>>;
}
```

- BFS avec `VecDeque<(NodeId, u32 depth)>`, set visited `HashSet<NodeId>`.
- Edge kind filter: callers/callees → `calls`; impact → `calls|references|imports|extends|implements`.
- Limite dure: 5000 visités, retourne `Truncated` flag.

`ImpactReport`:
```rust
pub struct ImpactReport {
    pub root: Node,
    pub direct: Vec<Node>,         // depth 1
    pub transitive: Vec<Node>,     // depth 2..=max
    pub by_kind: HashMap<NodeKind, u32>,
    pub truncated: bool,
}
```

## Crate `codegraph-context`

Compose les briques pour répondre "give me context for X" — analogue à `codegraph_context` MCP tool.

```rust
pub enum Format { Markdown, Json }

pub struct ContextRequest {
    pub query: String,            // symbol name OR free-text topic
    pub depth: u32,
    pub include_source: bool,
    pub format: Format,
}

pub fn build(db: &Db, req: &ContextRequest) -> Result<String>;
```

Algorithme (port du `archive/src/context/`):
1. `search_nodes(query)` → top N candidates par FTS rank.
2. Pour chaque candidate: charge node, callers (d=1), callees (d=1), file siblings.
3. Si `include_source`: charge slice `start_line..=end_line` depuis disque (cache LRU sur fichier).
4. Sérialise selon Format.

## Format markdown

```
## `getName` — function — src/foo.ts:42

```ts
<signature ou source>
```

**Callers** (3):
- `processUser` — src/users.ts:118 (calls)
- ...

**Callees** (2):
- `formatString` — src/utils.ts:5 (calls)
- ...
```

## Format json

Structure tagged identique surface MCP `codegraph_context` archive — agents prompts existants compatibles.

## Tests

- Fixture: 4 fichiers TS avec chaîne d'appels `A → B → C → D`.
- Assert: `callers(D, depth=3)` retourne A,B,C.
- Assert: `impact_radius(A, max=2).by_kind` count exact.

## Pièges

- Charger source à la demande → IO sur traversal large; cache file→string LRU 32 entrées suffit.
- Tronquage profondeur: documenter le flag dans la sortie markdown ("⚠ truncated at depth 5").
