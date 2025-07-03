# Contributing to Taproot Assets API Proxy

Thank you for your interest in contributing to the Taproot Assets API Proxy! This document provides guidelines and instructions for contributing.

## Code of Conduct

By participating in this project, you agree to abide by our Code of Conduct:

- Be respectful and inclusive
- Welcome newcomers and help them get started
- Focus on what is best for the community
- Show empathy towards other community members

## How to Contribute

### Reporting Issues

- Check if the issue already exists
- Use the issue templates when available
- Provide clear description and steps to reproduce
- Include relevant logs and error messages
- Specify your environment (OS, Rust version, tapd version)

### Suggesting Enhancements

- Open an issue to discuss your idea first
- Clearly describe the problem you're solving
- Provide examples of how the feature would work
- Consider the impact on existing users

### Pull Requests

1. **Fork and Clone**
   ```bash
   git clone https://github.com/privkeyio/taproot-assets-rest-gateway.git
   cd taproot-assets-rest-gateway
   ```

2. **Create a Branch**
   ```bash
   git checkout -b feature/your-feature-name
   ```

3. **Make Your Changes**
   - Follow the existing code style
   - Add tests for new functionality
   - Update documentation as needed
   - Keep commits focused and atomic

4. **Test Your Changes**
   ```bash
   # Run tests
   cargo test
   
   # Check formatting
   cargo fmt -- --check
   
   # Run linter
   cargo clippy -- -D warnings
   ```

5. **Submit PR**
   - Write a clear PR description
   - Reference any related issues
   - Ensure CI passes

## Development Guidelines

### Code Style

- Use `cargo fmt` to format code
- Follow Rust naming conventions
- Document public APIs with doc comments
- Keep functions focused and small
- Prefer explicit error handling

### Testing

- Write unit tests for new functions
- Add integration tests for new endpoints
- Aim for good test coverage
- Test error cases, not just happy paths

### Documentation

- Update README.md for user-facing changes
- Document all public APIs
- Include examples in documentation
- Keep documentation up to date

### Commit Messages

Follow conventional commits format:

```
type(scope): brief description

Longer explanation if needed

Fixes #123
```

Types: feat, fix, docs, style, refactor, test, chore

## Project Structure

```
src/
├── api/           # API endpoint implementations
│   ├── addresses.rs
│   ├── assets.rs
│   ├── mod.rs
│   └── ...
├── config.rs      # Configuration handling
├── error.rs       # Error types
├── middleware.rs  # HTTP middleware
├── types.rs       # Shared types
└── main.rs        # Application entry
```

## Adding New Endpoints

1. Create/update the appropriate module in `src/api/`
2. Define request/response types
3. Implement the handler function
4. Add routing in the module's `configure()` function
5. Write tests for the new endpoint

Example:
```rust
// In src/api/your_module.rs
pub async fn your_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    // Implementation
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/your-endpoint")
            .route(web::get().to(your_handler))
    );
}
```

## Testing with Polar

[Polar](https://lightningpolar.com/) is recommended for local development:

1. Create a network with LND nodes
2. Enable Taproot Assets on a node
3. Use the generated macaroons for testing

## Questions?

- Open a discussion for general questions
- Join our community chat [link]
- Check existing issues and discussions

Thank you for contributing!
