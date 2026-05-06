# Handoff

## In progress

Nothing actively in progress in this repo.

## Pick up next

- **Wire vidhi skills as slash commands** — update `~/.claude/skill-index.md` to reference `~/soft/manas/vidhi/vidhi-*/SKILL.md`
- **Bootstrap the brownfield project** — run `vidhi-init` in the target project to set up yojana as backend, then `vidhi-domain` → `vidhi-prd` → `vidhi-decompose` for the large feature
- **Dog-food yojana** — register yojana's own deferred issues (scoped cycle check, scoped ready detection, sequence number under pooling) as yojana tasks

## Context needed

- vidhi repo is at `~/soft/manas/vidhi/` (2 commits on main, MIT licensed)
- vidhi-implement is the yojana-aware dispatcher — routes by task category to tdd/diagnose/architecture
- vidhi-init now defaults to proposing yojana when MCP tools are available
- Commit message workaround: panda breaks heredoc syntax. Use `printf ... > /tmp/file && git commit -F /tmp/file`
- Db uses parking_lot::Mutex — public methods must not call other public methods (deadlock)
- Context assembler is pure — tool handler fetches data, assembler shapes it
- TaskUpdates nullable fields use `Option<Option<String>>`: None=keep, Some(None)=clear, Some(Some(v))=set
- Status is `in-progress` (hyphenated). Migrations versioned as `0006_*.sql`
