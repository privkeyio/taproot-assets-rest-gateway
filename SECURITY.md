# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.0.1   | :white_check_mark: |

## Reporting a Vulnerability

We take the security of the Taproot Assets Rust SDK seriously. If you believe you have found a security vulnerability, please report it to us as described below.

### Please do NOT:
- Open a public GitHub issue
- Discuss the vulnerability publicly before it's fixed
- Exploit the vulnerability for anything other than verification

### Please DO:
- Email us at: security@privkey.io
- Include "SECURITY" in the subject line
- Provide detailed steps to reproduce the issue
- Include the version of the software affected
- If possible, provide a proof of concept

### What to expect:
1. **Acknowledgment**: We will acknowledge receipt within 48 hours
2. **Investigation**: We will investigate and validate the issue
3. **Communication**: We will keep you informed of our progress
4. **Fix**: We will work on a fix and coordinate disclosure
5. **Credit**: We will credit you for the discovery (unless you prefer to remain anonymous)

## Security Best Practices

When using this proxy in production:

### Authentication
- Store macaroon files with restrictive permissions (600)
- Never commit macaroon files to version control
- Rotate macaroons regularly
- Use separate macaroons for different environments

### Network Security
- Always use TLS in production (keep `TLS_VERIFY=true`)
- Run the proxy behind a reverse proxy (nginx, caddy) with proper SSL
- Implement rate limiting at the reverse proxy level
- Use firewall rules to restrict access

### Configuration
- Use strong, unique passwords for all related services
- Keep environment variables secure
- Audit CORS origins regularly
- Monitor logs for suspicious activity

### Updates
- Keep the proxy updated to the latest version
- Monitor security advisories
- Update dependencies regularly with `cargo update`
- Run `cargo audit` to check for known vulnerabilities

## Known Security Considerations

1. **Macaroon Exposure**: The proxy handles macaroon authentication, which means it has full access to your Taproot Assets daemon. Ensure the proxy itself is properly secured.

2. **CORS Configuration**: Misconfigured CORS can allow unauthorized web applications to access your API. Only add trusted origins.

3. **TLS Verification**: The `TLS_VERIFY=false` option should ONLY be used in development environments.

## Responsible Disclosure

We believe in responsible disclosure and will:
- Work with security researchers to verify and fix issues
- Publicly disclose the issue once a fix is available
- Maintain a security advisory page for known issues

Thank you for helping keep the Taproot Assets ecosystem secure!
