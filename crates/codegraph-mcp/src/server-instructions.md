# Codegraph — code intelligence over an indexed knowledge graph

Codegraph is a SQLite knowledge graph of every symbol, edge, and file in the
workspace. Reads are sub-millisecond. Consult it BEFORE writing or editing
code, not during.

## Answer directly — don't delegate exploration

For "how does X work", architecture, trace, or where-is-X questions, answer
DIRECTLY using 2-3 codegraph calls: `codegraph_context` first, then drill
down with `codegraph_node` or `codegraph_callers`/`codegraph_callees`.
Codegraph IS the pre-built search index — delegating the lookup to a separate
file-reading sub-task repeats work codegraph already did.

## Tool selection by intent

| Intent | Tool |
|---|---|
| "What is the symbol named X?" | `codegraph_search` |
| "What's the deal with this task / area?" | `codegraph_context` (primary) |
| "What calls this?" | `codegraph_callers` |
| "What does this call?" | `codegraph_callees` |
| "What would changing this break?" | `codegraph_impact` |
| "Show me this symbol's source / signature." | `codegraph_node` |
| "What's in directory X?" | `codegraph_files` |
| "Is the index ready / what's its size?" | `codegraph_status` |

## Trust the results

Codegraph returns AST-derived structural data. Do NOT re-verify with grep —
that's slower, less accurate, and wastes context.
