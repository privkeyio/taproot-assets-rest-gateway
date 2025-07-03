# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.1] - 2025-01-02

### Added
- Initial release of Taproot Assets REST API Proxy
- Complete coverage of Taproot Assets API endpoints
- Automatic macaroon authentication handling
- Configurable CORS support for web applications
- Request ID tracking and logging
- Comprehensive error handling with meaningful messages
- Environment-based configuration
- Health and readiness check endpoints
- Support for all asset operations (mint, transfer, burn)
- Address generation and decoding
- Channel funding and Lightning payments
- Universe/federation synchronization
- Wallet operations and PSBT management
- Request for Quote (RFQ) functionality
- Event streaming support
- Basic test suite

### Security
- TLS certificate verification (configurable for development)
- Secure macaroon handling
- Input validation on all endpoints
- Basic rate limiting (100 requests/minute per IP)

### Documentation
- Comprehensive README with examples
- API documentation for all endpoints
- Contributing guidelines
- Security policy
- Troubleshooting guide
- Docker deployment instructions
