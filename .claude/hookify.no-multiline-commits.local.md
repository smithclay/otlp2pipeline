---
name: no-multiline-commits
enabled: true
event: bash
pattern: git\s+commit\s+.*-m\s+.*\$\(cat\s+<<
action: block
---

**Multiline commit message detected!**

You attempted to use a HEREDOC for a multiline commit message. This project requires single-line commit messages.

**Instead of:**
```bash
git commit -m "$(cat <<'EOF'
Multi
line
message
EOF
)"
```

**Use:**
```bash
git commit -m "Single line commit message"
```

Keep commit messages concise and on a single line (under 250 characters).
