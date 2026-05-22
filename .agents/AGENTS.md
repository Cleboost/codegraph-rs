# CodeGraph

This project has a CodeGraph MCP server configured. CodeGraph is a tree-sitter
knowledge graph of every symbol, edge, and file in the workspace. Reads are
sub-millisecond and return structural information grep cannot.

## When to prefer codegraph

Use codegraph for **structural** questions — what calls what, what would
break, where is X defined, what is X's signature. Use native grep/read only
for literal text queries.

| Question | Tool |
|---|---|
| "Where is X defined?" | `codegraph_search` |
| "What calls Y?" | `codegraph_callers` |
| "What does Y call?" | `codegraph_callees` |
| "What would break if I changed Z?" | `codegraph_impact` |
| "Show me Y's signature / source" | `codegraph_node` |
| "Give me focused context for a task" | `codegraph_context` |
| "What files exist under path/" | `codegraph_files` |
| "Is the index healthy?" | `codegraph_status` |

## Rules of thumb

- **Trust codegraph results.** They come from a full AST parse. Do NOT
  re-verify with grep.
- **Don't grep first** when looking up a symbol by name.
- **`codegraph_context` is one call** — don't chain search + node yourself.

## If `.codegraph/` doesn't exist

The MCP server returns "not initialized." Run `codegraph init -i` to build
the index.
