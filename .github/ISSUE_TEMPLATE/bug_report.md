---
name: Bug report
about: Create a report to help us improve
title: '[BUG] '
labels: bug
assignees: ''

---

**Describe the bug**
A clear and concise description of what the bug is.

**To Reproduce**
Steps to reproduce the behavior:
1. Configure proxy with '...'
2. Send request to '...'
3. See error

**Expected behavior**
A clear and concise description of what you expected to happen.

**Actual behavior**
What actually happened, including any error messages.

**Logs**
```
Please paste relevant logs here, including:
- Proxy logs (with RUST_LOG=debug if possible)
- Any error responses
```

**Environment (please complete the following information):**
- OS: [e.g. Ubuntu 22.04]
- Rust version: [e.g. 1.75.0]
- Proxy version/commit: [e.g. 0.1.0 or commit hash]
- Taproot Assets daemon version: [e.g. 0.3.0]
- LND version: [e.g. 0.17.0]

**Configuration**
```env
# Relevant parts of your .env.local (with sensitive data removed)
TAPROOT_ASSETS_HOST=
TLS_VERIFY=
CORS_ORIGINS=
```

**Additional context**
Add any other context about the problem here.
