.PHONY: verify fmt test clippy frontend mcp-doctor

# Canonical CI-equivalent checks (use via: make verify, or agent-body muscle_execute_bash)
verify: fmt test clippy frontend

fmt:
	cargo fmt --all -- --check

test:
	cargo test --workspace

clippy:
	cargo clippy --workspace -- -D warnings

frontend:
	bun run check

mcp-doctor:
	./scripts/mcp-doctor.sh
