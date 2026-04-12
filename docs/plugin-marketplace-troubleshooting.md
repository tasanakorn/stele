# Plugin Marketplace Troubleshooting

When registering this repo as a local Claude Code marketplace (`/plugin marketplace add <path>`), stale state can cause "Marketplace not found" errors. This applies to both plugins shipped from this repo (`plugins/stele/` and `plugins/steop/`).

## Known failure modes

1. **Stale `extraKnownMarketplaces` in `~/.claude/settings.json`** — If the marketplace was previously registered under a different name (e.g. `stele-plugins` → `stele-marketplace`), the old entry in `settings.json` persists and conflicts.
   - **Fix:** remove the old entry from `extraKnownMarketplaces` before re-adding.
2. **Orphaned plugin cache** — `~/.claude/plugins/cache/<marketplace-name>/` may contain `.orphaned_at` marker files from a previous failed resolution.
   - **Fix:** `rm -rf ~/.claude/plugins/cache/<marketplace-name>` then re-add.
3. **Resolution order** — Running `/plugin` to install individual plugins only works after the marketplace itself resolves cleanly.
   - **Fix:** remove the marketplace fully (`/plugin marketplace remove`), clear the cache, then re-add.

## Recovery recipe

```bash
# 1. Remove the marketplace from Claude Code
/plugin marketplace remove <marketplace-name>

# 2. Clean stale cache + settings entry
rm -rf ~/.claude/plugins/cache/<marketplace-name>
# edit ~/.claude/settings.json and delete any stale extraKnownMarketplaces entry

# 3. Re-add
/plugin marketplace add /path/to/stele
```
