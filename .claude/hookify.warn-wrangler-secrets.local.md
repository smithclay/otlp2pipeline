---
name: warn-wrangler-secrets
enabled: true
event: file
conditions:
  - field: file_path
    operator: regex_match
    pattern: wrangler\.toml
  - field: new_text
    operator: regex_match
    pattern: (secret|key|token|password|api_key)\s*=
action: warn
---

**Potential secret detected in wrangler.toml!**

Avoid hardcoding sensitive values in configuration files.

**Better alternatives:**
- Use `wrangler secret put <NAME>` for production secrets
- Use `.dev.vars` file for local development (add to .gitignore)
- Use environment variables with `[vars]` for non-sensitive config
