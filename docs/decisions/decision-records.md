# Decision Records

Use this directory for durable product or technical decisions whose context and tradeoffs should be preserved.

## File naming

```text
NNNN-short-title.md
```

Numbers are sequential and are never reused.

## Record format

```markdown
# NNNN: Decision title

- Status: Proposed | Accepted | Superseded
- Date: YYYY-MM-DD

## Context

What problem or constraint requires a decision?

## Decision

What was decided?

## Consequences

What benefits, costs, constraints, or follow-up work result from this decision?
```

When a record supersedes another, link both records and update the relevant specification.
