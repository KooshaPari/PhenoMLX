# Contributing to PhenoMLX

Thank you for considering contributing to PhenoMLX. This document covers the basics.

## Development Setup

```bash
# Clone the repo
git clone https://github.com/KooshaPari/PhenoMLX.git
cd PhenoMLX

# Create virtual environment
python3 -m venv .venv
source .venv/bin/activate

# Install in development mode
pip install -e .

# Install Rust perf-core (optional, for Rust FFI features)
cd perf-core && cargo build --release && cd ..
```

## Running Tests

```bash
# Python tests
python -m pytest tests/ -v

# Rust tests
cd perf-core && cargo test && cd ..

# Lint
ruff check src/
```

## Code Style

- **Python**: Follow PEP 8, use type hints where practical
- **Rust**: Follow `cargo fmt` defaults, use `cargo clippy` for linting
- **Line length**: 100 characters max
- **Files**: Keep under 500 lines (target 350)

## Pull Request Process

1. Fork the repository
2. Create a feature branch from `main`
3. Make your changes with tests
4. Run the full test suite: `python -m pytest tests/ -v`
5. Run linting: `ruff check src/`
6. Submit a pull request with a clear description

## What We're Looking For

- **Bug fixes** with regression tests
- **Performance improvements** with before/after benchmarks
- **New backend integrations** following the existing adapter pattern
- **Documentation** improvements
- **TurboQuant+** enhancements or extensions

## What Not to Submit

- Changes that break the OpenAI-compatible API
- Models or weights (use HuggingFace for those)
- Hardcoded secrets or credentials

## Reporting Issues

Use GitHub Issues. Include:
- OS and hardware (chip, RAM)
- Model and quantization used
- Steps to reproduce
- Expected vs actual behavior

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
