# Contributing to RHoiScribe

RHoiScribe is a local MCP server for HOI4 modding agents. Contributions should keep the project useful for MCP-compatible clients, predictable for mod authors, and safe for generated game files.

## Development Flow

1. Build and run the existing project before changing behavior:

   ```powershell
   cargo build
   ```

2. Keep changes scoped to one concern: prompts, resources, tools, documentation, or packaging.

3. Run the verification commands before submitting a change:

   ```powershell
   cargo fmt --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test
   cargo build --release
   ```

4. Use Conventional Commits for commits:

   ```text
   feat: add focus generation option
   fix: reject unsafe mod output paths
   docs: clarify mcp setup
   chore: move bundled resources
   ```

## MCP Compatibility

- Keep stdio support working by default.
- Do not add client-specific behavior to the server protocol path unless the behavior is optional and compatible with standard MCP clients.
- Document new prompts, resources, and tools in the root README and localized READMEs.
- Prefer structured JSON responses for tools so agents can inspect planned files before write mode.

## HOI4 Resource Guidelines

Bundled knowledge resources live under `resources/knowledge/`.

- Keep resource files versionable and reviewable.
- Do not copy long wiki pages or proprietary game files into the repository.
- Summarize knowledge in original wording and include source references.
- Keep topic IDs stable once published because agents may reference them directly.
- Add syntax examples, relationships, and validation guidance when expanding `resources/knowledge/hoi4/catalog.json`.

## Tool Safety

Generation tools must protect the target mod folder.

- Treat generated paths as mod-root-relative paths.
- Reject absolute paths, drive-prefixed paths, and traversal such as `../`.
- Use dry-run previews for new generation features before enabling write mode.
- Preserve HOI4 localisation requirements, including UTF-8 BOM for generated `.yml` localisation files.

## Licensing

By contributing, you agree that your contribution is provided under the repository license in `LICENSE`.
